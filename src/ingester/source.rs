//! Common signal source utilities

use super::{IngesterConfig, RawSignal, SignalSource};
use crate::error::Result;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Multi-source aggregator
pub struct SourceAggregator {
    sources: Vec<Arc<dyn SignalSource>>,
    config: IngesterConfig,
}

impl SourceAggregator {
    pub fn new(config: IngesterConfig) -> Self {
        Self {
            sources: Vec::new(),
            config,
        }
    }

    pub fn add_source(&mut self, source: Arc<dyn SignalSource>) {
        self.sources.push(source);
    }

    /// Get author trust score
    pub fn get_author_trust(&self, author: &str) -> f64 {
        self.config
            .author_trust
            .get(author)
            .copied()
            .unwrap_or(0.3) // Default low trust for unknown authors
    }

    /// Run all sources concurrently
    pub async fn run(&self, tx: mpsc::Sender<RawSignal>) -> Result<()> {
        let mut handles = Vec::new();

        for source in &self.sources {
            let source = Arc::clone(source);
            let tx = tx.clone();
            
            let handle = tokio::spawn(async move {
                if let Err(e) = source.run(tx).await {
                    tracing::error!("Source {} error: {}", source.name(), e);
                }
            });
            
            handles.push(handle);
        }

        // Wait for all sources (they should run forever)
        for handle in handles {
            let _ = handle.await;
        }

        Ok(())
    }
}
