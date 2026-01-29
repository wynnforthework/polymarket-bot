//! Tests for GradualExit strategy

use super::gradual_exit::*;
use crate::client::clob::{OrderBook, OrderBookLevel};
use crate::types::Side;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::HashMap;

fn create_test_orderbook(bids: Vec<(Decimal, Decimal)>) -> OrderBook {
    OrderBook {
        bids: bids
            .into_iter()
            .map(|(price, size)| OrderBookLevel { price, size })
            .collect(),
        asks: vec![],
    }
}

#[test]
fn test_default_thresholds() {
    let config = GradualExitConfig::default();

    assert_eq!(config.thresholds.len(), 3);
    assert_eq!(config.thresholds[0].price_threshold, dec!(0.85));
    assert_eq!(config.thresholds[0].sell_percentage, dec!(0.25));
    assert_eq!(config.thresholds[1].price_threshold, dec!(0.90));
    assert_eq!(config.thresholds[1].sell_percentage, dec!(0.25));
    assert_eq!(config.thresholds[2].price_threshold, dec!(0.95));
    assert_eq!(config.thresholds[2].sell_percentage, dec!(1.00));
}

#[test]
fn test_track_position() {
    let mut manager = GradualExitManager::default();

    manager.track_position(
        "token123",
        "market456",
        Side::Buy,
        dec!(100),
        dec!(0.50),
    );

    let position = manager.get_position("token123").unwrap();
    assert_eq!(position.token_id, "token123");
    assert_eq!(position.market_id, "market456");
    assert_eq!(position.original_size, dec!(100));
    assert_eq!(position.remaining_size, dec!(100));
    assert_eq!(position.entry_price, dec!(0.50));
    assert!(position.last_threshold_triggered.is_none());
}

#[test]
fn test_check_exit_below_threshold() {
    let mut manager = GradualExitManager::default();
    manager.track_position("token1", "market1", Side::Buy, dec!(100), dec!(0.50));

    let action = manager.check_exit("token1", dec!(0.80));
    assert!(action.is_none(), "Should not exit below first threshold");
}

#[test]
fn test_check_exit_first_threshold() {
    let mut manager = GradualExitManager::default();
    manager.track_position("token1", "market1", Side::Buy, dec!(100), dec!(0.50));

    let action = manager.check_exit("token1", dec!(0.86));

    assert!(action.is_some(), "Should trigger first threshold");
    let action = action.unwrap();
    assert_eq!(action.threshold_index, 0);
    assert_eq!(action.sell_size, dec!(25));
    assert_eq!(action.threshold_price, dec!(0.85));
}

#[test]
fn test_check_exit_second_threshold() {
    // Use no cooldown config for this test
    let config = GradualExitConfig {
        cooldown_secs: 0,
        ..Default::default()
    };
    let mut manager = GradualExitManager::new(config);
    manager.track_position("token1", "market1", Side::Buy, dec!(100), dec!(0.50));

    manager.record_exit("token1", dec!(25), 0);

    let action = manager.check_exit("token1", dec!(0.91));

    assert!(action.is_some(), "Should trigger second threshold");
    let action = action.unwrap();
    assert_eq!(action.threshold_index, 1);
    assert_eq!(action.sell_size, dec!(18.75));
}

#[test]
fn test_check_exit_final_threshold() {
    // Use no cooldown config for this test
    let config = GradualExitConfig {
        cooldown_secs: 0,
        ..Default::default()
    };
    let mut manager = GradualExitManager::new(config);
    manager.track_position("token1", "market1", Side::Buy, dec!(100), dec!(0.50));

    manager.record_exit("token1", dec!(25), 0);
    manager.record_exit("token1", dec!(18.75), 1);

    let action = manager.check_exit("token1", dec!(0.96));

    assert!(action.is_some(), "Should trigger final threshold");
    let action = action.unwrap();
    assert_eq!(action.threshold_index, 2);
    assert_eq!(action.sell_size, dec!(56.25));
}

#[test]
fn test_record_exit() {
    let mut manager = GradualExitManager::default();
    manager.track_position("token1", "market1", Side::Buy, dec!(100), dec!(0.50));

    manager.record_exit("token1", dec!(25), 0);

    let position = manager.get_position("token1").unwrap();
    assert_eq!(position.remaining_size, dec!(75));
    assert_eq!(position.last_threshold_triggered, Some(0));
    assert!(position.last_exit_time.is_some());
}

