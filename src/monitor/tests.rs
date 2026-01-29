//! Tests for monitor module

#[cfg(test)]
mod tests {
    use super::super::{Monitor, TradeRecord, PerformanceStats};
    use chrono::Utc;
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;

    #[test]
    fn test_performance_stats_default() {
        let stats = PerformanceStats::default();
        assert_eq!(stats.total_trades, 0);
        assert_eq!(stats.winning_trades, 0);
        assert_eq!(stats.losing_trades, 0);
        assert_eq!(stats.win_rate, Decimal::ZERO);
        assert_eq!(stats.total_pnl, Decimal::ZERO);
    }

    #[test]
    fn test_trade_record_creation() {
        let record = TradeRecord {
            timestamp: Utc::now(),
            market_id: "market1".to_string(),
            side: "BUY".to_string(),
            size: dec!(100),
            price: dec!(0.55),
            pnl: Some(dec!(10)),
        };
        
        assert_eq!(record.market_id, "market1");
        assert_eq!(record.side, "BUY");
        assert_eq!(record.pnl, Some(dec!(10)));
    }

    #[test]
    fn test_trade_record_without_pnl() {
        let record = TradeRecord {
            timestamp: Utc::now(),
            market_id: "m1".to_string(),
            side: "SELL".to_string(),
            size: dec!(50),
            price: dec!(0.60),
            pnl: None,
        };
        
        assert!(record.pnl.is_none());
    }

    #[test]
    fn test_trade_record_clone() {
        let record = TradeRecord {
            timestamp: Utc::now(),
            market_id: "m1".to_string(),
            side: "BUY".to_string(),
            size: dec!(100),
            price: dec!(0.50),
            pnl: Some(dec!(5)),
        };
        
        let cloned = record.clone();
        assert_eq!(record.market_id, cloned.market_id);
        assert_eq!(record.pnl, cloned.pnl);
    }

    #[test]
    fn test_performance_stats_clone() {
        let stats = PerformanceStats {
            total_trades: 10,
            winning_trades: 7,
            losing_trades: 3,
            win_rate: dec!(0.70),
            total_pnl: dec!(500),
            avg_pnl_per_trade: dec!(50),
            sharpe_ratio: Some(dec!(1.5)),
        };
        
        let cloned = stats.clone();
        assert_eq!(stats.total_trades, cloned.total_trades);
        assert_eq!(stats.win_rate, cloned.win_rate);
        assert_eq!(stats.sharpe_ratio, cloned.sharpe_ratio);
    }

    #[test]
    fn test_monitor_creation() {
        let monitor = Monitor::new(100);
        // Just verify it creates without panic
        let _ = monitor;
    }

    #[tokio::test]
    async fn test_monitor_record_trade() {
        let monitor = Monitor::new(10);
        
        let record = TradeRecord {
            timestamp: Utc::now(),
            market_id: "m1".to_string(),
            side: "BUY".to_string(),
            size: dec!(100),
            price: dec!(0.50),
            pnl: Some(dec!(10)),
        };
        
        monitor.record_trade(record).await;
        
        let stats = monitor.get_stats().await;
        assert_eq!(stats.total_trades, 1);
    }

    #[tokio::test]
    async fn test_monitor_multiple_trades() {
        let monitor = Monitor::new(10);
        
        // Record winning trade
        monitor.record_trade(TradeRecord {
            timestamp: Utc::now(),
            market_id: "m1".to_string(),
            side: "BUY".to_string(),
            size: dec!(100),
            price: dec!(0.50),
            pnl: Some(dec!(20)),
        }).await;
        
        // Record losing trade
        monitor.record_trade(TradeRecord {
            timestamp: Utc::now(),
            market_id: "m2".to_string(),
            side: "BUY".to_string(),
            size: dec!(100),
            price: dec!(0.60),
            pnl: Some(dec!(-10)),
        }).await;
        
        let stats = monitor.get_stats().await;
        assert_eq!(stats.total_trades, 2);
        assert_eq!(stats.winning_trades, 1);
        assert_eq!(stats.losing_trades, 1);
        assert_eq!(stats.total_pnl, dec!(10)); // 20 - 10
    }

    #[tokio::test]
    async fn test_monitor_win_rate() {
        let monitor = Monitor::new(10);
        
        // 3 wins
        for _ in 0..3 {
            monitor.record_trade(TradeRecord {
                timestamp: Utc::now(),
                market_id: "m".to_string(),
                side: "BUY".to_string(),
                size: dec!(100),
                price: dec!(0.50),
                pnl: Some(dec!(10)),
            }).await;
        }
        
        // 1 loss
        monitor.record_trade(TradeRecord {
            timestamp: Utc::now(),
            market_id: "m".to_string(),
            side: "BUY".to_string(),
            size: dec!(100),
            price: dec!(0.50),
            pnl: Some(dec!(-5)),
        }).await;
        
        let stats = monitor.get_stats().await;
        assert_eq!(stats.total_trades, 4);
        assert_eq!(stats.winning_trades, 3);
        assert_eq!(stats.win_rate, dec!(0.75));
    }

    #[tokio::test]
    async fn test_monitor_max_history() {
        let monitor = Monitor::new(3);  // Only keep 3 trades
        
        // Record 5 trades
        for i in 0..5 {
            monitor.record_trade(TradeRecord {
                timestamp: Utc::now(),
                market_id: format!("m{}", i),
                side: "BUY".to_string(),
                size: dec!(100),
                price: dec!(0.50),
                pnl: Some(dec!(10)),
            }).await;
        }
        
        let stats = monitor.get_stats().await;
        // Should only have last 3 trades
        assert_eq!(stats.total_trades, 3);
    }

    #[tokio::test]
    async fn test_monitor_empty_stats() {
        let monitor = Monitor::new(10);
        let stats = monitor.get_stats().await;
        
        assert_eq!(stats.total_trades, 0);
        assert_eq!(stats.win_rate, Decimal::ZERO);
        assert_eq!(stats.avg_pnl_per_trade, Decimal::ZERO);
    }

    #[tokio::test]
    async fn test_monitor_avg_pnl() {
        let monitor = Monitor::new(10);
        
        // Total PnL = 30, 3 trades, avg = 10
        monitor.record_trade(TradeRecord {
            timestamp: Utc::now(),
            market_id: "m1".to_string(),
            side: "BUY".to_string(),
            size: dec!(100),
            price: dec!(0.50),
            pnl: Some(dec!(20)),
        }).await;
        
        monitor.record_trade(TradeRecord {
            timestamp: Utc::now(),
            market_id: "m2".to_string(),
            side: "BUY".to_string(),
            size: dec!(100),
            price: dec!(0.50),
            pnl: Some(dec!(5)),
        }).await;
        
        monitor.record_trade(TradeRecord {
            timestamp: Utc::now(),
            market_id: "m3".to_string(),
            side: "BUY".to_string(),
            size: dec!(100),
            price: dec!(0.50),
            pnl: Some(dec!(5)),
        }).await;
        
        let stats = monitor.get_stats().await;
        assert_eq!(stats.total_pnl, dec!(30));
        assert_eq!(stats.avg_pnl_per_trade, dec!(10));
    }
}
