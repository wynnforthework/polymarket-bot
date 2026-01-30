//! Multi-Asset Crypto 15m Paper Trading with Binance WebSocket Feed
//! Real-time price streaming for BTC, ETH, SOL, XRP

use polymarket_bot::client::GammaClient;
use polymarket_bot::paper::{PaperTrader, PaperTraderConfig, PositionSide};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::sleep;
use tracing::{info, warn, error};
use futures_util::StreamExt;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use serde::Deserialize;

/// Asset configuration
#[derive(Clone)]
struct Asset {
    name: &'static str,
    binance: &'static str,      // Binance symbol (lowercase)
    poly_slug: &'static str,    // Polymarket slug prefix
}

const ASSETS: &[Asset] = &[
    Asset { name: "BTC", binance: "btcusdt", poly_slug: "btc-updown-15m" },
    Asset { name: "ETH", binance: "ethusdt", poly_slug: "eth-updown-15m" },
    Asset { name: "SOL", binance: "solusdt", poly_slug: "sol-updown-15m" },
    Asset { name: "XRP", binance: "xrpusdt", poly_slug: "xrp-updown-15m" },
];

/// Shared price state from WebSocket
#[derive(Default)]
struct PriceState {
    prices: HashMap<String, f64>,
    trends: HashMap<String, (f64, Vec<f64>)>, // (trend_pct, recent_prices)
}

#[derive(Deserialize)]
struct BinanceTicker {
    #[serde(rename = "s")]
    symbol: String,
    #[serde(rename = "c")]
    price: String,
    #[serde(rename = "P")]
    change_pct: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    
    let config = PaperTraderConfig {
        initial_balance: dec!(1000),
        max_position_pct: dec!(15),
        slippage_pct: dec!(0.5),
        fee_pct: dec!(0.1),
        save_interval: 30,
        state_file: Some("multi_crypto_paper.json".to_string()),
    };
    
    let gamma = GammaClient::new("https://gamma-api.polymarket.com")?;
    let trader = Arc::new(PaperTrader::new(config, gamma.clone()));
    let state = Arc::new(RwLock::new(PriceState::default()));
    
    info!("🚀 Multi-Asset 15m Trading with WebSocket Feed");
    info!("💰 Initial balance: $1000");
    
    // Spawn WebSocket price feed
    let state_ws = state.clone();
    tokio::spawn(async move {
        binance_websocket(state_ws).await;
    });
    
    // Main trading loop - check every 2 seconds (WebSocket updates prices continuously)
    let mut last_slot: u64 = 0;
    
    loop {
        let prices = {
            let s = state.read().await;
            s.prices.clone()
        };
        
        if !prices.is_empty() {
            if let Err(e) = trade_loop(&trader, &state, &prices, &mut last_slot).await {
                error!("Trade loop error: {}", e);
            }
        }
        
        sleep(Duration::from_secs(2)).await;
    }
}

async fn binance_websocket(state: Arc<RwLock<PriceState>>) {
    // Combined stream for all symbols
    let streams: Vec<String> = ASSETS.iter()
        .map(|a| format!("{}@ticker", a.binance))
        .collect();
    let url = format!("wss://stream.binance.com:9443/stream?streams={}", streams.join("/"));
    
    loop {
        info!("📡 Connecting to Binance WebSocket...");
        
        match connect_async(&url).await {
            Ok((ws, _)) => {
                info!("✅ Binance WebSocket connected");
                let (_, mut read) = ws.split();
                
                while let Some(msg) = read.next().await {
                    match msg {
                        Ok(Message::Text(text)) => {
                            // Parse combined stream format: {"stream":"btcusdt@ticker","data":{...}}
                            if let Ok(wrapper) = serde_json::from_str::<serde_json::Value>(&text) {
                                if let Some(data) = wrapper.get("data") {
                                    if let Ok(ticker) = serde_json::from_value::<BinanceTicker>(data.clone()) {
                                        let price: f64 = ticker.price.parse().unwrap_or(0.0);
                                        let change: f64 = ticker.change_pct.parse().unwrap_or(0.0);
                                        
                                        let mut s = state.write().await;
                                        s.prices.insert(ticker.symbol.clone(), price);
                                        
                                        // Update trend tracking
                                        let entry = s.trends.entry(ticker.symbol.clone())
                                            .or_insert((0.0, Vec::with_capacity(30)));
                                        entry.0 = change;
                                        entry.1.push(price);
                                        if entry.1.len() > 30 {
                                            entry.1.remove(0);
                                        }
                                    }
                                }
                            }
                        }
                        Ok(Message::Close(_)) => {
                            warn!("WebSocket closed, reconnecting...");
                            break;
                        }
                        Err(e) => {
                            error!("WebSocket error: {}", e);
                            break;
                        }
                        _ => {}
                    }
                }
            }
            Err(e) => {
                error!("WebSocket connect failed: {}", e);
            }
        }
        
        sleep(Duration::from_secs(3)).await;
    }
}

