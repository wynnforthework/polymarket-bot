//! Enhanced WebSocket with auto-reconnect and latency monitoring
//!
//! Features:
//! - Exponential backoff reconnection
//! - Heartbeat/ping-pong monitoring
//! - Latency tracking
//! - Connection state management

use crate::error::{BotError, Result};
use chrono::{DateTime, Utc};
use futures_util::{SinkExt, StreamExt};
use parking_lot::RwLock;
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::time::{interval, sleep, timeout};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn};

/// WebSocket connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ConnectionState {
    Disconnected = 0,
    Connecting = 1,
    Connected = 2,
    Reconnecting = 3,
    Failed = 4,
}

impl From<u8> for ConnectionState {
    fn from(val: u8) -> Self {
        match val {
            1 => ConnectionState::Connecting,
            2 => ConnectionState::Connected,
            3 => ConnectionState::Reconnecting,
            4 => ConnectionState::Failed,
            _ => ConnectionState::Disconnected,
        }
    }
}

/// Configuration for reconnecting WebSocket
#[derive(Debug, Clone)]
pub struct WebSocketConfig {
    /// Base URL for WebSocket connection
    pub url: String,
    /// Initial reconnect delay in milliseconds
    pub initial_reconnect_delay_ms: u64,
    /// Maximum reconnect delay in milliseconds
    pub max_reconnect_delay_ms: u64,
    /// Maximum number of reconnect attempts (0 = unlimited)
    pub max_reconnect_attempts: u32,
    /// Heartbeat interval in seconds
    pub heartbeat_interval_secs: u64,
    /// Heartbeat timeout in seconds
    pub heartbeat_timeout_secs: u64,
    /// Connection timeout in seconds
    pub connect_timeout_secs: u64,
    /// Read timeout in seconds (for detecting stale connections)
    pub read_timeout_secs: u64,
}

impl Default for WebSocketConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            initial_reconnect_delay_ms: 1000,
            max_reconnect_delay_ms: 60000,
            max_reconnect_attempts: 0, // unlimited
            heartbeat_interval_secs: 30,
            heartbeat_timeout_secs: 10,
            connect_timeout_secs: 10,
            read_timeout_secs: 90,
        }
    }
}

/// Latency statistics
#[derive(Debug, Clone, Default)]
pub struct LatencyStats {
    /// Minimum latency in microseconds
    pub min_us: u64,
    /// Maximum latency in microseconds
    pub max_us: u64,
    /// Average latency in microseconds
    pub avg_us: u64,
    /// Number of samples
    pub samples: u64,
    /// Last update time
    pub last_update: Option<DateTime<Utc>>,
}

impl LatencyStats {
    fn update(&mut self, latency_us: u64) {
        if self.samples == 0 {
            self.min_us = latency_us;
            self.max_us = latency_us;
            self.avg_us = latency_us;
        } else {
            self.min_us = self.min_us.min(latency_us);
            self.max_us = self.max_us.max(latency_us);
            // Exponential moving average
            self.avg_us = (self.avg_us * 9 + latency_us) / 10;
        }
        self.samples += 1;
        self.last_update = Some(Utc::now());
    }
}

/// Internal latency tracker (thread-safe)
struct LatencyTracker {
    min_us: AtomicU64,
    max_us: AtomicU64,
    avg_us: AtomicU64,
    samples: AtomicU64,
}

impl Default for LatencyTracker {
    fn default() -> Self {
        Self {
            min_us: AtomicU64::new(u64::MAX),
            max_us: AtomicU64::new(0),
            avg_us: AtomicU64::new(0),
            samples: AtomicU64::new(0),
        }
    }
}

impl LatencyTracker {
    fn update(&self, latency_us: u64) {
        // Update min (compare and swap loop)
        loop {
            let current = self.min_us.load(Ordering::Relaxed);
            if latency_us >= current {
                break;
            }
            if self.min_us.compare_exchange_weak(
                current, latency_us, Ordering::Relaxed, Ordering::Relaxed
            ).is_ok() {
                break;
            }
        }
        
        // Update max
        loop {
            let current = self.max_us.load(Ordering::Relaxed);
            if latency_us <= current {
                break;
            }
            if self.max_us.compare_exchange_weak(
                current, latency_us, Ordering::Relaxed, Ordering::Relaxed
            ).is_ok() {
                break;
            }
        }
        
        // EMA for average
        let samples = self.samples.fetch_add(1, Ordering::Relaxed);
        if samples == 0 {
            self.avg_us.store(latency_us, Ordering::Relaxed);
        } else {
            let current_avg = self.avg_us.load(Ordering::Relaxed);
            let new_avg = (current_avg * 9 + latency_us) / 10;
            self.avg_us.store(new_avg, Ordering::Relaxed);
        }
    }
    
