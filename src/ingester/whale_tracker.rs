//! Whale wallet tracking for copy trading signals
//!
//! Monitors large traders' on-chain activity and positions.

use crate::error::Result;
use chrono::{DateTime, Utc};
use reqwest::Client;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info, warn};

/// Whale activity event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhaleActivity {
    /// Whale identifier (address or username)
    pub whale_id: String,
    /// Event type
    pub activity_type: ActivityType,
    /// Market/token involved
    pub market_id: String,
    pub token_id: String,
    /// Position details
    pub side: String,  // "YES" or "NO"
    pub size: Decimal,
    pub price: Option<Decimal>,
    /// Whale stats
    pub whale_pnl: Option<Decimal>,
    pub whale_win_rate: Option<f64>,
    /// When detected
    pub timestamp: DateTime<Utc>,
    /// Additional context
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityType {
    /// Opened new position
    Open,
    /// Increased existing position
    Increase,
    /// Reduced position
    Reduce,
    /// Closed position
    Close,
    /// Large single trade
    LargeTrade,
}

/// Tracked whale info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackedWhale {
    /// Wallet address (lowercase)
    pub address: String,
    /// Polymarket username (if known)
    pub username: Option<String>,
    /// Historical PnL
    pub total_pnl: Decimal,
    /// Win rate
    pub win_rate: f64,
    /// Number of trades
    pub trade_count: u32,
    /// Tracking weight (priority)
    pub weight: f64,
    /// Last known positions
    #[serde(skip)]
    pub positions: HashMap<String, WhalePosition>,
    /// Added at
    pub added_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct WhalePosition {
    pub market_id: String,
    pub token_id: String,
    pub size: Decimal,
    pub avg_price: Decimal,
    pub last_seen: DateTime<Utc>,
}

/// Whale tracker service
pub struct WhaleTracker {
    http: Client,
    /// Tracked whales
    whales: Arc<RwLock<HashMap<String, TrackedWhale>>>,
    /// Position change threshold (minimum $ change to trigger event)
    change_threshold: Decimal,
    /// Large trade threshold ($ size to trigger LargeTrade event)
    large_trade_threshold: Decimal,
    /// Minimum PnL to auto-follow from leaderboard
    min_pnl_for_auto: Decimal,
}

impl Default for WhaleTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl WhaleTracker {
    pub fn new() -> Self {
        Self {
            http: Client::new(),
            whales: Arc::new(RwLock::new(HashMap::new())),
            change_threshold: Decimal::new(100, 0),       // $100
            large_trade_threshold: Decimal::new(1000, 0), // $1000
            min_pnl_for_auto: Decimal::new(10000, 0),     // $10k
        }
    }

    pub fn with_thresholds(mut self, change: Decimal, large_trade: Decimal) -> Self {
        self.change_threshold = change;
        self.large_trade_threshold = large_trade;
        self
    }

    /// Add a whale to track by address
    pub async fn track_address(&self, address: &str, weight: f64) -> Result<()> {
        let addr = address.to_lowercase();
        
        // Try to get whale info
        let whale = self.fetch_whale_info(&addr).await.unwrap_or_else(|_| {
            TrackedWhale {
                address: addr.clone(),
                username: None,
                total_pnl: Decimal::ZERO,
                win_rate: 0.5,
                trade_count: 0,
                weight,
                positions: HashMap::new(),
                added_at: Utc::now(),
            }
        });

        let mut whales = self.whales.write().await;
        whales.insert(addr.clone(), whale);
        info!("Now tracking whale: {}", addr);
        
        Ok(())
    }

    /// Add a whale by username
    pub async fn track_username(&self, username: &str, weight: f64) -> Result<()> {
        // Resolve username to address
        let address = self.resolve_username(username).await?;
        
        if let Some(addr) = address {
            self.track_address(&addr, weight).await?;
            
            // Update username
            let mut whales = self.whales.write().await;
            if let Some(whale) = whales.get_mut(&addr.to_lowercase()) {
                whale.username = Some(username.to_string());
            }
        } else {
            warn!("Could not resolve username: {}", username);
        }
        
        Ok(())
    }

