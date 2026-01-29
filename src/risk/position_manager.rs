//! Dynamic Position Management
//!
//! Manages position sizing based on:
//! - Signal confidence (0.5 - 1.0)
//! - Account balance percentage limits
//! - Kelly criterion with fractional scaling

use crate::config::RiskConfig;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

/// Dynamic position size calculator
pub struct DynamicPositionManager {
    config: RiskConfig,
}

/// Request for position size calculation
#[derive(Debug, Clone)]
pub struct PositionSizeRequest {
    /// Signal confidence (0.5 to 1.0)
    pub signal_confidence: Decimal,
    /// Edge (model probability - market probability)
    pub signal_edge: Decimal,
    /// Market ID
    pub market_id: String,
    /// Current account balance
    pub balance: Decimal,
    /// Current total exposure
    pub current_exposure: Decimal,
}

/// Result of position size calculation
#[derive(Debug, Clone)]
pub struct PositionSizeResult {
    /// Calculated position size in USDC
    pub size: Decimal,
    /// Base size before adjustments
    pub base_size: Decimal,
    /// Confidence multiplier applied
    pub confidence_multiplier: Decimal,
    /// Kelly fraction used
    pub kelly_fraction: Decimal,
    /// Whether size was capped by limits
    pub was_capped: bool,
    /// Reason for any cap applied
    pub cap_reason: Option<String>,
}

impl DynamicPositionManager {
    pub fn new(config: RiskConfig) -> Self {
        Self { config }
    }

    /// Calculate optimal position size
    pub fn calculate_size(&self, request: &PositionSizeRequest) -> PositionSizeResult {
        // 1. Calculate Kelly-optimal size
        let kelly_size = self.kelly_position_size(
            request.balance,
            request.signal_confidence,
            request.signal_edge,
        );

        // 2. Apply confidence-based scaling
        let confidence_multiplier = self.confidence_multiplier(request.signal_confidence);
        let scaled_size = kelly_size * confidence_multiplier;

        // 3. Apply position limits
        let (final_size, was_capped, cap_reason) = self.apply_limits(
            scaled_size,
            request.balance,
            request.current_exposure,
        );

        PositionSizeResult {
            size: final_size,
            base_size: kelly_size,
            confidence_multiplier,
            kelly_fraction: self.effective_kelly_fraction(request.signal_confidence),
            was_capped,
            cap_reason,
        }
    }

    /// Calculate Kelly-optimal position size
    fn kelly_position_size(
        &self,
        balance: Decimal,
        confidence: Decimal,
        edge: Decimal,
    ) -> Decimal {
        // Full Kelly: f* = (bp - q) / b
        // where b = odds (edge), p = probability of win, q = 1-p
        //
        // Simplified for prediction markets:
        // f* = edge * confidence
        
        let edge_factor = edge.abs();
        let kelly_fraction = self.effective_kelly_fraction(confidence);
        
        // Fraction of bankroll to bet
        let bet_fraction = edge_factor * kelly_fraction;
        
        // Calculate size
        let available = balance - self.config.min_balance_reserve;
        if available <= Decimal::ZERO {
            return Decimal::ZERO;
        }

        available * bet_fraction
    }

    /// Get effective Kelly fraction based on confidence
    /// Higher confidence = closer to full Kelly
    fn effective_kelly_fraction(&self, confidence: Decimal) -> Decimal {
        // Base Kelly fraction from config (e.g., 0.25 for quarter Kelly)
        let base = dec!(0.25); // Conservative default
        
        // Scale based on confidence: 50% confidence = 50% of base, 100% confidence = 100% of base
        // This maps [0.5, 1.0] confidence to [0.5, 1.0] multiplier
        let confidence_factor = (confidence - dec!(0.5)) * dec!(2);
        let confidence_factor = confidence_factor.max(Decimal::ZERO).min(Decimal::ONE);
        
        // Additional scaling: high confidence (>80%) gets more aggressive
        let aggression = if confidence > dec!(0.80) {
            dec!(1.2)
        } else if confidence > dec!(0.70) {
            dec!(1.0)
        } else {
            dec!(0.8)
        };
        
        base * (dec!(0.5) + confidence_factor * dec!(0.5)) * aggression
    }

    /// Calculate confidence-based multiplier
    /// Maps confidence from [0.5, 1.0] to a position size multiplier
    fn confidence_multiplier(&self, confidence: Decimal) -> Decimal {
        // Ensure confidence is in valid range
        let confidence = confidence.max(dec!(0.5)).min(Decimal::ONE);
        
        // Linear scaling: 50% confidence = 0.5x, 100% confidence = 1.5x
        // Formula: 0.5 + (confidence - 0.5) * 2
        dec!(0.5) + (confidence - dec!(0.5)) * dec!(2)
    }

