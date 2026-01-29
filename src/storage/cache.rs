//! In-memory cache layer for reducing API calls
//!
//! Provides TTL-based caching for frequently accessed data.

use chrono::{DateTime, Duration, Utc};
use parking_lot::RwLock;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Cache entry with TTL
#[derive(Debug, Clone)]
struct CacheEntry<T> {
    value: T,
    expires_at: DateTime<Utc>,
}

impl<T> CacheEntry<T> {
    fn new(value: T, ttl_secs: i64) -> Self {
        Self {
            value,
            expires_at: Utc::now() + Duration::seconds(ttl_secs),
        }
    }

    fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }
}

/// Market price cache
#[derive(Debug, Clone)]
pub struct PriceCache {
    /// Token ID -> (price, timestamp)
    prices: Arc<RwLock<HashMap<String, CacheEntry<PriceData>>>>,
    /// Default TTL in seconds
    default_ttl: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceData {
    pub best_bid: Option<Decimal>,
    pub best_ask: Option<Decimal>,
    pub last_price: Option<Decimal>,
    pub midpoint: Option<Decimal>,
    pub timestamp: DateTime<Utc>,
}

impl Default for PriceCache {
    fn default() -> Self {
        Self::new(5) // 5 second default TTL
    }
}

impl PriceCache {
    /// Create a new price cache
    pub fn new(default_ttl_secs: i64) -> Self {
        Self {
            prices: Arc::new(RwLock::new(HashMap::new())),
            default_ttl: default_ttl_secs,
        }
    }

    /// Update price for a token
    pub fn update(&self, token_id: &str, data: PriceData) {
        let mut cache = self.prices.write();
        cache.insert(
            token_id.to_string(),
            CacheEntry::new(data, self.default_ttl),
        );
    }

    /// Get price for a token (None if expired or not found)
    pub fn get(&self, token_id: &str) -> Option<PriceData> {
        let cache = self.prices.read();
        cache.get(token_id).and_then(|entry: &CacheEntry<PriceData>| {
            if entry.is_expired() {
                None
            } else {
                Some(entry.value.clone())
            }
        })
    }

    /// Get multiple prices at once
    pub fn get_many(&self, token_ids: &[String]) -> HashMap<String, PriceData> {
        let cache = self.prices.read();
        token_ids
            .iter()
            .filter_map(|id| {
                cache.get(id).and_then(|entry: &CacheEntry<PriceData>| {
                    if entry.is_expired() {
                        None
                    } else {
                        Some((id.clone(), entry.value.clone()))
                    }
                })
            })
            .collect()
    }

    /// Clear expired entries
    pub fn cleanup(&self) {
        let mut cache = self.prices.write();
        cache.retain(|_, entry: &mut CacheEntry<PriceData>| !entry.is_expired());
    }

    /// Get cache stats
    pub fn stats(&self) -> CacheStats {
        let cache = self.prices.read();
        let total = cache.len();
        let expired = cache.values().filter(|e| e.is_expired()).count();
        CacheStats {
            total_entries: total,
            expired_entries: expired,
            valid_entries: total - expired,
        }
    }
}

/// Order book cache
#[derive(Debug, Clone)]
pub struct OrderBookCache {
    books: Arc<RwLock<HashMap<String, CacheEntry<CachedOrderBook>>>>,
    default_ttl: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedOrderBook {
    pub token_id: String,
    pub bids: Vec<(Decimal, Decimal)>,  // (price, size)
    pub asks: Vec<(Decimal, Decimal)>,
    pub timestamp: DateTime<Utc>,
}

impl Default for OrderBookCache {
    fn default() -> Self {
        Self::new(2) // 2 second TTL for order books
    }
}

impl OrderBookCache {
    pub fn new(default_ttl_secs: i64) -> Self {
        Self {
            books: Arc::new(RwLock::new(HashMap::new())),
            default_ttl: default_ttl_secs,
        }
    }

    pub fn update(&self, token_id: &str, book: CachedOrderBook) {
        let mut cache = self.books.write();
        cache.insert(token_id.to_string(), CacheEntry::new(book, self.default_ttl));
    }

    pub fn get(&self, token_id: &str) -> Option<CachedOrderBook> {
        let cache = self.books.read();
        cache.get(token_id).and_then(|entry: &CacheEntry<CachedOrderBook>| {
            if entry.is_expired() {
                None
            } else {
                Some(entry.value.clone())
            }
        })
    }

    pub fn cleanup(&self) {
        let mut cache = self.books.write();
        cache.retain(|_, entry: &mut CacheEntry<CachedOrderBook>| !entry.is_expired());
    }
}

/// Market info cache (longer TTL)
#[derive(Debug, Clone)]
pub struct MarketCache {
    markets: Arc<RwLock<HashMap<String, CacheEntry<CachedMarket>>>>,
    default_ttl: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedMarket {
    pub id: String,
    pub question: String,
    pub description: Option<String>,
    pub end_date: Option<DateTime<Utc>>,
    pub volume: Decimal,
    pub liquidity: Decimal,
    pub yes_token_id: Option<String>,
    pub no_token_id: Option<String>,
    pub active: bool,
    pub closed: bool,
    pub fetched_at: DateTime<Utc>,
}

impl Default for MarketCache {
    fn default() -> Self {
        Self::new(300) // 5 minute TTL for market info
    }
}

impl MarketCache {
    pub fn new(default_ttl_secs: i64) -> Self {
        Self {
            markets: Arc::new(RwLock::new(HashMap::new())),
            default_ttl: default_ttl_secs,
        }
    }

