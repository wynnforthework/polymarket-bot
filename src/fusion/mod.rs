//! Multi-Factor Fusion System
//!
//! Combines signals from multiple sources (ML, technical analysis, market microstructure,
//! sentiment, on-chain data) into unified trading decisions using ensemble methods.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐
//! │ ML Signals  │  │TA Signals   │  │ Microstruc  │  │ Sentiment   │
//! └──────┬──────┘  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘
//!        │                │                │                │
//!        └────────────────┴────────────────┴────────────────┘
//!                                  │
//!                         ┌───────▼───────┐
//!                         │ Signal Decay  │
//!                         │   Adjuster    │
//!                         └───────┬───────┘
//!                                 │
//!                         ┌───────▼───────┐
//!                         │ Regime Filter │
//!                         └───────┬───────┘
//!                                 │
//!                         ┌───────▼───────┐
//!                         │  Confidence   │
//!                         │   Weighter    │
//!                         └───────┬───────┘
//!                                 │
//!                         ┌───────▼───────┐
//!                         │ Ensemble Vote │
//!                         └───────┬───────┘
//!                                 │
//!                         ┌───────▼───────┐
//!                         │ Trade Decision│
//!                         └───────────────┘
//! ```

use std::collections::HashMap;
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};

/// Individual signal from a data source
#[derive(Debug, Clone)]
pub struct Signal {
    /// Signal source identifier
    pub source: SignalSource,
    /// Direction: positive = bullish, negative = bearish
    pub direction: f64,
    /// Confidence level (0.0 to 1.0)
    pub confidence: f64,
    /// Signal strength magnitude
    pub strength: f64,
    /// When the signal was generated
    pub timestamp: Instant,
    /// Time-to-live before decay starts
    pub ttl: Duration,
    /// Additional metadata
    pub metadata: HashMap<String, f64>,
}

/// Enumeration of signal sources
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SignalSource {
    /// Machine learning model predictions
    MLPredictor,
    /// Technical analysis indicators (RSI, MACD, etc.)
    TechnicalAnalysis,
    /// Order book imbalance signals
    OrderBookImbalance,
    /// Market regime detection
    MarketRegime,
    /// Sentiment analysis (Twitter, news)
    Sentiment,
    /// On-chain data signals
    OnChain,
    /// Statistical arbitrage signals
    StatArb,
    /// Funding rate signals
    FundingRate,
    /// Volatility signals
    Volatility,
    /// Custom/external signals
    Custom(u32),
}

impl Default for SignalSource {
    fn default() -> Self {
        SignalSource::Custom(0)
    }
}

/// Market regime for signal filtering
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MarketRegime {
    /// Strong upward trend
    TrendingUp,
    /// Strong downward trend
    TrendingDown,
    /// Range-bound, mean-reverting
    RangeBound,
    /// High volatility, uncertain direction
    HighVolatility,
    /// Low activity, low spreads
    LowVolatility,
    /// Market crisis/black swan
    Crisis,
}

impl Default for MarketRegime {
    fn default() -> Self {
        MarketRegime::RangeBound
    }
}

/// Configuration for the fusion system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FusionConfig {
    /// Base weights for each signal source
    pub source_weights: HashMap<SignalSource, f64>,
    /// Minimum confidence threshold to include signal
    pub min_confidence: f64,
    /// Signal decay half-life in seconds
    pub decay_half_life_secs: f64,
    /// Minimum number of agreeing signals for consensus
    pub min_consensus_signals: usize,
    /// Ensemble method to use
    pub ensemble_method: EnsembleMethod,
    /// Regime-specific weight adjustments
    pub regime_adjustments: HashMap<MarketRegime, HashMap<SignalSource, f64>>,
    /// Maximum signal age before discard (seconds)
    pub max_signal_age_secs: u64,
    /// Conflict resolution strategy
    pub conflict_strategy: ConflictStrategy,
}

