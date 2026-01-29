//! Daily P&L Tracking with Loss Limits

use chrono::Utc;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Tracks daily profit and loss with automatic reset
#[derive(Debug, Clone)]
pub struct DailyPnlTracker {
    /// Current day's P&L
    state: DailyPnlState,
    /// Maximum daily loss as a percentage (e.g., 0.10 = 10%)
    max_loss_pct: Decimal,
    /// Starting balance for the day (set on first trade)
    starting_balance: Option<Decimal>,
}

/// Serializable state for persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyPnlState {
    pub date: String,
    pub realized_pnl: Decimal,
    pub trade_count: u32,
    pub winning_trades: u32,
    pub losing_trades: u32,
    pub largest_win: Decimal,
    pub largest_loss: Decimal,
}

impl DailyPnlState {
    pub fn new() -> Self {
        Self {
            date: Utc::now().format("%Y-%m-%d").to_string(),
            realized_pnl: Decimal::ZERO,
            trade_count: 0,
            winning_trades: 0,
            losing_trades: 0,
            largest_win: Decimal::ZERO,
            largest_loss: Decimal::ZERO,
        }
    }

    /// Check if this state is for the current day
    pub fn is_current_day(&self) -> bool {
        let today = Utc::now().format("%Y-%m-%d").to_string();
        self.date == today
    }
}

impl Default for DailyPnlState {
    fn default() -> Self {
        Self::new()
    }
}

impl DailyPnlTracker {
    /// Create a new tracker with the specified daily loss limit
    pub fn new(max_loss_pct: Decimal) -> Self {
        Self {
            state: DailyPnlState::new(),
            max_loss_pct,
            starting_balance: None,
        }
    }

    /// Set the starting balance for percentage calculations
    pub fn set_starting_balance(&mut self, balance: Decimal) {
        if self.starting_balance.is_none() {
            self.starting_balance = Some(balance);
        }
    }

    /// Record a trade's P&L
    pub fn record_pnl(&mut self, pnl: Decimal) {
        self.check_and_reset_day();
        
        self.state.realized_pnl += pnl;
        self.state.trade_count += 1;

        if pnl > Decimal::ZERO {
            self.state.winning_trades += 1;
            if pnl > self.state.largest_win {
                self.state.largest_win = pnl;
            }
        } else if pnl < Decimal::ZERO {
            self.state.losing_trades += 1;
            if pnl < self.state.largest_loss {
                self.state.largest_loss = pnl;
            }
        }
    }

    /// Check if daily loss limit has been reached
    pub fn is_limit_reached(&self) -> bool {
        if self.state.realized_pnl >= Decimal::ZERO {
            return false;
        }

        let Some(starting_balance) = self.starting_balance else {
            // If no starting balance set, use absolute threshold
            return self.state.realized_pnl < Decimal::new(-1000, 0); // $1000 default
        };

        let loss_pct = (self.state.realized_pnl.abs() / starting_balance) * Decimal::new(100, 0);
        loss_pct >= self.max_loss_pct * Decimal::new(100, 0)
    }

    /// Get current P&L
    pub fn current_pnl(&self) -> Decimal {
        self.state.realized_pnl
    }

    /// Get remaining loss budget before limit
    pub fn remaining_loss_budget(&self) -> Option<Decimal> {
        let starting = self.starting_balance?;
        let max_loss = starting * self.max_loss_pct;
        let current_loss = self.state.realized_pnl.abs();
        
        if self.state.realized_pnl >= Decimal::ZERO {
            Some(max_loss)
        } else {
            Some(max_loss - current_loss)
        }
    }

    /// Get the current state (for persistence)
    pub fn state(&self) -> &DailyPnlState {
        &self.state
    }

    /// Restore from persisted state
    pub fn restore_state(&mut self, state: DailyPnlState) {
        if state.is_current_day() {
            self.state = state;
        }
    }

    /// Reset for a new day
    pub fn reset(&mut self) {
        self.state = DailyPnlState::new();
        self.starting_balance = None;
    }

