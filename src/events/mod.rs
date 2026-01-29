//! Event-Driven Architecture (EDA) Module
//!
//! Professional-grade event-driven trading system architecture.
//! All top-tier quantitative trading systems are event-driven for:
//! - Minimal latency (react immediately to market events)
//! - Clean separation of concerns (handlers focus on single responsibility)
//! - Easy backtesting (replay events through same handlers)
//! - Fault tolerance (event sourcing enables recovery)
//!
//! # Architecture
//! ```text
//! MarketData -> EventBus -> [Handlers] -> Orders -> Exchange
//!     ^                         |
//!     |                         v
//! WebSocket             Signal/Risk/Execution
//! ```
//!
//! # Event Types
//! - `MarketDataEvent`: Price updates, order book changes
//! - `SignalEvent`: Trading signals from strategies
//! - `OrderEvent`: Order creation, modification, cancellation
//! - `FillEvent`: Trade executions
//! - `TimerEvent`: Scheduled tasks
//! - `RiskEvent`: Risk limit breaches, margin calls
//! - `SystemEvent`: Start, stop, heartbeat

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, RwLock};
use uuid::Uuid;

/// Event priority levels for ordering
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum EventPriority {
    /// Lowest priority (logging, metrics)
    Low = 0,
    /// Normal priority (signals, analysis)
    Normal = 1,
    /// High priority (order updates)
    High = 2,
    /// Critical priority (risk events, fills)
    Critical = 3,
    /// System priority (shutdown, emergency)
    System = 4,
}

impl Default for EventPriority {
    fn default() -> Self {
        Self::Normal
    }
}

/// Core event types in the trading system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventType {
    /// Market data update (price, volume, order book)
    MarketData,
    /// Trading signal generated
    Signal,
    /// Order lifecycle event
    Order,
    /// Trade execution / fill
    Fill,
    /// Timer / scheduled event
    Timer,
    /// Risk management event
    Risk,
    /// System lifecycle event
    System,
    /// Custom user-defined event
    Custom(String),
}

/// Base event wrapper with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// Unique event identifier
    pub id: String,
    /// Event type classification
    pub event_type: EventType,
    /// Event creation timestamp
    pub timestamp: DateTime<Utc>,
    /// Event priority for processing order
    pub priority: EventPriority,
    /// Source system/component
    pub source: String,
    /// Correlation ID for tracing event chains
    pub correlation_id: Option<String>,
    /// Causation ID (parent event that caused this)
    pub causation_id: Option<String>,
    /// Event payload
    pub payload: EventPayload,
    /// Custom metadata
    pub metadata: HashMap<String, String>,
}

impl Event {
    /// Create a new event with auto-generated ID and timestamp
    pub fn new(event_type: EventType, source: &str, payload: EventPayload) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            event_type,
            timestamp: Utc::now(),
            priority: EventPriority::Normal,
            source: source.to_string(),
            correlation_id: None,
            causation_id: None,
            payload,
            metadata: HashMap::new(),
        }
    }

    /// Set event priority
    pub fn with_priority(mut self, priority: EventPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Set correlation ID for tracing
    pub fn with_correlation_id(mut self, id: &str) -> Self {
        self.correlation_id = Some(id.to_string());
        self
    }

    /// Set causation ID (parent event)
    pub fn with_causation_id(mut self, id: &str) -> Self {
        self.causation_id = Some(id.to_string());
        self
    }

    /// Add metadata key-value pair
    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }

    /// Create a child event caused by this event
    pub fn create_child(&self, event_type: EventType, source: &str, payload: EventPayload) -> Self {
        let mut child = Event::new(event_type, source, payload);
        child.correlation_id = self.correlation_id.clone().or_else(|| Some(self.id.clone()));
        child.causation_id = Some(self.id.clone());
        child
    }
}

/// Event payloads for different event types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventPayload {
    /// Market data update
    MarketData(MarketDataPayload),
    /// Trading signal
    Signal(SignalPayload),
    /// Order event
    Order(OrderPayload),
    /// Fill / execution event
    Fill(FillPayload),
    /// Timer event
    Timer(TimerPayload),
    /// Risk event
    Risk(RiskPayload),
    /// System event
    System(SystemPayload),
    /// Custom JSON payload
    Custom(serde_json::Value),
}

