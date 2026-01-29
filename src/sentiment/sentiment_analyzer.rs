//! Sentiment Analysis Engine
//!
//! VADER-style sentiment analysis optimized for crypto/finance text.
//! Handles emojis, slang, and crypto-specific terminology.

use std::collections::HashMap;

/// Result of sentiment analysis
#[derive(Debug, Clone)]
pub struct SentimentResult {
    /// Positive sentiment (0.0 to 1.0)
    pub positive: f64,
    /// Negative sentiment (0.0 to 1.0)
    pub negative: f64,
    /// Neutral sentiment (0.0 to 1.0)
    pub neutral: f64,
    /// Compound score (-1.0 to 1.0)
    pub compound: f64,
}

impl SentimentResult {
    /// Returns true if overall positive
    pub fn is_positive(&self) -> bool {
        self.compound >= 0.05
    }

    /// Returns true if overall negative
    pub fn is_negative(&self) -> bool {
        self.compound <= -0.05
    }

    /// Returns true if neutral
    pub fn is_neutral(&self) -> bool {
        self.compound > -0.05 && self.compound < 0.05
    }
}

/// Sentiment score for a term
#[derive(Debug, Clone, Copy)]
pub struct SentimentScore {
    pub score: f64,
    pub intensity: f64,
}

impl Default for SentimentScore {
    fn default() -> Self {
        Self {
            score: 0.0,
            intensity: 1.0,
        }
    }
}

/// Sentiment analyzer using lexicon-based approach
pub struct SentimentAnalyzer {
    /// Word-level sentiment scores
    lexicon: HashMap<String, f64>,
    /// Crypto-specific terms
    crypto_lexicon: HashMap<String, f64>,
    /// Emoji sentiment scores
    emoji_lexicon: HashMap<char, f64>,
    /// Intensity modifiers (very, extremely, etc.)
    boosters: HashMap<String, f64>,
    /// Negation words
    negations: Vec<String>,
}

impl SentimentAnalyzer {
    /// Create a new sentiment analyzer with default lexicons
    pub fn new() -> Self {
        let mut analyzer = Self {
            lexicon: HashMap::new(),
            crypto_lexicon: HashMap::new(),
            emoji_lexicon: HashMap::new(),
            boosters: HashMap::new(),
            negations: Vec::new(),
        };
        analyzer.init_lexicons();
        analyzer
    }

