//! Gradual exit strategy - progressive position unwinding
//!
//! Exit positions in stages based on price thresholds:
//! - Price > 85%: Sell 25%
//! - Price > 90%: Sell 25%
//! - Price > 95%: Sell remaining
//!
//! Each sale checks liquidity before executing.

use crate::client::clob::{ClobClient, OrderBook};
use crate::error::Result;
use crate::types::{Side, Trade};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::HashMap;
use tracing::{info, warn};

/// Exit threshold configuration
#[derive(Debug, Clone)]
pub struct ExitThreshold {
    /// Price threshold (e.g., 0.85 = 85%)
    pub price_threshold: Decimal,
    /// Percentage of remaining position to sell (e.g., 0.25 = 25%)
    pub sell_percentage: Decimal,
}

impl ExitThreshold {
    pub fn new(price_threshold: Decimal, sell_percentage: Decimal) -> Self {
        Self {
            price_threshold,
            sell_percentage,
        }
    }
}

/// Gradual exit configuration
#[derive(Debug, Clone)]
pub struct GradualExitConfig {
    /// Exit thresholds (ordered by price, ascending)
    pub thresholds: Vec<ExitThreshold>,
    /// Minimum liquidity required to sell (USD)
    pub min_liquidity: Decimal,
    /// Maximum slippage allowed
    pub max_slippage: Decimal,
    /// Cooldown between exits (seconds)
    pub cooldown_secs: u64,
}

impl Default for GradualExitConfig {
    fn default() -> Self {
        Self {
            thresholds: vec![
                ExitThreshold::new(dec!(0.85), dec!(0.25)), // At 85%, sell 25%
                ExitThreshold::new(dec!(0.90), dec!(0.25)), // At 90%, sell 25% (of remaining)
                ExitThreshold::new(dec!(0.95), dec!(1.00)), // At 95%, sell all remaining
            ],
            min_liquidity: dec!(20), // $20 minimum
            max_slippage: dec!(0.03), // 3% max slippage
            cooldown_secs: 60,
        }
    }
}

/// Position tracking for gradual exit
#[derive(Debug, Clone)]
pub struct TrackedPosition {
    pub token_id: String,
    pub market_id: String,
    pub side: Side,
    pub original_size: Decimal,
    pub remaining_size: Decimal,
    pub entry_price: Decimal,
    /// Which threshold index was last triggered
    pub last_threshold_triggered: Option<usize>,
    /// Last exit timestamp
    pub last_exit_time: Option<chrono::DateTime<chrono::Utc>>,
}

/// Gradual exit manager
pub struct GradualExitManager {
    config: GradualExitConfig,
    positions: HashMap<String, TrackedPosition>,
}

impl GradualExitManager {
    pub fn new(config: GradualExitConfig) -> Self {
        Self {
            config,
            positions: HashMap::new(),
        }
    }

    /// Register a position for gradual exit tracking
    pub fn track_position(
        &mut self,
        token_id: &str,
        market_id: &str,
        side: Side,
        size: Decimal,
        entry_price: Decimal,
    ) {
        let position = TrackedPosition {
            token_id: token_id.to_string(),
            market_id: market_id.to_string(),
            side,
            original_size: size,
            remaining_size: size,
            entry_price,
            last_threshold_triggered: None,
            last_exit_time: None,
        };

        info!(
            "📊 Tracking position for gradual exit: {} {:.4} shares @ {:.2}%",
            token_id,
            size,
            entry_price * dec!(100)
        );

        self.positions.insert(token_id.to_string(), position);
    }

    /// Remove a position from tracking
    pub fn untrack_position(&mut self, token_id: &str) -> Option<TrackedPosition> {
        self.positions.remove(token_id)
    }