/// Market data event payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketDataPayload {
    /// Market/symbol identifier
    pub symbol: String,
    /// Best bid price
    pub bid: Option<Decimal>,
    /// Best ask price
    pub ask: Option<Decimal>,
    /// Last trade price
    pub last: Option<Decimal>,
    /// 24h volume
    pub volume: Option<Decimal>,
    /// Order book snapshot (price -> size)
    pub bids: Vec<(Decimal, Decimal)>,
    /// Order book asks
    pub asks: Vec<(Decimal, Decimal)>,
    /// Data source (binance, polymarket, etc.)
    pub source: String,
    /// Exchange timestamp
    pub exchange_timestamp: Option<DateTime<Utc>>,
}

impl MarketDataPayload {
    /// Create a simple price update
    pub fn price_update(symbol: &str, bid: Decimal, ask: Decimal, source: &str) -> Self {
        Self {
            symbol: symbol.to_string(),
            bid: Some(bid),
            ask: Some(ask),
            last: None,
            volume: None,
            bids: Vec::new(),
            asks: Vec::new(),
            source: source.to_string(),
            exchange_timestamp: None,
        }
    }

    /// Create an order book update
    pub fn orderbook_update(
        symbol: &str,
        bids: Vec<(Decimal, Decimal)>,
        asks: Vec<(Decimal, Decimal)>,
        source: &str,
    ) -> Self {
        let best_bid = bids.first().map(|(p, _)| *p);
        let best_ask = asks.first().map(|(p, _)| *p);
        Self {
            symbol: symbol.to_string(),
            bid: best_bid,
            ask: best_ask,
            last: None,
            volume: None,
            bids,
            asks,
            source: source.to_string(),
            exchange_timestamp: None,
        }
    }

    /// Get mid price
    pub fn mid_price(&self) -> Option<Decimal> {
        match (self.bid, self.ask) {
            (Some(b), Some(a)) => Some((b + a) / Decimal::TWO),
            _ => self.last,
        }
    }

    /// Get spread
    pub fn spread(&self) -> Option<Decimal> {
        match (self.bid, self.ask) {
            (Some(b), Some(a)) => Some(a - b),
            _ => None,
        }
    }
}

/// Signal event payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalPayload {
    /// Market/symbol identifier
    pub symbol: String,
    /// Signal direction (1 = long, -1 = short, 0 = flat)
    pub direction: i32,
    /// Signal strength (0.0 to 1.0)
    pub strength: Decimal,
    /// Expected edge / alpha
    pub edge: Decimal,
    /// Confidence level (0.0 to 1.0)
    pub confidence: Decimal,
    /// Strategy that generated the signal
    pub strategy: String,
    /// Signal features / factors
    pub features: HashMap<String, Decimal>,
    /// Time-to-live in seconds (signal validity)
    pub ttl_seconds: Option<u64>,
    /// Target price (if applicable)
    pub target_price: Option<Decimal>,
    /// Stop loss price (if applicable)
    pub stop_loss: Option<Decimal>,
}

impl SignalPayload {
    /// Create a new signal
    pub fn new(symbol: &str, direction: i32, strength: Decimal, strategy: &str) -> Self {
        Self {
            symbol: symbol.to_string(),
            direction,
            strength,
            edge: Decimal::ZERO,
            confidence: Decimal::ZERO,
            strategy: strategy.to_string(),
            features: HashMap::new(),
            ttl_seconds: None,
            target_price: None,
            stop_loss: None,
        }
    }

    /// Set edge and confidence
    pub fn with_edge(mut self, edge: Decimal, confidence: Decimal) -> Self {
        self.edge = edge;
        self.confidence = confidence;
        self
    }

    /// Add feature
    pub fn with_feature(mut self, name: &str, value: Decimal) -> Self {
        self.features.insert(name.to_string(), value);
        self
    }

    /// Check if signal is tradeable
    pub fn is_tradeable(&self, min_edge: Decimal, min_confidence: Decimal) -> bool {
        self.direction != 0 && self.edge >= min_edge && self.confidence >= min_confidence
    }
}

