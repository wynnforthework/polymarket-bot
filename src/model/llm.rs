//! LLM-based probability model
//!
//! Supports multiple LLM providers: DeepSeek, Anthropic, OpenAI, and OpenAI-compatible APIs.

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
}

#[derive(Debug, Clone)]
pub enum LlmProvider {
    DeepSeek {
        api_key: String,
        model: String,
    },
    Anthropic {
        api_key: String,
        model: String,
    },
    OpenAI {
        api_key: String,
        model: String,
        base_url: String,
    },
    /// OpenAI-compatible API (Ollama, vLLM, etc.)
    Compatible {
        api_key: Option<String>,
        model: String,
        base_url: String,
    },
}

// ============ Request/Response types ============

#[derive(Debug, Serialize)]
struct OpenAIRequest {
    model: String,
    messages: Vec<OpenAIMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<ResponseFormat>,
}

#[derive(Debug, Serialize)]
struct OpenAIMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct ResponseFormat {
    r#type: String,
}

#[derive(Debug, Deserialize)]
struct OpenAIResponse {
    choices: Vec<OpenAIChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAIChoice {
    message: OpenAIResponseMessage,
}

#[derive(Debug, Deserialize)]
struct OpenAIResponseMessage {
    content: String,
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
    pub fn new(provider: LlmProvider) -> Self {
        Self {
            http: Client::new(),
            provider,
        }
    }

    /// Create from config
    pub fn from_config(config: &crate::config::LlmConfig) -> Result<Self> {
        let provider = match config.provider.to_lowercase().as_str() {
            "deepseek" => LlmProvider::DeepSeek {
                api_key: config.api_key.clone(),
                model: config.model.clone().unwrap_or_else(|| "deepseek-chat".to_string()),
            },
            "anthropic" | "claude" => LlmProvider::Anthropic {
                api_key: config.api_key.clone(),
                model: config.model.clone().unwrap_or_else(|| "claude-sonnet-4-20250514".to_string()),
            },
            "openai" | "gpt" => LlmProvider::OpenAI {
                api_key: config.api_key.clone(),
                model: config.model.clone().unwrap_or_else(|| "gpt-4o-mini".to_string()),
                base_url: config.base_url.clone().unwrap_or_else(|| "https://api.openai.com".to_string()),
            },
            "ollama" => LlmProvider::Compatible {
                api_key: None,
                model: config.model.clone().unwrap_or_else(|| "qwen2.5:14b".to_string()),
                base_url: config.base_url.clone().unwrap_or_else(|| "http://localhost:11434".to_string()),
            },
            "compatible" | "custom" => LlmProvider::Compatible {
                api_key: if config.api_key.is_empty() { None } else { Some(config.api_key.clone()) },
                model: config.model.clone().ok_or_else(|| BotError::Config("model required for compatible provider".into()))?,
                base_url: config.base_url.clone().ok_or_else(|| BotError::Config("base_url required for compatible provider".into()))?,
            },
            _ => return Err(BotError::Config(format!("Unknown LLM provider: {}", config.provider))),
        };

        Ok(Self::new(provider))
    }

    /// Convenience constructors
    pub fn deepseek(api_key: String) -> Self {
        Self::new(LlmProvider::DeepSeek {
            api_key,
            model: "deepseek-chat".to_string(),
        })
    }

    pub fn anthropic(api_key: String) -> Self {
        Self::new(LlmProvider::Anthropic {
            api_key,
            model: "claude-sonnet-4-20250514".to_string(),
        })
    }

    pub fn openai(api_key: String) -> Self {
        Self::new(LlmProvider::OpenAI {
            api_key,
            model: "gpt-4o-mini".to_string(),
            base_url: "https://api.openai.com".to_string(),
        })
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

    async fn call_openai_compatible(
        &self,
        base_url: &str,
        api_key: Option<&str>,
        model: &str,
        prompt: &str,
    ) -> Result<String> {
        let request = OpenAIRequest {
            model: model.to_string(),
            messages: vec![OpenAIMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
            response_format: None, // DeepSeek doesn't need this
        };

        let mut req = self
            .http
            .post(format!("{}/v1/chat/completions", base_url))
            .header("content-type", "application/json");

        if let Some(key) = api_key {
            req = req.header("Authorization", format!("Bearer {}", key));
        }

        let resp = req.json(&request).send().await?;
        let text = resp.text().await?;
        tracing::debug!("LLM raw response: {}", &text[..text.len().min(500)]);
        
        let response: OpenAIResponse = serde_json::from_str(&text)
            .map_err(|e| BotError::Api(format!("JSON parse error: {} - response: {}", e, &text[..text.len().min(200)])))?;

        response
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .ok_or_else(|| BotError::Api("Empty response from LLM".into()))
    }

    async fn call_anthropic(&self, api_key: &str, model: &str, prompt: &str) -> Result<String> {
        let request = AnthropicRequest {
            model: model.to_string(),
            max_tokens: 500,
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
        };

        let response: AnthropicResponse = self
            .http
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", api_key)
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

    async fn call_llm(&self, prompt: &str) -> Result<String> {
        match &self.provider {
            LlmProvider::DeepSeek { api_key, model } => {
                self.call_openai_compatible("https://api.deepseek.com", Some(api_key), model, prompt)
                    .await
            }
            LlmProvider::Anthropic { api_key, model } => {
                self.call_anthropic(api_key, model, prompt).await
            }
            LlmProvider::OpenAI { api_key, model, base_url } => {
                self.call_openai_compatible(base_url, Some(api_key), model, prompt)
                    .await
            }
            LlmProvider::Compatible { api_key, model, base_url } => {
                self.call_openai_compatible(base_url, api_key.as_deref(), model, prompt)
                    .await
            }
        }
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

        let parsed: serde_json::Value = serde_json::from_str(json_str)
            .map_err(|e| BotError::Api(format!("Failed to parse LLM response: {}", e)))?;

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
            probability: Decimal::try_from(probability / 100.0).unwrap_or(Decimal::new(50, 2)),
            confidence: Decimal::try_from(confidence / 100.0).unwrap_or(Decimal::new(50, 2)),
            reasoning,
        })
    }
}

#[async_trait]
impl ProbabilityModel for LlmModel {
    async fn predict(&self, market: &Market) -> Result<Prediction> {
        let prompt = self.build_prompt(market);
        let response = self.call_llm(&prompt).await?;
        self.parse_response(&response)
    }

    fn name(&self) -> &str {
        match &self.provider {
            LlmProvider::DeepSeek { .. } => "DeepSeek",
            LlmProvider::Anthropic { .. } => "Claude",
            LlmProvider::OpenAI { .. } => "GPT",
            LlmProvider::Compatible { model, .. } => model,
        }
    }
}
