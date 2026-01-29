//! Backtesting engine for strategy evaluation
//!
//! Simulates trading strategies against historical data.

use crate::error::Result;
use crate::storage::history::{Candle, HistoryStore, OrderBookSnapshot};
use crate::types::{Side, Signal};
use chrono::{DateTime, Duration, Utc};
use rust_decimal::{Decimal, MathematicalOps};
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Backtest configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestConfig {
    /// Initial capital (USDC)
    pub initial_capital: Decimal,
    /// Maximum position size as % of capital
    pub max_position_pct: Decimal,
    /// Trading fee (as decimal, e.g., 0.001 = 0.1%)
    pub fee_rate: Decimal,
    /// Slippage model
    pub slippage_bps: Decimal,
    /// Start time
    pub start_time: DateTime<Utc>,
    /// End time
    pub end_time: DateTime<Utc>,
    /// Candle timeframe (seconds)
    pub timeframe: i64,
}

impl Default for BacktestConfig {
    fn default() -> Self {
        Self {
            initial_capital: dec!(10000),
            max_position_pct: dec!(0.10), // 10% max per position
            fee_rate: dec!(0.001),         // 0.1% fee
            slippage_bps: dec!(5),         // 5 bps slippage
            start_time: Utc::now() - Duration::days(30),
            end_time: Utc::now(),
            timeframe: 3600, // 1 hour candles
        }
    }
}

/// Simulated position
#[derive(Debug, Clone)]
pub struct SimPosition {
    pub token_id: String,
    pub side: Side,
    pub size: Decimal,
    pub entry_price: Decimal,
    pub entry_time: DateTime<Utc>,
}

/// Single trade in backtest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimTrade {
    pub token_id: String,
    pub side: Side,
    pub price: Decimal,
    pub size: Decimal,
    pub fee: Decimal,
    pub pnl: Option<Decimal>,
    pub timestamp: DateTime<Utc>,
}

/// Backtest results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestResults {
    /// Total PnL
    pub total_pnl: Decimal,
    /// Return percentage
    pub return_pct: Decimal,
    /// Number of trades
    pub num_trades: usize,
    /// Winning trades
    pub winning_trades: usize,
    /// Losing trades
    pub losing_trades: usize,
    /// Win rate
    pub win_rate: Decimal,
    /// Average win
    pub avg_win: Decimal,
    /// Average loss
    pub avg_loss: Decimal,
    /// Profit factor (gross profit / gross loss)
    pub profit_factor: Option<Decimal>,
    /// Maximum drawdown
    pub max_drawdown: Decimal,
    /// Max drawdown percentage
    pub max_drawdown_pct: Decimal,
    /// Sharpe ratio (if enough data)
    pub sharpe_ratio: Option<Decimal>,
    /// All trades
    pub trades: Vec<SimTrade>,
    /// Equity curve (timestamps -> equity)
    pub equity_curve: Vec<(DateTime<Utc>, Decimal)>,
}

/// Strategy trait for backtesting
pub trait BacktestStrategy: Send + Sync {
    /// Generate signals from candle data
    fn on_candle(&mut self, token_id: &str, candle: &Candle, position: Option<&SimPosition>) -> Option<Signal>;
    
    /// Optional: process orderbook snapshot
    fn on_orderbook(&mut self, _snapshot: &OrderBookSnapshot) {}
    
    /// Strategy name
    fn name(&self) -> &str;
}

/// Backtesting engine
pub struct BacktestEngine {
    config: BacktestConfig,
    capital: Decimal,
    positions: HashMap<String, SimPosition>,
    trades: Vec<SimTrade>,
    equity_curve: Vec<(DateTime<Utc>, Decimal)>,
    peak_equity: Decimal,
    max_drawdown: Decimal,
}

impl BacktestEngine {
    pub fn new(config: BacktestConfig) -> Self {
        let initial = config.initial_capital;
        Self {
            config,
            capital: initial,
            positions: HashMap::new(),
            trades: Vec::new(),
            equity_curve: Vec::new(),
            peak_equity: initial,
            max_drawdown: Decimal::ZERO,
        }
    }