/// Order event payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderPayload {
    /// Order ID (internal)
    pub order_id: String,
    /// Exchange order ID (if submitted)
    pub exchange_order_id: Option<String>,
    /// Market/symbol
    pub symbol: String,
    /// Order side (buy/sell)
    pub side: OrderSide,
    /// Order type
    pub order_type: OrderType,
    /// Order quantity
    pub quantity: Decimal,
    /// Limit price (for limit orders)
    pub price: Option<Decimal>,
    /// Order status
    pub status: OrderStatus,
    /// Filled quantity
    pub filled_quantity: Decimal,
    /// Average fill price
    pub average_price: Option<Decimal>,
    /// Remaining quantity
    pub remaining_quantity: Decimal,
    /// Time in force
    pub time_in_force: TimeInForce,
    /// Order creation time
    pub created_at: DateTime<Utc>,
    /// Last update time
    pub updated_at: DateTime<Utc>,
    /// Rejection reason (if rejected)
    pub reject_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderType {
    Market,
    Limit,
    StopMarket,
    StopLimit,
    TrailingStop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderStatus {
    Pending,
    Submitted,
    PartiallyFilled,
    Filled,
    Cancelled,
    Rejected,
    Expired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeInForce {
    GTC, // Good Till Cancelled
    IOC, // Immediate Or Cancel
    FOK, // Fill Or Kill
    GTD, // Good Till Date
}

impl OrderPayload {
    /// Create a new order
    pub fn new(symbol: &str, side: OrderSide, order_type: OrderType, quantity: Decimal) -> Self {
        let now = Utc::now();
        Self {
            order_id: Uuid::new_v4().to_string(),
            exchange_order_id: None,
            symbol: symbol.to_string(),
            side,
            order_type,
            quantity,
            price: None,
            status: OrderStatus::Pending,
            filled_quantity: Decimal::ZERO,
            average_price: None,
            remaining_quantity: quantity,
            time_in_force: TimeInForce::GTC,
            created_at: now,
            updated_at: now,
            reject_reason: None,
        }
    }

    /// Set limit price
    pub fn with_price(mut self, price: Decimal) -> Self {
        self.price = Some(price);
        self
    }

    /// Check if order is active
    pub fn is_active(&self) -> bool {
        matches!(
            self.status,
            OrderStatus::Pending | OrderStatus::Submitted | OrderStatus::PartiallyFilled
        )
    }

    /// Check if order is terminal
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.status,
            OrderStatus::Filled | OrderStatus::Cancelled | OrderStatus::Rejected | OrderStatus::Expired
        )
    }
}

/// Fill event payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FillPayload {
    /// Fill ID
    pub fill_id: String,
    /// Associated order ID
    pub order_id: String,
    /// Exchange trade ID
    pub exchange_trade_id: Option<String>,
    /// Market/symbol
    pub symbol: String,
    /// Fill side
    pub side: OrderSide,
    /// Fill quantity
    pub quantity: Decimal,
    /// Fill price
    pub price: Decimal,
    /// Commission paid
    pub commission: Decimal,
    /// Commission asset
    pub commission_asset: String,
    /// Execution timestamp
    pub executed_at: DateTime<Utc>,
    /// Is this fill the maker side
    pub is_maker: bool,
    /// Realized P&L (if closing position)
    pub realized_pnl: Option<Decimal>,
}

impl FillPayload {
    /// Create a new fill
    pub fn new(
        order_id: &str,
        symbol: &str,
        side: OrderSide,
        quantity: Decimal,
        price: Decimal,
    ) -> Self {
        Self {
            fill_id: Uuid::new_v4().to_string(),
            order_id: order_id.to_string(),
            exchange_trade_id: None,
            symbol: symbol.to_string(),
            side,
            quantity,
            price,
            commission: Decimal::ZERO,
            commission_asset: "USDC".to_string(),
            executed_at: Utc::now(),
            is_maker: false,
            realized_pnl: None,
        }
    }

    /// Get notional value
    pub fn notional(&self) -> Decimal {
        self.quantity * self.price
    }
}

/// Timer event payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimerPayload {
    /// Timer name/identifier
    pub name: String,
    /// Timer interval in milliseconds
    pub interval_ms: u64,
    /// Number of times this timer has fired
    pub tick_count: u64,
    /// Custom data
    pub data: Option<serde_json::Value>,
}

impl TimerPayload {
    /// Create a new timer event
    pub fn new(name: &str, interval_ms: u64, tick_count: u64) -> Self {
        Self {
            name: name.to_string(),
            interval_ms,
            tick_count,
            data: None,
        }
    }
}

