//! Sentiment analysis model (placeholder)

use super::{Prediction, ProbabilityModel};
use crate::error::Result;
use crate::types::Market;
use async_trait::async_trait;
use rust_decimal::Decimal;

/// Sentiment-based probability model
/// 
/// TODO: Implement actual sentiment analysis using news/social media data
pub struct SentimentModel;

impl SentimentModel {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SentimentModel {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ProbabilityModel for SentimentModel {
    async fn predict(&self, market: &Market) -> Result<Prediction> {
        // Placeholder: return market price as estimate with low confidence
        let probability = market.yes_price().unwrap_or(Decimal::new(50, 2));

        Ok(Prediction {
            probability,
            confidence: Decimal::new(30, 2), // Low confidence for placeholder
            reasoning: "Sentiment analysis not yet implemented".to_string(),
        })
    }

    fn name(&self) -> &str {
        "Sentiment"
    }
}
