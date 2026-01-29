//! Tests for SmartExecutor

use super::smart_executor::*;
use crate::client::clob::{OrderBook, OrderBookLevel};
use crate::types::Side;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

fn create_test_orderbook(bids: Vec<(Decimal, Decimal)>, asks: Vec<(Decimal, Decimal)>) -> OrderBook {
    OrderBook {
        bids: bids
            .into_iter()
            .map(|(price, size)| OrderBookLevel { price, size })
            .collect(),
        asks: asks
            .into_iter()
            .map(|(price, size)| OrderBookLevel { price, size })
            .collect(),
    }
}

// Standalone function to test analyze_depth without needing a full SmartExecutor
fn analyze_depth_standalone(
    config: &SmartExecutorConfig,
    book: &OrderBook,
    side: Side,
    size: Decimal,
) -> DepthAnalysis {
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

    let max_price_diff = best_price * config.max_slippage;
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
        && total_liquidity >= config.min_liquidity
        && expected_slippage <= config.max_slippage;

    DepthAnalysis {
        best_liquidity,
        total_liquidity,
        expected_price,
        expected_slippage,
        can_fill,
    }
}

#[test]
fn test_analyze_depth_buy_sufficient_liquidity() {
    let book = create_test_orderbook(
        vec![(dec!(0.50), dec!(100))],
        vec![
            (dec!(0.51), dec!(50)),
            (dec!(0.52), dec!(50)),
            (dec!(0.53), dec!(100)),
        ],
    );

    let config = SmartExecutorConfig {
        min_liquidity: dec!(20),
        max_slippage: dec!(0.05),
        ..Default::default()
    };

    let analysis = analyze_depth_standalone(&config, &book, Side::Buy, dec!(30));

    assert!(analysis.can_fill, "Should be able to fill with sufficient liquidity");
    assert!(analysis.expected_slippage <= dec!(0.05), "Slippage should be within tolerance");
    assert!(analysis.total_liquidity > dec!(20), "Should have enough liquidity");
}

#[test]
fn test_analyze_depth_buy_insufficient_liquidity() {
    let book = create_test_orderbook(
        vec![(dec!(0.50), dec!(10))],
        vec![(dec!(0.51), dec!(5))],
    );

    let config = SmartExecutorConfig {
        min_liquidity: dec!(50),
        max_slippage: dec!(0.02),
        ..Default::default()
    };

    let analysis = analyze_depth_standalone(&config, &book, Side::Buy, dec!(100));

    assert!(!analysis.can_fill, "Should not fill with insufficient liquidity");
}

#[test]
fn test_analyze_depth_sell_side() {
    let book = create_test_orderbook(
        vec![
            (dec!(0.50), dec!(100)),
            (dec!(0.49), dec!(100)),
            (dec!(0.48), dec!(100)),
        ],
        vec![(dec!(0.51), dec!(50))],
    );

    let config = SmartExecutorConfig {
        min_liquidity: dec!(20),
        max_slippage: dec!(0.05),
        ..Default::default()
    };

    let analysis = analyze_depth_standalone(&config, &book, Side::Sell, dec!(50));

    assert!(analysis.can_fill, "Should be able to sell with sufficient bids");
    assert_eq!(analysis.best_liquidity, dec!(50)); // 100 * 0.50
}

#[test]
fn test_analyze_depth_empty_orderbook() {
    let book = OrderBook {
        bids: vec![],
        asks: vec![],
    };

    let config = SmartExecutorConfig::default();
    let analysis = analyze_depth_standalone(&config, &book, Side::Buy, dec!(10));

    assert!(!analysis.can_fill, "Should not fill with empty orderbook");
    assert_eq!(analysis.total_liquidity, Decimal::ZERO);
    assert_eq!(analysis.expected_slippage, Decimal::ONE);
}

#[test]
fn test_analyze_depth_slippage_calculation() {
    // Setup: price levels with increasing prices where we CAN fill but with high slippage
    let book = create_test_orderbook(
        vec![(dec!(0.50), dec!(100))],
        vec![
            (dec!(0.50), dec!(10)), // Best ask
            (dec!(0.55), dec!(10)), // 10% higher - would cause slippage
        ],
    );

    let config = SmartExecutorConfig {
        min_liquidity: dec!(5),
        max_slippage: dec!(0.02), // 2% max - but we need to go to 0.55 level
        ..Default::default()
    };

    // Try to buy more than best level - this forces going to next price level
    // But next level is beyond slippage tolerance (10% > 2%)
    // So we can only fill 10 shares at 0.50, leaving 10 unfilled
    let analysis = analyze_depth_standalone(&config, &book, Side::Buy, dec!(20));

    // Should fail because we can't fill the full order within slippage limits
    assert!(!analysis.can_fill, "Should reject because order can't be fully filled within slippage");
}

