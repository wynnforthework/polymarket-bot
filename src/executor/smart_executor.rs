//! Smart order executor with depth checking, limit orders, and retry logic
//!
//! Features:
//! - Pre-trade orderbook depth analysis
//! - Limit orders to avoid slippage
//! - Automatic retry with backoff (max 3 attempts)
//! - Batch entry/exit support

use crate::client::clob::{ClobClient, OrderBook};
use crate::error::{BotError, Result};
use crate::types::{Order, OrderStatus, OrderType, Side, Trade};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn};

/// Configuration for smart execution
#[derive(Debug, Clone)]
pub struct SmartExecutorConfig {
    /// Maximum retries per order
    pub max_retries: u32,
    /// Base delay between retries (ms)
    pub retry_delay_ms: u64,
    /// Minimum liquidity required (in USD)
    pub min_liquidity: Decimal,
    /// Maximum allowed slippage (as decimal, e.g., 0.01 = 1%)
    pub max_slippage: Decimal,
    /// Number of batches for large orders
    pub batch_count: u32,
    /// Delay between batches (ms)
    pub batch_delay_ms: u64,
    /// Order timeout for fill check (seconds)
    pub order_timeout_secs: u64,
}

impl Default for SmartExecutorConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            retry_delay_ms: 1000,
            min_liquidity: dec!(50), // $50 minimum
            max_slippage: dec!(0.02), // 2% max slippage
            batch_count: 3,
            batch_delay_ms: 2000,
            order_timeout_secs: 30,
        }
    }
}

/// Depth analysis result
#[derive(Debug, Clone)]
pub struct DepthAnalysis {
    /// Available liquidity at best price
    pub best_liquidity: Decimal,
    /// Total liquidity within slippage tolerance
    pub total_liquidity: Decimal,
    /// Expected average fill price
    pub expected_price: Decimal,
    /// Expected slippage
    pub expected_slippage: Decimal,
    /// Whether the order can be filled
    pub can_fill: bool,
}

/// Smart order executor
pub struct SmartExecutor {
    clob: ClobClient,
    config: SmartExecutorConfig,
}

impl SmartExecutor {
    pub fn new(clob: ClobClient, config: SmartExecutorConfig) -> Self {
        Self { clob, config }
    }

    /// Analyze orderbook depth before placing order
    pub fn analyze_depth(&self, book: &OrderBook, side: Side, size: Decimal) -> DepthAnalysis {
        let levels = match side {
            Side::Buy => &book.asks,
            Side::Sell => &book.bids,
        };

        if levels.is_empty() {
            return DepthAnalysis {
                best_liquidity: Decimal::ZERO,
                total_liquidity: Decimal::ZERO,
                expected_price: Decimal::ZERO,
                expected_slippage: Decimal::ONE,
                can_fill: false,
            };
        }

        let best_price = levels[0].price;
        let best_liquidity = levels[0].size * best_price;

        // Calculate total liquidity within slippage tolerance
        let max_price_diff = best_price * self.config.max_slippage;
        let mut total_liquidity = Decimal::ZERO;
        let mut weighted_price = Decimal::ZERO;
        let mut remaining_size = size;

        for level in levels {
            let price_diff = match side {
                Side::Buy => level.price - best_price,
                Side::Sell => best_price - level.price,
            };

            if price_diff > max_price_diff {
                break;
            }

            let fill_size = remaining_size.min(level.size);
            total_liquidity += level.size * level.price;
            weighted_price += fill_size * level.price;
            remaining_size -= fill_size;

            if remaining_size <= Decimal::ZERO {
                break;
            }
        }

        let expected_price = if size > remaining_size && size > Decimal::ZERO {
            weighted_price / (size - remaining_size)
        } else {
            best_price
        };

        let expected_slippage = if best_price > Decimal::ZERO {
            (expected_price - best_price).abs() / best_price
        } else {
            Decimal::ZERO
        };

        let can_fill = remaining_size <= Decimal::ZERO 
            && total_liquidity >= self.config.min_liquidity
            && expected_slippage <= self.config.max_slippage;

        DepthAnalysis {
            best_liquidity,
            total_liquidity,
            expected_price,
            expected_slippage,
            can_fill,
        }
    }