/// Risk event payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskPayload {
    /// Risk event type
    pub risk_type: RiskEventType,
    /// Affected symbol (if applicable)
    pub symbol: Option<String>,
    /// Current value that triggered the event
    pub current_value: Decimal,
    /// Threshold that was breached
    pub threshold: Decimal,
    /// Severity level
    pub severity: RiskSeverity,
    /// Recommended action
    pub action: RiskAction,
    /// Human-readable message
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskEventType {
    MaxDrawdown,
    PositionLimit,
    DailyLossLimit,
    ConcentrationLimit,
    VarBreach,
    MarginCall,
    Liquidation,
    CircuitBreaker,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskSeverity {
    Info,
    Warning,
    Alert,
    Critical,
    Emergency,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskAction {
    None,
    ReducePosition,
    ClosePosition,
    HaltTrading,
    FlattenAll,
}

impl RiskPayload {
    /// Create a drawdown breach event
    pub fn drawdown_breach(current: Decimal, threshold: Decimal) -> Self {
        Self {
            risk_type: RiskEventType::MaxDrawdown,
            symbol: None,
            current_value: current,
            threshold,
            severity: RiskSeverity::Alert,
            action: RiskAction::ReducePosition,
            message: format!(
                "Max drawdown breached: {:.2}% > {:.2}%",
                current * Decimal::ONE_HUNDRED,
                threshold * Decimal::ONE_HUNDRED
            ),
        }
    }

    /// Create a position limit breach event
    pub fn position_limit(symbol: &str, current: Decimal, limit: Decimal) -> Self {
        Self {
            risk_type: RiskEventType::PositionLimit,
            symbol: Some(symbol.to_string()),
            current_value: current,
            threshold: limit,
            severity: RiskSeverity::Warning,
            action: RiskAction::None,
            message: format!("Position limit warning for {}: {} > {}", symbol, current, limit),
        }
    }
}

/// System event payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemPayload {
    /// System event type
    pub event_type: SystemEventType,
    /// Component name
    pub component: String,
    /// Event message
    pub message: String,
    /// Additional data
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SystemEventType {
    Startup,
    Shutdown,
    Heartbeat,
    ConfigChange,
    ConnectionUp,
    ConnectionDown,
    Error,
    Warning,
}

impl SystemPayload {
    /// Create a startup event
    pub fn startup(component: &str) -> Self {
        Self {
            event_type: SystemEventType::Startup,
            component: component.to_string(),
            message: format!("{} started", component),
            data: None,
        }
    }

    /// Create a shutdown event
    pub fn shutdown(component: &str) -> Self {
        Self {
            event_type: SystemEventType::Shutdown,
            component: component.to_string(),
            message: format!("{} shutting down", component),
            data: None,
        }
    }

    /// Create a heartbeat event
    pub fn heartbeat(component: &str) -> Self {
        Self {
            event_type: SystemEventType::Heartbeat,
            component: component.to_string(),
            message: format!("{} heartbeat", component),
            data: None,
        }
    }
}

// ============================================================================
// Event Bus - Core Event Distribution System
// ============================================================================

/// Event handler trait - implement this to handle events
#[async_trait::async_trait]
pub trait EventHandler: Send + Sync {
    /// Handler name for logging
    fn name(&self) -> &str;

    /// Event types this handler is interested in
    fn handles(&self) -> Vec<EventType>;

    /// Handle an event
    async fn handle(&self, event: &Event) -> Result<Vec<Event>, EventError>;
}

/// Event error types
#[derive(Debug, Clone)]
pub enum EventError {
    /// Handler error
    HandlerError(String),
    /// Serialization error
    SerializationError(String),
    /// Channel error
    ChannelError(String),
    /// Timeout
    Timeout,
    /// Event validation error
    ValidationError(String),
}

impl std::fmt::Display for EventError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HandlerError(s) => write!(f, "Handler error: {}", s),
            Self::SerializationError(s) => write!(f, "Serialization error: {}", s),
            Self::ChannelError(s) => write!(f, "Channel error: {}", s),
            Self::Timeout => write!(f, "Event timeout"),
            Self::ValidationError(s) => write!(f, "Validation error: {}", s),
        }
    }
}

impl std::error::Error for EventError {}

/// Event bus for distributing events to handlers
pub struct EventBus {
    /// Registered handlers
    handlers: RwLock<Vec<Arc<dyn EventHandler>>>,
    /// Event broadcast channel
    broadcast_tx: broadcast::Sender<Event>,
    /// Event count metrics
    event_count: RwLock<HashMap<String, u64>>,
    /// Running flag
    running: RwLock<bool>,
}

impl EventBus {
    /// Create a new event bus with specified channel capacity
    pub fn new(capacity: usize) -> Self {
        let (broadcast_tx, _) = broadcast::channel(capacity);
        Self {
            handlers: RwLock::new(Vec::new()),
            broadcast_tx,
            event_count: RwLock::new(HashMap::new()),
            running: RwLock::new(false),
        }
    }