impl Default for FusionConfig {
    fn default() -> Self {
        let mut source_weights = HashMap::new();
        source_weights.insert(SignalSource::MLPredictor, 1.5);
        source_weights.insert(SignalSource::TechnicalAnalysis, 1.0);
        source_weights.insert(SignalSource::OrderBookImbalance, 1.2);
        source_weights.insert(SignalSource::MarketRegime, 0.8);
        source_weights.insert(SignalSource::Sentiment, 0.7);
        source_weights.insert(SignalSource::OnChain, 0.9);
        source_weights.insert(SignalSource::StatArb, 1.1);
        source_weights.insert(SignalSource::FundingRate, 0.8);
        source_weights.insert(SignalSource::Volatility, 0.6);

        Self {
            source_weights,
            min_confidence: 0.3,
            decay_half_life_secs: 300.0, // 5 minutes
            min_consensus_signals: 2,
            ensemble_method: EnsembleMethod::WeightedAverage,
            regime_adjustments: Self::default_regime_adjustments(),
            max_signal_age_secs: 900, // 15 minutes
            conflict_strategy: ConflictStrategy::WeightedMajority,
        }
    }
}

impl FusionConfig {
    fn default_regime_adjustments() -> HashMap<MarketRegime, HashMap<SignalSource, f64>> {
        let mut adjustments = HashMap::new();

        // In trending markets, boost momentum signals
        let mut trending_up = HashMap::new();
        trending_up.insert(SignalSource::TechnicalAnalysis, 1.3);
        trending_up.insert(SignalSource::MLPredictor, 1.2);
        trending_up.insert(SignalSource::StatArb, 0.7); // Mean reversion less useful
        adjustments.insert(MarketRegime::TrendingUp, trending_up.clone());
        adjustments.insert(MarketRegime::TrendingDown, trending_up);

        // In range-bound markets, boost mean-reversion
        let mut range_bound = HashMap::new();
        range_bound.insert(SignalSource::StatArb, 1.4);
        range_bound.insert(SignalSource::OrderBookImbalance, 1.2);
        range_bound.insert(SignalSource::TechnicalAnalysis, 0.9);
        adjustments.insert(MarketRegime::RangeBound, range_bound);

        // In high volatility, boost quick signals
        let mut high_vol = HashMap::new();
        high_vol.insert(SignalSource::OrderBookImbalance, 1.5);
        high_vol.insert(SignalSource::Volatility, 1.3);
        high_vol.insert(SignalSource::Sentiment, 1.2);
        adjustments.insert(MarketRegime::HighVolatility, high_vol);

        // In crisis, reduce all signals, boost on-chain
        let mut crisis = HashMap::new();
        crisis.insert(SignalSource::OnChain, 1.5);
        crisis.insert(SignalSource::MLPredictor, 0.5);
        crisis.insert(SignalSource::TechnicalAnalysis, 0.4);
        adjustments.insert(MarketRegime::Crisis, crisis);

        adjustments
    }
}

/// Ensemble method for combining signals
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EnsembleMethod {
    /// Simple average of all signals
    SimpleAverage,
    /// Weighted average based on source weights and confidence
    WeightedAverage,
    /// Majority voting (direction only)
    MajorityVote,
    /// Bayesian combination
    BayesianFusion,
    /// Take the signal with highest confidence
    MaxConfidence,
    /// Stack meta-learner (uses historical accuracy)
    Stacking,
}

/// Strategy for resolving conflicting signals
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictStrategy {
    /// Use weighted majority
    WeightedMajority,
    /// Abstain if signals conflict
    Abstain,
    /// Reduce position size proportionally
    ReduceSize,
    /// Trust highest confidence signal
    TrustHighest,
}