async fn trade_loop(
    trader: &Arc<PaperTrader>,
    state: &Arc<RwLock<PriceState>>,
    prices: &HashMap<String, f64>,
    last_slot: &mut u64,
) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    let current_slot = now - (now % 900);
    let time_in_slot = now % 900;
    
    // New window detection
    if current_slot != *last_slot {
        *last_slot = current_slot;
        let price_str: String = ASSETS.iter()
            .filter_map(|a| {
                let sym = a.binance.to_uppercase();
                prices.get(&sym).map(|p| format!("{}: ${:.0}", a.name, p))
            })
            .collect::<Vec<_>>()
            .join(" | ");
        info!("🔄 New 15m window - {}", price_str);
    }
    
    let mut status_parts = Vec::new();
    
    for asset in ASSETS {
        let symbol = asset.binance.to_uppercase();
        let price = match prices.get(&symbol) {
            Some(p) => *p,
            None => continue,
        };
        
        // Get trend from state
        let (trend_pct, trend_dir) = {
            let s = state.read().await;
            if let Some((change, history)) = s.trends.get(&symbol) {
                let dir = if *change > 0.05 { "📈" }
                    else if *change < -0.05 { "📉" }
                    else { "➡️" };
                (*change, dir)
            } else {
                (0.0, "⏳")
            }
        };
        
        // Get Polymarket prices
        let market_slug = format!("{}-{}", asset.poly_slug, current_slot);
        let url = format!("https://gamma-api.polymarket.com/events?slug={}", market_slug);
        
        let resp: Vec<serde_json::Value> = match client.get(&url).send().await {
            Ok(r) => r.json().await.unwrap_or_default(),
            Err(_) => Vec::new(),
        };
        
        if let Some(event) = resp.first() {
            if let Some(market) = event["markets"].as_array().and_then(|m| m.first()) {
                let condition_id = market["conditionId"].as_str().unwrap_or("");
                let question = market["question"].as_str().unwrap_or("");
                
                if let Some(prices_str) = market["outcomePrices"].as_str() {
                    let p: Vec<&str> = prices_str.trim_matches(|c| c == '[' || c == ']' || c == '"')
                        .split("\", \"").collect();
                    
                    if p.len() >= 2 {
                        let up_price: f64 = p[0].parse().unwrap_or(0.5);
                        let down_price: f64 = p[1].parse().unwrap_or(0.5);
                        
                        status_parts.push(format!("{}{} ${:.0} U:{:.0}% D:{:.0}%",
                            trend_dir, asset.name, price, up_price * 100.0, down_price * 100.0));
                        
                        // Trading logic
                        let positions = trader.get_positions().await;
                        let has_position = positions.iter().any(|p|
                            p.market_id.contains(&asset.name.to_lowercase()) &&
                            p.market_id.contains(&current_slot.to_string())
                        );
                        
                        if time_in_slot < 780 && !has_position {
                            trade_asset(trader, asset, condition_id, question,
                                trend_pct, up_price, down_price, current_slot).await;
                        }
                        
                        check_stop_loss(trader, asset, trend_pct, current_slot).await;
                    }
                }
            }
        } else {
            status_parts.push(format!("{}{} ${:.0} (no market)", trend_dir, asset.name, price));
        }
    }
    
    // Compact status every 2 seconds
    info!("📊 {} | {}s", status_parts.join(" | "), time_in_slot);
    
    let summary = trader.get_summary().await;
    info!("💰 ${:.2} | P&L: ${:.2} ({:.2}%) | Trades: {}",
        summary.total_value,
        summary.total_pnl,
        summary.roi_percent,
        summary.trade_count
    );
    
    Ok(())
}