    /// Auto-populate from leaderboard
    pub async fn auto_track_top_traders(&self, count: usize) -> Result<usize> {
        let leaderboard = self.fetch_leaderboard(count).await?;
        let mut added = 0;

        for trader in leaderboard {
            if trader.total_pnl >= self.min_pnl_for_auto {
                if let Some(addr) = &trader.address {
                    // Weight based on PnL and win rate
                    let weight = (trader.win_rate * 0.5) + 
                        (trader.total_pnl.to_string().parse::<f64>().unwrap_or(0.0) / 100000.0).min(0.5);
                    
                    self.track_address(addr, weight).await?;
                    
                    // Update with username
                    let mut whales = self.whales.write().await;
                    if let Some(whale) = whales.get_mut(&addr.to_lowercase()) {
                        whale.username = Some(trader.username.clone());
                        whale.total_pnl = trader.total_pnl;
                        whale.win_rate = trader.win_rate;
                    }
                    
                    added += 1;
                }
            }
        }

        info!("Auto-tracked {} top traders", added);
        Ok(added)
    }

    /// Start monitoring loop
    pub async fn start(&self, tx: mpsc::Sender<WhaleActivity>) -> Result<()> {
        info!("Starting whale tracker");
        
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
        
        loop {
            interval.tick().await;
            
            let activities = self.check_all_whales().await;
            
            for activity in activities {
                if tx.send(activity).await.is_err() {
                    warn!("Whale activity channel closed");
                    return Ok(());
                }
            }
        }
    }

    /// Check all tracked whales for activity
    async fn check_all_whales(&self) -> Vec<WhaleActivity> {
        let mut activities = Vec::new();
        
        let whale_list: Vec<TrackedWhale> = {
            let whales = self.whales.read().await;
            whales.values().cloned().collect()
        };

        for whale in whale_list {
            match self.check_whale(&whale).await {
                Ok(mut whale_activities) => {
                    activities.append(&mut whale_activities);
                }
                Err(e) => {
                    debug!("Failed to check whale {}: {}", whale.address, e);
                }
            }
        }

        activities
    }

