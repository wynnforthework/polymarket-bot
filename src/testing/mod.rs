//! Testing framework for Polymarket Bot
//!
//! Provides:
//! - Integration test harness
//! - Dry run simulation
//! - Enhanced dry run with full lifecycle
//! - Optimized simulator for A/B testing
//! - Performance benchmarks
//! - Test data generators
//! - Boundary condition tests

pub mod dry_run;
pub mod integration;
pub mod generators;
pub mod benchmarks;
pub mod enhanced_dry_run;
pub mod optimized_simulator;

#[cfg(test)]
mod boundary_tests;
#[cfg(test)]
mod performance_tests;

pub use dry_run::{DryRunSimulator, SimulationResult, SimulatedTrade};
pub use integration::IntegrationTestHarness;
pub use generators::TestDataGenerator;
pub use enhanced_dry_run::{EnhancedDryRun, EnhancedDryRunConfig, EnhancedSimResult};
pub use optimized_simulator::EnhancedDryRunSimulator;
