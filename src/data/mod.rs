//! Data engineering module
//!
//! Enhanced data capabilities:
//! - WebSocket connection with auto-reconnect
//! - Data validation and cleaning
//! - Multi-source aggregation (Polymarket + Binance + others)
//! - Rate limiting and caching

pub mod aggregator;
pub mod cleaning;
pub mod websocket;

pub use aggregator::{DataAggregator, AggregatedPrice, DataSource};
pub use cleaning::{DataCleaner, CleaningConfig, ValidationResult, Anomaly};
pub use websocket::{ReconnectingWebSocket, WebSocketConfig, ConnectionState, LatencyStats};