    /// Initialize sentiment lexicons
    fn init_lexicons(&mut self) {
        // General positive words
        let positive_words = [
            ("good", 0.5),
            ("great", 0.7),
            ("excellent", 0.8),
            ("amazing", 0.8),
            ("awesome", 0.7),
            ("fantastic", 0.8),
            ("wonderful", 0.7),
            ("best", 0.8),
            ("love", 0.6),
            ("like", 0.3),
            ("happy", 0.6),
            ("beautiful", 0.6),
            ("strong", 0.5),
            ("win", 0.6),
            ("winning", 0.6),
            ("success", 0.7),
            ("successful", 0.7),
            ("profit", 0.6),
            ("profits", 0.6),
            ("gain", 0.5),
            ("gains", 0.5),
            ("up", 0.3),
            ("higher", 0.4),
            ("high", 0.3),
            ("rise", 0.4),
            ("rising", 0.4),
            ("positive", 0.5),
            ("opportunity", 0.5),
            ("opportunities", 0.5),
        ];

        // General negative words
        let negative_words = [
            ("bad", -0.5),
            ("terrible", -0.8),
            ("awful", -0.7),
            ("horrible", -0.8),
            ("poor", -0.5),
            ("worst", -0.8),
            ("hate", -0.7),
            ("dislike", -0.4),
            ("sad", -0.5),
            ("ugly", -0.5),
            ("weak", -0.5),
            ("lose", -0.6),
            ("losing", -0.6),
            ("loss", -0.6),
            ("losses", -0.6),
            ("fail", -0.6),
            ("failure", -0.7),
            ("down", -0.3),
            ("lower", -0.4),
            ("low", -0.3),
            ("fall", -0.4),
            ("falling", -0.4),
            ("drop", -0.4),
            ("dropping", -0.4),
            ("negative", -0.5),
            ("risk", -0.3),
            ("risky", -0.4),
            ("danger", -0.5),
            ("dangerous", -0.6),
            ("warning", -0.4),
            ("fear", -0.5),
            ("scared", -0.5),
            ("panic", -0.6),
            ("crash", -0.7),
            ("crashed", -0.7),
            ("dump", -0.6),
            ("dumping", -0.6),
        ];

        for (word, score) in positive_words.iter().chain(negative_words.iter()) {
            self.lexicon.insert(word.to_string(), *score);
        }

        // Crypto-specific terms
        let crypto_terms = [
            // Bullish terms
            ("moon", 0.8),
            ("mooning", 0.9),
            ("bullish", 0.7),
            ("bull", 0.5),
            ("bulls", 0.5),
            ("pump", 0.5),
            ("pumping", 0.6),
            ("breakout", 0.6),
            ("breaking", 0.4),
            ("ath", 0.7), // all-time high
            ("accumulate", 0.5),
            ("accumulating", 0.5),
            ("accumulation", 0.5),
            ("hodl", 0.4),
            ("hodling", 0.4),
            ("diamond", 0.5),
            ("diamondhands", 0.5),
            ("btfd", 0.4), // buy the dip
            ("dip", 0.3),  // neutral-positive (buying opportunity)
            ("support", 0.3),
            ("bounce", 0.4),
            ("bouncing", 0.4),
            ("reversal", 0.3),
            ("oversold", 0.3),
            ("undervalued", 0.4),
            ("institutional", 0.4),
            ("adoption", 0.5),
            ("bullrun", 0.7),
            // Bearish terms
            ("bearish", -0.7),
            ("bear", -0.5),
            ("bears", -0.5),
            ("rekt", -0.8),
            ("wrecked", -0.7),
            ("rugpull", -0.9),
            ("rug", -0.7),
            ("scam", -0.9),
            ("fraud", -0.9),
            ("ponzi", -0.9),
            ("bubble", -0.5),
            ("overbought", -0.3),
            ("overvalued", -0.4),
            ("resistance", -0.2),
            ("rejection", -0.4),
            ("liquidation", -0.6),
            ("liquidated", -0.7),
            ("capitulation", -0.6),
            ("fud", -0.5),
            ("bloodbath", -0.7),
            ("massacre", -0.7),
            ("nuke", -0.6),
            ("nuked", -0.7),
            ("short", -0.3),
            ("shorting", -0.4),
            ("shorts", -0.3),
        ];

        for (term, score) in crypto_terms {
            self.crypto_lexicon.insert(term.to_string(), score);
        }

        // Emoji sentiment
        let emojis = [
            ('🚀', 0.8),  // rocket - very bullish
            ('🌙', 0.7),  // moon
            ('💎', 0.6),  // diamond
            ('🙌', 0.5),  // raised hands
            ('💪', 0.5),  // flexed bicep
            ('🔥', 0.5),  // fire
            ('✅', 0.4),  // check
            ('👍', 0.4),  // thumbs up
            ('❤', 0.4),   // heart
            ('💰', 0.4),  // money bag
            ('📈', 0.6),  // chart up
            ('🎯', 0.4),  // target
            ('🏆', 0.5),  // trophy
            ('⚠', -0.4),  // warning
            ('❌', -0.4), // x
            ('👎', -0.4), // thumbs down
            ('😱', -0.5), // scared
            ('😰', -0.4), // anxious
            ('📉', -0.6), // chart down
            ('💀', -0.3), // skull (context dependent)
            ('🐻', -0.5), // bear
            ('🐂', 0.5),  // bull
            ('🤡', -0.4), // clown (mockery)
        ];

        for (emoji, score) in emojis {
            self.emoji_lexicon.insert(emoji, score);
        }

        // Intensity boosters
        let boosters = [
            ("very", 1.3),
            ("really", 1.3),
            ("extremely", 1.5),
            ("absolutely", 1.4),
            ("completely", 1.4),
            ("totally", 1.3),
            ("so", 1.2),
            ("super", 1.3),
            ("incredibly", 1.4),
            ("highly", 1.3),
            ("fucking", 1.5),
            ("insanely", 1.5),
            ("massively", 1.4),
            ("hugely", 1.4),
        ];

        for (word, factor) in boosters {
            self.boosters.insert(word.to_string(), factor);
        }

        // Negation words
        self.negations = vec![
            "not".to_string(),
            "no".to_string(),
            "never".to_string(),
            "none".to_string(),
            "neither".to_string(),
            "nobody".to_string(),
            "nothing".to_string(),
            "nowhere".to_string(),
            "isn't".to_string(),
            "aren't".to_string(),
            "wasn't".to_string(),
            "weren't".to_string(),
            "hasn't".to_string(),
            "haven't".to_string(),
            "hadn't".to_string(),
            "doesn't".to_string(),
            "don't".to_string(),
            "didn't".to_string(),
            "won't".to_string(),
            "wouldn't".to_string(),
            "can't".to_string(),
            "cannot".to_string(),
            "couldn't".to_string(),
            "shouldn't".to_string(),
        ];
    }

