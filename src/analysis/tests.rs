//! Unit tests for analysis module

#[cfg(test)]
mod tests {
    use super::super::*;
    use super::super::trader_profile::*;
    use crate::types::Side;
    use chrono::Utc;
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;

    fn make_trade(pnl: Decimal, outcome: TradeOutcome) -> TradeRecord {
        TradeRecord {
            trader: "test_trader".to_string(),
            market_id: "market-1".to_string(),
            market_question: "Test market?".to_string(),
            side: Side::Buy,
            entry_price: dec!(0.40),
            exit_price: Some(dec!(0.60)),
            size: dec!(100),
            entry_time: Utc::now(),
            exit_time: Some(Utc::now()),
            pnl: Some(pnl),
            outcome: Some(outcome),
        }
    }

    #[test]
    fn test_trade_analyzer_creation() {
        let analyzer = TradeAnalyzer::new();
        let insights = analyzer.analyze_trader("unknown");
        
        assert_eq!(insights.total_trades, 0);
        assert_eq!(insights.win_rate, 0.0);
    }

    #[test]
    fn test_trade_analyzer_with_trades() {
        let mut analyzer = TradeAnalyzer::new();
        
        // Add some winning trades
        analyzer.add_trade(make_trade(dec!(50), TradeOutcome::Win));
        analyzer.add_trade(make_trade(dec!(30), TradeOutcome::Win));
        analyzer.add_trade(make_trade(dec!(-20), TradeOutcome::Loss));
        
        let insights = analyzer.analyze_trader("test_trader");
        
        assert_eq!(insights.total_trades, 3);
        assert!((insights.win_rate - 0.666).abs() < 0.01);
        assert_eq!(insights.total_pnl, dec!(60));
    }

    #[test]
    fn test_trade_outcome_values() {
        assert_eq!(TradeOutcome::Win, TradeOutcome::Win);
        assert_ne!(TradeOutcome::Win, TradeOutcome::Loss);
        assert_ne!(TradeOutcome::Win, TradeOutcome::Pending);
    }

    #[test]
    fn test_entry_insights_default() {
        let entry = EntryInsights::default();
        
        assert_eq!(entry.preferred_price_range, (dec!(0.20), dec!(0.80)));
        assert!(!entry.event_timing);
    }

    #[test]
    fn test_exit_insights_default() {
        let exit = ExitInsights::default();
        
        assert!(exit.take_profit_mult.is_none());
        assert!(exit.stop_loss_pct.is_none());
    }

    #[test]
    fn test_trader_type_values() {
        assert_ne!(TraderType::Scalper, TraderType::Whale);
        assert_ne!(TraderType::Contrarian, TraderType::MomentumTrader);
    }

    #[test]
    fn test_risk_level_values() {
        assert_ne!(RiskLevel::Low, RiskLevel::High);
        assert_ne!(RiskLevel::Medium, RiskLevel::Extreme);
    }

    #[test]
    fn test_generate_recommendations() {
        let mut analyzer = TradeAnalyzer::new();
        
        // Add winning trades to get good insights
        for _ in 0..10 {
            analyzer.add_trade(make_trade(dec!(50), TradeOutcome::Win));
        }
        for _ in 0..3 {
            analyzer.add_trade(make_trade(dec!(-20), TradeOutcome::Loss));
        }
        
        let insights = analyzer.analyze_trader("test_trader");
        let _recs = analyzer.generate_recommendations(&insights);
        
        // Should have some recommendations
        assert!(insights.win_rate > 0.5);
    }

    #[test]
    fn test_trading_pattern_creation() {
        let pattern = TradingPattern {
            name: "Test Pattern".to_string(),
            description: "A test pattern".to_string(),
            win_rate: 0.65,
            avg_win: dec!(100),
            avg_loss: dec!(50),
            expected_value: dec!(25),
            sample_count: 20,
            confidence: 0.8,
        };
        
        assert_eq!(pattern.name, "Test Pattern");
        assert!(pattern.win_rate > 0.5);
        assert!(pattern.confidence > 0.5);
    }

    #[test]
    fn test_trade_record_creation() {
        let record = TradeRecord {
            trader: "trader1".to_string(),
            market_id: "m1".to_string(),
            market_question: "Test?".to_string(),
            side: Side::Buy,
            entry_price: dec!(0.50),
            exit_price: Some(dec!(0.60)),
            size: dec!(100),
            entry_time: Utc::now(),
            exit_time: Some(Utc::now()),
            pnl: Some(dec!(10)),
            outcome: Some(TradeOutcome::Win),
        };
        
        assert_eq!(record.trader, "trader1");
        assert_eq!(record.side, Side::Buy);
        assert_eq!(record.pnl, Some(dec!(10)));
    }

