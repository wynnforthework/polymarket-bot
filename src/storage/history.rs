//! Historical price data storage for backtesting
//!
//! Stores OHLCV candles and order book snapshots for market analysis.

use crate::error::Result;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use std::collections::HashMap;

/// OHLCV candle data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candle {
    pub token_id: String,
    pub timestamp: DateTime<Utc>,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: Decimal,
    /// Timeframe in seconds (60 = 1m, 3600 = 1h, etc.)
    pub timeframe: i64,
}

/// Order book snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookSnapshot {
    pub token_id: String,
    pub timestamp: DateTime<Utc>,
    pub best_bid: Decimal,
    pub best_ask: Decimal,
    pub bid_depth: Decimal,  // Total size within 5% of best bid
    pub ask_depth: Decimal,  // Total size within 5% of best ask
    pub spread: Decimal,
    pub midpoint: Decimal,
}

/// Price tick for high-resolution data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceTick {
    pub token_id: String,
    pub timestamp: DateTime<Utc>,
    pub price: Decimal,
    pub side: Option<String>,  // "bid", "ask", or "trade"
    pub size: Option<Decimal>,
}

/// Historical data store
pub struct HistoryStore {
    pool: SqlitePool,
}

impl HistoryStore {
    /// Create a new history store using existing pool
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Initialize history tables
    pub async fn init(&self) -> Result<()> {
        // Candles table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS candles (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                token_id TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                open TEXT NOT NULL,
                high TEXT NOT NULL,
                low TEXT NOT NULL,
                close TEXT NOT NULL,
                volume TEXT NOT NULL,
                timeframe INTEGER NOT NULL,
                UNIQUE(token_id, timestamp, timeframe)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Index for fast queries
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_candles_token_time 
            ON candles(token_id, timeframe, timestamp DESC)
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Order book snapshots
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS orderbook_snapshots (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                token_id TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                best_bid TEXT NOT NULL,
                best_ask TEXT NOT NULL,
                bid_depth TEXT NOT NULL,
                ask_depth TEXT NOT NULL,
                spread TEXT NOT NULL,
                midpoint TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_ob_token_time 
            ON orderbook_snapshots(token_id, timestamp DESC)
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Price ticks for high-resolution data
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS price_ticks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                token_id TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                price TEXT NOT NULL,
                side TEXT,
                size TEXT
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_ticks_token_time 
            ON price_ticks(token_id, timestamp DESC)
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Insert a candle (upsert)
    pub async fn insert_candle(&self, candle: &Candle) -> Result<()> {
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO candles 
            (token_id, timestamp, open, high, low, close, volume, timeframe)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&candle.token_id)
        .bind(candle.timestamp.to_rfc3339())
        .bind(candle.open.to_string())
        .bind(candle.high.to_string())
        .bind(candle.low.to_string())
        .bind(candle.close.to_string())
        .bind(candle.volume.to_string())
        .bind(candle.timeframe)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Insert multiple candles efficiently
    pub async fn insert_candles(&self, candles: &[Candle]) -> Result<()> {
        for candle in candles {
            self.insert_candle(candle).await?;
        }
        Ok(())
    }

    /// Get candles for a token
    pub async fn get_candles(
        &self,
        token_id: &str,
        timeframe: i64,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<Candle>> {
        let rows = sqlx::query_as::<_, CandleRow>(
            r#"
            SELECT token_id, timestamp, open, high, low, close, volume, timeframe
            FROM candles
            WHERE token_id = ? AND timeframe = ? 
              AND timestamp >= ? AND timestamp <= ?
            ORDER BY timestamp ASC
            "#,
        )
        .bind(token_id)
        .bind(timeframe)
        .bind(from.to_rfc3339())
        .bind(to.to_rfc3339())
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().filter_map(|r| r.try_into().ok()).collect())
    }

    /// Get latest N candles
    pub async fn get_latest_candles(
        &self,
        token_id: &str,
        timeframe: i64,
        limit: i64,
    ) -> Result<Vec<Candle>> {
        let rows = sqlx::query_as::<_, CandleRow>(
            r#"
            SELECT token_id, timestamp, open, high, low, close, volume, timeframe
            FROM candles
            WHERE token_id = ? AND timeframe = ?
            ORDER BY timestamp DESC
            LIMIT ?
            "#,
        )
        .bind(token_id)
        .bind(timeframe)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let mut candles: Vec<Candle> = rows.into_iter().filter_map(|r| r.try_into().ok()).collect();
        candles.reverse(); // Return in chronological order
        Ok(candles)
    }

    /// Insert order book snapshot
    pub async fn insert_orderbook_snapshot(&self, snapshot: &OrderBookSnapshot) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO orderbook_snapshots 
            (token_id, timestamp, best_bid, best_ask, bid_depth, ask_depth, spread, midpoint)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&snapshot.token_id)
        .bind(snapshot.timestamp.to_rfc3339())
        .bind(snapshot.best_bid.to_string())
        .bind(snapshot.best_ask.to_string())
        .bind(snapshot.bid_depth.to_string())
        .bind(snapshot.ask_depth.to_string())
        .bind(snapshot.spread.to_string())
        .bind(snapshot.midpoint.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get order book snapshots
    pub async fn get_orderbook_snapshots(
        &self,
        token_id: &str,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<OrderBookSnapshot>> {
        let rows = sqlx::query_as::<_, OrderBookRow>(
            r#"
            SELECT token_id, timestamp, best_bid, best_ask, bid_depth, ask_depth, spread, midpoint
            FROM orderbook_snapshots
            WHERE token_id = ? AND timestamp >= ? AND timestamp <= ?
            ORDER BY timestamp ASC
            "#,
        )
        .bind(token_id)
        .bind(from.to_rfc3339())
        .bind(to.to_rfc3339())
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().filter_map(|r| r.try_into().ok()).collect())
    }

    /// Insert price tick
    pub async fn insert_tick(&self, tick: &PriceTick) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO price_ticks (token_id, timestamp, price, side, size)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(&tick.token_id)
        .bind(tick.timestamp.to_rfc3339())
        .bind(tick.price.to_string())
        .bind(&tick.side)
        .bind(tick.size.map(|s| s.to_string()))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Aggregate ticks into candles
    pub async fn aggregate_to_candles(
        &self,
        token_id: &str,
        timeframe: i64,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<Candle>> {
        // Get all ticks in range
        let rows = sqlx::query_as::<_, TickRow>(
            r#"
            SELECT token_id, timestamp, price, side, size
            FROM price_ticks
            WHERE token_id = ? AND timestamp >= ? AND timestamp <= ?
            ORDER BY timestamp ASC
            "#,
        )
        .bind(token_id)
        .bind(from.to_rfc3339())
        .bind(to.to_rfc3339())
        .fetch_all(&self.pool)
        .await?;

        let ticks: Vec<PriceTick> = rows.into_iter().filter_map(|r| r.try_into().ok()).collect();
        
        // Group by time bucket and aggregate
        let mut candle_map: HashMap<i64, CandleBuilder> = HashMap::new();
        
        for tick in ticks {
            let bucket = (tick.timestamp.timestamp() / timeframe) * timeframe;
            let builder = candle_map.entry(bucket).or_insert_with(|| CandleBuilder::new(token_id, bucket, timeframe));
            builder.add_tick(&tick);
        }

        let mut candles: Vec<Candle> = candle_map
            .into_values()
            .filter_map(|b| b.build())
            .collect();
        
        candles.sort_by_key(|c| c.timestamp);
        Ok(candles)
    }

    /// Cleanup old data (keep last N days)
    pub async fn cleanup(&self, keep_days: i64) -> Result<u64> {
        let cutoff = Utc::now() - chrono::Duration::days(keep_days);
        let cutoff_str = cutoff.to_rfc3339();

        let mut deleted = 0u64;

        let result = sqlx::query("DELETE FROM price_ticks WHERE timestamp < ?")
            .bind(&cutoff_str)
            .execute(&self.pool)
            .await?;
        deleted += result.rows_affected();

        let result = sqlx::query("DELETE FROM orderbook_snapshots WHERE timestamp < ?")
            .bind(&cutoff_str)
            .execute(&self.pool)
            .await?;
        deleted += result.rows_affected();

        // Keep candles longer (30x)
        let candle_cutoff = Utc::now() - chrono::Duration::days(keep_days * 30);
        let result = sqlx::query("DELETE FROM candles WHERE timestamp < ?")
            .bind(candle_cutoff.to_rfc3339())
            .execute(&self.pool)
            .await?;
        deleted += result.rows_affected();

        Ok(deleted)
    }
}

// Helper struct for candle aggregation
struct CandleBuilder {
    token_id: String,
    timestamp: i64,
    timeframe: i64,
    open: Option<Decimal>,
    high: Option<Decimal>,
    low: Option<Decimal>,
    close: Option<Decimal>,
    volume: Decimal,
}

impl CandleBuilder {
    fn new(token_id: &str, timestamp: i64, timeframe: i64) -> Self {
        Self {
            token_id: token_id.to_string(),
            timestamp,
            timeframe,
            open: None,
            high: None,
            low: None,
            close: None,
            volume: Decimal::ZERO,
        }
    }

    fn add_tick(&mut self, tick: &PriceTick) {
        if self.open.is_none() {
            self.open = Some(tick.price);
        }
        self.close = Some(tick.price);
        self.high = Some(self.high.map_or(tick.price, |h| h.max(tick.price)));
        self.low = Some(self.low.map_or(tick.price, |l| l.min(tick.price)));
        if let Some(size) = tick.size {
            self.volume += size;
        }
    }

    fn build(self) -> Option<Candle> {
        Some(Candle {
            token_id: self.token_id,
            timestamp: DateTime::from_timestamp(self.timestamp, 0)?,
            open: self.open?,
            high: self.high?,
            low: self.low?,
            close: self.close?,
            volume: self.volume,
            timeframe: self.timeframe,
        })
    }
}

// SQLx row types
#[derive(Debug, sqlx::FromRow)]
struct CandleRow {
    token_id: String,
    timestamp: String,
    open: String,
    high: String,
    low: String,
    close: String,
    volume: String,
    timeframe: i64,
}

impl TryFrom<CandleRow> for Candle {
    type Error = anyhow::Error;

    fn try_from(row: CandleRow) -> std::result::Result<Self, Self::Error> {
        Ok(Candle {
            token_id: row.token_id,
            timestamp: row.timestamp.parse()?,
            open: row.open.parse()?,
            high: row.high.parse()?,
            low: row.low.parse()?,
            close: row.close.parse()?,
            volume: row.volume.parse()?,
            timeframe: row.timeframe,
        })
    }
}

#[derive(Debug, sqlx::FromRow)]
struct OrderBookRow {
    token_id: String,
    timestamp: String,
    best_bid: String,
    best_ask: String,
    bid_depth: String,
    ask_depth: String,
    spread: String,
    midpoint: String,
}

impl TryFrom<OrderBookRow> for OrderBookSnapshot {
    type Error = anyhow::Error;

    fn try_from(row: OrderBookRow) -> std::result::Result<Self, Self::Error> {
        Ok(OrderBookSnapshot {
            token_id: row.token_id,
            timestamp: row.timestamp.parse()?,
            best_bid: row.best_bid.parse()?,
            best_ask: row.best_ask.parse()?,
            bid_depth: row.bid_depth.parse()?,
            ask_depth: row.ask_depth.parse()?,
            spread: row.spread.parse()?,
            midpoint: row.midpoint.parse()?,
        })
    }
}

#[derive(Debug, sqlx::FromRow)]
struct TickRow {
    token_id: String,
    timestamp: String,
    price: String,
    side: Option<String>,
    size: Option<String>,
}

impl TryFrom<TickRow> for PriceTick {
    type Error = anyhow::Error;

    fn try_from(row: TickRow) -> std::result::Result<Self, Self::Error> {
        Ok(PriceTick {
            token_id: row.token_id,
            timestamp: row.timestamp.parse()?,
            price: row.price.parse()?,
            side: row.side,
            size: row.size.and_then(|s| s.parse().ok()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[tokio::test]
    async fn test_candle_builder() {
        let mut builder = CandleBuilder::new("test_token", 1704067200, 60);
        
        builder.add_tick(&PriceTick {
            token_id: "test_token".to_string(),
            timestamp: Utc::now(),
            price: dec!(0.50),
            side: Some("trade".to_string()),
            size: Some(dec!(100)),
        });
        
        builder.add_tick(&PriceTick {
            token_id: "test_token".to_string(),
            timestamp: Utc::now(),
            price: dec!(0.55),
            side: Some("trade".to_string()),
            size: Some(dec!(200)),
        });
        
        builder.add_tick(&PriceTick {
            token_id: "test_token".to_string(),
            timestamp: Utc::now(),
            price: dec!(0.45),
            side: Some("trade".to_string()),
            size: Some(dec!(150)),
        });

        let candle = builder.build().unwrap();
        
        assert_eq!(candle.open, dec!(0.50));
        assert_eq!(candle.high, dec!(0.55));
        assert_eq!(candle.low, dec!(0.45));
        assert_eq!(candle.close, dec!(0.45));
        assert_eq!(candle.volume, dec!(450));
    }

    #[test]
    fn test_candle_serialization() {
        let candle = Candle {
            token_id: "test".to_string(),
            timestamp: Utc::now(),
            open: dec!(0.5),
            high: dec!(0.6),
            low: dec!(0.4),
            close: dec!(0.55),
            volume: dec!(1000),
            timeframe: 60,
        };

        let json = serde_json::to_string(&candle).unwrap();
        let parsed: Candle = serde_json::from_str(&json).unwrap();
        
        assert_eq!(candle.token_id, parsed.token_id);
        assert_eq!(candle.open, parsed.open);
    }
}
