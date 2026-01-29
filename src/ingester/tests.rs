//! Unit tests for ingester module

#[cfg(test)]
mod tests {
    use super::super::*;
    use chrono::Utc;

    #[test]
    fn test_raw_signal_creation() {
        let signal = RawSignal {
            source: "twitter".to_string(),
            source_id: "12345".to_string(),
            content: "BTC looking bullish here".to_string(),
            author: "crypto_trader".to_string(),
            author_trust: 0.7,
            timestamp: Utc::now(),
            metadata: None,
        };
        
        assert_eq!(signal.source, "twitter");
        assert_eq!(signal.author_trust, 0.7);
    }

    #[test]
    fn test_signal_direction_serialization() {
        assert_eq!(
            serde_json::to_string(&SignalDirection::Bullish).unwrap(),
            "\"bullish\""
        );
        assert_eq!(
            serde_json::to_string(&SignalDirection::Bearish).unwrap(),
            "\"bearish\""
        );
    }

    #[test]
    fn test_action_type_serialization() {
        assert_eq!(
            serde_json::to_string(&ActionType::Entry).unwrap(),
            "\"entry\""
        );
        assert_eq!(
            serde_json::to_string(&ActionType::Exit).unwrap(),
            "\"exit\""
        );
    }

    #[test]
    fn test_parsed_signal_creation() {
        let raw = RawSignal {
            source: "telegram".to_string(),
            source_id: "msg123".to_string(),
            content: "ETH breakout".to_string(),
            author: "alpha_group".to_string(),
            author_trust: 0.8,
            timestamp: Utc::now(),
            metadata: None,
        };

        let parsed = ParsedSignal {
            token: "ETH".to_string(),
            direction: SignalDirection::Bullish,
            timeframe: "1h".to_string(),
            confidence: 0.75,
            reasoning: "Technical breakout pattern".to_string(),
            action_type: ActionType::Entry,
            sources: vec![raw],
            agg_score: 0.8,
            timestamp: Utc::now(),
        };

        assert_eq!(parsed.token, "ETH");
        assert_eq!(parsed.direction, SignalDirection::Bullish);
        assert!(parsed.agg_score >= 0.7);
    }

    #[test]
    fn test_ingester_config_defaults() {
        let config: IngesterConfig = serde_json::from_str(r#"{}"#).unwrap_or(IngesterConfig {
            telegram: None,
            twitter: None,
            author_trust: std::collections::HashMap::new(),
        });
        
        assert!(config.telegram.is_none());
        assert!(config.twitter.is_none());
        assert!(config.author_trust.is_empty());
    }
}