#[test]
fn test_cooldown_prevents_exit() {
    let config = GradualExitConfig {
        cooldown_secs: 3600,
        ..Default::default()
    };
    let mut manager = GradualExitManager::new(config);
    manager.track_position("token1", "market1", Side::Buy, dec!(100), dec!(0.50));

    manager.record_exit("token1", dec!(25), 0);

    let action = manager.check_exit("token1", dec!(0.92));
    assert!(action.is_none(), "Should be blocked by cooldown");
}

#[test]
fn test_check_liquidity_sufficient() {
    let manager = GradualExitManager::default();

    let book = create_test_orderbook(vec![
        (dec!(0.85), dec!(100)),
        (dec!(0.84), dec!(100)),
    ]);

    let check = manager.check_liquidity(&book, dec!(50));

    assert!(check.sufficient, "Should have sufficient liquidity");
    assert!(check.available_liquidity >= dec!(20));
    assert!(check.expected_slippage <= dec!(0.03));
}

#[test]
fn test_check_liquidity_insufficient() {
    let config = GradualExitConfig {
        min_liquidity: dec!(100),
        ..Default::default()
    };
    let manager = GradualExitManager::new(config);

    let book = create_test_orderbook(vec![
        (dec!(0.50), dec!(10)),
    ]);

    let check = manager.check_liquidity(&book, dec!(50));

    assert!(!check.sufficient, "Should have insufficient liquidity");
}

#[test]
fn test_check_liquidity_empty_orderbook() {
    let manager = GradualExitManager::default();

    let book = OrderBook {
        bids: vec![],
        asks: vec![],
    };

    let check = manager.check_liquidity(&book, dec!(50));

    assert!(!check.sufficient);
    assert_eq!(check.available_liquidity, Decimal::ZERO);
    assert_eq!(check.expected_slippage, Decimal::ONE);
}

#[test]
fn test_check_liquidity_slippage() {
    let config = GradualExitConfig {
        min_liquidity: dec!(10),
        max_slippage: dec!(0.01),
        ..Default::default()
    };
    let manager = GradualExitManager::new(config);

    let book = create_test_orderbook(vec![
        (dec!(0.50), dec!(10)),
        (dec!(0.45), dec!(100)),
    ]);

    let check = manager.check_liquidity(&book, dec!(50));

    assert!(!check.sufficient, "Should fail due to slippage");
    assert!(check.expected_slippage > dec!(0.01));
}

#[test]
fn test_sell_position_inverted_price() {
    let mut manager = GradualExitManager::default();
    manager.track_position("token1", "market1", Side::Sell, dec!(100), dec!(0.50));

    let action = manager.check_exit("token1", dec!(0.10));

    assert!(action.is_some(), "Should trigger for short position when price drops");
}

#[test]
fn test_untrack_position() {
    let mut manager = GradualExitManager::default();
    manager.track_position("token1", "market1", Side::Buy, dec!(100), dec!(0.50));

    let removed = manager.untrack_position("token1");
    assert!(removed.is_some());

    let position = manager.get_position("token1");
    assert!(position.is_none());
}

#[test]
fn test_get_positions() {
    let mut manager = GradualExitManager::default();
    manager.track_position("token1", "market1", Side::Buy, dec!(100), dec!(0.50));
    manager.track_position("token2", "market2", Side::Buy, dec!(50), dec!(0.60));

    let positions = manager.get_positions();
    assert_eq!(positions.len(), 2);
}

#[test]
fn test_check_all_exits() {
    let mut manager = GradualExitManager::default();
    manager.track_position("token1", "market1", Side::Buy, dec!(100), dec!(0.50));
    manager.track_position("token2", "market2", Side::Buy, dec!(100), dec!(0.50));

    let mut prices = HashMap::new();
    prices.insert("token1".to_string(), dec!(0.86));
    prices.insert("token2".to_string(), dec!(0.70));

    let actions = manager.check_all_exits(&prices);

    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].token_id, "token1");
}

#[test]
fn test_calculate_profit_buy() {
    let mut manager = GradualExitManager::default();
    manager.track_position("token1", "market1", Side::Buy, dec!(100), dec!(0.50));

    let profit = manager.calculate_profit("token1", dec!(0.60)).unwrap();

    assert!(profit.unrealized_pnl > Decimal::ZERO);
    assert!(profit.pnl_percentage > Decimal::ZERO);
}

