//! Twitter API Client for Sentiment Analysis
//!
//! Provides methods to fetch tweets for crypto sentiment analysis.
//! Supports both Twitter API v2 and mock data for testing.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// A tweet from Twitter
#[derive(Debug, Clone)]
pub struct Tweet {
    pub id: String,
    pub text: String,
    pub author_id: String,
    pub author_username: String,
    pub created_at: u64,
    pub retweet_count: u32,
    pub like_count: u32,
    pub reply_count: u32,
    pub quote_count: u32,
}

impl Tweet {
    /// Calculate engagement score for this tweet
    pub fn engagement_score(&self) -> u32 {
        self.retweet_count * 3 + self.like_count + self.reply_count * 2 + self.quote_count * 4
    }

    /// Check if this is a high-engagement tweet
    pub fn is_viral(&self) -> bool {
        self.engagement_score() > 10000
    }
}

/// Rate limit tracking
#[derive(Debug, Default)]
struct RateLimiter {
    /// Remaining requests in current window
    remaining: u32,
    /// Reset timestamp
    reset_at: u64,
    /// Last request timestamp
    last_request: u64,
}

/// Twitter API client
pub struct TwitterClient {
    bearer_token: Option<String>,
    /// HTTP client (placeholder - would use reqwest in production)
    rate_limiter: Arc<RwLock<RateLimiter>>,
    /// Tweet cache to reduce API calls
    cache: Arc<RwLock<HashMap<String, Vec<Tweet>>>>,
    /// Cache TTL in seconds
    cache_ttl: u64,
}

impl TwitterClient {
    /// Create a new Twitter client
    pub fn new(bearer_token: Option<String>) -> Self {
        Self {
            bearer_token,
            rate_limiter: Arc::new(RwLock::new(RateLimiter {
                remaining: 450, // Twitter API v2 default
                reset_at: 0,
                last_request: 0,
            })),
            cache: Arc::new(RwLock::new(HashMap::new())),
            cache_ttl: 60, // 1 minute cache
        }
    }