    /// Analyze if liquidity is sufficient for exit
    pub fn check_liquidity(&self, book: &OrderBook, size: Decimal) -> LiquidityCheck {
        // For exit, we're selling, so check bids
        let bids = &book.bids;

        if bids.is_empty() {
            return LiquidityCheck {
                sufficient: false,
                available_liquidity: Decimal::ZERO,
                expected_price: Decimal::ZERO,
                expected_slippage: Decimal::ONE,
            };
        }

        let best_bid = bids[0].price;
        let mut available = Decimal::ZERO;
        let mut weighted_price = Decimal::ZERO;
        let mut remaining = size;

        for level in bids {
            let fill_size = remaining.min(level.size);
            available += level.size * level.price;
            weighted_price += fill_size * level.price;
            remaining -= fill_size;

            if remaining <= Decimal::ZERO {
                break;
            }
        }

        let expected_price = if size > remaining && size > Decimal::ZERO {
            weighted_price / (size - remaining)
        } else {
            best_bid
        };

        let slippage = if best_bid > Decimal::ZERO {
            (best_bid - expected_price) / best_bid
        } else {
            Decimal::ONE
        };

        let sufficient = available >= self.config.min_liquidity
            && slippage <= self.config.max_slippage
            && remaining <= Decimal::ZERO;

        LiquidityCheck {
            sufficient,
            available_liquidity: available,
            expected_price,
            expected_slippage: slippage,
        }
    }

    /// Check if a position should exit at current price
    pub fn check_exit(&self, token_id: &str, current_price: Decimal) -> Option<ExitAction> {
        let position = self.positions.get(token_id)?;

        // Check cooldown
        if let Some(last_exit) = position.last_exit_time {
            let elapsed = chrono::Utc::now() - last_exit;
            if elapsed.num_seconds() < self.config.cooldown_secs as i64 {
                return None; // Still in cooldown
            }
        }

        // For buy positions, we exit when price goes up
        // For sell positions, we exit when price goes down (but we invert the check)
        let effective_price = match position.side {
            Side::Buy => current_price,
            Side::Sell => Decimal::ONE - current_price, // Invert for short positions
        };

        // Find the next threshold to trigger
        let start_idx = position.last_threshold_triggered.map(|i| i + 1).unwrap_or(0);

        for (idx, threshold) in self.config.thresholds.iter().enumerate().skip(start_idx) {
            if effective_price >= threshold.price_threshold {
                // Calculate sell size
                let sell_size = if threshold.sell_percentage >= Decimal::ONE {
                    position.remaining_size // Sell all
                } else {
                    position.remaining_size * threshold.sell_percentage
                };

                if sell_size <= Decimal::ZERO {
                    continue;
                }

                info!(
                    "🎯 Threshold triggered: {} @ {:.1}% >= {:.1}% → sell {:.1}% ({:.4} shares)",
                    token_id,
                    effective_price * dec!(100),
                    threshold.price_threshold * dec!(100),
                    threshold.sell_percentage * dec!(100),
                    sell_size
                );

                return Some(ExitAction {
                    token_id: token_id.to_string(),
                    market_id: position.market_id.clone(),
                    sell_size,
                    threshold_index: idx,
                    threshold_price: threshold.price_threshold,
                    current_price,
                });
            }
        }

        None
    }

    /// Record that an exit was executed
    pub fn record_exit(&mut self, token_id: &str, sold_size: Decimal, threshold_index: usize) {
        if let Some(position) = self.positions.get_mut(token_id) {
            position.remaining_size -= sold_size;
            position.last_threshold_triggered = Some(threshold_index);
            position.last_exit_time = Some(chrono::Utc::now());

            info!(
                "✅ Recorded exit: {} sold {:.4}, remaining {:.4}",
                token_id, sold_size, position.remaining_size
            );

            // Remove if fully exited
            if position.remaining_size <= dec!(0.0001) {
                info!("📤 Position fully exited: {}", token_id);
            }
        }
    }

    /// Get all positions that should exit at current prices
    pub fn check_all_exits(
        &self,
        current_prices: &HashMap<String, Decimal>,
    ) -> Vec<ExitAction> {
        let mut actions = Vec::new();

        for (token_id, price) in current_prices {
            if let Some(action) = self.check_exit(token_id, *price) {
                actions.push(action);
            }
        }

        actions
    }

    /// Get tracked positions
    pub fn get_positions(&self) -> Vec<&TrackedPosition> {
        self.positions.values().collect()
    }

    /// Get a specific position
    pub fn get_position(&self, token_id: &str) -> Option<&TrackedPosition> {
        self.positions.get(token_id)
    }