    #[test]
    fn test_trade_record_without_exit() {
        let record = TradeRecord {
            trader: "trader1".to_string(),
            market_id: "m1".to_string(),
            market_question: "Test?".to_string(),
            side: Side::Sell,
            entry_price: dec!(0.70),
            exit_price: None,
            size: dec!(50),
            entry_time: Utc::now(),
            exit_time: None,
            pnl: None,
            outcome: Some(TradeOutcome::Pending),
        };
        
        assert!(record.exit_price.is_none());
        assert!(record.pnl.is_none());
    }

    #[test]
    fn test_trader_insights_default() {
        let insights = TraderInsights {
            trader: "test".to_string(),
            total_trades: 0,
            win_rate: 0.0,
            total_pnl: Decimal::ZERO,
            avg_position_size: Decimal::ZERO,
            avg_hold_time_hours: 0.0,
            patterns: vec![],
            preferred_categories: vec![],
            active_hours: vec![],
            entry_insights: EntryInsights::default(),
            exit_insights: ExitInsights::default(),
        };
        
        assert_eq!(insights.total_trades, 0);
        assert!(insights.patterns.is_empty());
    }

    #[test]
    fn test_trader_insights_with_patterns() {
        let pattern = TradingPattern {
            name: "Contrarian".to_string(),
            description: "Bets against consensus".to_string(),
            win_rate: 0.70,
            avg_win: dec!(80),
            avg_loss: dec!(40),
            expected_value: dec!(30),
            sample_count: 50,
            confidence: 0.9,
        };
        
        let insights = TraderInsights {
            trader: "pro_trader".to_string(),
            total_trades: 100,
            win_rate: 0.65,
            total_pnl: dec!(5000),
            avg_position_size: dec!(200),
            avg_hold_time_hours: 24.0,
            patterns: vec![pattern],
            preferred_categories: vec!["Crypto".to_string()],
            active_hours: vec![9, 10, 11, 12, 13, 14, 15, 16],
            entry_insights: EntryInsights::default(),
            exit_insights: ExitInsights::default(),
        };
        
        assert_eq!(insights.patterns.len(), 1);
        assert_eq!(insights.patterns[0].name, "Contrarian");
    }

    #[test]
    fn test_multiple_trades_different_outcomes() {
        let mut analyzer = TradeAnalyzer::new();
        
        analyzer.add_trade(make_trade(dec!(100), TradeOutcome::Win));
        analyzer.add_trade(make_trade(dec!(-50), TradeOutcome::Loss));
        analyzer.add_trade(make_trade(dec!(0), TradeOutcome::Pending));
        
        let insights = analyzer.analyze_trader("test_trader");
        assert_eq!(insights.total_trades, 3);
    }

    #[test]
    fn test_all_losing_trades() {
        let mut analyzer = TradeAnalyzer::new();
        
        for _ in 0..5 {
            analyzer.add_trade(make_trade(dec!(-30), TradeOutcome::Loss));
        }
        
        let insights = analyzer.analyze_trader("test_trader");
        assert_eq!(insights.win_rate, 0.0);
        assert_eq!(insights.total_pnl, dec!(-150));
    }

    #[test]
    fn test_all_winning_trades() {
        let mut analyzer = TradeAnalyzer::new();
        
        for _ in 0..5 {
            analyzer.add_trade(make_trade(dec!(40), TradeOutcome::Win));
        }
        
        let insights = analyzer.analyze_trader("test_trader");
        assert_eq!(insights.win_rate, 1.0);
        assert_eq!(insights.total_pnl, dec!(200));
    }

    #[test]
    fn test_pattern_clone() {
        let pattern = TradingPattern {
            name: "Test".to_string(),
            description: "Desc".to_string(),
            win_rate: 0.5,
            avg_win: dec!(10),
            avg_loss: dec!(5),
            expected_value: dec!(2.5),
            sample_count: 10,
            confidence: 0.7,
        };
        
        let cloned = pattern.clone();
        assert_eq!(pattern.name, cloned.name);
        assert_eq!(pattern.win_rate, cloned.win_rate);
    }

    #[test]
    fn test_trader_type_clone() {
        let trader_type = TraderType::Whale;
        let cloned = trader_type.clone();
        assert_eq!(trader_type, cloned);
    }

    #[test]
    fn test_risk_level_clone() {
        let risk = RiskLevel::High;
        let cloned = risk.clone();
        assert_eq!(risk, cloned);
    }
}
