//! Integration tests for risk management module

use super::*;
use crate::config::RiskConfig;
use crate::types::{Market, Outcome, Position, Side, Signal};
use chrono::Utc;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

fn test_risk_config() -> RiskConfig {
    RiskConfig {
        max_position_pct: dec!(0.05),
        max_exposure_pct: dec!(0.50),
        max_daily_loss_pct: dec!(0.10),
        min_balance_reserve: dec!(100),
        max_open_positions: 5,
    }
}

fn test_market() -> Market {
    Market {
        id: "test-market-1".to_string(),
        question: "Will it rain tomorrow?".to_string(),
        description: None,
        end_date: None,
        volume: dec!(10000),
        liquidity: dec!(5000),
        outcomes: vec![
            Outcome {
                token_id: "yes-token".to_string(),
                outcome: "Yes".to_string(),
                price: dec!(0.45),
            },
            Outcome {
                token_id: "no-token".to_string(),
                outcome: "No".to_string(),
                price: dec!(0.55),
            },
        ],
        active: true,
        closed: false,
    }
}

fn test_signal() -> Signal {
    Signal {
        market_id: "test-market-1".to_string(),
        token_id: "yes-token".to_string(),
        side: Side::Buy,
        model_probability: dec!(0.55),
        market_probability: dec!(0.45),
        edge: dec!(0.10),
        confidence: dec!(0.75),
        suggested_size: dec!(50),
        timestamp: Utc::now(),
    }
}

fn test_position() -> Position {
    Position {
        token_id: "token-1".to_string(),
        market_id: "market-1".to_string(),
        side: Side::Buy,
        size: dec!(100),
        avg_entry_price: dec!(0.45),
        current_price: dec!(0.50),
        unrealized_pnl: dec!(10),
    }
}

// =============================================================================
// RiskManager Integration Tests
// =============================================================================

#[test]
fn test_risk_manager_creation() {
    let config = test_risk_config();
    let manager = RiskManager::new(config);
    
    assert_eq!(manager.daily_pnl(), Decimal::ZERO);
}

#[test]
fn test_risk_manager_can_trade_initially() {
    let manager = RiskManager::new(test_risk_config());
    
    match manager.can_trade() {
        RiskCheckResult::Allowed => (),
        RiskCheckResult::Blocked { reason } => {
            panic!("Expected Allowed, got Blocked: {}", reason);
        }
    }
}

#[test]
fn test_risk_manager_blocks_after_loss_limit() {
    let mut manager = RiskManager::new(test_risk_config());
    manager.pnl_tracker.set_starting_balance(dec!(1000));
    
    // Record losses up to limit
    manager.record_trade(dec!(-100)); // 10% loss
    
    match manager.can_trade() {
        RiskCheckResult::Blocked { .. } => (),
        RiskCheckResult::Allowed => {
            panic!("Expected Blocked after reaching daily loss limit");
        }
    }
}

#[test]
fn test_risk_manager_calculate_position_size() {
    let mut manager = RiskManager::new(test_risk_config());
    let market = test_market();
    let signal = test_signal();
    
    let size = manager.calculate_position_size(
        &signal,
        &market,
        dec!(1000),
        &[],
    );
    
    assert!(size.is_some());
    let size = size.unwrap();
    assert!(size > Decimal::ZERO);
    assert!(size <= dec!(50)); // Max 5% of 1000
}

#[test]
fn test_risk_manager_respects_position_limit() {
    let mut manager = RiskManager::new(test_risk_config());
    let market = test_market();
    let signal = test_signal();
    
    // Create max positions
    let positions: Vec<Position> = (0..5)
        .map(|i| Position {
            market_id: format!("market-{}", i),
            ..test_position()
        })
        .collect();
    
    let size = manager.calculate_position_size(
        &signal,
        &market,
        dec!(1000),
        &positions,
    );
    
    assert!(size.is_none());
}

#[test]
fn test_risk_manager_volatility_adjustment() {
    let mut manager = RiskManager::new(test_risk_config());
    let market = test_market();
    let signal = test_signal();
    
    // Add high volatility data
    let volatile_prices = vec![
        dec!(0.30), dec!(0.60), dec!(0.25), dec!(0.65), dec!(0.35),
    ];
    for price in volatile_prices {
        manager.update_volatility(&market.id, price);
    }
    
    let volatile_size = manager.calculate_position_size(
        &signal,
        &market,
        dec!(1000),
        &[],
    );
    
    // Reset and add stable data
    let mut manager2 = RiskManager::new(test_risk_config());
    let stable_prices = vec![
        dec!(0.450), dec!(0.452), dec!(0.448), dec!(0.451), dec!(0.449),
    ];
    for price in stable_prices {
        manager2.update_volatility(&market.id, price);
    }
    
    let stable_size = manager2.calculate_position_size(
        &signal,
        &market,
        dec!(1000),
        &[],
    );
    
    // Stable market should allow larger position
    assert!(stable_size.unwrap_or(Decimal::ZERO) >= volatile_size.unwrap_or(Decimal::ZERO));
}

