//! Testing framework for Polymarket Bot
//!
//! Provides:
//! - Integration test harness
//! - Dry run simulation
//! - Performance benchmarks
//! - Test data generators

pub mod dry_run;
pub mod integration;
pub mod generators;
pub mod benchmarks;

pub use dry_run::{DryRunSimulator, SimulationResult, SimulatedTrade};
pub use integration::IntegrationTestHarness;
pub use generators::TestDataGenerator;
