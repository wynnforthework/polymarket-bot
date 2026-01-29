//! WebSocket client for real-time market data

use crate::error::{BotError, Result};
use futures_util::{SinkExt, StreamExt};
use rust_decimal::Decimal;
use serde::Deserialize;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};

/// Real-time market data stream
pub struct MarketStream {
    rx: mpsc::Receiver<MarketUpdate>,
}

/// Market update from WebSocket
#[derive(Debug, Clone)]
pub struct MarketUpdate {
    pub token_id: String,
    pub best_bid: Option<Decimal>,
    pub best_ask: Option<Decimal>,
    pub last_price: Option<Decimal>,
    pub timestamp: u64,
}

#[derive(Debug, Deserialize)]
struct WsMessage {
    #[serde(rename = "type")]
    msg_type: String,
    data: Option<serde_json::Value>,
}

impl MarketStream {
    /// Connect to WebSocket and subscribe to markets
    pub async fn connect(base_url: &str, token_ids: Vec<String>) -> Result<Self> {
        let ws_url = base_url
            .replace("https://", "wss://")
            .replace("http://", "ws://");
        let ws_url = format!("{}/ws/market", ws_url);

        let (ws_stream, _) = connect_async(&ws_url)
            .await
            .map_err(|e| BotError::WebSocket(e.to_string()))?;

        let (mut write, mut read) = ws_stream.split();
        let (tx, rx) = mpsc::channel(1000);

        // Subscribe to tokens
        let subscribe_msg = serde_json::json!({
            "type": "subscribe",
            "channel": "market",
            "assets": token_ids,
        });
        write
            .send(Message::Text(subscribe_msg.to_string().into()))
            .await
            .map_err(|e| BotError::WebSocket(e.to_string()))?;

        // Spawn reader task
        tokio::spawn(async move {
            while let Some(msg) = read.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        if let Ok(ws_msg) = serde_json::from_str::<WsMessage>(&text) {
                            if let Some(update) = Self::parse_update(&ws_msg) {
                                if tx.send(update).await.is_err() {
                                    break;
                                }
                            }
                        }
                    }
                    Ok(Message::Ping(data)) => {
                        // Respond to ping (handled automatically by tungstenite)
                        let _ = data;
                    }
                    Ok(Message::Close(_)) => break,
                    Err(_) => break,
                    _ => {}
                }
            }
        });

        Ok(Self { rx })
    }

    fn parse_update(msg: &WsMessage) -> Option<MarketUpdate> {
        if msg.msg_type != "price_change" {
            return None;
        }

        let data = msg.data.as_ref()?;
        Some(MarketUpdate {
            token_id: data["asset_id"].as_str()?.to_string(),
            best_bid: data["best_bid"].as_str().and_then(|s| s.parse().ok()),
            best_ask: data["best_ask"].as_str().and_then(|s| s.parse().ok()),
            last_price: data["price"].as_str().and_then(|s| s.parse().ok()),
            timestamp: data["timestamp"].as_u64().unwrap_or(0),
        })
    }

    /// Receive next market update
    pub async fn recv(&mut self) -> Option<MarketUpdate> {
        self.rx.recv().await
    }
}