#[test]
fn test_risk_manager_correlation_adjustment() {
    let mut manager = RiskManager::new(test_risk_config());
    let market = test_market();
    let signal = test_signal();
    
    // Add correlation data - market1 and test-market-1 are correlated
    for i in 1..=10 {
        let price = Decimal::from(i) / dec!(10);
        manager.update_correlation(&market.id, price, i as i64);
        manager.update_correlation("correlated-market", price, i as i64);
    }
    
    // Size without existing correlated position
    let size_no_corr = manager.calculate_position_size(
        &signal,
        &market,
        dec!(1000),
        &[],
    );
    
    // Size with existing correlated position
    let existing = vec![Position {
        market_id: "correlated-market".to_string(),
        ..test_position()
    }];
    
    let size_with_corr = manager.calculate_position_size(
        &signal,
        &market,
        dec!(1000),
        &existing,
    );
    
    // Position with correlation should be smaller
    assert!(size_with_corr.unwrap_or(Decimal::ZERO) <= size_no_corr.unwrap_or(Decimal::ZERO));
}

#[test]
fn test_risk_manager_record_trade() {
    let mut manager = RiskManager::new(test_risk_config());
    manager.pnl_tracker.set_starting_balance(dec!(1000));
    
    manager.record_trade(dec!(50));
    assert_eq!(manager.daily_pnl(), dec!(50));
    
    manager.record_trade(dec!(-30));
    assert_eq!(manager.daily_pnl(), dec!(20));
}

#[test]
fn test_risk_manager_reset_daily() {
    let mut manager = RiskManager::new(test_risk_config());
    manager.record_trade(dec!(100));
    
    manager.reset_daily();
    
    assert_eq!(manager.daily_pnl(), Decimal::ZERO);
}

// =============================================================================
// RiskCheckResult Tests
// =============================================================================

#[test]
fn test_risk_check_result_equality() {
    assert_eq!(RiskCheckResult::Allowed, RiskCheckResult::Allowed);
    
    let blocked1 = RiskCheckResult::Blocked {
        reason: "test".to_string(),
    };
    let blocked2 = RiskCheckResult::Blocked {
        reason: "test".to_string(),
    };
    assert_eq!(blocked1, blocked2);
}

// =============================================================================
// End-to-End Scenario Tests
// =============================================================================

#[test]
fn test_scenario_gradual_loss_to_limit() {
    let mut manager = RiskManager::new(test_risk_config());
    manager.pnl_tracker.set_starting_balance(dec!(1000));
    
    // Trade 1: Small loss
    manager.record_trade(dec!(-20));
    assert!(matches!(manager.can_trade(), RiskCheckResult::Allowed));
    
    // Trade 2: Another loss
    manager.record_trade(dec!(-30));
    assert!(matches!(manager.can_trade(), RiskCheckResult::Allowed));
    
    // Trade 3: Big loss, hits limit
    manager.record_trade(dec!(-50)); // Total: -100 = 10%
    assert!(matches!(manager.can_trade(), RiskCheckResult::Blocked { .. }));
}

#[test]
fn test_scenario_recovery_after_reset() {
    let mut manager = RiskManager::new(test_risk_config());
    manager.pnl_tracker.set_starting_balance(dec!(1000));
    
    // Hit limit
    manager.record_trade(dec!(-100));
    assert!(matches!(manager.can_trade(), RiskCheckResult::Blocked { .. }));
    
    // New day reset
    manager.reset_daily();
    assert!(matches!(manager.can_trade(), RiskCheckResult::Allowed));
}

#[test]
fn test_scenario_mixed_signals() {
    let mut manager = RiskManager::new(test_risk_config());
    let market = test_market();
    
    // High confidence signal
    let high_conf_signal = Signal {
        confidence: dec!(0.90),
        edge: dec!(0.15),
        ..test_signal()
    };
    
    let high_size = manager.calculate_position_size(
        &high_conf_signal,
        &market,
        dec!(1000),
        &[],
    );
    
    // Low confidence signal
    let low_conf_signal = Signal {
        confidence: dec!(0.55),
        edge: dec!(0.05),
        ..test_signal()
    };
    
    let low_size = manager.calculate_position_size(
        &low_conf_signal,
        &market,
        dec!(1000),
        &[],
    );
    
    // High confidence should get larger position
    assert!(high_size.unwrap_or(Decimal::ZERO) > low_size.unwrap_or(Decimal::ZERO));
}

#[test]
fn test_scenario_exposure_buildup() {
    let mut manager = RiskManager::new(test_risk_config());
    let signal = test_signal();
    
    // First position
    let market1 = Market {
        id: "market-1".to_string(),
        ..test_market()
    };
    let size1 = manager.calculate_position_size(
        &signal,
        &market1,
        dec!(1000),
        &[],
    );
    assert!(size1.is_some());
    
    // Build up positions
    let positions: Vec<Position> = (0..4)
        .map(|i| Position {
            market_id: format!("market-{}", i),
            size: dec!(100),
            current_price: dec!(1),
            ..test_position()
        })
        .collect();
    
    // Near exposure limit
    let market5 = Market {
        id: "market-5".to_string(),
        ..test_market()
    };
    
    let size5 = manager.calculate_position_size(
        &signal,
        &market5,
        dec!(1000),
        &positions, // 400 exposure
    );
    
    // Should still be able to trade but with reduced size
    assert!(size5.is_some());
    let size5 = size5.unwrap();
    assert!(size5 <= dec!(100)); // Limited by remaining exposure
}