/// Final fused trading decision
#[derive(Debug, Clone)]
pub struct FusedDecision {
    /// Trading direction: positive = long/up, negative = short/down
    pub direction: f64,
    /// Overall confidence (0.0 to 1.0)
    pub confidence: f64,
    /// Recommended position size multiplier (0.0 to 1.0)
    pub size_multiplier: f64,
    /// Current market regime
    pub regime: MarketRegime,
    /// Number of signals that contributed
    pub signal_count: usize,
    /// Degree of consensus among signals (0.0 to 1.0)
    pub consensus: f64,
    /// Individual signal contributions
    pub contributions: Vec<SignalContribution>,
    /// Timestamp of fusion
    pub timestamp: Instant,
    /// Reason if decision is to abstain
    pub abstain_reason: Option<String>,
}

/// Individual signal's contribution to the decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalContribution {
    pub source: SignalSource,
    pub weight: f64,
    pub direction: f64,
    pub decay_factor: f64,
}

/// Multi-Factor Fusion Engine
pub struct FusionEngine {
    config: FusionConfig,
    signals: Vec<Signal>,
    current_regime: MarketRegime,
    historical_accuracy: HashMap<SignalSource, f64>,
}

impl FusionEngine {
    /// Create a new fusion engine with default config
    pub fn new() -> Self {
        Self::with_config(FusionConfig::default())
    }

    /// Create a new fusion engine with custom config
    pub fn with_config(config: FusionConfig) -> Self {
        Self {
            config,
            signals: Vec::new(),
            current_regime: MarketRegime::default(),
            historical_accuracy: HashMap::new(),
        }
    }

    /// Set the current market regime
    pub fn set_regime(&mut self, regime: MarketRegime) {
        self.current_regime = regime;
    }

    /// Update historical accuracy for a signal source
    pub fn update_accuracy(&mut self, source: SignalSource, accuracy: f64) {
        self.historical_accuracy.insert(source, accuracy.clamp(0.0, 1.0));
    }

    /// Add a new signal to the fusion engine
    pub fn add_signal(&mut self, signal: Signal) {
        // Remove old signals from the same source
        self.signals.retain(|s| s.source != signal.source);
        self.signals.push(signal);
    }

    /// Clear all signals
    pub fn clear_signals(&mut self) {
        self.signals.clear();
    }

    /// Calculate decay factor for a signal based on age
    fn calculate_decay(&self, signal: &Signal) -> f64 {
        let age = signal.timestamp.elapsed();
        
        // If within TTL, no decay
        if age < signal.ttl {
            return 1.0;
        }

        // Exponential decay after TTL
        let decay_time = age.as_secs_f64() - signal.ttl.as_secs_f64();
        let half_life = self.config.decay_half_life_secs;
        
        (-decay_time * 0.693 / half_life).exp()
    }

    /// Get effective weight for a signal source in current regime
    fn get_effective_weight(&self, source: SignalSource) -> f64 {
        let base_weight = self.config.source_weights.get(&source).copied().unwrap_or(1.0);
        
        // Apply regime adjustment
        let regime_factor = self.config.regime_adjustments
            .get(&self.current_regime)
            .and_then(|adj| adj.get(&source))
            .copied()
            .unwrap_or(1.0);

        // Apply historical accuracy if available (stacking)
        let accuracy_factor = if self.config.ensemble_method == EnsembleMethod::Stacking {
            self.historical_accuracy.get(&source).copied().unwrap_or(0.5) * 2.0
        } else {
            1.0
        };

        base_weight * regime_factor * accuracy_factor
    }

    /// Filter signals based on age and confidence
    fn filter_signals(&self) -> Vec<&Signal> {
        let now = Instant::now();
        let max_age = Duration::from_secs(self.config.max_signal_age_secs);

        self.signals.iter()
            .filter(|s| {
                let age = now.duration_since(s.timestamp);
                age < max_age && s.confidence >= self.config.min_confidence
            })
            .collect()
    }