    /// Run backtest with historical data
    pub async fn run<S: BacktestStrategy>(
        &mut self,
        strategy: &mut S,
        history: &HistoryStore,
        token_ids: &[String],
    ) -> Result<BacktestResults> {
        tracing::info!(
            "Starting backtest: {} from {} to {}",
            strategy.name(),
            self.config.start_time,
            self.config.end_time
        );

        // Load candles for all tokens
        let mut all_candles: Vec<(String, Candle)> = Vec::new();
        
        for token_id in token_ids {
            let candles = history
                .get_candles(
                    token_id,
                    self.config.timeframe,
                    self.config.start_time,
                    self.config.end_time,
                )
                .await?;
            
            for candle in candles {
                all_candles.push((token_id.clone(), candle));
            }
        }

        // Sort by timestamp
        all_candles.sort_by_key(|(_, c)| c.timestamp);

        // Process each candle
        for (token_id, candle) in all_candles {
            let position = self.positions.get(&token_id);
            
            // Get strategy signal
            if let Some(signal) = strategy.on_candle(&token_id, &candle, position) {
                self.process_signal(&signal, &candle)?;
            }

            // Update equity curve
            let equity = self.calculate_equity(&candle);
            self.equity_curve.push((candle.timestamp, equity));

            // Update drawdown
            if equity > self.peak_equity {
                self.peak_equity = equity;
            }
            let drawdown = self.peak_equity - equity;
            if drawdown > self.max_drawdown {
                self.max_drawdown = drawdown;
            }
        }

        // Close any remaining positions at last price
        self.close_all_positions()?;

        Ok(self.calculate_results())
    }

    /// Process a signal
    fn process_signal(&mut self, signal: &Signal, candle: &Candle) -> Result<()> {
        let existing_position = self.positions.get(&signal.token_id);
        
        match (existing_position, signal.side) {
            // Open new position
            (None, _) => {
                let max_size = self.capital * self.config.max_position_pct;
                let size = signal.suggested_size.min(max_size);
                
                if size > Decimal::ZERO {
                    let price = self.apply_slippage(candle.close, signal.side);
                    let fee = size * self.config.fee_rate;
                    
                    if size + fee <= self.capital {
                        self.capital -= size + fee;
                        
                        self.positions.insert(signal.token_id.clone(), SimPosition {
                            token_id: signal.token_id.clone(),
                            side: signal.side,
                            size,
                            entry_price: price,
                            entry_time: candle.timestamp,
                        });

                        self.trades.push(SimTrade {
                            token_id: signal.token_id.clone(),
                            side: signal.side,
                            price,
                            size,
                            fee,
                            pnl: None,
                            timestamp: candle.timestamp,
                        });
                    }
                }
            }
            // Close position (opposite signal)
            (Some(pos), side) if side != pos.side => {
                let exit_price = self.apply_slippage(candle.close, side);
                let pnl = self.calculate_pnl(pos, exit_price);
                let fee = pos.size * self.config.fee_rate;
                
                self.capital += pos.size + pnl - fee;
                
                self.trades.push(SimTrade {
                    token_id: signal.token_id.clone(),
                    side,
                    price: exit_price,
                    size: pos.size,
                    fee,
                    pnl: Some(pnl),
                    timestamp: candle.timestamp,
                });
                
                self.positions.remove(&signal.token_id);
            }
            // Same direction - could increase position, skip for now
            _ => {}
        }

        Ok(())
    }

    fn apply_slippage(&self, price: Decimal, side: Side) -> Decimal {
        let slippage = price * self.config.slippage_bps / dec!(10000);
        match side {
            Side::Buy => price + slippage,
            Side::Sell => price - slippage,
        }
    }

    fn calculate_pnl(&self, position: &SimPosition, exit_price: Decimal) -> Decimal {
        match position.side {
            Side::Buy => (exit_price - position.entry_price) * position.size / position.entry_price,
            Side::Sell => (position.entry_price - exit_price) * position.size / position.entry_price,
        }
    }

    fn calculate_equity(&self, current_candle: &Candle) -> Decimal {
        let mut equity = self.capital;
        
        for position in self.positions.values() {
            // Simple: use close price of current candle
            let unrealized = self.calculate_pnl(position, current_candle.close);
            equity += position.size + unrealized;
        }
        
        equity
    }

