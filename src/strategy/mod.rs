//! Trading strategy implementation

use crate::config::{RiskConfig, StrategyConfig};
use crate::model::Prediction;
use crate::types::{Market, Side, Signal};
use chrono::Utc;
use rust_decimal::Decimal;

/// Signal generator based on model predictions
pub struct SignalGenerator {
    config: StrategyConfig,
    risk_config: RiskConfig,
}

impl SignalGenerator {
    pub fn new(config: StrategyConfig, risk_config: RiskConfig) -> Self {
        Self { config, risk_config }
    }

    /// Generate trading signal from market and prediction
    pub fn generate(&self, market: &Market, prediction: &Prediction) -> Option<Signal> {
        let market_prob = market.yes_price()?;
        let model_prob = prediction.probability;
        let edge = model_prob - market_prob;

        // Check if edge is significant
        if edge.abs() < self.config.min_edge {
            return None;
        }

        // Check confidence threshold
        if prediction.confidence < self.config.min_confidence {
            return None;
        }

        // Determine side
        let (side, token_id) = if edge > Decimal::ZERO {
            // Model thinks Yes is underpriced -> Buy Yes
            let token_id = market
                .outcomes
                .iter()
                .find(|o| o.outcome.to_lowercase() == "yes")
                .map(|o| o.token_id.clone())?;
            (Side::Buy, token_id)
        } else {
            // Model thinks Yes is overpriced -> Buy No (or sell Yes)
            let token_id = market
                .outcomes
                .iter()
                .find(|o| o.outcome.to_lowercase() == "no")
                .map(|o| o.token_id.clone())?;
            (Side::Buy, token_id)
        };

        // Calculate position size using Kelly criterion
        let suggested_size = self.calculate_kelly_size(edge.abs(), prediction.confidence);

        Some(Signal {
            market_id: market.id.clone(),
            token_id,
            side,
            model_probability: model_prob,
            market_probability: market_prob,
            edge,
            confidence: prediction.confidence,
            suggested_size,
            timestamp: Utc::now(),
        })
    }

    /// Calculate position size using fractional Kelly criterion
    fn calculate_kelly_size(&self, edge: Decimal, confidence: Decimal) -> Decimal {
        // Kelly formula: f* = (bp - q) / b
        // Where b = odds, p = probability of win, q = probability of loss
        //
        // Simplified for binary markets:
        // f* = edge / (1 - market_price)
        //
        // We use fractional Kelly for safety

        let base_kelly = edge / (Decimal::ONE - edge);
        let fractional_kelly = base_kelly * self.config.kelly_fraction;

        // Apply confidence adjustment
        let adjusted = fractional_kelly * confidence;

        // Cap at max position size
        adjusted.min(self.risk_config.max_position_pct)
    }
}