    /// Calculate consensus among signals (0.0 = complete disagreement, 1.0 = complete agreement)
    fn calculate_consensus(&self, signals: &[&Signal]) -> f64 {
        if signals.len() < 2 {
            return 1.0;
        }

        let mut agreement_sum = 0.0;
        let mut comparison_count = 0;

        for i in 0..signals.len() {
            for j in (i + 1)..signals.len() {
                let dir_i = signals[i].direction.signum();
                let dir_j = signals[j].direction.signum();
                
                if dir_i == dir_j {
                    agreement_sum += 1.0;
                } else if dir_i == 0.0 || dir_j == 0.0 {
                    agreement_sum += 0.5;
                }
                
                comparison_count += 1;
            }
        }

        if comparison_count > 0 {
            agreement_sum / comparison_count as f64
        } else {
            1.0
        }
    }

    /// Fuse all signals into a trading decision
    pub fn fuse(&self) -> FusedDecision {
        let signals = self.filter_signals();
        let timestamp = Instant::now();

        // Check minimum signals requirement
        if signals.len() < self.config.min_consensus_signals {
            return FusedDecision {
                direction: 0.0,
                confidence: 0.0,
                size_multiplier: 0.0,
                regime: self.current_regime,
                signal_count: signals.len(),
                consensus: 0.0,
                contributions: Vec::new(),
                timestamp,
                abstain_reason: Some(format!(
                    "Insufficient signals: {} < {}",
                    signals.len(),
                    self.config.min_consensus_signals
                )),
            };
        }

        let consensus = self.calculate_consensus(&signals);

        // Build contributions with weights
        let contributions: Vec<SignalContribution> = signals.iter()
            .map(|s| {
                let decay = self.calculate_decay(s);
                let weight = self.get_effective_weight(s.source) * s.confidence * decay;
                SignalContribution {
                    source: s.source,
                    weight,
                    direction: s.direction,
                    decay_factor: decay,
                }
            })
            .collect();

        // Apply ensemble method
        let (direction, confidence) = match self.config.ensemble_method {
            EnsembleMethod::SimpleAverage => self.simple_average(&contributions),
            EnsembleMethod::WeightedAverage => self.weighted_average(&contributions),
            EnsembleMethod::MajorityVote => self.majority_vote(&contributions),
            EnsembleMethod::BayesianFusion => self.bayesian_fusion(&contributions),
            EnsembleMethod::MaxConfidence => self.max_confidence(&signals),
            EnsembleMethod::Stacking => self.weighted_average(&contributions),
        };

        // Handle conflicts
        let (final_direction, size_multiplier, abstain_reason) = 
            self.resolve_conflicts(direction, confidence, consensus);

        FusedDecision {
            direction: final_direction,
            confidence,
            size_multiplier,
            regime: self.current_regime,
            signal_count: signals.len(),
            consensus,
            contributions,
            timestamp,
            abstain_reason,
        }
    }

    fn simple_average(&self, contributions: &[SignalContribution]) -> (f64, f64) {
        if contributions.is_empty() {
            return (0.0, 0.0);
        }

        let sum_direction: f64 = contributions.iter().map(|c| c.direction).sum();
        let direction = sum_direction / contributions.len() as f64;
        let confidence = direction.abs().min(1.0);

        (direction, confidence)
    }

    fn weighted_average(&self, contributions: &[SignalContribution]) -> (f64, f64) {
        if contributions.is_empty() {
            return (0.0, 0.0);
        }

        let total_weight: f64 = contributions.iter().map(|c| c.weight).sum();
        
        if total_weight < 0.0001 {
            return (0.0, 0.0);
        }

        let weighted_direction: f64 = contributions.iter()
            .map(|c| c.direction * c.weight)
            .sum();

        let direction = weighted_direction / total_weight;
        
        // Confidence is the agreement-weighted average
        let confidence = contributions.iter()
            .map(|c| c.weight * (c.direction.signum() == direction.signum()) as i32 as f64)
            .sum::<f64>() / total_weight;

        (direction, confidence.clamp(0.0, 1.0))
    }

