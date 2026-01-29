//! Polymarket API client
//!
//! This module provides interfaces to interact with Polymarket's APIs:
//! - CLOB API: Order placement, cancellation, and management
//! - Gamma API: Market data and information
//! - WebSocket: Real-time price updates

pub mod clob;
mod gamma;
mod auth;
mod websocket;
#[cfg(test)]
mod tests;

pub use clob::{ClobClient, OrderBook, OrderBookLevel};
pub use gamma::GammaClient;
pub use auth::PolySigner;
pub use websocket::MarketStream;

use crate::config::PolymarketConfig;
use crate::error::Result;

/// Unified Polymarket client
pub struct PolymarketClient {
    pub clob: ClobClient,
    pub gamma: GammaClient,
    config: PolymarketConfig,
}

impl PolymarketClient {
    /// Create a new Polymarket client
    pub async fn new(config: PolymarketConfig) -> Result<Self> {
        let signer = PolySigner::from_private_key(&config.private_key, config.chain_id)?;
        let clob = ClobClient::new(&config.clob_url, signer, config.funder_address.clone())?;
        let gamma = GammaClient::new(&config.gamma_url)?;

        Ok(Self { clob, gamma, config })
    }

    /// Create a WebSocket stream for real-time market data
    pub async fn market_stream(&self, token_ids: Vec<String>) -> Result<MarketStream> {
        MarketStream::connect(&self.config.clob_url, token_ids).await
    }
}
