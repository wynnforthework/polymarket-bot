//! Tests for error types

#[cfg(test)]
mod tests {
    use super::super::error::BotError;
    use rust_decimal_macros::dec;

    #[test]
    fn test_api_error() {
        let err = BotError::Api("API unavailable".to_string());
        assert!(err.to_string().contains("API error"));
        assert!(err.to_string().contains("API unavailable"));
    }

    #[test]
    fn test_auth_error() {
        let err = BotError::Auth("Invalid signature".to_string());
        assert!(err.to_string().contains("Authentication error"));
    }

    #[test]
    fn test_websocket_error() {
        let err = BotError::WebSocket("Connection lost".to_string());
        assert!(err.to_string().contains("WebSocket error"));
    }

    #[test]
    fn test_config_error() {
        let err = BotError::Config("Missing API key".to_string());
        assert!(err.to_string().contains("Configuration error"));
    }

    #[test]
    fn test_strategy_error() {
        let err = BotError::Strategy("Invalid signal".to_string());
        assert!(err.to_string().contains("Strategy error"));
    }

    #[test]
    fn test_execution_error() {
        let err = BotError::Execution("Order failed".to_string());
        assert!(err.to_string().contains("Execution error"));
    }

    #[test]
    fn test_risk_limit_error() {
        let err = BotError::RiskLimit("Max exposure exceeded".to_string());
        assert!(err.to_string().contains("Risk limit exceeded"));
    }

    #[test]
    fn test_market_not_found() {
        let err = BotError::MarketNotFound("market123".to_string());
        assert!(err.to_string().contains("Market not found"));
        assert!(err.to_string().contains("market123"));
    }

    #[test]
    fn test_insufficient_balance() {
        let err = BotError::InsufficientBalance {
            required: dec!(1000),
            available: dec!(500),
        };
        let msg = err.to_string();
        assert!(msg.contains("Insufficient balance"));
        assert!(msg.contains("1000"));
        assert!(msg.contains("500"));
    }

    #[test]
    fn test_order_rejected() {
        let err = BotError::OrderRejected("Price slippage too high".to_string());
        assert!(err.to_string().contains("Order rejected"));
    }

    #[test]
    fn test_rate_limited() {
        let err = BotError::RateLimited { retry_after_secs: 30 };
        let msg = err.to_string();
        assert!(msg.contains("Rate limited"));
        assert!(msg.contains("30"));
    }

    #[test]
    fn test_internal_error() {
        let err = BotError::Internal("Unexpected state".to_string());
        assert!(err.to_string().contains("Internal error"));
    }

    #[test]
    fn test_error_is_debug() {
        let err = BotError::Api("test".to_string());
        let debug = format!("{:?}", err);
        assert!(debug.contains("Api"));
    }

    #[test]
    fn test_error_variants_distinct() {
        let api = BotError::Api("test".to_string());
        let auth = BotError::Auth("test".to_string());
        
        // They have different Display outputs
        assert_ne!(api.to_string(), auth.to_string());
    }
}