    /// Check single whale for position changes
    async fn check_whale(&self, whale: &TrackedWhale) -> Result<Vec<WhaleActivity>> {
        let mut activities = Vec::new();
        
        // Fetch current positions
        let current_positions = self.fetch_positions(&whale.address).await?;
        
        // Get stored positions
        let old_positions = {
            let whales = self.whales.read().await;
            whales.get(&whale.address)
                .map(|w| w.positions.clone())
                .unwrap_or_default()
        };

        let current_markets: HashSet<_> = current_positions.keys().cloned().collect();
        let old_markets: HashSet<_> = old_positions.keys().cloned().collect();

        // New positions
        for market_id in current_markets.difference(&old_markets) {
            if let Some(pos) = current_positions.get(market_id) {
                if pos.size.abs() >= self.change_threshold {
                    activities.push(WhaleActivity {
                        whale_id: whale.username.clone().unwrap_or_else(|| whale.address.clone()),
                        activity_type: ActivityType::Open,
                        market_id: market_id.clone(),
                        token_id: pos.token_id.clone(),
                        side: if pos.size > Decimal::ZERO { "YES".to_string() } else { "NO".to_string() },
                        size: pos.size.abs(),
                        price: Some(pos.avg_price),
                        whale_pnl: Some(whale.total_pnl),
                        whale_win_rate: Some(whale.win_rate),
                        timestamp: Utc::now(),
                        metadata: None,
                    });
                    
                    info!(
                        "🐋 Whale {} opened {} position in {} (${:.0})",
                        whale.username.as_ref().unwrap_or(&whale.address),
                        if pos.size > Decimal::ZERO { "YES" } else { "NO" },
                        market_id,
                        pos.size.abs()
                    );
                }
            }
        }

        // Closed positions
        for market_id in old_markets.difference(&current_markets) {
            if let Some(old_pos) = old_positions.get(market_id) {
                activities.push(WhaleActivity {
                    whale_id: whale.username.clone().unwrap_or_else(|| whale.address.clone()),
                    activity_type: ActivityType::Close,
                    market_id: market_id.clone(),
                    token_id: old_pos.token_id.clone(),
                    side: if old_pos.size > Decimal::ZERO { "YES".to_string() } else { "NO".to_string() },
                    size: old_pos.size.abs(),
                    price: None,
                    whale_pnl: Some(whale.total_pnl),
                    whale_win_rate: Some(whale.win_rate),
                    timestamp: Utc::now(),
                    metadata: None,
                });
                
                info!(
                    "🐋 Whale {} closed position in {}",
                    whale.username.as_ref().unwrap_or(&whale.address),
                    market_id
                );
            }
        }

        // Changed positions
        for market_id in current_markets.intersection(&old_markets) {
            if let (Some(old), Some(new)) = (old_positions.get(market_id), current_positions.get(market_id)) {
                let change = new.size - old.size;
                
                if change.abs() >= self.change_threshold {
                    let activity_type = if change > Decimal::ZERO {
                        ActivityType::Increase
                    } else {
                        ActivityType::Reduce
                    };
                    
                    activities.push(WhaleActivity {
                        whale_id: whale.username.clone().unwrap_or_else(|| whale.address.clone()),
                        activity_type,
                        market_id: market_id.clone(),
                        token_id: new.token_id.clone(),
                        side: if new.size > Decimal::ZERO { "YES".to_string() } else { "NO".to_string() },
                        size: change.abs(),
                        price: Some(new.avg_price),
                        whale_pnl: Some(whale.total_pnl),
                        whale_win_rate: Some(whale.win_rate),
                        timestamp: Utc::now(),
                        metadata: None,
                    });

                    info!(
                        "🐋 Whale {} {} position in {} by ${:.0}",
                        whale.username.as_ref().unwrap_or(&whale.address),
                        if change > Decimal::ZERO { "increased" } else { "reduced" },
                        market_id,
                        change.abs()
                    );
                }
            }
        }

        // Update stored positions
        {
            let mut whales = self.whales.write().await;
            if let Some(w) = whales.get_mut(&whale.address) {
                w.positions = current_positions;
            }
        }

        Ok(activities)
    }

    /// Fetch positions for an address
    async fn fetch_positions(&self, address: &str) -> Result<HashMap<String, WhalePosition>> {
        let url = format!("https://clob.polymarket.com/positions?user={}", address);
        
        let resp = self.http.get(&url).send().await?;
        
        if !resp.status().is_success() {
            return Ok(HashMap::new());
        }

        let data: serde_json::Value = resp.json().await?;
        let mut positions = HashMap::new();

        if let Some(arr) = data.as_array() {
            for item in arr {
                let size: Decimal = item.get("size")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(Decimal::ZERO);

                if size.abs() > Decimal::ZERO {
                    let market_id = item.get("market")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    
                    positions.insert(market_id.clone(), WhalePosition {
                        market_id,
                        token_id: item.get("asset")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        size,
                        avg_price: item.get("avgCost")
                            .and_then(|v| v.as_str())
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(Decimal::ZERO),
                        last_seen: Utc::now(),
                    });
                }
            }
        }

        Ok(positions)
    }