    /// Calculate total unrealized profit for a position
    pub fn calculate_profit(&self, token_id: &str, current_price: Decimal) -> Option<ProfitInfo> {
        let position = self.positions.get(token_id)?;

        let value_at_entry = position.original_size * position.entry_price;
        let current_value = position.remaining_size * current_price;
        let _sold_value = (position.original_size - position.remaining_size) * current_price; // Approximate

        let unrealized_pnl = match position.side {
            Side::Buy => current_value - (position.remaining_size * position.entry_price),
            Side::Sell => (position.remaining_size * position.entry_price) - current_value,
        };

        let pnl_pct = if value_at_entry > Decimal::ZERO {
            unrealized_pnl / (position.remaining_size * position.entry_price) * dec!(100)
        } else {
            Decimal::ZERO
        };

        Some(ProfitInfo {
            unrealized_pnl,
            pnl_percentage: pnl_pct,
            remaining_value: current_value,
            original_value: value_at_entry,
        })
    }
}

/// Liquidity check result
#[derive(Debug, Clone)]
pub struct LiquidityCheck {
    pub sufficient: bool,
    pub available_liquidity: Decimal,
    pub expected_price: Decimal,
    pub expected_slippage: Decimal,
}

/// Exit action to execute
#[derive(Debug, Clone)]
pub struct ExitAction {
    pub token_id: String,
    pub market_id: String,
    pub sell_size: Decimal,
    pub threshold_index: usize,
    pub threshold_price: Decimal,
    pub current_price: Decimal,
}

/// Profit information
#[derive(Debug, Clone)]
pub struct ProfitInfo {
    pub unrealized_pnl: Decimal,
    pub pnl_percentage: Decimal,
    pub remaining_value: Decimal,
    pub original_value: Decimal,
}

impl Default for GradualExitManager {
    fn default() -> Self {
        Self::new(GradualExitConfig::default())
    }
}

/// Integrated executor that combines SmartExecutor with GradualExit
pub struct GradualExitExecutor {
    pub exit_manager: GradualExitManager,
    clob: ClobClient,
}

impl GradualExitExecutor {
    pub fn new(clob: ClobClient, config: GradualExitConfig) -> Self {
        Self {
            exit_manager: GradualExitManager::new(config),
            clob,
        }
    }

    /// Execute a gradual exit action with liquidity check
    pub async fn execute_exit(&mut self, action: &ExitAction) -> Result<Option<Trade>> {
        // Get orderbook and check liquidity
        let book = self.clob.get_order_book(&action.token_id).await?;
        let liquidity = self.exit_manager.check_liquidity(&book, action.sell_size);

        if !liquidity.sufficient {
            warn!(
                "⚠️ Insufficient liquidity for exit: ${:.2} available, {:.2}% slippage",
                liquidity.available_liquidity,
                liquidity.expected_slippage * dec!(100)
            );
            return Ok(None);
        }

        // Place limit order at expected price (slightly below to ensure fill)
        let limit_price = liquidity.expected_price * dec!(0.999); // 0.1% below

        let order = crate::types::Order {
            token_id: action.token_id.clone(),
            side: Side::Sell,
            price: limit_price,
            size: action.sell_size,
            order_type: crate::types::OrderType::GTC,
        };

        info!(
            "📤 Placing exit order: SELL {:.4} @ {:.4} (threshold {:.0}%)",
            action.sell_size,
            limit_price,
            action.threshold_price * dec!(100)
        );

        let order_status = self.clob.place_order(&order).await?;

        // Record the exit
        self.exit_manager.record_exit(
            &action.token_id,
            action.sell_size, // Optimistically record full size
            action.threshold_index,
        );

        let trade = Trade {
            id: uuid::Uuid::new_v4().to_string(),
            order_id: order_status.order_id,
            token_id: action.token_id.clone(),
            market_id: action.market_id.clone(),
            side: Side::Sell,
            price: limit_price,
            size: action.sell_size,
            fee: Decimal::ZERO,
            timestamp: chrono::Utc::now(),
        };

        Ok(Some(trade))
    }

    /// Process all exit actions for current prices
    pub async fn process_exits(
        &mut self,
        current_prices: &HashMap<String, Decimal>,
    ) -> Vec<Result<Option<Trade>>> {
        let actions = self.exit_manager.check_all_exits(current_prices);
        let mut results = Vec::new();

        for action in actions {
            results.push(self.execute_exit(&action).await);
        }

        results
    }
}

// Tests in gradual_exit_tests.rs