    /// Apply position size limits
    fn apply_limits(
        &self,
        size: Decimal,
        balance: Decimal,
        current_exposure: Decimal,
    ) -> (Decimal, bool, Option<String>) {
        let mut final_size = size;
        let mut was_capped = false;
        let mut cap_reason = None;

        // 1. Maximum position size as percentage of portfolio
        let max_position = balance * self.config.max_position_pct;
        if final_size > max_position {
            final_size = max_position;
            was_capped = true;
            cap_reason = Some(format!(
                "Position capped at {}% of portfolio",
                self.config.max_position_pct * dec!(100)
            ));
        }

        // 2. Maximum total exposure
        let max_exposure = balance * self.config.max_exposure_pct;
        let new_exposure = current_exposure + final_size;
        if new_exposure > max_exposure {
            let remaining_exposure = max_exposure - current_exposure;
            if remaining_exposure > Decimal::ZERO {
                final_size = remaining_exposure;
                was_capped = true;
                cap_reason = Some(format!(
                    "Position reduced to stay within {}% total exposure",
                    self.config.max_exposure_pct * dec!(100)
                ));
            } else {
                final_size = Decimal::ZERO;
                was_capped = true;
                cap_reason = Some("Maximum exposure limit reached".to_string());
            }
        }

        // 3. Ensure minimum reserve
        let available = balance - self.config.min_balance_reserve;
        if final_size > available {
            final_size = available.max(Decimal::ZERO);
            was_capped = true;
            cap_reason = Some(format!(
                "Position reduced to maintain ${} reserve",
                self.config.min_balance_reserve
            ));
        }

        // 4. Minimum viable position size
        let min_position = dec!(5); // $5 minimum
        if final_size < min_position && final_size > Decimal::ZERO {
            final_size = Decimal::ZERO;
            was_capped = true;
            cap_reason = Some("Position too small (< $5 minimum)".to_string());
        }

        (final_size, was_capped, cap_reason)
    }

    /// Calculate recommended position based on account balance percentage
    pub fn balance_based_size(&self, balance: Decimal, percentage: Decimal) -> Decimal {
        let available = balance - self.config.min_balance_reserve;
        if available <= Decimal::ZERO {
            return Decimal::ZERO;
        }

        let size = available * percentage;
        let max_position = balance * self.config.max_position_pct;
        
        size.min(max_position)
    }

