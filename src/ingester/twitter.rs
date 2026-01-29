//! Twitter/X signal collection
//!
//! Monitors KOL accounts for trading signals.
//! Supports both API v2 and RSS fallback.

use super::{RawSignal, SignalSource, TwitterIngesterConfig};
use crate::error::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;
use tokio::sync::mpsc;

/// Twitter API v2 source
pub struct TwitterSource {
    config: TwitterIngesterConfig,
    http: reqwest::Client,
    author_trust: std::collections::HashMap<String, f64>,
}

impl TwitterSource {
    pub fn new(
        config: TwitterIngesterConfig,
        author_trust: std::collections::HashMap<String, f64>,
    ) -> Self {
        Self {
            config,
            http: reqwest::Client::new(),
            author_trust,
        }
    }

    fn get_trust(&self, author: &str) -> f64 {
        self.author_trust.get(author).copied().unwrap_or(0.3)
    }

    async fn fetch_user_tweets(&self, user_id: &str) -> Result<Vec<Tweet>> {
        let bearer = self.config.bearer_token.as_ref()
            .ok_or_else(|| crate::error::BotError::Config("Twitter bearer token required".into()))?;

        let url = format!(
            "https://api.twitter.com/2/users/{}/tweets?max_results=10&tweet.fields=created_at,author_id",
            user_id
        );

        let resp = self.http
            .get(&url)
            .header("Authorization", format!("Bearer {}", bearer))
            .send()
            .await?;

        let data: TwitterResponse = resp.json().await?;
        Ok(data.data.unwrap_or_default())
    }
}

#[derive(Debug, Deserialize)]
struct TwitterResponse {
    data: Option<Vec<Tweet>>,
}

#[derive(Debug, Deserialize)]
struct Tweet {
    id: String,
    text: String,
    author_id: Option<String>,
    created_at: Option<String>,
}

#[async_trait]
impl SignalSource for TwitterSource {
    fn name(&self) -> &str {
        "twitter"
    }

    async fn run(&self, tx: mpsc::Sender<RawSignal>) -> Result<()> {
        tracing::info!(
            "Twitter source starting, monitoring {} users",
            self.config.watch_users.len()
        );

        let mut seen_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));

        loop {
            interval.tick().await;

            for user_id in &self.config.watch_users {
                match self.fetch_user_tweets(user_id).await {
                    Ok(tweets) => {
                        for tweet in tweets {
                            // Skip if already seen
                            if seen_ids.contains(&tweet.id) {
                                continue;
                            }
                            seen_ids.insert(tweet.id.clone());

                            // Filter by keywords if configured
                            if !self.config.keywords.is_empty() {
                                let text_lower = tweet.text.to_lowercase();
                                let has_keyword = self.config.keywords
                                    .iter()
                                    .any(|k| text_lower.contains(&k.to_lowercase()));
                                if !has_keyword {
                                    continue;
                                }
                            }

                            let author = tweet.author_id.clone().unwrap_or_else(|| user_id.clone());
                            let signal = RawSignal {
                                source: "twitter".to_string(),
                                source_id: tweet.id,
                                content: tweet.text,
                                author: author.clone(),
                                author_trust: self.get_trust(&author),
                                timestamp: Utc::now(),
                                metadata: Some(serde_json::json!({
                                    "user_id": user_id,
                                    "created_at": tweet.created_at
                                })),
                            };

                            if tx.send(signal).await.is_err() {
                                tracing::warn!("Failed to send signal, channel closed");
                                return Ok(());
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to fetch tweets for {}: {}", user_id, e);
                    }
                }
            }

            // Limit seen cache size
            if seen_ids.len() > 10000 {
                seen_ids.clear();
            }
        }
    }
}

/// RSS-based Twitter source (using Nitter or similar)
pub struct TwitterRssSource {
    nitter_instance: String,
    usernames: Vec<String>,
    keywords: Vec<String>,
    http: reqwest::Client,
}

impl TwitterRssSource {
    pub fn new(nitter_instance: String, usernames: Vec<String>, keywords: Vec<String>) -> Self {
        Self {
            nitter_instance,
            usernames,
            keywords,
            http: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl SignalSource for TwitterRssSource {
    fn name(&self) -> &str {
        "twitter_rss"
    }

    async fn run(&self, tx: mpsc::Sender<RawSignal>) -> Result<()> {
        tracing::info!(
            "Twitter RSS source starting via {}, monitoring {} users",
            self.nitter_instance,
            self.usernames.len()
        );

        let mut seen_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(120));

        loop {
            interval.tick().await;

            for username in &self.usernames {
                let url = format!("{}/{}/rss", self.nitter_instance, username);
                
                match self.http.get(&url).send().await {
                    Ok(resp) => {
                        if let Ok(text) = resp.text().await {
                            // Simple RSS parsing (in production, use a proper RSS parser)
                            // This is a basic implementation
                            for item in extract_rss_items(&text) {
                                if seen_ids.contains(&item.guid) {
                                    continue;
                                }
                                seen_ids.insert(item.guid.clone());

                                // Keyword filter
                                if !self.keywords.is_empty() {
                                    let text_lower = item.description.to_lowercase();
                                    let has_keyword = self.keywords
                                        .iter()
                                        .any(|k| text_lower.contains(&k.to_lowercase()));
                                    if !has_keyword {
                                        continue;
                                    }
                                }

                                let signal = RawSignal {
                                    source: "twitter".to_string(),
                                    source_id: item.guid,
                                    content: item.description,
                                    author: username.clone(),
                                    author_trust: 0.5,
                                    timestamp: Utc::now(),
                                    metadata: Some(serde_json::json!({
                                        "username": username,
                                        "link": item.link
                                    })),
                                };

                                if tx.send(signal).await.is_err() {
                                    return Ok(());
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to fetch RSS for {}: {}", username, e);
                    }
                }
            }

            if seen_ids.len() > 10000 {
                seen_ids.clear();
            }
        }
    }
}

struct RssItem {
    guid: String,
    description: String,
    link: String,
}

fn extract_rss_items(xml: &str) -> Vec<RssItem> {
    let mut items = Vec::new();
    
    // Very basic XML parsing - in production use quick-xml or similar
    let mut in_item = false;
    let mut current_guid = String::new();
    let mut current_desc = String::new();
    let mut current_link = String::new();

    for line in xml.lines() {
        let line = line.trim();
        
        if line.contains("<item>") {
            in_item = true;
            current_guid.clear();
            current_desc.clear();
            current_link.clear();
        } else if line.contains("</item>") {
            if in_item && !current_guid.is_empty() {
                items.push(RssItem {
                    guid: current_guid.clone(),
                    description: current_desc.clone(),
                    link: current_link.clone(),
                });
            }
            in_item = false;
        } else if in_item {
            if line.starts_with("<guid>") {
                current_guid = extract_tag_content(line, "guid");
            } else if line.starts_with("<description>") {
                current_desc = extract_tag_content(line, "description");
                // Decode basic HTML entities
                current_desc = current_desc
                    .replace("&lt;", "<")
                    .replace("&gt;", ">")
                    .replace("&amp;", "&")
                    .replace("&quot;", "\"");
            } else if line.starts_with("<link>") {
                current_link = extract_tag_content(line, "link");
            }
        }
    }

    items
}

fn extract_tag_content(line: &str, tag: &str) -> String {
    let start_tag = format!("<{}>", tag);
    let end_tag = format!("</{}>", tag);
    
    if let Some(start) = line.find(&start_tag) {
        if let Some(end) = line.find(&end_tag) {
            return line[start + start_tag.len()..end].to_string();
        }
    }
    String::new()
}