    /// Get win rate as a percentage
    pub fn win_rate(&self) -> Option<f64> {
        if self.state.trade_count == 0 {
            return None;
        }
        Some(self.state.winning_trades as f64 / self.state.trade_count as f64 * 100.0)
    }

    /// Check if we've crossed into a new day and reset if needed
    fn check_and_reset_day(&mut self) {
        if !self.state.is_current_day() {
            self.reset();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_tracker() {
        let tracker = DailyPnlTracker::new(Decimal::new(10, 2)); // 10%
        assert_eq!(tracker.current_pnl(), Decimal::ZERO);
        assert!(!tracker.is_limit_reached());
    }

    #[test]
    fn test_record_winning_trade() {
        let mut tracker = DailyPnlTracker::new(Decimal::new(10, 2));
        tracker.record_pnl(Decimal::new(50, 0));
        
        assert_eq!(tracker.current_pnl(), Decimal::new(50, 0));
        assert_eq!(tracker.state.trade_count, 1);
        assert_eq!(tracker.state.winning_trades, 1);
        assert_eq!(tracker.state.largest_win, Decimal::new(50, 0));
    }

    #[test]
    fn test_record_losing_trade() {
        let mut tracker = DailyPnlTracker::new(Decimal::new(10, 2));
        tracker.record_pnl(Decimal::new(-30, 0));
        
        assert_eq!(tracker.current_pnl(), Decimal::new(-30, 0));
        assert_eq!(tracker.state.losing_trades, 1);
        assert_eq!(tracker.state.largest_loss, Decimal::new(-30, 0));
    }

    #[test]
    fn test_daily_loss_limit() {
        let mut tracker = DailyPnlTracker::new(Decimal::new(10, 2)); // 10%
        tracker.set_starting_balance(Decimal::new(1000, 0));
        
        // Not reached yet
        tracker.record_pnl(Decimal::new(-50, 0));
        assert!(!tracker.is_limit_reached());
        
        // Now at limit
        tracker.record_pnl(Decimal::new(-50, 0));
        assert!(tracker.is_limit_reached());
    }

    #[test]
    fn test_remaining_budget() {
        let mut tracker = DailyPnlTracker::new(Decimal::new(10, 2)); // 10%
        tracker.set_starting_balance(Decimal::new(1000, 0));
        
        let budget = tracker.remaining_loss_budget().unwrap();
        assert_eq!(budget, Decimal::new(100, 0)); // 10% of 1000
        
        tracker.record_pnl(Decimal::new(-30, 0));
        let budget = tracker.remaining_loss_budget().unwrap();
        assert_eq!(budget, Decimal::new(70, 0));
    }

    #[test]
    fn test_win_rate() {
        let mut tracker = DailyPnlTracker::new(Decimal::new(10, 2));
        
        assert!(tracker.win_rate().is_none());
        
        tracker.record_pnl(Decimal::new(10, 0));
        tracker.record_pnl(Decimal::new(-5, 0));
        tracker.record_pnl(Decimal::new(20, 0));
        tracker.record_pnl(Decimal::new(15, 0));
        
        let rate = tracker.win_rate().unwrap();
        assert!((rate - 75.0).abs() < 0.01);
    }

    #[test]
    fn test_reset() {
        let mut tracker = DailyPnlTracker::new(Decimal::new(10, 2));
        tracker.set_starting_balance(Decimal::new(1000, 0));
        tracker.record_pnl(Decimal::new(100, 0));
        
        tracker.reset();
        
        assert_eq!(tracker.current_pnl(), Decimal::ZERO);
        assert_eq!(tracker.state.trade_count, 0);
    }

    #[test]
    fn test_state_persistence() {
        let mut tracker = DailyPnlTracker::new(Decimal::new(10, 2));
        tracker.record_pnl(Decimal::new(100, 0));
        
        let state = tracker.state().clone();
        
        let mut new_tracker = DailyPnlTracker::new(Decimal::new(10, 2));
        new_tracker.restore_state(state);
        
        assert_eq!(new_tracker.current_pnl(), Decimal::new(100, 0));
    }
}