    fn snapshot(&self) -> LatencyStats {
        let samples = self.samples.load(Ordering::Relaxed);
        if samples == 0 {
            return LatencyStats::default();
        }
        LatencyStats {
            min_us: self.min_us.load(Ordering::Relaxed),
            max_us: self.max_us.load(Ordering::Relaxed),
            avg_us: self.avg_us.load(Ordering::Relaxed),
            samples,
            last_update: Some(Utc::now()),
        }
    }
}

/// Reconnecting WebSocket client
pub struct ReconnectingWebSocket {
    config: WebSocketConfig,
    state: Arc<AtomicU8>,
    latency: Arc<LatencyTracker>,
    reconnect_count: Arc<AtomicU64>,
    last_message_time: Arc<RwLock<Option<Instant>>>,
    shutdown_tx: Option<broadcast::Sender<()>>,
}

impl ReconnectingWebSocket {
    /// Create a new reconnecting WebSocket
    pub fn new(config: WebSocketConfig) -> Self {
        Self {
            config,
            state: Arc::new(AtomicU8::new(ConnectionState::Disconnected as u8)),
            latency: Arc::new(LatencyTracker::default()),
            reconnect_count: Arc::new(AtomicU64::new(0)),
            last_message_time: Arc::new(RwLock::new(None)),
            shutdown_tx: None,
        }
    }

    /// Get current connection state
    pub fn state(&self) -> ConnectionState {
        ConnectionState::from(self.state.load(Ordering::Relaxed))
    }

    /// Get latency statistics
    pub fn latency_stats(&self) -> LatencyStats {
        self.latency.snapshot()
    }

    /// Get number of reconnections
    pub fn reconnect_count(&self) -> u64 {
        self.reconnect_count.load(Ordering::Relaxed)
    }

    /// Check if connected
    pub fn is_connected(&self) -> bool {
        self.state() == ConnectionState::Connected
    }

    /// Connect and start receiving messages
    pub async fn connect(
        &mut self,
        subscribe_msg: serde_json::Value,
    ) -> Result<mpsc::Receiver<String>> {
        let (tx, rx) = mpsc::channel(10000);
        let (shutdown_tx, _) = broadcast::channel(1);
        self.shutdown_tx = Some(shutdown_tx.clone());

        let config = self.config.clone();
        let state = Arc::clone(&self.state);
        let latency = Arc::clone(&self.latency);
        let reconnect_count = Arc::clone(&self.reconnect_count);
        let last_message_time = Arc::clone(&self.last_message_time);

        tokio::spawn(async move {
            Self::connection_loop(
                config,
                subscribe_msg,
                tx,
                shutdown_tx,
                state,
                latency,
                reconnect_count,
                last_message_time,
            )
            .await;
        });

        // Wait for initial connection
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(self.config.connect_timeout_secs) {
            if self.is_connected() {
                return Ok(rx);
            }
            if self.state() == ConnectionState::Failed {
                return Err(BotError::WebSocket("Failed to connect".to_string()));
            }
            sleep(Duration::from_millis(100)).await;
        }

        Err(BotError::WebSocket("Connection timeout".to_string()))
    }

    /// Shutdown the WebSocket
    pub fn shutdown(&self) {
        if let Some(tx) = &self.shutdown_tx {
            let _ = tx.send(());
        }
        self.state.store(ConnectionState::Disconnected as u8, Ordering::Relaxed);
    }