    fn majority_vote(&self, contributions: &[SignalContribution]) -> (f64, f64) {
        if contributions.is_empty() {
            return (0.0, 0.0);
        }

        let bullish: f64 = contributions.iter()
            .filter(|c| c.direction > 0.0)
            .map(|c| c.weight)
            .sum();

        let bearish: f64 = contributions.iter()
            .filter(|c| c.direction < 0.0)
            .map(|c| c.weight)
            .sum();

        let total = bullish + bearish;
        
        if total < 0.0001 {
            return (0.0, 0.0);
        }

        let direction = if bullish > bearish { 1.0 } else { -1.0 };
        let confidence = (bullish - bearish).abs() / total;

        (direction, confidence)
    }

    fn bayesian_fusion(&self, contributions: &[SignalContribution]) -> (f64, f64) {
        // Simplified Bayesian fusion using log-odds
        if contributions.is_empty() {
            return (0.0, 0.0);
        }

        let mut log_odds_sum = 0.0;

        for c in contributions {
            // Convert direction (-1 to 1) to probability (0 to 1)
            let prob = (c.direction + 1.0) / 2.0;
            let prob = prob.clamp(0.01, 0.99); // Avoid infinities
            
            // Log odds with weight
            let log_odds = (prob / (1.0 - prob)).ln() * c.weight;
            log_odds_sum += log_odds;
        }

        // Convert back from log-odds
        let combined_prob = 1.0 / (1.0 + (-log_odds_sum).exp());
        let direction = combined_prob * 2.0 - 1.0;
        let confidence = (combined_prob - 0.5).abs() * 2.0;

        (direction, confidence)
    }

    fn max_confidence(&self, signals: &[&Signal]) -> (f64, f64) {
        signals.iter()
            .max_by(|a, b| a.confidence.partial_cmp(&b.confidence).unwrap())
            .map(|s| (s.direction, s.confidence))
            .unwrap_or((0.0, 0.0))
    }

    fn resolve_conflicts(
        &self,
        direction: f64,
        confidence: f64,
        consensus: f64,
    ) -> (f64, f64, Option<String>) {
        // Low consensus indicates conflict
        if consensus < 0.4 {
            match self.config.conflict_strategy {
                ConflictStrategy::Abstain => {
                    return (0.0, 0.0, Some("Signal conflict - abstaining".to_string()));
                }
                ConflictStrategy::ReduceSize => {
                    let size_mult = consensus * 0.5;
                    return (direction, size_mult, None);
                }
                ConflictStrategy::TrustHighest => {
                    // Already handled by ensemble method
                }
                ConflictStrategy::WeightedMajority => {
                    // Default behavior
                }
            }
        }

        // Crisis regime reduces position size
        let regime_size_mult = match self.current_regime {
            MarketRegime::Crisis => 0.2,
            MarketRegime::HighVolatility => 0.6,
            _ => 1.0,
        };

        let size_multiplier = (consensus * confidence * regime_size_mult).clamp(0.0, 1.0);
        
        (direction, size_multiplier, None)
    }
}

impl Default for FusionEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for creating signals
pub struct SignalBuilder {
    source: SignalSource,
    direction: f64,
    confidence: f64,
    strength: f64,
    ttl: Duration,
    metadata: HashMap<String, f64>,
}

impl SignalBuilder {
    pub fn new(source: SignalSource) -> Self {
        Self {
            source,
            direction: 0.0,
            confidence: 0.5,
            strength: 1.0,
            ttl: Duration::from_secs(60),
            metadata: HashMap::new(),
        }
    }

    pub fn direction(mut self, direction: f64) -> Self {
        self.direction = direction.clamp(-1.0, 1.0);
        self
    }

    pub fn confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    pub fn strength(mut self, strength: f64) -> Self {
        self.strength = strength;
        self
    }

    pub fn ttl(mut self, ttl: Duration) -> Self {
        self.ttl = ttl;
        self
    }

    pub fn metadata(mut self, key: &str, value: f64) -> Self {
        self.metadata.insert(key.to_string(), value);
        self
    }

