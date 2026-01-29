//! Tests for storage module

#[cfg(test)]
mod tests {
    use crate::types::{Trade, Side};
    use chrono::Utc;
    use rust_decimal_macros::dec;

    #[test]
    fn test_trade_creation() {
        let trade = Trade {
            id: "trade1".to_string(),
            order_id: "order1".to_string(),
            token_id: "token1".to_string(),
            market_id: "market1".to_string(),
            side: Side::Buy,
            price: dec!(0.55),
            size: dec!(100),
            fee: dec!(0.50),
            timestamp: Utc::now(),
        };
        
        assert_eq!(trade.id, "trade1");
        assert_eq!(trade.side, Side::Buy);
        assert_eq!(trade.fee, dec!(0.50));
    }

    #[test]
    fn test_trade_serialization() {
        let trade = Trade {
            id: "t1".to_string(),
            order_id: "o1".to_string(),
            token_id: "tk1".to_string(),
            market_id: "m1".to_string(),
            side: Side::Sell,
            price: dec!(0.65),
            size: dec!(200),
            fee: dec!(1.00),
            timestamp: Utc::now(),
        };
        
        let json = serde_json::to_string(&trade).unwrap();
        assert!(json.contains("\"id\":\"t1\""));
        assert!(json.contains("\"side\":\"SELL\""));
    }

    #[test]
    fn test_trade_deserialization() {
        let json = r#"{
            "id": "trade123",
            "order_id": "order123",
            "token_id": "token123",
            "market_id": "market123",
            "side": "BUY",
            "price": "0.45",
            "size": "500",
            "fee": "2.50",
            "timestamp": "2024-06-15T12:00:00Z"
        }"#;
        
        let trade: Trade = serde_json::from_str(json).unwrap();
        assert_eq!(trade.id, "trade123");
        assert_eq!(trade.side, Side::Buy);
        assert_eq!(trade.price, dec!(0.45));
        assert_eq!(trade.size, dec!(500));
        assert_eq!(trade.fee, dec!(2.50));
    }

    #[test]
    fn test_trade_pnl_calculation_buy() {
        // Bought at 0.45, market at 0.55 = 0.10 profit per share
        let entry = dec!(0.45);
        let current = dec!(0.55);
        let size = dec!(100);
        
        let pnl = (current - entry) * size;
        assert_eq!(pnl, dec!(10));
    }

    #[test]
    fn test_trade_pnl_calculation_sell() {
        // Sold at 0.60, market at 0.50 = 0.10 profit per share
        let entry = dec!(0.60);
        let current = dec!(0.50);
        let size = dec!(100);
        
        // For sell, profit is (entry - current) * size
        let pnl = (entry - current) * size;
        assert_eq!(pnl, dec!(10));
    }

    #[test]
    fn test_trade_loss_scenario() {
        let entry = dec!(0.65);
        let current = dec!(0.55);
        let size = dec!(100);
        
        // Buy at 0.65, market dropped to 0.55 = loss
        let pnl = (current - entry) * size;
        assert_eq!(pnl, dec!(-10));
    }

    #[test]
    fn test_fee_impact() {
        let gross_profit = dec!(50);
        let fee = dec!(5);
        let net_profit = gross_profit - fee;
        assert_eq!(net_profit, dec!(45));
    }
}
