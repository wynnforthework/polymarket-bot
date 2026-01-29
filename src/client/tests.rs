//! Tests for client module

#[cfg(test)]
mod tests {
    use crate::types::{Order, OrderType, Side};
    use rust_decimal_macros::dec;

    #[test]
    fn test_order_for_api() {
        let order = Order {
            token_id: "token123".to_string(),
            side: Side::Buy,
            price: dec!(0.55),
            size: dec!(100),
            order_type: OrderType::GTC,
        };
        
        assert_eq!(order.token_id, "token123");
        assert_eq!(order.side, Side::Buy);
        assert_eq!(order.price, dec!(0.55));
    }

    #[test]
    fn test_order_type_gtc() {
        let order = Order {
            token_id: "t".to_string(),
            side: Side::Buy,
            price: dec!(0.5),
            size: dec!(10),
            order_type: OrderType::GTC,
        };
        assert_eq!(order.order_type, OrderType::GTC);
    }

    #[test]
    fn test_order_type_fok() {
        let order = Order {
            token_id: "t".to_string(),
            side: Side::Sell,
            price: dec!(0.5),
            size: dec!(10),
            order_type: OrderType::FOK,
        };
        assert_eq!(order.order_type, OrderType::FOK);
    }

    #[test]
    fn test_order_type_gtd() {
        let order = Order {
            token_id: "t".to_string(),
            side: Side::Buy,
            price: dec!(0.5),
            size: dec!(10),
            order_type: OrderType::GTD,
        };
        assert_eq!(order.order_type, OrderType::GTD);
    }

    #[test]
    fn test_side_buy() {
        assert_eq!(Side::Buy, Side::Buy);
        assert_ne!(Side::Buy, Side::Sell);
    }

    #[test]
    fn test_side_sell() {
        assert_eq!(Side::Sell, Side::Sell);
    }

    #[test]
    fn test_order_serialization() {
        let order = Order {
            token_id: "abc".to_string(),
            side: Side::Buy,
            price: dec!(0.45),
            size: dec!(50),
            order_type: OrderType::GTC,
        };
        let json = serde_json::to_string(&order).unwrap();
        assert!(json.contains("\"token_id\":\"abc\""));
        assert!(json.contains("\"side\":\"BUY\""));
    }