    /// Register an event handler
    pub async fn register_handler(&self, handler: Arc<dyn EventHandler>) {
        let mut handlers = self.handlers.write().await;
        handlers.push(handler);
    }

    /// Subscribe to event broadcast
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.broadcast_tx.subscribe()
    }

    /// Publish an event to all handlers
    pub async fn publish(&self, event: Event) -> Result<Vec<Event>, EventError> {
        // Update metrics
        {
            let mut counts = self.event_count.write().await;
            let key = format!("{:?}", event.event_type);
            *counts.entry(key).or_insert(0) += 1;
        }

        // Broadcast event
        let _ = self.broadcast_tx.send(event.clone());

        // Dispatch to handlers
        let handlers = self.handlers.read().await;
        let mut result_events = Vec::new();

        for handler in handlers.iter() {
            // Check if handler is interested in this event type
            let handles = handler.handles();
            let should_handle = handles.iter().any(|h| {
                matches!(
                    (&event.event_type, h),
                    (EventType::MarketData, EventType::MarketData)
                        | (EventType::Signal, EventType::Signal)
                        | (EventType::Order, EventType::Order)
                        | (EventType::Fill, EventType::Fill)
                        | (EventType::Timer, EventType::Timer)
                        | (EventType::Risk, EventType::Risk)
                        | (EventType::System, EventType::System)
                        | (EventType::Custom(_), EventType::Custom(_))
                )
            });

            if should_handle {
                match handler.handle(&event).await {
                    Ok(events) => result_events.extend(events),
                    Err(e) => {
                        tracing::error!(
                            "Handler {} failed for event {}: {}",
                            handler.name(),
                            event.id,
                            e
                        );
                    }
                }
            }
        }

        Ok(result_events)
    }

    /// Publish multiple events
    pub async fn publish_all(&self, events: Vec<Event>) -> Result<Vec<Event>, EventError> {
        let mut all_results = Vec::new();
        for event in events {
            let results = self.publish(event).await?;
            all_results.extend(results);
        }
        Ok(all_results)
    }

    /// Get event count metrics
    pub async fn get_metrics(&self) -> HashMap<String, u64> {
        self.event_count.read().await.clone()
    }

    /// Reset metrics
    pub async fn reset_metrics(&self) {
        self.event_count.write().await.clear();
    }

    /// Start the event bus
    pub async fn start(&self) {
        *self.running.write().await = true;
    }

    /// Stop the event bus
    pub async fn stop(&self) {
        *self.running.write().await = false;
    }

    /// Check if running
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }
}

// ============================================================================
// Event Store - Persistence and Replay
// ============================================================================

/// Event store for persistence and replay
pub struct EventStore {
    /// In-memory event log
    events: RwLock<Vec<Event>>,
    /// Maximum events to keep in memory
    max_events: usize,
    /// Snapshot interval
    snapshot_interval: u64,
    /// Event count since last snapshot
    events_since_snapshot: RwLock<u64>,
}

impl EventStore {
    /// Create a new event store
    pub fn new(max_events: usize) -> Self {
        Self {
            events: RwLock::new(Vec::new()),
            max_events,
            snapshot_interval: 1000,
            events_since_snapshot: RwLock::new(0),
        }
    }

    /// Append an event to the store
    pub async fn append(&self, event: Event) -> Result<(), EventError> {
        let mut events = self.events.write().await;
        events.push(event);

        // Trim if over capacity
        if events.len() > self.max_events {
            let drain_count = events.len() - self.max_events;
            events.drain(0..drain_count);
        }

        // Update snapshot counter
        let mut count = self.events_since_snapshot.write().await;
        *count += 1;

        Ok(())
    }