#[test]
fn test_calculate_profit_after_partial_exit() {
    let mut manager = GradualExitManager::default();
    manager.track_position("token1", "market1", Side::Buy, dec!(100), dec!(0.50));
    manager.record_exit("token1", dec!(25), 0);

    let profit = manager.calculate_profit("token1", dec!(0.60)).unwrap();

    assert_eq!(
        manager.get_position("token1").unwrap().remaining_size,
        dec!(75)
    );
    assert!(profit.unrealized_pnl > Decimal::ZERO);
}

#[test]
fn test_exit_threshold_new() {
    let threshold = ExitThreshold::new(dec!(0.85), dec!(0.25));

    assert_eq!(threshold.price_threshold, dec!(0.85));
    assert_eq!(threshold.sell_percentage, dec!(0.25));
}

#[test]
fn test_custom_thresholds() {
    let config = GradualExitConfig {
        thresholds: vec![
            ExitThreshold::new(dec!(0.70), dec!(0.50)),
            ExitThreshold::new(dec!(0.80), dec!(1.00)),
        ],
        ..Default::default()
    };
    let mut manager = GradualExitManager::new(config);
    manager.track_position("token1", "market1", Side::Buy, dec!(100), dec!(0.50));

    let action = manager.check_exit("token1", dec!(0.75));

    assert!(action.is_some());
    let action = action.unwrap();
    assert_eq!(action.threshold_index, 0);
    assert_eq!(action.sell_size, dec!(50));
}

#[test]
fn test_progressive_exit_sequence() {
    let mut manager = GradualExitManager::new(GradualExitConfig {
        cooldown_secs: 0,
        ..Default::default()
    });
    manager.track_position("token1", "market1", Side::Buy, dec!(100), dec!(0.50));

    // Stage 1
    let action1 = manager.check_exit("token1", dec!(0.86)).unwrap();
    assert_eq!(action1.sell_size, dec!(25));
    manager.record_exit("token1", dec!(25), 0);

    // Stage 2
    let action2 = manager.check_exit("token1", dec!(0.91)).unwrap();
    assert_eq!(action2.sell_size, dec!(18.75));
    manager.record_exit("token1", dec!(18.75), 1);

    // Stage 3
    let action3 = manager.check_exit("token1", dec!(0.96)).unwrap();
    assert_eq!(action3.sell_size, dec!(56.25));
    manager.record_exit("token1", dec!(56.25), 2);

    let position = manager.get_position("token1").unwrap();
    assert_eq!(position.remaining_size, Decimal::ZERO);
}

#[test]
fn test_liquidity_check_structure() {
    let check = LiquidityCheck {
        sufficient: true,
        available_liquidity: dec!(500),
        expected_price: dec!(0.85),
        expected_slippage: dec!(0.01),
    };

    assert!(check.sufficient);
    assert_eq!(check.available_liquidity, dec!(500));
    assert_eq!(check.expected_price, dec!(0.85));
    assert_eq!(check.expected_slippage, dec!(0.01));
}

#[test]
fn test_exit_action_structure() {
    let action = ExitAction {
        token_id: "token123".to_string(),
        market_id: "market456".to_string(),
        sell_size: dec!(25),
        threshold_index: 0,
        threshold_price: dec!(0.85),
        current_price: dec!(0.87),
    };

    assert_eq!(action.token_id, "token123");
    assert_eq!(action.sell_size, dec!(25));
    assert_eq!(action.threshold_index, 0);
}

#[test]
fn test_profit_info_structure() {
    let profit = ProfitInfo {
        unrealized_pnl: dec!(10),
        pnl_percentage: dec!(20),
        remaining_value: dec!(60),
        original_value: dec!(50),
    };

    assert_eq!(profit.unrealized_pnl, dec!(10));
    assert_eq!(profit.pnl_percentage, dec!(20));
}

#[test]
fn test_tracked_position_fields() {
    let position = TrackedPosition {
        token_id: "token1".to_string(),
        market_id: "market1".to_string(),
        side: Side::Buy,
        original_size: dec!(100),
        remaining_size: dec!(75),
        entry_price: dec!(0.50),
        last_threshold_triggered: Some(0),
        last_exit_time: Some(chrono::Utc::now()),
    };

    assert_eq!(position.token_id, "token1");
    assert_eq!(position.original_size, dec!(100));
    assert_eq!(position.remaining_size, dec!(75));
    assert_eq!(position.last_threshold_triggered, Some(0));
}