    async fn connection_loop(
        config: WebSocketConfig,
        subscribe_msg: serde_json::Value,
        tx: mpsc::Sender<String>,
        shutdown_tx: broadcast::Sender<()>,
        state: Arc<AtomicU8>,
        latency: Arc<LatencyTracker>,
        reconnect_count: Arc<AtomicU64>,
        last_message_time: Arc<RwLock<Option<Instant>>>,
    ) {
        let mut shutdown_rx = shutdown_tx.subscribe();
        let mut attempt = 0u32;
        let mut delay_ms = config.initial_reconnect_delay_ms;

        loop {
            // Check for shutdown
            if shutdown_rx.try_recv().is_ok() {
                info!("WebSocket shutdown requested");
                break;
            }

            // Attempt connection
            state.store(ConnectionState::Connecting as u8, Ordering::Relaxed);
            info!("Connecting to WebSocket: {} (attempt {})", config.url, attempt + 1);

            match Self::connect_once(
                &config,
                &subscribe_msg,
                &tx,
                &mut shutdown_rx,
                &latency,
                &last_message_time,
            )
            .await
            {
                Ok(()) => {
                    // Normal disconnect, reset backoff
                    attempt = 0;
                    delay_ms = config.initial_reconnect_delay_ms;
                }
                Err(e) => {
                    error!("WebSocket error: {}", e);
                    attempt += 1;
                    
                    // Check max attempts
                    if config.max_reconnect_attempts > 0 && attempt >= config.max_reconnect_attempts {
                        error!("Max reconnect attempts reached");
                        state.store(ConnectionState::Failed as u8, Ordering::Relaxed);
                        break;
                    }
                }
            }

            // Reconnect with backoff
            state.store(ConnectionState::Reconnecting as u8, Ordering::Relaxed);
            reconnect_count.fetch_add(1, Ordering::Relaxed);
            
            info!("Reconnecting in {}ms...", delay_ms);
            tokio::select! {
                _ = sleep(Duration::from_millis(delay_ms)) => {}
                _ = shutdown_rx.recv() => {
                    info!("WebSocket shutdown during backoff");
                    break;
                }
            }

            // Exponential backoff
            delay_ms = (delay_ms * 2).min(config.max_reconnect_delay_ms);
        }

        state.store(ConnectionState::Disconnected as u8, Ordering::Relaxed);
    }

    async fn connect_once(
        config: &WebSocketConfig,
        subscribe_msg: &serde_json::Value,
        tx: &mpsc::Sender<String>,
        shutdown_rx: &mut broadcast::Receiver<()>,
        latency: &Arc<LatencyTracker>,
        last_message_time: &Arc<RwLock<Option<Instant>>>,
    ) -> Result<()> {
        // Connect with timeout
        let ws_url = config.url.replace("https://", "wss://").replace("http://", "ws://");
        
        let connect_result = timeout(
            Duration::from_secs(config.connect_timeout_secs),
            connect_async(&ws_url),
        )
        .await
        .map_err(|_| BotError::WebSocket("Connection timeout".to_string()))?
        .map_err(|e| BotError::WebSocket(e.to_string()))?;

        let (mut write, mut read) = connect_result.0.split();

        // Send subscribe message
        write
            .send(Message::Text(subscribe_msg.to_string().into()))
            .await
            .map_err(|e| BotError::WebSocket(e.to_string()))?;

        info!("WebSocket connected and subscribed");

        // Start heartbeat task
        let (ping_tx, mut ping_rx) = mpsc::channel::<oneshot::Sender<()>>(1);
        let heartbeat_interval = Duration::from_secs(config.heartbeat_interval_secs);
        let heartbeat_timeout = Duration::from_secs(config.heartbeat_timeout_secs);
        
        let latency_clone = Arc::clone(latency);
        tokio::spawn(async move {
            let mut interval = interval(heartbeat_interval);
            loop {
                interval.tick().await;
                let (response_tx, response_rx) = oneshot::channel();
                if ping_tx.send(response_tx).await.is_err() {
                    break;
                }
                
                let ping_start = Instant::now();
                match timeout(heartbeat_timeout, response_rx).await {
                    Ok(Ok(())) => {
                        let latency_us = ping_start.elapsed().as_micros() as u64;
                        latency_clone.update(latency_us);
                        debug!("Heartbeat latency: {}us", latency_us);
                    }
                    _ => {
                        warn!("Heartbeat timeout");
                        break;
                    }
                }
            }
        });

        let read_timeout = Duration::from_secs(config.read_timeout_secs);
        let mut pending_pong: Option<oneshot::Sender<()>> = None;

        loop {
            tokio::select! {
                // Check for shutdown
                _ = shutdown_rx.recv() => {
                    info!("Shutdown during read");
                    return Ok(());
                }
                
                // Check for ping request
                pong_tx = ping_rx.recv() => {
                    if let Some(pong_tx) = pong_tx {
                        // Send ping
                        if write.send(Message::Ping(vec![].into())).await.is_err() {
                            warn!("Failed to send ping");
                            return Err(BotError::WebSocket("Ping failed".to_string()));
                        }
                        pending_pong = Some(pong_tx);
                    } else {
                        // Ping channel closed, heartbeat failed
                        return Err(BotError::WebSocket("Heartbeat failed".to_string()));
                    }
                }
                
                // Read messages with timeout
                msg = timeout(read_timeout, read.next()) => {
                    match msg {
                        Ok(Some(Ok(msg))) => {
                            // Update last message time
                            *last_message_time.write() = Some(Instant::now());
                            
                            match msg {
                                Message::Text(text) => {
                                    if tx.send(text.to_string()).await.is_err() {
                                        info!("Message channel closed");
                                        return Ok(());
                                    }
                                }
                                Message::Pong(_) => {
                                    // Complete pending pong
                                    if let Some(pong_tx) = pending_pong.take() {
                                        let _ = pong_tx.send(());
                                    }
                                }
                                Message::Ping(data) => {
                                    // Respond to ping
                                    let _ = write.send(Message::Pong(data)).await;
                                }
                                Message::Close(_) => {
                                    info!("Server closed connection");
                                    return Ok(());
                                }
                                _ => {}
                            }
                        }
                        Ok(Some(Err(e))) => {
                            return Err(BotError::WebSocket(e.to_string()));
                        }
                        Ok(None) => {
                            return Ok(()); // Stream ended
                        }
                        Err(_) => {
                            warn!("Read timeout - connection stale");
                            return Err(BotError::WebSocket("Read timeout".to_string()));
                        }
                    }
                }
            }
        }
    }
}