    /// Get the maximum allowable position size given current state
    pub fn max_allowable_position(&self, balance: Decimal, current_exposure: Decimal) -> Decimal {
        let max_by_position = balance * self.config.max_position_pct;
        let max_by_exposure = (balance * self.config.max_exposure_pct) - current_exposure;
        let max_by_reserve = balance - self.config.min_balance_reserve;

        max_by_position
            .min(max_by_exposure)
            .min(max_by_reserve)
            .max(Decimal::ZERO)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> RiskConfig {
        RiskConfig {
            max_position_pct: dec!(0.05),      // 5%
            max_exposure_pct: dec!(0.50),       // 50%
            max_daily_loss_pct: dec!(0.10),     // 10%
            min_balance_reserve: dec!(100),     // $100
            max_open_positions: 10,
        }
    }

    #[test]
    fn test_new_position_manager() {
        let config = test_config();
        let _manager = DynamicPositionManager::new(config);
    }

    #[test]
    fn test_confidence_multiplier() {
        let manager = DynamicPositionManager::new(test_config());
        
        // 50% confidence = 0.5x multiplier
        let mult = manager.confidence_multiplier(dec!(0.50));
        assert_eq!(mult, dec!(0.5));
        
        // 75% confidence = 1.0x multiplier
        let mult = manager.confidence_multiplier(dec!(0.75));
        assert_eq!(mult, dec!(1.0));
        
        // 100% confidence = 1.5x multiplier
        let mult = manager.confidence_multiplier(dec!(1.00));
        assert_eq!(mult, dec!(1.5));
    }

    #[test]
    fn test_calculate_size_basic() {
        let manager = DynamicPositionManager::new(test_config());
        
        let request = PositionSizeRequest {
            signal_confidence: dec!(0.80),
            signal_edge: dec!(0.10),
            market_id: "test-market".to_string(),
            balance: dec!(1000),
            current_exposure: dec!(0),
        };
        
        let result = manager.calculate_size(&request);
        
        // Should have a non-zero size
        assert!(result.size > Decimal::ZERO);
        // Should be within position limits
        assert!(result.size <= dec!(50)); // 5% of 1000
    }

    #[test]
    fn test_calculate_size_high_confidence() {
        let manager = DynamicPositionManager::new(test_config());
        
        let low_conf = PositionSizeRequest {
            signal_confidence: dec!(0.55),
            signal_edge: dec!(0.10),
            market_id: "test".to_string(),
            balance: dec!(1000),
            current_exposure: dec!(0),
        };
        
        let high_conf = PositionSizeRequest {
            signal_confidence: dec!(0.95),
            signal_edge: dec!(0.10),
            market_id: "test".to_string(),
            balance: dec!(1000),
            current_exposure: dec!(0),
        };
        
        let low_result = manager.calculate_size(&low_conf);
        let high_result = manager.calculate_size(&high_conf);
        
        // High confidence should result in larger position
        assert!(high_result.size > low_result.size);
    }

    #[test]
    fn test_position_cap_by_max_position() {
        let manager = DynamicPositionManager::new(test_config());
        
        let request = PositionSizeRequest {
            signal_confidence: dec!(1.0),
            signal_edge: dec!(0.50), // Large edge would give large size
            market_id: "test".to_string(),
            balance: dec!(1000),
            current_exposure: dec!(0),
        };
        
        let result = manager.calculate_size(&request);
        
        // Should be capped at 5% of balance = $50
        assert!(result.size <= dec!(50));
        assert!(result.was_capped);
    }

    #[test]
    fn test_position_cap_by_exposure() {
        let config = test_config();
        let manager = DynamicPositionManager::new(config);
        
        let request = PositionSizeRequest {
            signal_confidence: dec!(0.80),
            signal_edge: dec!(0.10),
            market_id: "test".to_string(),
            balance: dec!(1000),
            current_exposure: dec!(490), // Already near 50% limit
        };
        
        let result = manager.calculate_size(&request);
        
        // Should be limited by remaining exposure budget
        assert!(result.size <= dec!(10)); // Only $10 remaining in exposure budget
    }

    #[test]
    fn test_position_zero_when_at_exposure_limit() {
        let manager = DynamicPositionManager::new(test_config());
        
        let request = PositionSizeRequest {
            signal_confidence: dec!(0.80),
            signal_edge: dec!(0.10),
            market_id: "test".to_string(),
            balance: dec!(1000),
            current_exposure: dec!(500), // At 50% limit
        };
        
        let result = manager.calculate_size(&request);
        
        assert_eq!(result.size, Decimal::ZERO);
        assert!(result.was_capped);
    }

    #[test]
    fn test_reserve_protection() {
        let mut config = test_config();
        config.min_balance_reserve = dec!(900); // Very high reserve
        let manager = DynamicPositionManager::new(config);
        
        let request = PositionSizeRequest {
            signal_confidence: dec!(0.80),
            signal_edge: dec!(0.10),
            market_id: "test".to_string(),
            balance: dec!(1000),
            current_exposure: dec!(0),
        };
        
        let result = manager.calculate_size(&request);
        
        // Only $100 available, position should be small or zero
        assert!(result.size <= dec!(100));
    }

    #[test]
    fn test_balance_based_size() {
        let manager = DynamicPositionManager::new(test_config());
        
        let size = manager.balance_based_size(dec!(1000), dec!(0.02)); // 2%
        
        // Should be 2% of (1000 - 100 reserve) = $18
        assert_eq!(size, dec!(18));
    }

    #[test]
    fn test_max_allowable_position() {
        let manager = DynamicPositionManager::new(test_config());
        
        // No exposure
        let max = manager.max_allowable_position(dec!(1000), dec!(0));
        assert_eq!(max, dec!(50)); // 5% of 1000
        
        // Some exposure
        let max = manager.max_allowable_position(dec!(1000), dec!(400));
        assert_eq!(max, dec!(50)); // Still limited by position size, not exposure
        
        // Near exposure limit
        let max = manager.max_allowable_position(dec!(1000), dec!(480));
        assert_eq!(max, dec!(20)); // Limited by remaining exposure budget
    }

    #[test]
    fn test_effective_kelly_fraction() {
        let manager = DynamicPositionManager::new(test_config());
        
        // Low confidence = lower Kelly
        let low = manager.effective_kelly_fraction(dec!(0.55));
        // High confidence = higher Kelly
        let high = manager.effective_kelly_fraction(dec!(0.90));
        
        assert!(high > low);
    }

    #[test]
    fn test_minimum_position_threshold() {
        let manager = DynamicPositionManager::new(test_config());
        
        let request = PositionSizeRequest {
            signal_confidence: dec!(0.51), // Very low confidence
            signal_edge: dec!(0.001), // Very small edge
            market_id: "test".to_string(),
            balance: dec!(100),
            current_exposure: dec!(0),
        };
        
        let result = manager.calculate_size(&request);
        
        // Should be zero because calculated size is below minimum
        assert_eq!(result.size, Decimal::ZERO);
    }

    #[test]
    fn test_zero_balance() {
        let manager = DynamicPositionManager::new(test_config());
        
        let request = PositionSizeRequest {
            signal_confidence: dec!(0.80),
            signal_edge: dec!(0.10),
            market_id: "test".to_string(),
            balance: dec!(0),
            current_exposure: dec!(0),
        };
        
        let result = manager.calculate_size(&request);
        assert_eq!(result.size, Decimal::ZERO);
    }
}