    pub fn build(self) -> Signal {
        Signal {
            source: self.source,
            direction: self.direction,
            confidence: self.confidence,
            strength: self.strength,
            timestamp: Instant::now(),
            ttl: self.ttl,
            metadata: self.metadata,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signal_builder() {
        let signal = SignalBuilder::new(SignalSource::MLPredictor)
            .direction(0.8)
            .confidence(0.9)
            .strength(1.5)
            .ttl(Duration::from_secs(120))
            .metadata("rsi", 45.0)
            .build();

        assert_eq!(signal.source, SignalSource::MLPredictor);
        assert!((signal.direction - 0.8).abs() < 0.001);
        assert!((signal.confidence - 0.9).abs() < 0.001);
        assert_eq!(signal.metadata.get("rsi"), Some(&45.0));
    }

    #[test]
    fn test_fusion_insufficient_signals() {
        let engine = FusionEngine::new();
        let decision = engine.fuse();

        assert!(decision.abstain_reason.is_some());
        assert!((decision.direction - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_fusion_weighted_average() {
        let mut engine = FusionEngine::new();
        
        // Add bullish ML signal
        engine.add_signal(
            SignalBuilder::new(SignalSource::MLPredictor)
                .direction(0.8)
                .confidence(0.9)
                .build()
        );

        // Add bullish TA signal
        engine.add_signal(
            SignalBuilder::new(SignalSource::TechnicalAnalysis)
                .direction(0.6)
                .confidence(0.7)
                .build()
        );

        // Add bearish sentiment (minority)
        engine.add_signal(
            SignalBuilder::new(SignalSource::Sentiment)
                .direction(-0.3)
                .confidence(0.5)
                .build()
        );

        let decision = engine.fuse();

        // Should be bullish overall
        assert!(decision.direction > 0.0);
        assert!(decision.confidence > 0.0);
        assert_eq!(decision.signal_count, 3);
    }

    #[test]
    fn test_fusion_consensus_calculation() {
        let mut engine = FusionEngine::new();
        
        // All signals agree
        engine.add_signal(
            SignalBuilder::new(SignalSource::MLPredictor)
                .direction(0.8)
                .confidence(0.9)
                .build()
        );
        engine.add_signal(
            SignalBuilder::new(SignalSource::TechnicalAnalysis)
                .direction(0.7)
                .confidence(0.8)
                .build()
        );

        let decision = engine.fuse();
        assert!(decision.consensus > 0.9); // High consensus
    }

    #[test]
    fn test_fusion_conflict_signals() {
        let mut engine = FusionEngine::with_config(FusionConfig {
            conflict_strategy: ConflictStrategy::ReduceSize,
            ..FusionConfig::default()
        });
        
        // Conflicting signals
        engine.add_signal(
            SignalBuilder::new(SignalSource::MLPredictor)
                .direction(0.9)
                .confidence(0.9)
                .build()
        );
        engine.add_signal(
            SignalBuilder::new(SignalSource::TechnicalAnalysis)
                .direction(-0.8)
                .confidence(0.85)
                .build()
        );

        let decision = engine.fuse();
        
        // Low consensus should reduce size
        assert!(decision.consensus < 0.5);
        assert!(decision.size_multiplier < 0.5);
    }

    #[test]
    fn test_regime_adjustment() {
        let mut engine = FusionEngine::new();
        engine.set_regime(MarketRegime::Crisis);

        engine.add_signal(
            SignalBuilder::new(SignalSource::MLPredictor)
                .direction(0.8)
                .confidence(0.9)
                .build()
        );
        engine.add_signal(
            SignalBuilder::new(SignalSource::OnChain)
                .direction(0.7)
                .confidence(0.8)
                .build()
        );

        let decision = engine.fuse();
        
        // In crisis, size should be reduced
        assert!(decision.size_multiplier < 0.5);
        assert_eq!(decision.regime, MarketRegime::Crisis);
    }

    #[test]
    fn test_signal_decay() {
        let engine = FusionEngine::new();
        
        // Signal within TTL - no decay
        let signal = SignalBuilder::new(SignalSource::MLPredictor)
            .ttl(Duration::from_secs(300))
            .build();
        let decay = engine.calculate_decay(&signal);
        assert!((decay - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_bayesian_fusion() {
        let mut engine = FusionEngine::with_config(FusionConfig {
            ensemble_method: EnsembleMethod::BayesianFusion,
            ..FusionConfig::default()
        });

        engine.add_signal(
            SignalBuilder::new(SignalSource::MLPredictor)
                .direction(0.6)
                .confidence(0.8)
                .build()
        );
        engine.add_signal(
            SignalBuilder::new(SignalSource::TechnicalAnalysis)
                .direction(0.5)
                .confidence(0.7)
                .build()
        );

        let decision = engine.fuse();
        assert!(decision.direction > 0.0);
    }

    #[test]
    fn test_majority_vote() {
        let mut engine = FusionEngine::with_config(FusionConfig {
            ensemble_method: EnsembleMethod::MajorityVote,
            ..FusionConfig::default()
        });

        // 2 bullish, 1 bearish
        engine.add_signal(
            SignalBuilder::new(SignalSource::MLPredictor)
                .direction(0.8)
                .confidence(0.7)
                .build()
        );
        engine.add_signal(
            SignalBuilder::new(SignalSource::TechnicalAnalysis)
                .direction(0.6)
                .confidence(0.6)
                .build()
        );
        engine.add_signal(
            SignalBuilder::new(SignalSource::Sentiment)
                .direction(-0.5)
                .confidence(0.5)
                .build()
        );

        let decision = engine.fuse();
        assert_eq!(decision.direction, 1.0); // Bullish wins
    }

    #[test]
    fn test_stacking_with_accuracy() {
        let mut engine = FusionEngine::with_config(FusionConfig {
            ensemble_method: EnsembleMethod::Stacking,
            ..FusionConfig::default()
        });

        // Set historical accuracy
        engine.update_accuracy(SignalSource::MLPredictor, 0.75);
        engine.update_accuracy(SignalSource::TechnicalAnalysis, 0.55);

        engine.add_signal(
            SignalBuilder::new(SignalSource::MLPredictor)
                .direction(0.6)
                .confidence(0.7)
                .build()
        );
        engine.add_signal(
            SignalBuilder::new(SignalSource::TechnicalAnalysis)
                .direction(-0.5)
                .confidence(0.7)
                .build()
        );

        let decision = engine.fuse();
        
        // ML has higher accuracy, should pull direction positive
        assert!(decision.direction > 0.0);
    }

    #[test]
    fn test_abstain_strategy() {
        let mut engine = FusionEngine::with_config(FusionConfig {
            conflict_strategy: ConflictStrategy::Abstain,
            ..FusionConfig::default()
        });

        // Highly conflicting signals
        engine.add_signal(
            SignalBuilder::new(SignalSource::MLPredictor)
                .direction(1.0)
                .confidence(0.9)
                .build()
        );
        engine.add_signal(
            SignalBuilder::new(SignalSource::TechnicalAnalysis)
                .direction(-1.0)
                .confidence(0.9)
                .build()
        );

        let decision = engine.fuse();
        
        assert!(decision.abstain_reason.is_some());
        assert!((decision.direction - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_contributions_tracking() {
        let mut engine = FusionEngine::new();

        engine.add_signal(
            SignalBuilder::new(SignalSource::MLPredictor)
                .direction(0.7)
                .confidence(0.8)
                .build()
        );
        engine.add_signal(
            SignalBuilder::new(SignalSource::OrderBookImbalance)
                .direction(0.5)
                .confidence(0.6)
                .build()
        );

        let decision = engine.fuse();

        assert_eq!(decision.contributions.len(), 2);
        
        // Check ML contribution exists
        let ml_contrib = decision.contributions.iter()
            .find(|c| c.source == SignalSource::MLPredictor);
        assert!(ml_contrib.is_some());
    }
}