    /// Fetch whale info from API
    async fn fetch_whale_info(&self, address: &str) -> Result<TrackedWhale> {
        let url = format!("https://gamma-api.polymarket.com/users/{}", address);
        let resp = self.http.get(&url).send().await?;
        
        if !resp.status().is_success() {
            return Err(crate::error::BotError::Api("User not found".into()).into());
        }

        let data: serde_json::Value = resp.json().await?;
        
        Ok(TrackedWhale {
            address: address.to_string(),
            username: data.get("username").and_then(|v| v.as_str()).map(String::from),
            total_pnl: data.get("pnl")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse().ok())
                .unwrap_or(Decimal::ZERO),
            win_rate: data.get("winRate")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.5),
            trade_count: data.get("tradeCount")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
            weight: 1.0,
            positions: HashMap::new(),
            added_at: Utc::now(),
        })
    }

    /// Resolve username to address
    async fn resolve_username(&self, username: &str) -> Result<Option<String>> {
        let url = format!("https://gamma-api.polymarket.com/users?username={}", username);
        let resp = self.http.get(&url).send().await?;
        
        if !resp.status().is_success() {
            return Ok(None);
        }

        let data: serde_json::Value = resp.json().await?;
        
        Ok(data.get("proxyWallet")
            .or_else(|| data.get("address"))
            .and_then(|v| v.as_str())
            .map(String::from))
    }

    /// Fetch leaderboard
    async fn fetch_leaderboard(&self, limit: usize) -> Result<Vec<LeaderboardEntry>> {
        let url = format!("https://gamma-api.polymarket.com/leaderboard?limit={}", limit);
        let resp = self.http.get(&url).send().await?;
        
        if !resp.status().is_success() {
            return Ok(Vec::new());
        }

        let data: serde_json::Value = resp.json().await?;
        let mut entries = Vec::new();

        if let Some(arr) = data.as_array() {
            for item in arr {
                entries.push(LeaderboardEntry {
                    username: item.get("username")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    address: item.get("address")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    total_pnl: item.get("pnl")
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(Decimal::ZERO),
                    win_rate: item.get("winRate")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.5),
                });
            }
        }

        Ok(entries)
    }

    /// Get list of tracked whales
    pub async fn list_whales(&self) -> Vec<TrackedWhale> {
        let whales = self.whales.read().await;
        whales.values().cloned().collect()
    }

    /// Remove a whale from tracking
    pub async fn untrack(&self, address: &str) {
        let mut whales = self.whales.write().await;
        whales.remove(&address.to_lowercase());
    }
}

#[derive(Debug, Clone)]
struct LeaderboardEntry {
    username: String,
    address: Option<String>,
    total_pnl: Decimal,
    win_rate: f64,
}

/// Configuration for whale tracking
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WhaleTrackerConfig {
    /// Enable whale tracking
    #[serde(default)]
    pub enabled: bool,
    /// Addresses to track
    #[serde(default)]
    pub track_addresses: Vec<String>,
    /// Usernames to track
    #[serde(default)]
    pub track_usernames: Vec<String>,
    /// Auto-track top N from leaderboard
    #[serde(default)]
    pub auto_track_top: Option<usize>,
    /// Minimum position change to report ($)
    #[serde(default = "default_change_threshold")]
    pub change_threshold: f64,
    /// Large trade threshold ($)
    #[serde(default = "default_large_trade")]
    pub large_trade_threshold: f64,
}

fn default_change_threshold() -> f64 {
    100.0
}

fn default_large_trade() -> f64 {
    1000.0
}

impl Default for WhaleTrackerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            track_addresses: Vec::new(),
            track_usernames: Vec::new(),
            auto_track_top: None,
            change_threshold: 100.0,
            large_trade_threshold: 1000.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_activity_type_serialize() {
        let activity = WhaleActivity {
            whale_id: "test_whale".to_string(),
            activity_type: ActivityType::Open,
            market_id: "market1".to_string(),
            token_id: "token1".to_string(),
            side: "YES".to_string(),
            size: dec!(1000),
            price: Some(dec!(0.55)),
            whale_pnl: Some(dec!(50000)),
            whale_win_rate: Some(0.65),
            timestamp: Utc::now(),
            metadata: None,
        };

        let json = serde_json::to_string(&activity).unwrap();
        assert!(json.contains("open"));
        assert!(json.contains("test_whale"));
    }

    #[test]
    fn test_tracker_config() {
        let config = WhaleTrackerConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.change_threshold, 100.0);
    }

    #[tokio::test]
    async fn test_tracker_list_empty() {
        let tracker = WhaleTracker::new();
        let whales = tracker.list_whales().await;
        assert!(whales.is_empty());
    }
}
