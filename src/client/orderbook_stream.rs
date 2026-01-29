//! Real-time order book WebSocket stream
//!
//! Provides full order book depth updates via WebSocket.

use crate::error::{BotError, Result};
use crate::storage::cache::{CachedOrderBook, OrderBookCache, PriceCache, PriceData};
use futures_util::{SinkExt, StreamExt};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn};
use chrono::Utc;

/// Full order book manager
#[derive(Clone)]
pub struct OrderBookManager {
    /// Local order book state: token_id -> OrderBook
    books: Arc<RwLock<HashMap<String, LocalOrderBook>>>,
    /// Price cache for quick lookups
    price_cache: PriceCache,
    /// Order book cache
    book_cache: OrderBookCache,
    /// Subscribed tokens
    subscribed: Arc<RwLock<Vec<String>>>,
}

/// Local order book state
#[derive(Debug, Clone)]
pub struct LocalOrderBook {
    pub token_id: String,
    pub bids: HashMap<String, OrderBookEntry>,  // price string -> entry
    pub asks: HashMap<String, OrderBookEntry>,
    pub sequence: u64,
    pub last_update: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookEntry {
    pub price: Decimal,
    pub size: Decimal,
}

/// Order book update event
#[derive(Debug, Clone)]
pub struct OrderBookUpdate {
    pub token_id: String,
    pub update_type: UpdateType,
    pub side: BookSide,
    pub price: Decimal,
    pub size: Decimal,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UpdateType {
    Snapshot,
    Delta,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BookSide {
    Bid,
    Ask,
}

/// WebSocket message types from Polymarket
#[derive(Debug, Deserialize)]
struct WsBookMessage {
    #[serde(rename = "type")]
    msg_type: String,
    asset_id: Option<String>,
    market: Option<String>,
    bids: Option<Vec<WsBookLevel>>,
    asks: Option<Vec<WsBookLevel>>,
    price: Option<String>,
    size: Option<String>,
    side: Option<String>,
    timestamp: Option<u64>,
    hash: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WsBookLevel {
    price: String,
    size: String,
}

impl Default for OrderBookManager {
    fn default() -> Self {
        Self::new()
    }
}

impl OrderBookManager {
    pub fn new() -> Self {
        Self {
            books: Arc::new(RwLock::new(HashMap::new())),
            price_cache: PriceCache::new(10),
            book_cache: OrderBookCache::new(5),
            subscribed: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Get current best bid/ask for a token
    pub async fn get_bbo(&self, token_id: &str) -> Option<(Decimal, Decimal)> {
        // Try cache first
        if let Some(price) = self.price_cache.get(token_id) {
            if let (Some(bid), Some(ask)) = (price.best_bid, price.best_ask) {
                return Some((bid, ask));
            }
        }

        // Fall back to local book
        let books = self.books.read().await;
        let book = books.get(token_id)?;
        
        let best_bid = book.bids.values().map(|e| e.price).max()?;
        let best_ask = book.asks.values().map(|e| e.price).min()?;
        
        Some((best_bid, best_ask))
    }

    /// Get full order book for a token
    pub async fn get_book(&self, token_id: &str) -> Option<LocalOrderBook> {
        let books = self.books.read().await;
        books.get(token_id).cloned()
    }

    /// Get sorted bids (highest first)
    pub async fn get_bids(&self, token_id: &str, depth: usize) -> Vec<OrderBookEntry> {
        let books = self.books.read().await;
        if let Some(book) = books.get(token_id) {
            let mut bids: Vec<_> = book.bids.values().cloned().collect();
            bids.sort_by(|a, b| b.price.cmp(&a.price));
            bids.truncate(depth);
            bids
        } else {
            Vec::new()
        }
    }

    /// Get sorted asks (lowest first)
    pub async fn get_asks(&self, token_id: &str, depth: usize) -> Vec<OrderBookEntry> {
        let books = self.books.read().await;
        if let Some(book) = books.get(token_id) {
            let mut asks: Vec<_> = book.asks.values().cloned().collect();
            asks.sort_by(|a, b| a.price.cmp(&b.price));
            asks.truncate(depth);
            asks
        } else {
            Vec::new()
        }
    }

    /// Calculate market depth within percentage of best price
    pub async fn get_depth(&self, token_id: &str, pct: Decimal) -> Option<(Decimal, Decimal)> {
        let books = self.books.read().await;
        let book = books.get(token_id)?;

        let best_bid = book.bids.values().map(|e| e.price).max()?;
        let best_ask = book.asks.values().map(|e| e.price).min()?;

        let bid_threshold = best_bid * (Decimal::ONE - pct / Decimal::from(100));
        let ask_threshold = best_ask * (Decimal::ONE + pct / Decimal::from(100));

        let bid_depth: Decimal = book.bids.values()
            .filter(|e| e.price >= bid_threshold)
            .map(|e| e.size)
            .sum();

        let ask_depth: Decimal = book.asks.values()
            .filter(|e| e.price <= ask_threshold)
            .map(|e| e.size)
            .sum();

        Some((bid_depth, ask_depth))
    }

    /// Start WebSocket connection
    pub async fn connect(&self, base_url: &str, token_ids: Vec<String>) -> Result<mpsc::Receiver<OrderBookUpdate>> {
        let ws_url = base_url
            .replace("https://", "wss://")
            .replace("http://", "ws://");
        let ws_url = format!("{}/ws/market", ws_url);

        info!("Connecting to order book WebSocket: {}", ws_url);

        let (ws_stream, _) = connect_async(&ws_url)
            .await
            .map_err(|e| BotError::WebSocket(e.to_string()))?;

        let (mut write, mut read) = ws_stream.split();
        let (tx, rx) = mpsc::channel(10000);

        // Store subscribed tokens
        {
            let mut subscribed = self.subscribed.write().await;
            *subscribed = token_ids.clone();
        }

        // Subscribe to order book channels
        let subscribe_msg = serde_json::json!({
            "type": "subscribe",
            "channel": "book",
            "assets": token_ids,
        });
        write
            .send(Message::Text(subscribe_msg.to_string().into()))
            .await
            .map_err(|e| BotError::WebSocket(e.to_string()))?;

        // Also subscribe to price updates
        let price_subscribe = serde_json::json!({
            "type": "subscribe", 
            "channel": "market",
            "assets": token_ids,
        });
        write
            .send(Message::Text(price_subscribe.to_string().into()))
            .await
            .map_err(|e| BotError::WebSocket(e.to_string()))?;

        info!("Subscribed to {} token order books", token_ids.len());

        // Clone what we need for the task
        let books = Arc::clone(&self.books);
        let price_cache = self.price_cache.clone();
        let book_cache = self.book_cache.clone();

        // Spawn reader task
        tokio::spawn(async move {
            while let Some(msg) = read.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        if let Ok(ws_msg) = serde_json::from_str::<WsBookMessage>(&text) {
                            if let Some(updates) = Self::process_message(&ws_msg, &books, &price_cache, &book_cache).await {
                                for update in updates {
                                    if tx.send(update).await.is_err() {
                                        warn!("Order book update channel closed");
                                        return;
                                    }
                                }
                            }
                        }
                    }
                    Ok(Message::Ping(_)) => {
                        debug!("Received ping");
                    }
                    Ok(Message::Close(_)) => {
                        warn!("Order book WebSocket closed");
                        break;
                    }
                    Err(e) => {
                        error!("WebSocket error: {}", e);
                        break;
                    }
                    _ => {}
                }
            }
        });

        Ok(rx)
    }

    async fn process_message(
        msg: &WsBookMessage,
        books: &Arc<RwLock<HashMap<String, LocalOrderBook>>>,
        price_cache: &PriceCache,
        book_cache: &OrderBookCache,
    ) -> Option<Vec<OrderBookUpdate>> {
        let mut updates = Vec::new();
        let token_id = msg.asset_id.as_ref().or(msg.market.as_ref())?;

        match msg.msg_type.as_str() {
            "book_snapshot" => {
                // Full book snapshot
                let mut book = LocalOrderBook {
                    token_id: token_id.clone(),
                    bids: HashMap::new(),
                    asks: HashMap::new(),
                    sequence: 0,
                    last_update: chrono::Utc::now(),
                };

                if let Some(bids) = &msg.bids {
                    for level in bids {
                        if let (Ok(price), Ok(size)) = (level.price.parse::<Decimal>(), level.size.parse::<Decimal>()) {
                            if size > Decimal::ZERO {
                                book.bids.insert(level.price.clone(), OrderBookEntry { price, size });
                                updates.push(OrderBookUpdate {
                                    token_id: token_id.clone(),
                                    update_type: UpdateType::Snapshot,
                                    side: BookSide::Bid,
                                    price,
                                    size,
                                    timestamp: msg.timestamp.unwrap_or(0),
                                });
                            }
                        }
                    }
                }

                if let Some(asks) = &msg.asks {
                    for level in asks {
                        if let (Ok(price), Ok(size)) = (level.price.parse::<Decimal>(), level.size.parse::<Decimal>()) {
                            if size > Decimal::ZERO {
                                book.asks.insert(level.price.clone(), OrderBookEntry { price, size });
                                updates.push(OrderBookUpdate {
                                    token_id: token_id.clone(),
                                    update_type: UpdateType::Snapshot,
                                    side: BookSide::Ask,
                                    price,
                                    size,
                                    timestamp: msg.timestamp.unwrap_or(0),
                                });
                            }
                        }
                    }
                }

                // Update caches
                Self::update_caches(&book, price_cache, book_cache);

                let mut books = books.write().await;
                books.insert(token_id.clone(), book);

                debug!("Received book snapshot for {}", token_id);
            }
            "book_delta" | "book" => {
                // Incremental update
                let price = msg.price.as_ref()?.parse::<Decimal>().ok()?;
                let size = msg.size.as_ref()?.parse::<Decimal>().ok()?;
                let side = msg.side.as_ref()?;

                let mut books = books.write().await;
                let book = books.entry(token_id.clone()).or_insert_with(|| LocalOrderBook {
                    token_id: token_id.clone(),
                    bids: HashMap::new(),
                    asks: HashMap::new(),
                    sequence: 0,
                    last_update: chrono::Utc::now(),
                });

                let price_key = msg.price.clone().unwrap_or_default();
                let (side_map, book_side) = if side == "BUY" || side == "bid" {
                    (&mut book.bids, BookSide::Bid)
                } else {
                    (&mut book.asks, BookSide::Ask)
                };

                if size == Decimal::ZERO {
                    side_map.remove(&price_key);
                } else {
                    side_map.insert(price_key, OrderBookEntry { price, size });
                }

                book.last_update = chrono::Utc::now();

                updates.push(OrderBookUpdate {
                    token_id: token_id.clone(),
                    update_type: UpdateType::Delta,
                    side: book_side,
                    price,
                    size,
                    timestamp: msg.timestamp.unwrap_or(0),
                });

                // Update caches
                Self::update_caches(book, price_cache, book_cache);
            }
            "price_change" => {
                // Just a price update, update cache
                let price = msg.price.as_ref()?.parse::<Decimal>().ok()?;
                
                price_cache.update(token_id, PriceData {
                    best_bid: None,
                    best_ask: None,
                    last_price: Some(price),
                    midpoint: None,
                    timestamp: Utc::now(),
                });
            }
            _ => {}
        }

        if updates.is_empty() {
            None
        } else {
            Some(updates)
        }
    }

    fn update_caches(book: &LocalOrderBook, price_cache: &PriceCache, book_cache: &OrderBookCache) {
        // Calculate BBO
        let best_bid = book.bids.values().map(|e| e.price).max();
        let best_ask = book.asks.values().map(|e| e.price).min();
        let midpoint = match (best_bid, best_ask) {
            (Some(b), Some(a)) => Some((b + a) / Decimal::from(2)),
            _ => None,
        };

        price_cache.update(&book.token_id, PriceData {
            best_bid,
            best_ask,
            last_price: midpoint,
            midpoint,
            timestamp: Utc::now(),
        });

        // Update order book cache
        let cached_bids: Vec<_> = book.bids.values()
            .map(|e| (e.price, e.size))
            .collect();
        let cached_asks: Vec<_> = book.asks.values()
            .map(|e| (e.price, e.size))
            .collect();

        book_cache.update(&book.token_id, CachedOrderBook {
            token_id: book.token_id.clone(),
            bids: cached_bids,
            asks: cached_asks,
            timestamp: Utc::now(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[tokio::test]
    async fn test_orderbook_manager() {
        let manager = OrderBookManager::new();
        
        // Manually insert test data
        {
            let mut books = manager.books.write().await;
            let mut book = LocalOrderBook {
                token_id: "test_token".to_string(),
                bids: HashMap::new(),
                asks: HashMap::new(),
                sequence: 1,
                last_update: chrono::Utc::now(),
            };
            
            book.bids.insert("0.45".to_string(), OrderBookEntry {
                price: dec!(0.45),
                size: dec!(100),
            });
            book.bids.insert("0.44".to_string(), OrderBookEntry {
                price: dec!(0.44),
                size: dec!(200),
            });
            
            book.asks.insert("0.55".to_string(), OrderBookEntry {
                price: dec!(0.55),
                size: dec!(100),
            });
            book.asks.insert("0.56".to_string(), OrderBookEntry {
                price: dec!(0.56),
                size: dec!(150),
            });
            
            books.insert("test_token".to_string(), book);
        }
        
        // Test BBO
        let bbo = manager.get_bbo("test_token").await;
        assert!(bbo.is_some());
        let (bid, ask) = bbo.unwrap();
        assert_eq!(bid, dec!(0.45));
        assert_eq!(ask, dec!(0.55));
        
        // Test bids
        let bids = manager.get_bids("test_token", 10).await;
        assert_eq!(bids.len(), 2);
        assert_eq!(bids[0].price, dec!(0.45));  // Highest first
        
        // Test asks
        let asks = manager.get_asks("test_token", 10).await;
        assert_eq!(asks.len(), 2);
        assert_eq!(asks[0].price, dec!(0.55));  // Lowest first
    }

    #[test]
    fn test_orderbook_entry() {
        let entry = OrderBookEntry {
            price: dec!(0.50),
            size: dec!(1000),
        };
        
        assert_eq!(entry.price, dec!(0.50));
        assert_eq!(entry.size, dec!(1000));
    }
}
