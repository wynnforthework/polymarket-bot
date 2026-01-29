//! Test Data Generators
//!
//! Utilities for generating test data

use crate::types::{Market, Outcome, Signal, Side, Order, OrderType, Trade};
use chrono::{Utc, Duration};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

/// Generator for test data
pub struct TestDataGenerator {
    counter: u32,
}

impl TestDataGenerator {
    pub fn new() -> Self {
        Self { counter: 0 }
    }

    /// Generate a random market
    pub fn market(&mut self) -> Market {
        self.counter += 1;
        let yes_price = self.random_price();
        let no_price = Decimal::ONE - yes_price;
        
        Market {
            id: format!("market_{}", self.counter),
            question: format!("Test question #{}?", self.counter),
            description: Some(format!("Test market description #{}", self.counter)),
            end_date: Some(Utc::now() + Duration::days(30)),
            volume: self.random_volume(),
            liquidity: self.random_liquidity(),
            outcomes: vec![
                Outcome {
                    token_id: format!("yes_{}", self.counter),
                    outcome: "Yes".to_string(),
                    price: yes_price,
                },
                Outcome {
                    token_id: format!("no_{}", self.counter),
                    outcome: "No".to_string(),
                    price: no_price,
                },
            ],
            active: true,
            closed: false,
        }
    }

    /// Generate a crypto market
    pub fn crypto_market(&mut self, coin: &str) -> Market {
        self.counter += 1;
        Market {
            id: format!("{}_up_{}", coin.to_lowercase(), self.counter),
            question: format!("Will {} go up in the next 24 hours?", coin),
            description: Some(format!("{} daily price movement market", coin)),
            end_date: Some(Utc::now() + Duration::hours(24)),
            volume: dec!(50000),
            liquidity: dec!(20000),
            outcomes: vec![
                Outcome {
                    token_id: format!("{}_yes_{}", coin.to_lowercase(), self.counter),
                    outcome: "Yes".to_string(),
                    price: dec!(0.52),
                },
                Outcome {
                    token_id: format!("{}_no_{}", coin.to_lowercase(), self.counter),
                    outcome: "No".to_string(),
                    price: dec!(0.48),
                },
            ],
            active: true,
            closed: false,
        }
    }

    /// Generate multiple markets
    pub fn markets(&mut self, count: usize) -> Vec<Market> {
        (0..count).map(|_| self.market()).collect()
    }

    /// Generate a signal
    pub fn signal(&mut self, side: Side) -> Signal {
        self.counter += 1;
        let model_prob = self.random_price();
        let market_prob = model_prob - dec!(0.08);
        
        Signal {
            market_id: format!("market_{}", self.counter),
            token_id: format!("token_{}", self.counter),
            side,
            model_probability: model_prob,
            market_probability: market_prob,
            edge: model_prob - market_prob,
            confidence: dec!(0.75),
            suggested_size: dec!(0.03),
            timestamp: Utc::now(),
        }
    }

    /// Generate an order
    pub fn order(&mut self, side: Side) -> Order {
        self.counter += 1;
        Order {
            token_id: format!("token_{}", self.counter),
            side,
            price: self.random_price(),
            size: Decimal::from(10 + (self.counter % 100)),
            order_type: OrderType::GTC,
        }
    }

    /// Generate a trade
    pub fn trade(&mut self, side: Side) -> Trade {
        self.counter += 1;
        Trade {
            id: format!("trade_{}", self.counter),
            order_id: format!("order_{}", self.counter),
            token_id: format!("token_{}", self.counter),
            market_id: format!("market_{}", self.counter),
            side,
            price: self.random_price(),
            size: Decimal::from(50),
            fee: dec!(0.50),
            timestamp: Utc::now(),
        }
    }

    fn random_price(&self) -> Decimal {
        let base = (self.counter * 17 + 31) % 80 + 10;
        Decimal::new(base as i64, 2)
    }

    fn random_volume(&self) -> Decimal {
        Decimal::from((self.counter * 12345) % 1000000 + 10000)
    }

    fn random_liquidity(&self) -> Decimal {
        Decimal::from((self.counter * 6789) % 100000 + 5000)
    }
}

impl Default for TestDataGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_market_generation() {
        let mut gen = TestDataGenerator::new();
        let market = gen.market();
        assert!(!market.id.is_empty());
        assert!(!market.question.is_empty());
        assert_eq!(market.outcomes.len(), 2);
    }

    #[test]
    fn test_multiple_markets() {
        let mut gen = TestDataGenerator::new();
        let markets = gen.markets(5);
        assert_eq!(markets.len(), 5);
        
        // All IDs should be unique
        let ids: Vec<_> = markets.iter().map(|m| &m.id).collect();
        let unique_ids: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(ids.len(), unique_ids.len());
    }

    #[test]
    fn test_signal_generation() {
        let mut gen = TestDataGenerator::new();
        let signal = gen.signal(Side::Buy);
        assert!(signal.edge > Decimal::ZERO);
        assert_eq!(signal.side, Side::Buy);
    }

    #[test]
    fn test_order_generation() {
        let mut gen = TestDataGenerator::new();
        let order = gen.order(Side::Sell);
        assert_eq!(order.side, Side::Sell);
        assert!(order.price > Decimal::ZERO);
    }

    #[test]
    fn test_crypto_market() {
        let mut gen = TestDataGenerator::new();
        let market = gen.crypto_market("BTC");
        assert!(market.question.contains("BTC"));
        assert!(market.id.contains("btc"));
    }
}