    #[test]
    fn test_order_deserialization() {
        let json = r#"{
            "token_id": "xyz",
            "side": "SELL",
            "price": "0.75",
            "size": "200",
            "order_type": "FOK"
        }"#;
        let order: Order = serde_json::from_str(json).unwrap();
        assert_eq!(order.token_id, "xyz");
        assert_eq!(order.side, Side::Sell);
        assert_eq!(order.order_type, OrderType::FOK);
    }

    // Test market data parsing
    #[test]
    fn test_market_json_parsing() {
        let json = r#"{
            "id": "market1",
            "question": "Will it rain?",
            "description": "Weather market",
            "volume": "10000",
            "liquidity": "5000",
            "outcomes": [
                {"token_id": "yes", "outcome": "Yes", "price": "0.65"},
                {"token_id": "no", "outcome": "No", "price": "0.35"}
            ],
            "active": true,
            "closed": false
        }"#;
        let market: crate::types::Market = serde_json::from_str(json).unwrap();
        assert_eq!(market.id, "market1");
        assert_eq!(market.question, "Will it rain?");
        assert!(market.active);
        assert!(!market.closed);
    }

    #[test]
    fn test_market_with_outcomes() {
        let json = r#"{
            "id": "m1",
            "question": "Test?",
            "volume": "1000",
            "liquidity": "500",
            "outcomes": [
                {"token_id": "y", "outcome": "Yes", "price": "0.70"},
                {"token_id": "n", "outcome": "No", "price": "0.30"}
            ],
            "active": true,
            "closed": false
        }"#;
        let market: crate::types::Market = serde_json::from_str(json).unwrap();
        assert_eq!(market.outcomes.len(), 2);
        assert_eq!(market.yes_price(), Some(dec!(0.70)));
        assert_eq!(market.no_price(), Some(dec!(0.30)));
    }

    #[test]
    fn test_order_status_parsing() {
        let json = r#"{
            "order_id": "order123",
            "status": "FILLED",
            "filled_size": "100",
            "remaining_size": "0",
            "avg_price": "0.55"
        }"#;
        let status: crate::types::OrderStatus = serde_json::from_str(json).unwrap();
        assert_eq!(status.order_id, "order123");
        assert_eq!(status.status, "FILLED");
        assert_eq!(status.filled_size, dec!(100));
        assert_eq!(status.remaining_size, dec!(0));
    }

    #[test]
    fn test_order_status_partial_fill() {
        let json = r#"{
            "order_id": "order456",
            "status": "PARTIAL",
            "filled_size": "50",
            "remaining_size": "50"
        }"#;
        let status: crate::types::OrderStatus = serde_json::from_str(json).unwrap();
        assert_eq!(status.status, "PARTIAL");
        assert_eq!(status.filled_size, dec!(50));
        assert!(status.avg_price.is_none());
    }

    #[test]
    fn test_position_parsing() {
        let json = r#"{
            "token_id": "tok1",
            "market_id": "mkt1",
            "side": "BUY",
            "size": "500",
            "avg_entry_price": "0.45",
            "current_price": "0.55",
            "unrealized_pnl": "50"
        }"#;
        let pos: crate::types::Position = serde_json::from_str(json).unwrap();
        assert_eq!(pos.token_id, "tok1");
        assert_eq!(pos.side, Side::Buy);
        assert_eq!(pos.unrealized_pnl, dec!(50));
    }

    #[test]
    fn test_trade_parsing() {
        let json = r#"{
            "id": "trade1",
            "order_id": "order1",
            "token_id": "token1",
            "market_id": "market1",
            "side": "SELL",
            "price": "0.60",
            "size": "100",
            "fee": "0.50",
            "timestamp": "2024-01-15T10:30:00Z"
        }"#;
        let trade: crate::types::Trade = serde_json::from_str(json).unwrap();
        assert_eq!(trade.id, "trade1");
        assert_eq!(trade.side, Side::Sell);
        assert_eq!(trade.fee, dec!(0.50));
    }

    #[test]
    fn test_crypto_search_queries_exist() {
        use crate::client::gamma::CRYPTO_SEARCH_QUERIES;
        assert!(CRYPTO_SEARCH_QUERIES.len() >= 4);
        assert!(CRYPTO_SEARCH_QUERIES.contains(&"bitcoin up or down"));
        assert!(CRYPTO_SEARCH_QUERIES.contains(&"ethereum up or down"));
    }

    #[test]
    fn test_crypto_series_exist() {
        use crate::client::gamma::CRYPTO_SERIES;
        assert!(CRYPTO_SERIES.len() >= 6);
        // Check BTC series
        assert!(CRYPTO_SERIES.iter().any(|(name, _, _)| name.contains("BTC")));
        // Check ETH series
        assert!(CRYPTO_SERIES.iter().any(|(name, _, _)| name.contains("ETH")));
    }

    #[test]
    fn test_hourly_market_question_detection() {
        // Test dynamic hourly market format detection
        let questions = vec![
            ("Bitcoin Up or Down - January 29, 5PM-6PM ET", true),
            ("Ethereum Up or Down - January 29, 10AM ET", true),
            ("Solana Up or Down - January 29", true),
            ("XRP Up or Down - January 29, 11PM ET", true),
            ("Will Trump win?", false),
            ("Bitcoin price tomorrow", false),
        ];

        for (question, expected) in questions {
            let q_lower = question.to_lowercase();
            let is_crypto_up_down = q_lower.contains("up or down") 
                && (q_lower.contains("bitcoin") || q_lower.contains("ethereum")
                    || q_lower.contains("solana") || q_lower.contains("xrp"));
            assert_eq!(is_crypto_up_down, expected, "Failed for: {}", question);
        }
    }
}
