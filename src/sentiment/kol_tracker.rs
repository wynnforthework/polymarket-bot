//! KOL (Key Opinion Leader) Tracker
//!
//! Tracks influential crypto personalities and weights their sentiment
//! based on historical accuracy and influence metrics.

use std::collections::HashMap;

/// Influence weight categories
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InfluenceWeight {
    /// Elite tier: proven track record, massive following (e.g., Elon, CZ)
    Elite,
    /// Whale tier: large traders with market-moving capability
    Whale,
    /// Analyst tier: respected analysts with good track record
    Analyst,
    /// Influencer tier: popular accounts with large following
    Influencer,
    /// Standard tier: regular accounts
    Standard,
}

impl InfluenceWeight {
    /// Get numeric weight for calculations
    pub fn weight(&self) -> f64 {
        match self {
            InfluenceWeight::Elite => 5.0,
            InfluenceWeight::Whale => 3.0,
            InfluenceWeight::Analyst => 2.5,
            InfluenceWeight::Influencer => 1.5,
            InfluenceWeight::Standard => 1.0,
        }
    }
}

/// Profile for a tracked KOL
#[derive(Debug, Clone)]
pub struct KolProfile {
    /// Twitter user ID
    pub user_id: String,
    /// Twitter username
    pub username: String,
    /// Display name
    pub display_name: String,
    /// Influence tier
    pub tier: InfluenceWeight,
    /// Follower count
    pub followers: u64,
    /// Historical prediction accuracy (0.0 to 1.0)
    pub accuracy: f64,
    /// Average engagement per tweet
    pub avg_engagement: u32,
    /// Focused assets (e.g., ["BTC", "ETH"])
    pub focus_assets: Vec<String>,
    /// Notes about this KOL
    pub notes: String,
}

impl KolProfile {
    /// Create a new KOL profile
    pub fn new(
        user_id: impl Into<String>,
        username: impl Into<String>,
        tier: InfluenceWeight,
    ) -> Self {
        Self {
            user_id: user_id.into(),
            username: username.into(),
            display_name: String::new(),
            tier,
            followers: 0,
            accuracy: 0.5, // Default 50% accuracy
            avg_engagement: 0,
            focus_assets: Vec::new(),
            notes: String::new(),
        }
    }

    /// Calculate influence weight considering all factors
    pub fn influence_weight(&self) -> f64 {
        let base = self.tier.weight();
        let accuracy_factor = 0.5 + self.accuracy; // 0.5 to 1.5
        let follower_factor = (self.followers as f64).ln().max(1.0) / 20.0; // Log scale

        base * accuracy_factor * (1.0 + follower_factor * 0.1)
    }

    /// Check if this KOL focuses on a specific asset
    pub fn focuses_on(&self, asset: &str) -> bool {
        self.focus_assets.is_empty()
            || self
                .focus_assets
                .iter()
                .any(|a| a.eq_ignore_ascii_case(asset))
    }

    /// Builder: set display name
    pub fn with_display_name(mut self, name: impl Into<String>) -> Self {
        self.display_name = name.into();
        self
    }

    /// Builder: set followers
    pub fn with_followers(mut self, count: u64) -> Self {
        self.followers = count;
        self
    }

    /// Builder: set accuracy
    pub fn with_accuracy(mut self, accuracy: f64) -> Self {
        self.accuracy = accuracy.clamp(0.0, 1.0);
        self
    }

    /// Builder: set focus assets
    pub fn with_focus(mut self, assets: Vec<String>) -> Self {
        self.focus_assets = assets;
        self
    }

    /// Builder: set notes
    pub fn with_notes(mut self, notes: impl Into<String>) -> Self {
        self.notes = notes.into();
        self
    }
}

/// KOL tracker and manager
pub struct KolTracker {
    /// KOLs indexed by user ID
    kols: HashMap<String, KolProfile>,
    /// Username to user ID mapping
    username_index: HashMap<String, String>,
    /// Prediction tracking for accuracy updates
    predictions: HashMap<String, Vec<(bool, u64)>>,
}

impl KolTracker {
    /// Create a new KOL tracker
    pub fn new() -> Self {
        let mut tracker = Self {
            kols: HashMap::new(),
            username_index: HashMap::new(),
            predictions: HashMap::new(),
        };
        tracker.init_default_kols();
        tracker
    }