    /// Analyze sentiment of text
    pub fn analyze(&self, text: &str) -> SentimentResult {
        let lower = text.to_lowercase();
        let words: Vec<&str> = lower.split_whitespace().collect();

        let mut scores: Vec<f64> = Vec::new();

        // Check for emojis in original text
        for c in text.chars() {
            if let Some(&score) = self.emoji_lexicon.get(&c) {
                scores.push(score);
            }
        }

        // Analyze words
        let mut i = 0;
        while i < words.len() {
            let word = self.clean_word(words[i]);

            // Check for crypto terms first (higher priority)
            if let Some(&score) = self.crypto_lexicon.get(&word) {
                let modified = self.apply_modifiers(&words, i, score);
                scores.push(modified);
            } else if let Some(&score) = self.lexicon.get(&word) {
                let modified = self.apply_modifiers(&words, i, score);
                scores.push(modified);
            }

            i += 1;
        }

        // Calculate final scores
        if scores.is_empty() {
            return SentimentResult {
                positive: 0.0,
                negative: 0.0,
                neutral: 1.0,
                compound: 0.0,
            };
        }

        let positive_sum: f64 = scores.iter().filter(|&&s| s > 0.0).sum();
        let negative_sum: f64 = scores.iter().filter(|&&s| s < 0.0).map(|s| s.abs()).sum();

        let total = positive_sum + negative_sum;

        let positive = if total > 0.0 {
            positive_sum / total
        } else {
            0.0
        };
        let negative = if total > 0.0 {
            negative_sum / total
        } else {
            0.0
        };
        let neutral = 1.0 - positive - negative;

        // Compound score using normalization
        let sum: f64 = scores.iter().sum();
        let compound = self.normalize(sum);

        SentimentResult {
            positive,
            negative,
            neutral: neutral.max(0.0),
            compound,
        }
    }

    /// Clean a word by removing punctuation
    fn clean_word(&self, word: &str) -> String {
        word.chars()
            .filter(|c| c.is_alphanumeric() || *c == '\'' || *c == '-')
            .collect::<String>()
            .to_lowercase()
    }

    /// Apply modifiers (boosters, negations) to a score
    fn apply_modifiers(&self, words: &[&str], index: usize, mut score: f64) -> f64 {
        // Check previous words for modifiers (up to 3 words back)
        let start = index.saturating_sub(3);

        for i in start..index {
            let prev_word = self.clean_word(words[i]);

            // Check for boosters
            if let Some(&factor) = self.boosters.get(&prev_word) {
                score *= factor;
            }

            // Check for negations
            if self.negations.contains(&prev_word) {
                score *= -0.5; // Flip and dampen
            }
        }

        score.clamp(-1.0, 1.0)
    }

    /// Normalize score to -1 to 1 range
    fn normalize(&self, score: f64) -> f64 {
        let alpha = 15.0; // Normalization constant
        score / (score.abs() + alpha).sqrt()
    }

    /// Get sentiment label
    pub fn get_label(&self, result: &SentimentResult) -> &'static str {
        if result.compound >= 0.5 {
            "very positive"
        } else if result.compound >= 0.05 {
            "positive"
        } else if result.compound <= -0.5 {
            "very negative"
        } else if result.compound <= -0.05 {
            "negative"
        } else {
            "neutral"
        }
    }

    /// Batch analyze multiple texts
    pub fn analyze_batch(&self, texts: &[&str]) -> Vec<SentimentResult> {
        texts.iter().map(|t| self.analyze(t)).collect()
    }

    /// Get average sentiment from batch
    pub fn batch_average(&self, results: &[SentimentResult]) -> SentimentResult {
        if results.is_empty() {
            return SentimentResult {
                positive: 0.0,
                negative: 0.0,
                neutral: 1.0,
                compound: 0.0,
            };
        }

        let n = results.len() as f64;
        SentimentResult {
            positive: results.iter().map(|r| r.positive).sum::<f64>() / n,
            negative: results.iter().map(|r| r.negative).sum::<f64>() / n,
            neutral: results.iter().map(|r| r.neutral).sum::<f64>() / n,
            compound: results.iter().map(|r| r.compound).sum::<f64>() / n,
        }
    }
}