impl Drop for ReconnectingWebSocket {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_state_conversion() {
        assert_eq!(ConnectionState::from(0), ConnectionState::Disconnected);
        assert_eq!(ConnectionState::from(1), ConnectionState::Connecting);
        assert_eq!(ConnectionState::from(2), ConnectionState::Connected);
        assert_eq!(ConnectionState::from(3), ConnectionState::Reconnecting);
        assert_eq!(ConnectionState::from(4), ConnectionState::Failed);
        assert_eq!(ConnectionState::from(255), ConnectionState::Disconnected);
    }

    #[test]
    fn test_websocket_config_default() {
        let config = WebSocketConfig::default();
        assert_eq!(config.initial_reconnect_delay_ms, 1000);
        assert_eq!(config.max_reconnect_delay_ms, 60000);
        assert_eq!(config.heartbeat_interval_secs, 30);
    }

    #[test]
    fn test_latency_stats_update() {
        let mut stats = LatencyStats::default();
        assert_eq!(stats.samples, 0);
        
        stats.update(1000);
        assert_eq!(stats.min_us, 1000);
        assert_eq!(stats.max_us, 1000);
        assert_eq!(stats.avg_us, 1000);
        assert_eq!(stats.samples, 1);
        
        stats.update(2000);
        assert_eq!(stats.min_us, 1000);
        assert_eq!(stats.max_us, 2000);
        assert_eq!(stats.samples, 2);
    }

    #[test]
    fn test_latency_tracker() {
        let tracker = LatencyTracker::default();
        
        tracker.update(1000);
        tracker.update(500);
        tracker.update(2000);
        
        let stats = tracker.snapshot();
        assert_eq!(stats.min_us, 500);
        assert_eq!(stats.max_us, 2000);
        assert_eq!(stats.samples, 3);
    }

    #[test]
    fn test_reconnecting_websocket_initial_state() {
        let config = WebSocketConfig {
            url: "wss://test.com".to_string(),
            ..Default::default()
        };
        let ws = ReconnectingWebSocket::new(config);
        
        assert_eq!(ws.state(), ConnectionState::Disconnected);
        assert_eq!(ws.reconnect_count(), 0);
        assert!(!ws.is_connected());
    }

    #[test]
    fn test_latency_stats_ema() {
        let mut stats = LatencyStats::default();
        
        // First sample sets the average
        stats.update(1000);
        assert_eq!(stats.avg_us, 1000);
        
        // Subsequent samples use EMA
        stats.update(2000);
        // avg = (1000 * 9 + 2000) / 10 = 1100
        assert_eq!(stats.avg_us, 1100);
    }
}