    /// Initialize with well-known crypto KOLs
    fn init_default_kols(&mut self) {
        // Elite tier
        self.add_kol(
            KolProfile::new("44196397", "elonmusk", InfluenceWeight::Elite)
                .with_display_name("Elon Musk")
                .with_followers(170_000_000)
                .with_accuracy(0.55)
                .with_focus(vec!["BTC".to_string(), "DOGE".to_string()])
                .with_notes("CEO Tesla/SpaceX, major market mover"),
        );

        self.add_kol(
            KolProfile::new("902926941413453824", "caborek", InfluenceWeight::Elite)
                .with_display_name("CZ Binance")
                .with_followers(9_000_000)
                .with_accuracy(0.60)
                .with_notes("Former Binance CEO"),
        );

        self.add_kol(
            KolProfile::new("357312062", "VitalikButerin", InfluenceWeight::Elite)
                .with_display_name("Vitalik Buterin")
                .with_followers(5_000_000)
                .with_accuracy(0.65)
                .with_focus(vec!["ETH".to_string()])
                .with_notes("Ethereum co-founder"),
        );

        // Whale tier
        self.add_kol(
            KolProfile::new("1400000000001", "whale_alert", InfluenceWeight::Whale)
                .with_display_name("Whale Alert")
                .with_followers(2_500_000)
                .with_accuracy(0.70)
                .with_notes("On-chain whale movement tracker"),
        );

        // Analyst tier
        self.add_kol(
            KolProfile::new("1400000000002", "PlanB", InfluenceWeight::Analyst)
                .with_display_name("PlanB")
                .with_followers(1_900_000)
                .with_accuracy(0.55)
                .with_focus(vec!["BTC".to_string()])
                .with_notes("Stock-to-flow model creator"),
        );

        self.add_kol(
            KolProfile::new("1400000000003", "WClementeIII", InfluenceWeight::Analyst)
                .with_display_name("Will Clemente")
                .with_followers(700_000)
                .with_accuracy(0.60)
                .with_focus(vec!["BTC".to_string()])
                .with_notes("On-chain analyst"),
        );

        // Influencer tier
        self.add_kol(
            KolProfile::new("1400000000004", "APompliano", InfluenceWeight::Influencer)
                .with_display_name("Anthony Pompliano")
                .with_followers(1_700_000)
                .with_accuracy(0.50)
                .with_notes("Pomp Investments"),
        );

        self.add_kol(
            KolProfile::new("1400000000005", "CryptoCobain", InfluenceWeight::Influencer)
                .with_display_name("Crypto Cobain")
                .with_followers(500_000)
                .with_accuracy(0.55)
                .with_notes("Popular crypto trader"),
        );
    }

    /// Add a KOL to tracker
    pub fn add_kol(&mut self, profile: KolProfile) {
        self.username_index
            .insert(profile.username.to_lowercase(), profile.user_id.clone());
        self.kols.insert(profile.user_id.clone(), profile);
    }

    /// Get a KOL by user ID
    pub fn get_kol(&self, user_id: &str) -> Option<&KolProfile> {
        self.kols.get(user_id)
    }

    /// Get a KOL by username
    pub fn get_kol_by_username(&self, username: &str) -> Option<&KolProfile> {
        self.username_index
            .get(&username.to_lowercase())
            .and_then(|id| self.kols.get(id))
    }

    /// Check if a user ID is a tracked KOL
    pub fn is_kol(&self, user_id: &str) -> bool {
        self.kols.contains_key(user_id)
    }

    /// Get all KOLs
    pub fn all_kols(&self) -> impl Iterator<Item = &KolProfile> {
        self.kols.values()
    }

    /// Get KOLs by tier
    pub fn kols_by_tier(&self, tier: InfluenceWeight) -> Vec<&KolProfile> {
        self.kols.values().filter(|k| k.tier == tier).collect()
    }

    /// Get KOLs focused on a specific asset
    pub fn kols_for_asset(&self, asset: &str) -> Vec<&KolProfile> {
        self.kols.values().filter(|k| k.focuses_on(asset)).collect()
    }

    /// Record a prediction outcome for accuracy tracking
    pub fn record_prediction(&mut self, user_id: &str, correct: bool) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        self.predictions
            .entry(user_id.to_string())
            .or_default()
            .push((correct, now));

        // Update accuracy if we have enough data
        self.update_accuracy(user_id);
    }

    /// Update KOL accuracy based on recorded predictions
    fn update_accuracy(&mut self, user_id: &str) {
        let predictions = match self.predictions.get(user_id) {
            Some(p) if p.len() >= 10 => p,
            _ => return,
        };

        // Use last 100 predictions with time decay
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let recent: Vec<_> = predictions.iter().rev().take(100).collect();

        let mut weighted_correct = 0.0;
        let mut total_weight = 0.0;

        for (correct, timestamp) in recent {
            // Time decay: older predictions count less
            let age_days = (now - timestamp) / 86400;
            let weight = 1.0 / (1.0 + age_days as f64 * 0.1);

            if *correct {
                weighted_correct += weight;
            }
            total_weight += weight;
        }

        if total_weight > 0.0 {
            if let Some(kol) = self.kols.get_mut(user_id) {
                kol.accuracy = weighted_correct / total_weight;
            }
        }
    }

    /// Get total number of tracked KOLs
    pub fn count(&self) -> usize {
        self.kols.len()
    }

    /// Remove a KOL from tracking
    pub fn remove_kol(&mut self, user_id: &str) -> Option<KolProfile> {
        if let Some(profile) = self.kols.remove(user_id) {
            self.username_index.remove(&profile.username.to_lowercase());
            self.predictions.remove(user_id);
            Some(profile)
        } else {
            None
        }
    }
}

