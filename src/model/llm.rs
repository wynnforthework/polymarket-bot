//! LLM-based probability model
//!
//! Uses Claude or GPT to analyze market questions and estimate probabilities.

use super::{Prediction, ProbabilityModel};
use crate::error::{BotError, Result};
use crate::types::Market;
use async_trait::async_trait;
use reqwest::Client;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// LLM model for probability estimation
pub struct LlmModel {
    http: Client,
    provider: LlmProvider,
    api_key: String,
    model: String,
}

#[derive(Debug, Clone)]
pub enum LlmProvider {
    Anthropic,
    OpenAI,
}

#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<AnthropicMessage>,
}

#[derive(Debug, Serialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContent>,
}

#[derive(Debug, Deserialize)]
struct AnthropicContent {
    text: String,
}

impl LlmModel {
    pub fn new(provider: LlmProvider, api_key: String, model: String) -> Self {
        Self {
            http: Client::new(),
            provider,
            api_key,
            model,
        }
    }

    pub fn anthropic(api_key: String) -> Self {
        Self::new(
            LlmProvider::Anthropic,
            api_key,
            "claude-sonnet-4-20250514".to_string(),
        )
    }

    fn build_prompt(&self, market: &Market) -> String {
        let yes_price = market.yes_price().unwrap_or(Decimal::new(50, 2));
        
        format!(
            r#"You are an expert prediction market analyst. Analyze the following market and estimate the probability of the "Yes" outcome.

Market Question: {}

Description: {}

Current Market Price: Yes = {:.2}% / No = {:.2}%

Instructions:
1. Consider all relevant factors, news, and historical precedents
2. Be objective and avoid cognitive biases
3. If you're uncertain, reflect that in your confidence score

Respond with ONLY a JSON object in this exact format:
{{"probability": <number 0-100>, "confidence": <number 0-100>, "reasoning": "<brief explanation>"}}

Example response:
{{"probability": 65, "confidence": 70, "reasoning": "Based on recent polling data and historical trends..."}}
"#,
            market.question,
            market.description.as_deref().unwrap_or("No description"),
            yes_price * Decimal::ONE_HUNDRED,
            (Decimal::ONE - yes_price) * Decimal::ONE_HUNDRED,
        )
    }

    async fn call_anthropic(&self, prompt: &str) -> Result<String> {
        let request = AnthropicRequest {
            model: self.model.clone(),
            max_tokens: 500,
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
        };

        let response: AnthropicResponse = self
            .http
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await?
            .json()
            .await?;

        response
            .content
            .first()
            .map(|c| c.text.clone())
            .ok_or_else(|| BotError::Api("Empty response from Anthropic".into()))
    }

    fn parse_response(&self, response: &str) -> Result<Prediction> {
        // Try to extract JSON from the response
        let json_str = if response.contains('{') {
            let start = response.find('{').unwrap();
            let end = response.rfind('}').unwrap_or(response.len() - 1) + 1;
            &response[start..end]
        } else {
            response
        };

        let parsed: serde_json::Value =
            serde_json::from_str(json_str).map_err(|e| BotError::Api(format!("Failed to parse LLM response: {}", e)))?;

        let probability = parsed["probability"]
            .as_f64()
            .ok_or_else(|| BotError::Api("Missing probability in response".into()))?;

        let confidence = parsed["confidence"]
            .as_f64()
            .ok_or_else(|| BotError::Api("Missing confidence in response".into()))?;

        let reasoning = parsed["reasoning"]
            .as_str()
            .unwrap_or("No reasoning provided")
            .to_string();

        Ok(Prediction {
            probability: Decimal::try_from(probability / 100.0)
                .unwrap_or(Decimal::new(50, 2)),
            confidence: Decimal::try_from(confidence / 100.0)
                .unwrap_or(Decimal::new(50, 2)),
            reasoning,
        })
    }
}

#[async_trait]
impl ProbabilityModel for LlmModel {
    async fn predict(&self, market: &Market) -> Result<Prediction> {
        let prompt = self.build_prompt(market);

        let response = match self.provider {
            LlmProvider::Anthropic => self.call_anthropic(&prompt).await?,
            LlmProvider::OpenAI => {
                // TODO: Implement OpenAI support
                return Err(BotError::Api("OpenAI not implemented yet".into()));
            }
        };

        self.parse_response(&response)
    }

    fn name(&self) -> &str {
        match self.provider {
            LlmProvider::Anthropic => "Claude",
            LlmProvider::OpenAI => "GPT",
        }
    }
}