async fn trade_asset(
    trader: &PaperTrader,
    asset: &Asset,
    condition_id: &str,
    question: &str,
    trend_pct: f64,
    up_price: f64,
    down_price: f64,
    current_slot: u64,
) {
    // Follow trend + look for mispricing
    if trend_pct > 0.08 && up_price < 0.55 {
        let amount = if trend_pct > 0.15 { dec!(80) } else { dec!(50) };
        buy_position(trader, asset, "UP", condition_id, question, up_price, amount, current_slot).await;
    }
    else if trend_pct < -0.08 && down_price < 0.55 {
        let amount = if trend_pct < -0.15 { dec!(80) } else { dec!(50) };
        buy_position(trader, asset, "DOWN", condition_id, question, down_price, amount, current_slot).await;
    }
    // Extreme mispricing
    else if up_price < 0.12 && trend_pct > -0.10 {
        buy_position(trader, asset, "UP", condition_id, question, up_price, dec!(60), current_slot).await;
    }
    else if down_price < 0.12 && trend_pct < 0.10 {
        buy_position(trader, asset, "DOWN", condition_id, question, down_price, dec!(60), current_slot).await;
    }
}

async fn check_stop_loss(trader: &PaperTrader, asset: &Asset, trend_pct: f64, current_slot: u64) {
    let positions = trader.get_positions().await;
    let slot_str = current_slot.to_string();
    let asset_lower = asset.name.to_lowercase();
    
    for pos in &positions {
        if pos.market_id.contains(&asset_lower) && pos.market_id.contains(&slot_str) {
            let is_up = pos.market_id.contains("-up");
            let is_down = pos.market_id.contains("-down");
            
            let should_stop = if is_up && trend_pct < -0.12 {
                info!("⚠️ {} STOP: UP but crashing {:.2}%", asset.name, trend_pct);
                true
            } else if is_down && trend_pct > 0.12 {
                info!("⚠️ {} STOP: DOWN but mooning {:.2}%", asset.name, trend_pct);
                true
            } else { false };
            
            if should_stop {
                if let Ok(record) = trader.sell(&pos.id, format!("{} stop-loss", asset.name)).await {
                    let pnl = record.pnl.unwrap_or(dec!(0));
                    info!("🛑 {} CLOSED P&L: ${:.2}", asset.name, pnl);
                }
            }
        }
    }
}

async fn buy_position(
    trader: &PaperTrader,
    asset: &Asset,
    side: &str,
    condition_id: &str,
    question: &str,
    price: f64,
    amount: Decimal,
    current_slot: u64,
) {
    let market_id = format!("{}-15m-{}-{}", asset.name.to_lowercase(), current_slot, side.to_lowercase());
    
    let mock_market = polymarket_bot::types::Market {
        id: market_id,
        question: question.to_string(),
        description: None,
        outcomes: vec![
            polymarket_bot::types::Outcome {
                token_id: condition_id.to_string(),
                outcome: "Yes".to_string(),
                price: Decimal::from_f64_retain(price).unwrap_or(dec!(0.1)),
            },
        ],
        volume: dec!(0),
        liquidity: dec!(0),
        end_date: None,
        active: true,
        closed: false,
    };
    
    match trader.buy(&mock_market, PositionSide::Yes, amount,
        format!("{} {} @ {:.1}%", asset.name, side, price * 100.0)).await
    {
        Ok(_) => info!("🎰 {} BUY {} @ {:.1}% - ${}", asset.name, side, price * 100.0, amount),
        Err(e) => warn!("{} buy failed: {}", asset.name, e),
    }
}