    /// Execute order with retry logic
    pub async fn execute_with_retry(
        &self,
        token_id: &str,
        side: Side,
        size: Decimal,
        market_id: &str,
    ) -> Result<ExecutionResult> {
        let mut last_error = None;
        let mut total_filled = Decimal::ZERO;
        let mut trades = Vec::new();

        for attempt in 1..=self.config.max_retries {
            info!(
                "Execution attempt {}/{} for {} {} shares of {}",
                attempt,
                self.config.max_retries,
                match side {
                    Side::Buy => "BUY",
                    Side::Sell => "SELL",
                },
                size - total_filled,
                token_id
            );

            match self.try_execute(token_id, side, size - total_filled, market_id).await {
                Ok(result) => {
                    total_filled += result.filled_size;
                    trades.extend(result.trades);

                    if total_filled >= size * dec!(0.99) {
                        // Consider it filled if 99%+ done
                        return Ok(ExecutionResult {
                            filled_size: total_filled,
                            average_price: result.average_price,
                            trades,
                            attempts: attempt,
                        });
                    }

                    info!(
                        "Partial fill: {:.2}/{:.2} shares. Retrying...",
                        total_filled, size
                    );
                }
                Err(e) => {
                    warn!("Attempt {} failed: {}", attempt, e);
                    last_error = Some(e);
                }
            }

            if attempt < self.config.max_retries {
                // Exponential backoff with overflow protection (max 8x multiplier)
                let shift = (attempt - 1).min(3) as u32;
                let multiplier = 1u64 << shift;
                let delay = self.config.retry_delay_ms.saturating_mul(multiplier);
                sleep(Duration::from_millis(delay)).await;
            }
        }

        if total_filled > Decimal::ZERO {
            // Partial success
            let avg_price = trades
                .iter()
                .map(|t| t.price * t.size)
                .sum::<Decimal>()
                / total_filled;
            
            Ok(ExecutionResult {
                filled_size: total_filled,
                average_price: avg_price,
                trades,
                attempts: self.config.max_retries,
            })
        } else {
            Err(last_error.unwrap_or_else(|| BotError::Execution("Max retries exceeded".into())))
        }
    }

    /// Single execution attempt
    async fn try_execute(
        &self,
        token_id: &str,
        side: Side,
        size: Decimal,
        market_id: &str,
    ) -> Result<ExecutionResult> {
        // Get orderbook and analyze depth
        let book = self.clob.get_order_book(token_id).await?;
        let analysis = self.analyze_depth(&book, side, size);

        if !analysis.can_fill {
            return Err(BotError::Execution(format!(
                "Insufficient liquidity: ${:.2} available, ${:.2} required, slippage {:.2}%",
                analysis.total_liquidity,
                self.config.min_liquidity,
                analysis.expected_slippage * dec!(100)
            )));
        }

        // Use limit order at expected price (with small buffer)
        let limit_price = match side {
            Side::Buy => analysis.expected_price * (Decimal::ONE + dec!(0.001)), // 0.1% above
            Side::Sell => analysis.expected_price * (Decimal::ONE - dec!(0.001)), // 0.1% below
        };

        let order = Order {
            token_id: token_id.to_string(),
            side,
            price: limit_price,
            size,
            order_type: OrderType::GTC,
        };

        info!(
            "Placing limit order: {} {:.4} @ {:.4} (expected fill: {:.4})",
            match side {
                Side::Buy => "BUY",
                Side::Sell => "SELL",
            },
            size,
            limit_price,
            analysis.expected_price
        );

        let order_status = self.clob.place_order(&order).await?;

        // Wait for fill
        let filled_status = self.wait_for_fill(&order_status.order_id).await?;

        if filled_status.filled_size > Decimal::ZERO {
            let trade = Trade {
                id: uuid::Uuid::new_v4().to_string(),
                order_id: order_status.order_id.clone(),
                token_id: token_id.to_string(),
                market_id: market_id.to_string(),
                side,
                price: filled_status.avg_price.unwrap_or(limit_price),
                size: filled_status.filled_size,
                fee: Decimal::ZERO,
                timestamp: chrono::Utc::now(),
            };

            Ok(ExecutionResult {
                filled_size: filled_status.filled_size,
                average_price: filled_status.avg_price.unwrap_or(limit_price),
                trades: vec![trade],
                attempts: 1,
            })
        } else {
            // Cancel unfilled order
            let _ = self.clob.cancel_order(&order_status.order_id).await;
            Err(BotError::Execution("Order not filled".into()))
        }
    }

