//! Performance Benchmarks
//!
//! Measures execution times for critical paths

use crate::client::mock::{MockClobClient, MockGammaClient, ClobClientTrait, GammaClientTrait};
use crate::strategy::SignalGenerator;
use crate::config::{StrategyConfig, RiskConfig};
use crate::types::{Order, OrderType, Side};
use crate::testing::generators::TestDataGenerator;
use rust_decimal_macros::dec;
use std::time::{Duration, Instant};
use serde::{Serialize, Deserialize};

/// Benchmark result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub name: String,
    pub iterations: u32,
    pub total_time_ms: u128,
    pub avg_time_ms: f64,
    pub min_time_ms: u128,
    pub max_time_ms: u128,
    pub ops_per_sec: f64,
}

/// Performance benchmark suite
pub struct BenchmarkSuite {
    results: Vec<BenchmarkResult>,
}

impl BenchmarkSuite {
    pub fn new() -> Self {
        Self { results: Vec::new() }
    }

    /// Run all benchmarks
    pub async fn run_all(&mut self) -> Vec<BenchmarkResult> {
        self.results.clear();
        
        self.bench_order_placement().await;
        self.bench_market_fetch().await;
        self.bench_signal_generation().await;
        self.bench_market_analysis().await;
        
        self.results.clone()
    }

    async fn bench_order_placement(&mut self) {
        let clob = MockClobClient::new().with_balance(dec!(1000000));
        let mut times = Vec::new();
        let iterations = 100;

        for i in 0..iterations {
            let order = Order {
                token_id: format!("token_{}", i),
                side: Side::Buy,
                price: dec!(0.50),
                size: dec!(10),
                order_type: OrderType::GTC,
            };

            let start = Instant::now();
            let _ = clob.place_order(&order).await;
            times.push(start.elapsed());
        }

        self.record_result("Order Placement", iterations, times);
    }

    async fn bench_market_fetch(&mut self) {
        let gamma = MockGammaClient::new();
        let mut times = Vec::new();
        let iterations = 50;

        for _ in 0..iterations {
            let start = Instant::now();
            let _ = gamma.get_top_markets(20).await;
            times.push(start.elapsed());
        }

        self.record_result("Market Fetch", iterations, times);
    }

    async fn bench_signal_generation(&mut self) {
        let signal_gen = SignalGenerator::new(StrategyConfig::default(), RiskConfig::default());
        let mut gen = TestDataGenerator::new();
        let mut times = Vec::new();
        let iterations = 100;

        let markets: Vec<_> = (0..iterations).map(|_| gen.market()).collect();
        let prediction = crate::model::Prediction {
            probability: dec!(0.65),
            confidence: dec!(0.80),
            reasoning: "Benchmark".to_string(),
        };

        for market in &markets {
            let start = Instant::now();
            let _ = signal_gen.generate(market, &prediction);
            times.push(start.elapsed());
        }

        self.record_result("Signal Generation", iterations, times);
    }

    async fn bench_market_analysis(&mut self) {
        let gamma = MockGammaClient::new();
        let signal_gen = SignalGenerator::new(StrategyConfig::default(), RiskConfig::default());
        let mut times = Vec::new();
        let iterations = 20;

        let prediction = crate::model::Prediction {
            probability: dec!(0.60),
            confidence: dec!(0.75),
            reasoning: "Analysis".to_string(),
        };

        for _ in 0..iterations {
            let start = Instant::now();
            
            // Simulate full market scan
            if let Ok(markets) = gamma.get_top_markets(20).await {
                for market in markets {
                    let _ = signal_gen.generate(&market, &prediction);
                }
            }
            
            times.push(start.elapsed());
        }

        self.record_result("Full Market Scan", iterations, times);
    }

    fn record_result(&mut self, name: &str, iterations: u32, times: Vec<Duration>) {
        let total_ms: u128 = times.iter().map(|t| t.as_millis()).sum();
        let min_ms = times.iter().map(|t| t.as_millis()).min().unwrap_or(0);
        let max_ms = times.iter().map(|t| t.as_millis()).max().unwrap_or(0);
        let avg_ms = total_ms as f64 / iterations as f64;
        let ops_per_sec = if avg_ms > 0.0 { 1000.0 / avg_ms } else { 0.0 };

        self.results.push(BenchmarkResult {
            name: name.to_string(),
            iterations,
            total_time_ms: total_ms,
            avg_time_ms: avg_ms,
            min_time_ms: min_ms,
            max_time_ms: max_ms,
            ops_per_sec,
        });
    }

    /// Generate benchmark report
    pub fn generate_report(&self) -> String {
        let mut report = String::new();
        
        report.push_str("# ⚡ Performance Benchmark Report\n\n");
        
        report.push_str("| Benchmark | Iterations | Avg (ms) | Min (ms) | Max (ms) | Ops/sec |\n");
        report.push_str("|-----------|------------|----------|----------|----------|----------|\n");
        
        for result in &self.results {
            report.push_str(&format!(
                "| {} | {} | {:.2} | {} | {} | {:.0} |\n",
                result.name,
                result.iterations,
                result.avg_time_ms,
                result.min_time_ms,
                result.max_time_ms,
                result.ops_per_sec
            ));
        }
        
        report.push_str("\n## Analysis\n\n");
        
        // Find slowest operation
        if let Some(slowest) = self.results.iter().max_by(|a, b| {
            a.avg_time_ms.partial_cmp(&b.avg_time_ms).unwrap_or(std::cmp::Ordering::Equal)
        }) {
            report.push_str(&format!("- **Slowest operation**: {} ({:.2}ms avg)\n", slowest.name, slowest.avg_time_ms));
        }
        
        // Find fastest operation
        if let Some(fastest) = self.results.iter().min_by(|a, b| {
            a.avg_time_ms.partial_cmp(&b.avg_time_ms).unwrap_or(std::cmp::Ordering::Equal)
        }) {
            report.push_str(&format!("- **Fastest operation**: {} ({:.2}ms avg)\n", fastest.name, fastest.avg_time_ms));
        }

        // Calculate total throughput potential
        let total_time: f64 = self.results.iter().map(|r| r.avg_time_ms).sum();
        if total_time > 0.0 {
            report.push_str(&format!("- **Estimated cycle time**: {:.2}ms\n", total_time));
            report.push_str(&format!("- **Max theoretical cycles/sec**: {:.1}\n", 1000.0 / total_time));
        }
        
        report
    }
}

impl Default for BenchmarkSuite {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_benchmark_suite() {
        let mut suite = BenchmarkSuite::new();
        let results = suite.run_all().await;
        assert!(!results.is_empty());
        
        // All benchmarks should complete
        for result in &results {
            assert!(result.iterations > 0);
            assert!(result.total_time_ms >= 0);
        }
    }

    #[tokio::test]
    async fn test_report_generation() {
        let mut suite = BenchmarkSuite::new();
        suite.run_all().await;
        let report = suite.generate_report();
        assert!(report.contains("Performance Benchmark Report"));
        assert!(report.contains("Ops/sec"));
    }
}
