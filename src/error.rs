//! Error types for the trading bot

use thiserror::Error;

#[derive(Error, Debug)]
pub enum BotError {
    #[error("API error: {0}")]
    Api(String),

    #[error("Authentication error: {0}")]
    Auth(String),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("WebSocket error: {0}")]
    WebSocket(String),

    #[error("JSON parsing error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Strategy error: {0}")]
    Strategy(String),

    #[error("Execution error: {0}")]
    Execution(String),

    #[error("Risk limit exceeded: {0}")]
    RiskLimit(String),

    #[error("Market not found: {0}")]
    MarketNotFound(String),

    #[error("Insufficient balance: required {required}, available {available}")]
    InsufficientBalance {
        required: rust_decimal::Decimal,
        available: rust_decimal::Decimal,
    },

    #[error("Order rejected: {0}")]
    OrderRejected(String),

    #[error("Rate limited: retry after {retry_after_secs} seconds")]
    RateLimited { retry_after_secs: u64 },

    #[error("Internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, BotError>;