    fn close_all_positions(&mut self) -> Result<()> {
        let positions: Vec<_> = self.positions.values().cloned().collect();
        let now = Utc::now();
        
        for pos in positions {
            // Use entry price as last known price (conservative)
            let exit_price = pos.entry_price;
            let pnl = self.calculate_pnl(&pos, exit_price);
            let fee = pos.size * self.config.fee_rate;
            
            self.capital += pos.size + pnl - fee;
            
            self.trades.push(SimTrade {
                token_id: pos.token_id.clone(),
                side: match pos.side {
                    Side::Buy => Side::Sell,
                    Side::Sell => Side::Buy,
                },
                price: exit_price,
                size: pos.size,
                fee,
                pnl: Some(pnl),
                timestamp: now,
            });
        }
        
        self.positions.clear();
        Ok(())
    }

    fn calculate_results(&self) -> BacktestResults {
        let initial = self.config.initial_capital;
        let final_capital = self.capital;
        let total_pnl = final_capital - initial;
        let return_pct = total_pnl / initial * dec!(100);

        let closed_trades: Vec<_> = self.trades.iter()
            .filter(|t| t.pnl.is_some())
            .collect();

        let num_trades = closed_trades.len();
        let winning_trades = closed_trades.iter()
            .filter(|t| t.pnl.unwrap_or(Decimal::ZERO) > Decimal::ZERO)
            .count();
        let losing_trades = num_trades - winning_trades;

        let win_rate = if num_trades > 0 {
            Decimal::from(winning_trades) / Decimal::from(num_trades) * dec!(100)
        } else {
            Decimal::ZERO
        };

        let wins: Vec<Decimal> = closed_trades.iter()
            .filter_map(|t| t.pnl)
            .filter(|p| *p > Decimal::ZERO)
            .collect();
        let losses: Vec<Decimal> = closed_trades.iter()
            .filter_map(|t| t.pnl)
            .filter(|p| *p < Decimal::ZERO)
            .map(|p| p.abs())
            .collect();

        let avg_win = if !wins.is_empty() {
            wins.iter().sum::<Decimal>() / Decimal::from(wins.len())
        } else {
            Decimal::ZERO
        };

        let avg_loss = if !losses.is_empty() {
            losses.iter().sum::<Decimal>() / Decimal::from(losses.len())
        } else {
            Decimal::ZERO
        };

        let gross_profit: Decimal = wins.iter().sum();
        let gross_loss: Decimal = losses.iter().sum();
        let profit_factor = if gross_loss > Decimal::ZERO {
            Some(gross_profit / gross_loss)
        } else {
            None
        };

        let max_drawdown_pct = if self.peak_equity > Decimal::ZERO {
            self.max_drawdown / self.peak_equity * dec!(100)
        } else {
            Decimal::ZERO
        };

        // Calculate Sharpe ratio
        let sharpe_ratio = self.calculate_sharpe();

        BacktestResults {
            total_pnl,
            return_pct,
            num_trades,
            winning_trades,
            losing_trades,
            win_rate,
            avg_win,
            avg_loss,
            profit_factor,
            max_drawdown: self.max_drawdown,
            max_drawdown_pct,
            sharpe_ratio,
            trades: self.trades.clone(),
            equity_curve: self.equity_curve.clone(),
        }
    }

    fn calculate_sharpe(&self) -> Option<Decimal> {
        if self.equity_curve.len() < 10 {
            return None;
        }

        // Calculate returns
        let returns: Vec<Decimal> = self.equity_curve
            .windows(2)
            .filter_map(|w| {
                if w[0].1 > Decimal::ZERO {
                    Some((w[1].1 - w[0].1) / w[0].1)
                } else {
                    None
                }
            })
            .collect();

        if returns.is_empty() {
            return None;
        }

        let n = Decimal::from(returns.len());
        let mean = returns.iter().sum::<Decimal>() / n;
        
        let variance: Decimal = returns.iter()
            .map(|r| (*r - mean) * (*r - mean))
            .sum::<Decimal>() / n;

        // Simple stddev approximation
        let stddev = variance.sqrt()?;
        
        if stddev == Decimal::ZERO {
            return None;
        }

        // Annualize (assuming hourly data)
        let annualized_return = mean * dec!(8760); // 24 * 365
        let annualized_std = stddev * Decimal::from(8760).sqrt()?;

        Some(annualized_return / annualized_std)
    }
}