    pub fn update(&self, market_id: &str, market: CachedMarket) {
        let mut cache = self.markets.write();
        cache.insert(market_id.to_string(), CacheEntry::new(market, self.default_ttl));
    }

    pub fn get(&self, market_id: &str) -> Option<CachedMarket> {
        let cache = self.markets.read();
        cache.get(market_id).and_then(|entry: &CacheEntry<CachedMarket>| {
            if entry.is_expired() {
                None
            } else {
                Some(entry.value.clone())
            }
        })
    }

    /// Get all cached markets (including expired - caller decides)
    pub fn get_all(&self, include_expired: bool) -> Vec<CachedMarket> {
        let cache = self.markets.read();
        cache
            .values()
            .filter(|entry| include_expired || !entry.is_expired())
            .map(|entry| entry.value.clone())
            .collect()
    }

    pub fn cleanup(&self) {
        let mut cache = self.markets.write();
        cache.retain(|_, entry: &mut CacheEntry<CachedMarket>| !entry.is_expired());
    }
}

/// Cache statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    pub total_entries: usize,
    pub expired_entries: usize,
    pub valid_entries: usize,
}

/// Combined cache manager
#[derive(Clone)]
pub struct CacheManager {
    pub prices: PriceCache,
    pub orderbooks: OrderBookCache,
    pub markets: MarketCache,
}

impl Default for CacheManager {
    fn default() -> Self {
        Self::new()
    }
}

impl CacheManager {
    pub fn new() -> Self {
        Self {
            prices: PriceCache::default(),
            orderbooks: OrderBookCache::default(),
            markets: MarketCache::default(),
        }
    }

    /// Create with custom TTLs
    pub fn with_ttls(price_ttl: i64, orderbook_ttl: i64, market_ttl: i64) -> Self {
        Self {
            prices: PriceCache::new(price_ttl),
            orderbooks: OrderBookCache::new(orderbook_ttl),
            markets: MarketCache::new(market_ttl),
        }
    }

    /// Cleanup all caches
    pub fn cleanup_all(&self) {
        self.prices.cleanup();
        self.orderbooks.cleanup();
        self.markets.cleanup();
    }

    /// Start background cleanup task
    pub fn start_cleanup_task(self: Arc<Self>, interval_secs: u64) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(interval_secs));
            loop {
                interval.tick().await;
                self.cleanup_all();
                tracing::debug!("Cache cleanup completed");
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_price_cache() {
        let cache = PriceCache::new(1); // 1 second TTL

        let data = PriceData {
            best_bid: Some(dec!(0.45)),
            best_ask: Some(dec!(0.55)),
            last_price: Some(dec!(0.50)),
            midpoint: Some(dec!(0.50)),
            timestamp: Utc::now(),
        };

        cache.update("token1", data.clone());
        
        // Should be available immediately
        let result = cache.get("token1");
        assert!(result.is_some());
        assert_eq!(result.unwrap().best_bid, Some(dec!(0.45)));
    }

    #[test]
    fn test_cache_miss() {
        let cache = PriceCache::new(1);
        
        // Non-existent key
        assert!(cache.get("nonexistent").is_none());
    }

    #[test]
    fn test_orderbook_cache() {
        let cache = OrderBookCache::new(5);

        let book = CachedOrderBook {
            token_id: "token1".to_string(),
            bids: vec![(dec!(0.45), dec!(100)), (dec!(0.44), dec!(200))],
            asks: vec![(dec!(0.55), dec!(100)), (dec!(0.56), dec!(150))],
            timestamp: Utc::now(),
        };

        cache.update("token1", book);
        
        let result = cache.get("token1");
        assert!(result.is_some());
        assert_eq!(result.unwrap().bids.len(), 2);
    }

    #[test]
    fn test_market_cache() {
        let cache = MarketCache::new(300);

        let market = CachedMarket {
            id: "market1".to_string(),
            question: "Will it rain?".to_string(),
            description: None,
            end_date: None,
            volume: dec!(10000),
            liquidity: dec!(5000),
            yes_token_id: Some("yes1".to_string()),
            no_token_id: Some("no1".to_string()),
            active: true,
            closed: false,
            fetched_at: Utc::now(),
        };

        cache.update("market1", market);
        
        let result = cache.get("market1");
        assert!(result.is_some());
        assert_eq!(result.unwrap().question, "Will it rain?");
    }

    #[test]
    fn test_cache_stats() {
        let cache = PriceCache::new(3600); // 1 hour TTL

        let data = PriceData {
            best_bid: None,
            best_ask: None,
            last_price: Some(dec!(0.50)),
            midpoint: None,
            timestamp: Utc::now(),
        };

        cache.update("token1", data.clone());
        cache.update("token2", data.clone());
        cache.update("token3", data);

        let stats = cache.stats();
        assert_eq!(stats.total_entries, 3);
        assert_eq!(stats.valid_entries, 3);
        assert_eq!(stats.expired_entries, 0);
    }

    #[test]
    fn test_cache_manager() {
        let manager = CacheManager::new();
        
        let price = PriceData {
            best_bid: Some(dec!(0.5)),
            best_ask: Some(dec!(0.6)),
            last_price: None,
            midpoint: Some(dec!(0.55)),
            timestamp: Utc::now(),
        };

        manager.prices.update("t1", price);
        
        assert!(manager.prices.get("t1").is_some());
    }
}