    /// Get events in time range
    pub async fn get_events(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Vec<Event> {
        let events = self.events.read().await;
        events
            .iter()
            .filter(|e| e.timestamp >= from && e.timestamp <= to)
            .cloned()
            .collect()
    }

    /// Get events by type
    pub async fn get_events_by_type(&self, event_type: &EventType) -> Vec<Event> {
        let events = self.events.read().await;
        events
            .iter()
            .filter(|e| matches!(
                (&e.event_type, event_type),
                (EventType::MarketData, EventType::MarketData)
                    | (EventType::Signal, EventType::Signal)
                    | (EventType::Order, EventType::Order)
                    | (EventType::Fill, EventType::Fill)
                    | (EventType::Timer, EventType::Timer)
                    | (EventType::Risk, EventType::Risk)
                    | (EventType::System, EventType::System)
            ))
            .cloned()
            .collect()
    }

    /// Get events by correlation ID
    pub async fn get_by_correlation(&self, correlation_id: &str) -> Vec<Event> {
        let events = self.events.read().await;
        events
            .iter()
            .filter(|e| e.correlation_id.as_deref() == Some(correlation_id))
            .cloned()
            .collect()
    }

    /// Get event count
    pub async fn count(&self) -> usize {
        self.events.read().await.len()
    }

    /// Clear all events
    pub async fn clear(&self) {
        self.events.write().await.clear();
    }

    /// Replay events through an event bus
    pub async fn replay(
        &self,
        bus: &EventBus,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<Event>, EventError> {
        let events = self.get_events(from, to).await;
        let mut all_results = Vec::new();

        for event in events {
            let results = bus.publish(event).await?;
            all_results.extend(results);
        }

        Ok(all_results)
    }
}

// ============================================================================
// Event-Driven Engine - Main Orchestrator
// ============================================================================

/// Configuration for the event engine
#[derive(Debug, Clone)]
pub struct EventEngineConfig {
    /// Event bus channel capacity
    pub bus_capacity: usize,
    /// Event store max events
    pub store_max_events: usize,
    /// Enable event persistence
    pub enable_persistence: bool,
    /// Heartbeat interval in milliseconds
    pub heartbeat_interval_ms: u64,
}

impl Default for EventEngineConfig {
    fn default() -> Self {
        Self {
            bus_capacity: 10_000,
            store_max_events: 100_000,
            enable_persistence: true,
            heartbeat_interval_ms: 1000,
        }
    }
}

/// Event-driven trading engine
pub struct EventEngine {
    /// Event bus
    pub bus: Arc<EventBus>,
    /// Event store
    pub store: Arc<EventStore>,
    /// Configuration
    config: EventEngineConfig,
    /// Shutdown signal sender
    shutdown_tx: Option<mpsc::Sender<()>>,
}

impl EventEngine {
    /// Create a new event engine
    pub fn new(config: EventEngineConfig) -> Self {
        let bus = Arc::new(EventBus::new(config.bus_capacity));
        let store = Arc::new(EventStore::new(config.store_max_events));

        Self {
            bus,
            store,
            config,
            shutdown_tx: None,
        }
    }

    /// Register an event handler
    pub async fn register_handler(&self, handler: Arc<dyn EventHandler>) {
        self.bus.register_handler(handler).await;
    }

    /// Publish an event
    pub async fn publish(&self, event: Event) -> Result<Vec<Event>, EventError> {
        // Store event if persistence enabled
        if self.config.enable_persistence {
            self.store.append(event.clone()).await?;
        }

        // Publish to bus
        let results = self.bus.publish(event).await?;

        // Store result events
        if self.config.enable_persistence {
            for result in &results {
                self.store.append(result.clone()).await?;
            }
        }

        Ok(results)
    }

    /// Start the engine with heartbeat
    pub async fn start(&self) -> mpsc::Receiver<()> {
        self.bus.start().await;

        // Publish startup event
        let startup = Event::new(
            EventType::System,
            "EventEngine",
            EventPayload::System(SystemPayload::startup("EventEngine")),
        );
        let _ = self.publish(startup).await;

        // Create shutdown channel
        let (tx, rx) = mpsc::channel(1);

        // Spawn heartbeat task
        let bus = self.bus.clone();
        let store = self.store.clone();
        let interval = self.config.heartbeat_interval_ms;
        let enable_persistence = self.config.enable_persistence;

        tokio::spawn(async move {
            let mut tick = 0u64;
            loop {
                tokio::time::sleep(tokio::time::Duration::from_millis(interval)).await;

                if !bus.is_running().await {
                    break;
                }

                tick += 1;
                let heartbeat = Event::new(
                    EventType::Timer,
                    "EventEngine",
                    EventPayload::Timer(TimerPayload::new("heartbeat", interval, tick)),
                );

                if enable_persistence {
                    let _ = store.append(heartbeat.clone()).await;
                }
                let _ = bus.publish(heartbeat).await;
            }
        });

        rx
    }

    /// Stop the engine
    pub async fn stop(&self) {
        // Publish shutdown event
        let shutdown = Event::new(
            EventType::System,
            "EventEngine",
            EventPayload::System(SystemPayload::shutdown("EventEngine")),
        );
        let _ = self.publish(shutdown).await;

        self.bus.stop().await;

        if let Some(tx) = &self.shutdown_tx {
            let _ = tx.send(()).await;
        }
    }

    /// Get metrics
    pub async fn get_metrics(&self) -> EventEngineMetrics {
        EventEngineMetrics {
            event_counts: self.bus.get_metrics().await,
            store_size: self.store.count().await,
            is_running: self.bus.is_running().await,
        }
    }
}

/// Event engine metrics
#[derive(Debug, Clone)]
pub struct EventEngineMetrics {
    /// Event counts by type
    pub event_counts: HashMap<String, u64>,
    /// Number of events in store
    pub store_size: usize,
    /// Is engine running
    pub is_running: bool,
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_event_creation() {
        let payload = EventPayload::System(SystemPayload::startup("test"));
        let event = Event::new(EventType::System, "test", payload);

        assert!(!event.id.is_empty());
        assert_eq!(event.source, "test");
        assert_eq!(event.priority, EventPriority::Normal);
    }

    #[test]
    fn test_event_with_priority() {
        let payload = EventPayload::System(SystemPayload::startup("test"));
        let event = Event::new(EventType::System, "test", payload).with_priority(EventPriority::Critical);

        assert_eq!(event.priority, EventPriority::Critical);
    }

    #[test]
    fn test_event_child_creation() {
        let parent_payload = EventPayload::System(SystemPayload::startup("parent"));
        let parent = Event::new(EventType::System, "parent", parent_payload);

        let child_payload = EventPayload::System(SystemPayload::heartbeat("child"));
        let child = parent.create_child(EventType::System, "child", child_payload);

        assert_eq!(child.causation_id, Some(parent.id.clone()));
        assert!(child.correlation_id.is_some());
    }

    #[test]
    fn test_market_data_payload() {
        let md = MarketDataPayload::price_update("BTCUSDT", dec!(50000), dec!(50001), "binance");

        assert_eq!(md.symbol, "BTCUSDT");
        assert_eq!(md.bid, Some(dec!(50000)));
        assert_eq!(md.ask, Some(dec!(50001)));
        assert_eq!(md.mid_price(), Some(dec!(50000.5)));
        assert_eq!(md.spread(), Some(dec!(1)));
    }

    #[test]
    fn test_signal_payload() {
        let signal = SignalPayload::new("BTCUSDT", 1, dec!(0.8), "momentum")
            .with_edge(dec!(0.05), dec!(0.7))
            .with_feature("rsi", dec!(65));

        assert!(signal.is_tradeable(dec!(0.02), dec!(0.5)));
        assert!(!signal.is_tradeable(dec!(0.1), dec!(0.5)));
    }

    #[test]
    fn test_order_payload() {
        let order = OrderPayload::new("BTCUSDT", OrderSide::Buy, OrderType::Limit, dec!(0.1))
            .with_price(dec!(50000));

        assert!(order.is_active());
        assert!(!order.is_terminal());
        assert_eq!(order.price, Some(dec!(50000)));
    }

    #[test]
    fn test_fill_payload() {
        let fill = FillPayload::new("order123", "BTCUSDT", OrderSide::Buy, dec!(0.1), dec!(50000));

        assert_eq!(fill.notional(), dec!(5000));
    }

    #[test]
    fn test_risk_payload() {
        let risk = RiskPayload::drawdown_breach(dec!(0.15), dec!(0.10));

        assert_eq!(risk.risk_type, RiskEventType::MaxDrawdown);
        assert_eq!(risk.severity, RiskSeverity::Alert);
        assert_eq!(risk.action, RiskAction::ReducePosition);
    }

    #[tokio::test]
    async fn test_event_bus_creation() {
        let bus = EventBus::new(1000);
        assert!(!bus.is_running().await);

        bus.start().await;
        assert!(bus.is_running().await);

        bus.stop().await;
        assert!(!bus.is_running().await);
    }

    #[tokio::test]
    async fn test_event_bus_publish() {
        let bus = EventBus::new(1000);
        bus.start().await;

        let payload = EventPayload::System(SystemPayload::heartbeat("test"));
        let event = Event::new(EventType::System, "test", payload);

        let results = bus.publish(event).await.unwrap();
        assert!(results.is_empty()); // No handlers registered

        let metrics = bus.get_metrics().await;
        assert_eq!(metrics.get("System"), Some(&1));
    }

    #[tokio::test]
    async fn test_event_store() {
        let store = EventStore::new(100);

        let payload = EventPayload::System(SystemPayload::heartbeat("test"));
        let event = Event::new(EventType::System, "test", payload);

        store.append(event).await.unwrap();
        assert_eq!(store.count().await, 1);

        store.clear().await;
        assert_eq!(store.count().await, 0);
    }

    #[tokio::test]
    async fn test_event_store_time_range() {
        let store = EventStore::new(100);
        let now = Utc::now();

        // Add events
        for i in 0..5 {
            let payload = EventPayload::Timer(TimerPayload::new("test", 1000, i));
            let mut event = Event::new(EventType::Timer, "test", payload);
            event.timestamp = now + chrono::Duration::seconds(i as i64);
            store.append(event).await.unwrap();
        }

        let from = now + chrono::Duration::seconds(1);
        let to = now + chrono::Duration::seconds(3);
        let events = store.get_events(from, to).await;

        assert_eq!(events.len(), 3);
    }

    #[tokio::test]
    async fn test_event_engine() {
        let config = EventEngineConfig {
            bus_capacity: 100,
            store_max_events: 1000,
            enable_persistence: true,
            heartbeat_interval_ms: 100,
        };

        let engine = EventEngine::new(config);

        let payload = EventPayload::System(SystemPayload::startup("test"));
        let event = Event::new(EventType::System, "test", payload);

        let _ = engine.publish(event).await.unwrap();
        assert_eq!(engine.store.count().await, 1);
    }

    #[tokio::test]
    async fn test_event_bus_subscribe() {
        let bus = EventBus::new(100);
        let mut rx = bus.subscribe();

        bus.start().await;

        let payload = EventPayload::System(SystemPayload::heartbeat("test"));
        let event = Event::new(EventType::System, "test", payload);
        let event_id = event.id.clone();

        bus.publish(event).await.unwrap();

        let received = rx.recv().await.unwrap();
        assert_eq!(received.id, event_id);
    }

    #[test]
    fn test_priority_ordering() {
        assert!(EventPriority::Low < EventPriority::Normal);
        assert!(EventPriority::Normal < EventPriority::High);
        assert!(EventPriority::High < EventPriority::Critical);
        assert!(EventPriority::Critical < EventPriority::System);
    }

    #[test]
    fn test_order_status_checks() {
        let mut order = OrderPayload::new("BTCUSDT", OrderSide::Buy, OrderType::Market, dec!(1));

        assert!(order.is_active());
        assert!(!order.is_terminal());

        order.status = OrderStatus::Filled;
        assert!(!order.is_active());
        assert!(order.is_terminal());
    }

    #[tokio::test]
    async fn test_event_store_max_capacity() {
        let store = EventStore::new(5);

        for i in 0..10 {
            let payload = EventPayload::Timer(TimerPayload::new("test", 1000, i));
            let event = Event::new(EventType::Timer, "test", payload);
            store.append(event).await.unwrap();
        }

        // Should only keep last 5
        assert_eq!(store.count().await, 5);
    }

    #[tokio::test]
    async fn test_event_store_by_correlation() {
        let store = EventStore::new(100);
        let correlation_id = "corr-123";

        // Add events with same correlation
        for i in 0..3 {
            let payload = EventPayload::Timer(TimerPayload::new("test", 1000, i));
            let event = Event::new(EventType::Timer, "test", payload)
                .with_correlation_id(correlation_id);
            store.append(event).await.unwrap();
        }

        // Add event without correlation
        let payload = EventPayload::Timer(TimerPayload::new("test", 1000, 99));
        let event = Event::new(EventType::Timer, "test", payload);
        store.append(event).await.unwrap();

        let correlated = store.get_by_correlation(correlation_id).await;
        assert_eq!(correlated.len(), 3);
    }

    #[test]
    fn test_orderbook_payload() {
        let bids = vec![(dec!(50000), dec!(1.5)), (dec!(49999), dec!(2.0))];
        let asks = vec![(dec!(50001), dec!(1.0)), (dec!(50002), dec!(1.5))];

        let ob = MarketDataPayload::orderbook_update("BTCUSDT", bids, asks, "binance");

        assert_eq!(ob.bid, Some(dec!(50000)));
        assert_eq!(ob.ask, Some(dec!(50001)));
        assert_eq!(ob.spread(), Some(dec!(1)));
    }
}