/// Simple momentum strategy for testing
pub struct MomentumStrategy {
    lookback: usize,
    threshold: Decimal,
    history: HashMap<String, Vec<Decimal>>,
}

impl MomentumStrategy {
    pub fn new(lookback: usize, threshold: Decimal) -> Self {
        Self {
            lookback,
            threshold,
            history: HashMap::new(),
        }
    }
}

impl BacktestStrategy for MomentumStrategy {
    fn on_candle(&mut self, token_id: &str, candle: &Candle, position: Option<&SimPosition>) -> Option<Signal> {
        let prices = self.history.entry(token_id.to_string()).or_default();
        prices.push(candle.close);
        
        if prices.len() > self.lookback * 2 {
            prices.remove(0);
        }

        if prices.len() < self.lookback * 2 {
            return None;
        }

        let recent_avg = prices.iter().rev().take(self.lookback).sum::<Decimal>() 
            / Decimal::from(self.lookback);
        let older_avg = prices.iter().skip(prices.len().saturating_sub(self.lookback * 2)).take(self.lookback).sum::<Decimal>()
            / Decimal::from(self.lookback);

        let momentum = (recent_avg - older_avg) / older_avg;

        // Generate signal based on momentum
        if momentum > self.threshold && position.is_none() {
            Some(Signal {
                market_id: token_id.to_string(),
                token_id: token_id.to_string(),
                side: Side::Buy,
                model_probability: candle.close,
                market_probability: candle.close,
                edge: momentum,
                confidence: dec!(0.7),
                suggested_size: dec!(100),
                timestamp: candle.timestamp,
            })
        } else if momentum < -self.threshold && position.is_some() {
            Some(Signal {
                market_id: token_id.to_string(),
                token_id: token_id.to_string(),
                side: Side::Sell,
                model_probability: candle.close,
                market_probability: candle.close,
                edge: momentum.abs(),
                confidence: dec!(0.7),
                suggested_size: dec!(100),
                timestamp: candle.timestamp,
            })
        } else {
            None
        }
    }

    fn name(&self) -> &str {
        "momentum"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_backtest_config_default() {
        let config = BacktestConfig::default();
        assert_eq!(config.initial_capital, dec!(10000));
        assert_eq!(config.fee_rate, dec!(0.001));
    }

    #[test]
    fn test_slippage() {
        let config = BacktestConfig::default();
        let engine = BacktestEngine::new(config);
        
        let buy_price = engine.apply_slippage(dec!(0.50), Side::Buy);
        assert!(buy_price > dec!(0.50)); // Buy slippage increases price
        
        let sell_price = engine.apply_slippage(dec!(0.50), Side::Sell);
        assert!(sell_price < dec!(0.50)); // Sell slippage decreases price
    }

    #[test]
    fn test_pnl_calculation() {
        let config = BacktestConfig::default();
        let engine = BacktestEngine::new(config);

        let position = SimPosition {
            token_id: "test".to_string(),
            side: Side::Buy,
            size: dec!(100),
            entry_price: dec!(0.50),
            entry_time: Utc::now(),
        };

        // Price went up 10%
        let pnl = engine.calculate_pnl(&position, dec!(0.55));
        assert_eq!(pnl, dec!(10)); // 10% of 100
    }

    #[test]
    fn test_momentum_strategy() {
        let mut strategy = MomentumStrategy::new(3, dec!(0.05));
        
        // Create candles with upward momentum
        let candles: Vec<Candle> = (0..10).map(|i| Candle {
            token_id: "test".to_string(),
            timestamp: Utc::now(),
            open: Decimal::from(50 + i),
            high: Decimal::from(51 + i),
            low: Decimal::from(49 + i),
            close: Decimal::from(50 + i),
            volume: dec!(1000),
            timeframe: 60,
        }).collect();

        // Process candles
        for candle in &candles {
            let _ = strategy.on_candle("test", candle, None);
        }

        // Should have some history
        assert!(!strategy.history.is_empty());
    }
}