impl Default for SentimentAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_positive_sentiment() {
        let analyzer = SentimentAnalyzer::new();
        let result = analyzer.analyze("This is great news for BTC! 🚀");
        assert!(result.is_positive());
        assert!(result.compound > 0.3);
    }

    #[test]
    fn test_negative_sentiment() {
        let analyzer = SentimentAnalyzer::new();
        let result = analyzer.analyze("Market crash incoming, very bearish 📉");
        assert!(result.is_negative());
        assert!(result.compound < -0.3);
    }

    #[test]
    fn test_neutral_sentiment() {
        let analyzer = SentimentAnalyzer::new();
        let result = analyzer.analyze("The price is at 50000.");
        assert!(result.is_neutral());
    }

    #[test]
    fn test_crypto_terms() {
        let analyzer = SentimentAnalyzer::new();

        let bullish = analyzer.analyze("BTC mooning! Diamond hands hodl!");
        assert!(bullish.is_positive());

        let bearish = analyzer.analyze("Got rekt. Total rugpull scam.");
        assert!(bearish.is_negative());
    }

    #[test]
    fn test_emoji_sentiment() {
        let analyzer = SentimentAnalyzer::new();

        let rockets = analyzer.analyze("🚀🚀🚀");
        assert!(rockets.compound > 0.5);

        let bears = analyzer.analyze("📉📉📉");
        assert!(bears.compound < -0.3);
    }

    #[test]
    fn test_booster_words() {
        let analyzer = SentimentAnalyzer::new();

        let normal = analyzer.analyze("This is good");
        let boosted = analyzer.analyze("This is extremely good");

        assert!(boosted.compound > normal.compound);
    }

    #[test]
    fn test_negation() {
        let analyzer = SentimentAnalyzer::new();

        let positive = analyzer.analyze("This is good");
        let negated = analyzer.analyze("This is not good");

        assert!(positive.compound > 0.0);
        assert!(negated.compound < positive.compound);
    }

    #[test]
    fn test_get_label() {
        let analyzer = SentimentAnalyzer::new();

        let very_pos = SentimentResult {
            positive: 0.8,
            negative: 0.1,
            neutral: 0.1,
            compound: 0.7,
        };
        assert_eq!(analyzer.get_label(&very_pos), "very positive");

        let very_neg = SentimentResult {
            positive: 0.1,
            negative: 0.8,
            neutral: 0.1,
            compound: -0.7,
        };
        assert_eq!(analyzer.get_label(&very_neg), "very negative");
    }

    #[test]
    fn test_batch_analyze() {
        let analyzer = SentimentAnalyzer::new();
        let texts = vec!["Great news!", "Terrible crash", "Price is stable"];
        let results = analyzer.analyze_batch(&texts);
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_batch_average() {
        let analyzer = SentimentAnalyzer::new();
        let results = vec![
            SentimentResult {
                positive: 0.8,
                negative: 0.1,
                neutral: 0.1,
                compound: 0.6,
            },
            SentimentResult {
                positive: 0.2,
                negative: 0.7,
                neutral: 0.1,
                compound: -0.4,
            },
        ];
        let avg = analyzer.batch_average(&results);
        assert!((avg.compound - 0.1).abs() < 0.01);
    }

    #[test]
    fn test_sentiment_result_states() {
        let pos = SentimentResult {
            positive: 0.7,
            negative: 0.1,
            neutral: 0.2,
            compound: 0.5,
        };
        assert!(pos.is_positive());
        assert!(!pos.is_negative());
        assert!(!pos.is_neutral());

        let neg = SentimentResult {
            positive: 0.1,
            negative: 0.7,
            neutral: 0.2,
            compound: -0.5,
        };
        assert!(!neg.is_positive());
        assert!(neg.is_negative());
        assert!(!neg.is_neutral());

        let neu = SentimentResult {
            positive: 0.3,
            negative: 0.3,
            neutral: 0.4,
            compound: 0.01,
        };
        assert!(!neu.is_positive());
        assert!(!neu.is_negative());
        assert!(neu.is_neutral());
    }

    #[test]
    fn test_clean_word() {
        let analyzer = SentimentAnalyzer::new();
        assert_eq!(analyzer.clean_word("hello!"), "hello");
        assert_eq!(analyzer.clean_word("GREAT!!!"), "great");
        assert_eq!(analyzer.clean_word("don't"), "don't");
    }

    #[test]
    fn test_empty_text() {
        let analyzer = SentimentAnalyzer::new();
        let result = analyzer.analyze("");
        assert_eq!(result.compound, 0.0);
        assert_eq!(result.neutral, 1.0);
    }

    #[test]
    fn test_default_impl() {
        let analyzer = SentimentAnalyzer::default();
        let result = analyzer.analyze("good");
        assert!(result.is_positive());
    }
}