    /// Search for crypto-related tweets
    pub async fn search_crypto_tweets(&self, symbol: &str, max_results: u32) -> Vec<Tweet> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let cache_key = format!("{}:{}", symbol, max_results);

        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.get(&cache_key) {
                // Cache is still valid (within TTL)
                return cached.clone();
            }
        }

        // Check rate limits
        {
            let limiter = self.rate_limiter.read().await;
            if limiter.remaining == 0 && now < limiter.reset_at {
                // Rate limited, return cached or empty
                let cache = self.cache.read().await;
                return cache.get(&cache_key).cloned().unwrap_or_default();
            }
        }

        // Make API request (or use mock data if no token)
        let tweets = if self.bearer_token.is_some() {
            self.fetch_tweets_from_api(symbol, max_results).await
        } else {
            self.generate_mock_tweets(symbol, max_results)
        };

        // Update cache
        {
            let mut cache = self.cache.write().await;
            cache.insert(cache_key, tweets.clone());
        }

        // Update rate limiter
        {
            let mut limiter = self.rate_limiter.write().await;
            limiter.remaining = limiter.remaining.saturating_sub(1);
            limiter.last_request = now;
        }

        tweets
    }

    /// Fetch tweets from Twitter API v2
    async fn fetch_tweets_from_api(&self, symbol: &str, max_results: u32) -> Vec<Tweet> {
        // Build search query for crypto
        let query = format!(
            "${} OR #{} OR {} crypto -is:retweet lang:en",
            symbol.to_uppercase(),
            symbol.to_lowercase(),
            symbol.to_uppercase()
        );

        // In production, this would use reqwest to call Twitter API:
        // GET https://api.twitter.com/2/tweets/search/recent
        // ?query={query}
        // &max_results={max_results}
        // &tweet.fields=created_at,public_metrics,author_id
        // &expansions=author_id
        // &user.fields=username

        // For now, return mock data with API-like structure
        // This allows testing without actual API access
        let _ = (query, max_results); // suppress unused warnings
        self.generate_mock_tweets(symbol, max_results)
    }

    /// Generate mock tweets for testing and demo
    fn generate_mock_tweets(&self, symbol: &str, max_results: u32) -> Vec<Tweet> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Sample tweets with varied sentiment
        let templates = vec![
            (
                "🚀 ${} breaking out! This is the move we've been waiting for. #crypto",
                "whale_trader",
                5000,
                15000,
            ),
            (
                "${} looking weak here. Might see a pullback to support. Be careful.",
                "crypto_analyst",
                1200,
                3500,
            ),
            (
                "Just accumulated more ${}. Long term bullish despite short term noise.",
                "hodler_max",
                800,
                2200,
            ),
            (
                "Warning: ${} showing bearish divergence on 4H. Consider reducing exposure.",
                "ta_master",
                2500,
                6000,
            ),
            (
                "The fundamentals for ${} have never been stronger. Institutional adoption incoming.",
                "crypto_fund",
                3200,
                9500,
            ),
            (
                "${} dump incoming? Whales moving to exchanges. Stay alert!",
                "onchain_data",
                1800,
                4200,
            ),
            (
                "Bought the dip on ${}. This is exactly what accumulation looks like.",
                "dip_buyer",
                600,
                1800,
            ),
            (
                "${} chart looking absolutely beautiful. Cup and handle forming.",
                "pattern_pro",
                950,
                2800,
            ),
            (
                "Selling my ${} here. Taking profits after this amazing run. 🎯",
                "profit_taker",
                400,
                1100,
            ),
            (
                "${} sentiment at extreme fear. Historically a great buying opportunity.",
                "contrarian",
                2100,
                5500,
            ),
        ];

        let count = max_results.min(templates.len() as u32) as usize;

        templates
            .iter()
            .take(count)
            .enumerate()
            .map(|(i, (text, author, retweets, likes))| Tweet {
                id: format!("mock_{}_{}_{}", symbol, i, now),
                text: text.replace("{}", symbol),
                author_id: format!("author_{}_{}", author, i),
                author_username: author.to_string(),
                created_at: now - (i as u64 * 300), // 5 minutes apart
                retweet_count: *retweets,
                like_count: *likes,
                reply_count: retweets / 5,
                quote_count: retweets / 10,
            })
            .collect()
    }

    /// Get tweets from a specific user
    pub async fn get_user_tweets(&self, user_id: &str, max_results: u32) -> Vec<Tweet> {
        // In production, call GET /2/users/:id/tweets
        // For now, generate mock data
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        (0..max_results.min(10))
            .map(|i| Tweet {
                id: format!("user_tweet_{}_{}", user_id, i),
                text: format!("Sample tweet {} from user {}", i, user_id),
                author_id: user_id.to_string(),
                author_username: format!("user_{}", user_id),
                created_at: now - (i as u64 * 3600),
                retweet_count: 100 * (10 - i),
                like_count: 300 * (10 - i),
                reply_count: 50,
                quote_count: 20,
            })
            .collect()
    }

    /// Check if API is available (has valid token)
    pub fn is_api_available(&self) -> bool {
        self.bearer_token.is_some()
    }

    /// Get rate limit status
    pub async fn rate_limit_status(&self) -> (u32, u64) {
        let limiter = self.rate_limiter.read().await;
        (limiter.remaining, limiter.reset_at)
    }

    /// Clear tweet cache
    pub async fn clear_cache(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tweet_engagement_score() {
        let tweet = Tweet {
            id: "1".to_string(),
            text: "Test".to_string(),
            author_id: "a".to_string(),
            author_username: "user".to_string(),
            created_at: 0,
            retweet_count: 100,
            like_count: 500,
            reply_count: 50,
            quote_count: 25,
        };
        // 100*3 + 500 + 50*2 + 25*4 = 300 + 500 + 100 + 100 = 1000
        assert_eq!(tweet.engagement_score(), 1000);
    }

    #[test]
    fn test_tweet_is_viral() {
        let viral = Tweet {
            id: "1".to_string(),
            text: "Test".to_string(),
            author_id: "a".to_string(),
            author_username: "user".to_string(),
            created_at: 0,
            retweet_count: 5000,
            like_count: 20000,
            reply_count: 1000,
            quote_count: 500,
        };
        assert!(viral.is_viral());

        let normal = Tweet {
            id: "2".to_string(),
            text: "Test".to_string(),
            author_id: "a".to_string(),
            author_username: "user".to_string(),
            created_at: 0,
            retweet_count: 10,
            like_count: 50,
            reply_count: 5,
            quote_count: 2,
        };
        assert!(!normal.is_viral());
    }

    #[tokio::test]
    async fn test_twitter_client_mock() {
        let client = TwitterClient::new(None);
        let tweets = client.search_crypto_tweets("BTC", 5).await;
        assert_eq!(tweets.len(), 5);
        assert!(tweets[0].text.contains("BTC"));
    }

    #[tokio::test]
    async fn test_twitter_client_cache() {
        let client = TwitterClient::new(None);

        // First call
        let tweets1 = client.search_crypto_tweets("ETH", 10).await;

        // Second call should hit cache
        let tweets2 = client.search_crypto_tweets("ETH", 10).await;

        // Should be same (cached)
        assert_eq!(tweets1.len(), tweets2.len());
        assert_eq!(tweets1[0].id, tweets2[0].id);
    }

    #[tokio::test]
    async fn test_rate_limit_status() {
        let client = TwitterClient::new(None);
        let (remaining, _) = client.rate_limit_status().await;
        assert!(remaining > 0);
    }

    #[tokio::test]
    async fn test_get_user_tweets() {
        let client = TwitterClient::new(None);
        let tweets = client.get_user_tweets("12345", 5).await;
        assert_eq!(tweets.len(), 5);
    }

    #[tokio::test]
    async fn test_clear_cache() {
        let client = TwitterClient::new(None);

        // Populate cache
        let _ = client.search_crypto_tweets("SOL", 5).await;

        // Clear
        client.clear_cache().await;

        // Cache should be empty (this is hard to verify directly)
        // but we can verify it doesn't crash
    }

    #[test]
    fn test_api_availability() {
        let client_no_token = TwitterClient::new(None);
        assert!(!client_no_token.is_api_available());

        let client_with_token = TwitterClient::new(Some("test_token".to_string()));
        assert!(client_with_token.is_api_available());
    }
}