impl Default for KolTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_influence_weight_values() {
        assert_eq!(InfluenceWeight::Elite.weight(), 5.0);
        assert_eq!(InfluenceWeight::Whale.weight(), 3.0);
        assert_eq!(InfluenceWeight::Analyst.weight(), 2.5);
        assert_eq!(InfluenceWeight::Influencer.weight(), 1.5);
        assert_eq!(InfluenceWeight::Standard.weight(), 1.0);
    }

    #[test]
    fn test_kol_profile_creation() {
        let profile = KolProfile::new("123", "testuser", InfluenceWeight::Analyst)
            .with_display_name("Test User")
            .with_followers(100_000)
            .with_accuracy(0.7)
            .with_focus(vec!["BTC".to_string(), "ETH".to_string()]);

        assert_eq!(profile.user_id, "123");
        assert_eq!(profile.username, "testuser");
        assert_eq!(profile.display_name, "Test User");
        assert_eq!(profile.followers, 100_000);
        assert!((profile.accuracy - 0.7).abs() < 0.001);
    }

    #[test]
    fn test_kol_influence_weight() {
        let low_influence = KolProfile::new("1", "low", InfluenceWeight::Standard)
            .with_followers(1000)
            .with_accuracy(0.5);

        let high_influence = KolProfile::new("2", "high", InfluenceWeight::Elite)
            .with_followers(10_000_000)
            .with_accuracy(0.8);

        assert!(high_influence.influence_weight() > low_influence.influence_weight());
    }

    #[test]
    fn test_focuses_on() {
        let btc_focused =
            KolProfile::new("1", "btc_guy", InfluenceWeight::Analyst).with_focus(vec![
                "BTC".to_string(),
                "Lightning".to_string(),
            ]);

        let general = KolProfile::new("2", "general", InfluenceWeight::Analyst);

        assert!(btc_focused.focuses_on("BTC"));
        assert!(btc_focused.focuses_on("btc")); // Case insensitive
        assert!(!btc_focused.focuses_on("ETH"));

        // General KOL focuses on everything
        assert!(general.focuses_on("BTC"));
        assert!(general.focuses_on("ETH"));
    }

    #[test]
    fn test_kol_tracker_creation() {
        let tracker = KolTracker::new();
        assert!(tracker.count() > 0); // Has default KOLs
    }

    #[test]
    fn test_add_and_get_kol() {
        let mut tracker = KolTracker::new();
        let initial_count = tracker.count();

        let profile = KolProfile::new("999", "newkol", InfluenceWeight::Influencer);
        tracker.add_kol(profile);

        assert_eq!(tracker.count(), initial_count + 1);
        assert!(tracker.get_kol("999").is_some());
        assert!(tracker.get_kol_by_username("newkol").is_some());
    }

    #[test]
    fn test_is_kol() {
        let tracker = KolTracker::new();
        assert!(tracker.is_kol("44196397")); // Elon
        assert!(!tracker.is_kol("nonexistent"));
    }

    #[test]
    fn test_kols_by_tier() {
        let tracker = KolTracker::new();
        let elite = tracker.kols_by_tier(InfluenceWeight::Elite);
        assert!(!elite.is_empty());

        for kol in elite {
            assert_eq!(kol.tier, InfluenceWeight::Elite);
        }
    }

    #[test]
    fn test_kols_for_asset() {
        let tracker = KolTracker::new();
        let btc_kols = tracker.kols_for_asset("BTC");

        // Should include Elon (focuses on BTC/DOGE) and general KOLs
        assert!(!btc_kols.is_empty());
    }

    #[test]
    fn test_remove_kol() {
        let mut tracker = KolTracker::new();
        let profile = KolProfile::new("to_remove", "removeme", InfluenceWeight::Standard);
        tracker.add_kol(profile);

        assert!(tracker.is_kol("to_remove"));

        let removed = tracker.remove_kol("to_remove");
        assert!(removed.is_some());
        assert!(!tracker.is_kol("to_remove"));
    }

    #[test]
    fn test_record_prediction() {
        let mut tracker = KolTracker::new();
        let profile =
            KolProfile::new("pred_test", "preduser", InfluenceWeight::Analyst).with_accuracy(0.5);

        tracker.add_kol(profile);

        // Record predictions
        for _ in 0..15 {
            tracker.record_prediction("pred_test", true);
        }

        // After enough correct predictions, accuracy should increase
        let kol = tracker.get_kol("pred_test").unwrap();
        assert!(kol.accuracy > 0.5);
    }

    #[test]
    fn test_default_kols_exist() {
        let tracker = KolTracker::new();

        // Check some known KOLs are loaded
        assert!(tracker.get_kol_by_username("elonmusk").is_some());
        assert!(tracker.get_kol_by_username("VitalikButerin").is_some());
    }

    #[test]
    fn test_accuracy_clamp() {
        let profile = KolProfile::new("1", "test", InfluenceWeight::Standard)
            .with_accuracy(1.5) // Over 1.0
            .with_accuracy(-0.5); // Under 0.0

        assert!(profile.accuracy >= 0.0);
        assert!(profile.accuracy <= 1.0);
    }

    #[test]
    fn test_kol_tracker_default() {
        let tracker = KolTracker::default();
        assert!(tracker.count() > 0);
    }
}
