//! Gamma API client for market data
//!
//! Fetches market information, prices, and metadata.

use crate::error::{BotError, Result};
use crate::types::{Market, Outcome};
use reqwest::Client;
use rust_decimal::Decimal;
use serde::Deserialize;

/// Gamma API client for market data
pub struct GammaClient {
    http: Client,
    base_url: String,
}

#[derive(Debug, Deserialize)]
struct GammaMarket {
    id: String,
    question: String,
    description: Option<String>,
    #[serde(rename = "endDate")]
    end_date: Option<String>,
    volume: Option<String>,
    liquidity: Option<String>,
    active: bool,
    closed: bool,
    outcomes: Option<String>, // JSON string
    #[serde(rename = "outcomePrices")]
    outcome_prices: Option<String>, // JSON string "[0.55, 0.45]"
    #[serde(rename = "clobTokenIds")]
    clob_token_ids: Option<String>, // JSON string
}

impl GammaClient {
    /// Create a new Gamma client
    pub fn new(base_url: &str) -> Result<Self> {
        let http = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        Ok(Self {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
        })
    }

    /// Get all active markets
    pub async fn get_markets(&self) -> Result<Vec<Market>> {
        let url = format!("{}/markets", self.base_url);
        let resp: Vec<GammaMarket> = self
            .http
            .get(&url)
            .query(&[("active", "true"), ("closed", "false")])
            .send()
            .await?
            .json()
            .await?;

        Ok(resp.into_iter().filter_map(|m| self.parse_market(m)).collect())
    }

    /// Get a specific market by ID
    pub async fn get_market(&self, market_id: &str) -> Result<Market> {
        let url = format!("{}/markets/{}", self.base_url, market_id);
        let resp: GammaMarket = self.http.get(&url).send().await?.json().await?;

        self.parse_market(resp)
            .ok_or_else(|| BotError::MarketNotFound(market_id.to_string()))
    }

    /// Search markets by keyword
    pub async fn search_markets(&self, query: &str) -> Result<Vec<Market>> {
        let url = format!("{}/markets", self.base_url);
        let resp: Vec<GammaMarket> = self
            .http
            .get(&url)
            .query(&[("_q", query)])
            .send()
            .await?
            .json()
            .await?;

        Ok(resp.into_iter().filter_map(|m| self.parse_market(m)).collect())
    }

    /// Get markets by volume (top markets)
    pub async fn get_top_markets(&self, limit: usize) -> Result<Vec<Market>> {
        let url = format!("{}/markets", self.base_url);
        let resp: Vec<GammaMarket> = self
            .http
            .get(&url)
            .query(&[
                ("active", "true"),
                ("closed", "false"),
                ("_sort", "volume:desc"),
                ("_limit", &limit.to_string()),
            ])
            .send()
            .await?
            .json()
            .await?;

        Ok(resp.into_iter().filter_map(|m| self.parse_market(m)).collect())
    }

    fn parse_market(&self, gm: GammaMarket) -> Option<Market> {
        // Parse outcome prices - API returns string array like ["0.55", "0.45"]
        let prices: Vec<f64> = gm
            .outcome_prices
            .as_ref()
            .and_then(|s| {
                // Try parsing as Vec<String> first (API format)
                if let Ok(string_prices) = serde_json::from_str::<Vec<String>>(s) {
                    let parsed: Vec<f64> = string_prices
                        .iter()
                        .filter_map(|p| p.parse::<f64>().ok())
                        .collect();
                    if !parsed.is_empty() {
                        return Some(parsed);
                    }
                }
                // Fallback: try parsing as Vec<f64> directly
                serde_json::from_str(s).ok()
            })
            .unwrap_or_default();

        // Parse token IDs
        let token_ids: Vec<String> = gm
            .clob_token_ids
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();

        // Parse outcome names
        let outcome_names: Vec<String> = gm
            .outcomes
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_else(|| vec!["Yes".to_string(), "No".to_string()]);

        // Build outcomes
        let outcomes: Vec<Outcome> = outcome_names
            .into_iter()
            .enumerate()
            .map(|(i, name)| Outcome {
                token_id: token_ids.get(i).cloned().unwrap_or_default(),
                outcome: name,
                price: prices
                    .get(i)
                    .map(|&p| Decimal::try_from(p).unwrap_or(Decimal::ZERO))
                    .unwrap_or(Decimal::ZERO),
            })
            .collect();

        Some(Market {
            id: gm.id,
            question: gm.question,
            description: gm.description,
            end_date: gm.end_date.as_ref().and_then(|s| s.parse().ok()),
            volume: gm
                .volume
                .as_ref()
                .and_then(|s| s.parse().ok())
                .unwrap_or(Decimal::ZERO),
            liquidity: gm
                .liquidity
                .as_ref()
                .and_then(|s| s.parse().ok())
                .unwrap_or(Decimal::ZERO),
            outcomes,
            active: gm.active,
            closed: gm.closed,
        })
    }
}
