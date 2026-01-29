//! Take-profit strategy - exit positions early when profitable
//!
//! Don't wait for market settlement, sell when price moves in our favor.

use crate::error::Result;
use crate::types::{Side, Signal};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::HashMap;
use tracing::info;

/// Position tracking for take-profit
#[derive(Debug, Clone)]
pub struct Position {
    pub market_id: String,
    pub token_id: String,
    pub side: Side,
    pub entry_price: Decimal,
    pub size: Decimal,
    pub entry_time: DateTime<Utc>,
}

/// Take-profit manager
pub struct TakeProfitManager {
    /// Active positions
    positions: HashMap<String, Position>,
    /// Take-profit threshold (e.g., 0.05 = 5% profit)
    take_profit_pct: Decimal,
    /// Stop-loss threshold (e.g., 0.10 = 10% loss)  
    stop_loss_pct: Decimal,
    /// Max hold time before forced exit (hours)
    max_hold_hours: u32,
}

impl TakeProfitManager {
    pub fn new() -> Self {
        Self {
            positions: HashMap::new(),
            take_profit_pct: dec!(0.08),  // 8% profit target
            stop_loss_pct: dec!(0.15),    // 15% stop loss
            max_hold_hours: 4,            // Exit after 4 hours max
        }
    }

    /// Record a new position
    pub fn open_position(&mut self, signal: &Signal, size: Decimal) {
        let position = Position {
            market_id: signal.market_id.clone(),
            token_id: signal.token_id.clone(),
            side: signal.side.clone(),
            entry_price: signal.market_probability,
            size,
            entry_time: Utc::now(),
        };
        
        info!("📈 Opened position: {} @ {:.1}% size ${:.2}", 
            signal.market_id, signal.market_probability * dec!(100), size);
        
        self.positions.insert(signal.market_id.clone(), position);
    }

    /// Check if we should exit a position
    pub fn check_exit(&self, market_id: &str, current_price: Decimal) -> Option<ExitSignal> {
        let position = self.positions.get(market_id)?;
        
        let pnl_pct = match position.side {
            Side::Buy => (current_price - position.entry_price) / position.entry_price,
            Side::Sell => (position.entry_price - current_price) / position.entry_price,
        };
        
        let hold_hours = (Utc::now() - position.entry_time).num_hours() as u32;
        
        // Take profit
        if pnl_pct >= self.take_profit_pct {
            info!("💰 TAKE PROFIT: {} +{:.1}% @ {:.1}%", 
                market_id, pnl_pct * dec!(100), current_price * dec!(100));
            return Some(ExitSignal {
                market_id: market_id.to_string(),
                token_id: position.token_id.clone(),
                reason: ExitReason::TakeProfit,
                pnl_pct,
                size: position.size,
            });
        }
        
        // Stop loss
        if pnl_pct <= -self.stop_loss_pct {
            info!("🛑 STOP LOSS: {} {:.1}% @ {:.1}%", 
                market_id, pnl_pct * dec!(100), current_price * dec!(100));
            return Some(ExitSignal {
                market_id: market_id.to_string(),
                token_id: position.token_id.clone(),
                reason: ExitReason::StopLoss,
                pnl_pct,
                size: position.size,
            });
        }
        
        // Time-based exit
        if hold_hours >= self.max_hold_hours {
            info!("⏰ TIME EXIT: {} held {}h, pnl {:.1}%", 
                market_id, hold_hours, pnl_pct * dec!(100));
            return Some(ExitSignal {
                market_id: market_id.to_string(),
                token_id: position.token_id.clone(),
                reason: ExitReason::TimeLimit,
                pnl_pct,
                size: position.size,
            });
        }
        
        None
    }

    /// Close a position
    pub fn close_position(&mut self, market_id: &str) -> Option<Position> {
        self.positions.remove(market_id)
    }

    /// Get all open positions
    pub fn get_positions(&self) -> Vec<&Position> {
        self.positions.values().collect()
    }

    /// Check all positions for exits
    pub fn check_all_exits(&self, current_prices: &HashMap<String, Decimal>) -> Vec<ExitSignal> {
        let mut exits = Vec::new();
        
        for (market_id, price) in current_prices {
            if let Some(exit) = self.check_exit(market_id, *price) {
                exits.push(exit);
            }
        }
        
        exits
    }
}

#[derive(Debug, Clone)]
pub struct ExitSignal {
    pub market_id: String,
    pub token_id: String,
    pub reason: ExitReason,
    pub pnl_pct: Decimal,
    pub size: Decimal,
}

#[derive(Debug, Clone)]
pub enum ExitReason {
    TakeProfit,
    StopLoss,
    TimeLimit,
}

impl Default for TakeProfitManager {
    fn default() -> Self {
        Self::new()
    }
}
