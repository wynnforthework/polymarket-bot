//! Advanced Risk Management Module
//!
//! Provides comprehensive risk controls:
//! - Daily P&L tracking with loss limits
//! - Volatility-adaptive position sizing
//! - Market correlation detection
//! - Dynamic position management

mod daily_pnl;
mod volatility_sizer;
mod correlation;
mod position_manager;

#[cfg(test)]
mod tests;

pub use daily_pnl::{DailyPnlTracker, DailyPnlState};
pub use volatility_sizer::{VolatilityPositionSizer, VolatilityConfig};
pub use correlation::{CorrelationDetector, CorrelationMatrix, MarketCorrelation};
pub use position_manager::{DynamicPositionManager, PositionSizeRequest, PositionSizeResult};

use crate::config::RiskConfig;
use crate::types::{Market, Position, Signal};
use rust_decimal::Decimal;

/// Integrated risk manager combining all risk controls
pub struct RiskManager {
    pub config: RiskConfig,
    pub pnl_tracker: DailyPnlTracker,
    pub volatility_sizer: VolatilityPositionSizer,
    pub correlation_detector: CorrelationDetector,
    pub position_manager: DynamicPositionManager,
}

impl RiskManager {
    /// Create a new risk manager with the given configuration
    pub fn new(config: RiskConfig) -> Self {
        let volatility_config = VolatilityConfig::default();
        
        Self {
            pnl_tracker: DailyPnlTracker::new(config.max_daily_loss_pct),
            volatility_sizer: VolatilityPositionSizer::new(volatility_config),
            correlation_detector: CorrelationDetector::new(0.7), // 70% correlation threshold
            position_manager: DynamicPositionManager::new(config.clone()),
            config,
        }
    }

    /// Check if trading is allowed based on all risk constraints
    pub fn can_trade(&self) -> RiskCheckResult {
        // Check daily loss limit
        if self.pnl_tracker.is_limit_reached() {
            return RiskCheckResult::Blocked {
                reason: "Daily loss limit reached".to_string(),
            };
        }

        RiskCheckResult::Allowed
    }

    /// Calculate the maximum position size for a signal
    pub fn calculate_position_size(
        &mut self,
        signal: &Signal,
        market: &Market,
        balance: Decimal,
        current_positions: &[Position],
    ) -> Option<Decimal> {
        // First check if we can trade at all
        if let RiskCheckResult::Blocked { .. } = self.can_trade() {
            return None;
        }

        // Check position count limit
        if current_positions.len() >= self.config.max_open_positions {
            return None;
        }

        // Build position size request
        let request = PositionSizeRequest {
            signal_confidence: signal.confidence,
            signal_edge: signal.edge,
            market_id: market.id.clone(),
            balance,
            current_exposure: self.calculate_exposure(current_positions),
        };

        // Get base position size from dynamic manager
        let result = self.position_manager.calculate_size(&request);

        // Adjust for volatility
        let volatility_multiplier = self.volatility_sizer.get_size_multiplier(&market.id);
        let adjusted_size = result.size * volatility_multiplier;

        // Check correlation - reduce size if highly correlated with existing positions
        let position_markets: Vec<String> = current_positions
            .iter()
            .map(|p| p.market_id.clone())
            .collect();
        
        let correlation_penalty = self.correlation_detector
            .get_correlation_penalty(&market.id, &position_markets);
        
        let final_size = adjusted_size * correlation_penalty;

        // Ensure minimum viable size
        if final_size < Decimal::new(1, 0) {
            return None;
        }

        Some(final_size)
    }

    /// Record a trade execution for P&L tracking
    pub fn record_trade(&mut self, pnl: Decimal) {
        self.pnl_tracker.record_pnl(pnl);
    }

    /// Update volatility data for a market
    pub fn update_volatility(&mut self, market_id: &str, price: Decimal) {
        self.volatility_sizer.add_price_point(market_id, price);
    }

    /// Update correlation data
    pub fn update_correlation(&mut self, market_id: &str, price: Decimal, timestamp: i64) {
        self.correlation_detector.add_price_point(market_id, price, timestamp);
    }

    /// Get current daily P&L
    pub fn daily_pnl(&self) -> Decimal {
        self.pnl_tracker.current_pnl()
    }

    /// Reset daily trackers (call at start of new day)
    pub fn reset_daily(&mut self) {
        self.pnl_tracker.reset();
    }

    /// Calculate total exposure from positions
    fn calculate_exposure(&self, positions: &[Position]) -> Decimal {
        positions.iter()
            .map(|p| p.size * p.current_price)
            .sum()
    }
}

/// Result of a risk check
#[derive(Debug, Clone, PartialEq)]
pub enum RiskCheckResult {
    Allowed,
    Blocked { reason: String },
}