    /// Wait for order to fill (with timeout)
    async fn wait_for_fill(&self, order_id: &str) -> Result<OrderStatus> {
        let timeout = Duration::from_secs(self.config.order_timeout_secs);
        let start = std::time::Instant::now();
        let poll_interval = Duration::from_millis(500);

        loop {
            let status = self.clob.get_order(order_id).await?;

            if status.status == "FILLED" || status.remaining_size == Decimal::ZERO {
                return Ok(status);
            }

            if status.status == "CANCELLED" || status.status == "REJECTED" {
                return Err(BotError::Execution(format!(
                    "Order {} was {}",
                    order_id, status.status
                )));
            }

            if start.elapsed() > timeout {
                return Ok(status); // Return partial fill status
            }

            sleep(poll_interval).await;
        }
    }

    /// Execute in batches (for large orders)
    pub async fn execute_batched(
        &self,
        token_id: &str,
        side: Side,
        total_size: Decimal,
        market_id: &str,
    ) -> Result<ExecutionResult> {
        let batch_size = total_size / Decimal::from(self.config.batch_count);
        let mut total_filled = Decimal::ZERO;
        let mut all_trades = Vec::new();
        let mut total_cost = Decimal::ZERO;

        info!(
            "Starting batched execution: {} batches of {:.4} shares",
            self.config.batch_count, batch_size
        );

        for batch_num in 1..=self.config.batch_count {
            let remaining = total_size - total_filled;
            let this_batch = if batch_num == self.config.batch_count {
                remaining // Last batch takes remainder
            } else {
                batch_size.min(remaining)
            };

            if this_batch <= Decimal::ZERO {
                break;
            }

            info!("Executing batch {}/{}: {:.4} shares", batch_num, self.config.batch_count, this_batch);

            match self.execute_with_retry(token_id, side, this_batch, market_id).await {
                Ok(result) => {
                    total_filled += result.filled_size;
                    total_cost += result.average_price * result.filled_size;
                    all_trades.extend(result.trades);
                }
                Err(e) => {
                    warn!("Batch {} failed: {}. Stopping.", batch_num, e);
                    break;
                }
            }

            if batch_num < self.config.batch_count {
                sleep(Duration::from_millis(self.config.batch_delay_ms)).await;
            }
        }

        if total_filled > Decimal::ZERO {
            Ok(ExecutionResult {
                filled_size: total_filled,
                average_price: total_cost / total_filled,
                trades: all_trades,
                attempts: self.config.batch_count,
            })
        } else {
            Err(BotError::Execution("No batches filled".into()))
        }
    }

    /// Check if order can be executed given current depth
    pub async fn can_execute(&self, token_id: &str, side: Side, size: Decimal) -> Result<bool> {
        let book = self.clob.get_order_book(token_id).await?;
        let analysis = self.analyze_depth(&book, side, size);
        Ok(analysis.can_fill)
    }
}

/// Result of an execution
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub filled_size: Decimal,
    pub average_price: Decimal,
    pub trades: Vec<Trade>,
    pub attempts: u32,
}

// Tests in smart_executor_tests.rs