#[test]
fn test_analyze_depth_high_slippage_detectable() {
    // Setup where we CAN fill but with measurable slippage
    let book = create_test_orderbook(
        vec![(dec!(0.50), dec!(100))],
        vec![
            (dec!(0.50), dec!(10)),  // Best ask
            (dec!(0.505), dec!(10)), // 1% higher - within 2% tolerance
        ],
    );

    let config = SmartExecutorConfig {
        min_liquidity: dec!(5),
        max_slippage: dec!(0.02), // 2% max
        ..Default::default()
    };

    // Buy 15 shares: 10 @ 0.50 + 5 @ 0.505
    let analysis = analyze_depth_standalone(&config, &book, Side::Buy, dec!(15));

    // Should fill successfully with small slippage
    assert!(analysis.can_fill, "Should fill within slippage tolerance");
    assert!(analysis.expected_slippage > Decimal::ZERO, "Should have some slippage");
    assert!(analysis.expected_slippage <= dec!(0.02), "Slippage should be within limit");
}

#[test]
fn test_expected_price_calculation() {
    let book = create_test_orderbook(
        vec![(dec!(0.50), dec!(100))],
        vec![
            (dec!(0.50), dec!(10)),
            (dec!(0.52), dec!(10)),
        ],
    );

    let config = SmartExecutorConfig {
        min_liquidity: dec!(5),
        max_slippage: dec!(0.10),
        ..Default::default()
    };

    let analysis = analyze_depth_standalone(&config, &book, Side::Buy, dec!(15));
    let expected = (dec!(10) * dec!(0.50) + dec!(5) * dec!(0.52)) / dec!(15);
    
    assert_eq!(
        analysis.expected_price.round_dp(4),
        expected.round_dp(4),
        "Expected price should be weighted average"
    );
}

#[test]
fn test_config_default_values() {
    let config = SmartExecutorConfig::default();

    assert_eq!(config.max_retries, 3);
    assert_eq!(config.retry_delay_ms, 1000);
    assert_eq!(config.min_liquidity, dec!(50));
    assert_eq!(config.max_slippage, dec!(0.02));
    assert_eq!(config.batch_count, 3);
}

#[test]
fn test_execution_result_structure() {
    let result = ExecutionResult {
        filled_size: dec!(100),
        average_price: dec!(0.55),
        trades: vec![],
        attempts: 2,
    };

    assert_eq!(result.filled_size, dec!(100));
    assert_eq!(result.average_price, dec!(0.55));
    assert_eq!(result.attempts, 2);
}

#[test]
fn test_depth_analysis_partial_fill() {
    let book = create_test_orderbook(
        vec![(dec!(0.50), dec!(100))],
        vec![(dec!(0.51), dec!(50))],
    );

    let config = SmartExecutorConfig {
        min_liquidity: dec!(10),
        max_slippage: dec!(0.05),
        ..Default::default()
    };

    let analysis = analyze_depth_standalone(&config, &book, Side::Buy, dec!(100));

    assert!(!analysis.can_fill, "Should not fill when size exceeds available");
}

#[test]
fn test_depth_analysis_structure() {
    let analysis = DepthAnalysis {
        best_liquidity: dec!(100),
        total_liquidity: dec!(500),
        expected_price: dec!(0.52),
        expected_slippage: dec!(0.01),
        can_fill: true,
    };

    assert_eq!(analysis.best_liquidity, dec!(100));
    assert_eq!(analysis.total_liquidity, dec!(500));
    assert!(analysis.can_fill);
}

#[test]
fn test_config_custom_values() {
    let config = SmartExecutorConfig {
        max_retries: 5,
        retry_delay_ms: 2000,
        min_liquidity: dec!(100),
        max_slippage: dec!(0.01),
        batch_count: 5,
        batch_delay_ms: 3000,
        order_timeout_secs: 60,
    };

    assert_eq!(config.max_retries, 5);
    assert_eq!(config.retry_delay_ms, 2000);
    assert_eq!(config.min_liquidity, dec!(100));
    assert_eq!(config.max_slippage, dec!(0.01));
    assert_eq!(config.batch_count, 5);
}
