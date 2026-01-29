//! Configuration management

use rust_decimal::Decimal;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub polymarket: PolymarketConfig,
    pub strategy: StrategyConfig,
    pub risk: RiskConfig,
    pub database: DatabaseConfig,
    pub llm: Option<LlmConfig>,
    pub telegram: Option<TelegramConfig>,
    pub ingester: Option<IngesterConfig>,
    pub copy_trade: Option<CopyTradeConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CopyTradeConfig {
    /// Enable copy trading
    #[serde(default)]
    pub enabled: bool,
    /// Usernames to follow
    #[serde(default)]
    pub follow_users: Vec<String>,
    /// Wallet addresses to follow
    #[serde(default)]
    pub follow_addresses: Vec<String>,
    /// Copy ratio (0.0 - 1.0)
    #[serde(default = "default_copy_ratio")]
    pub copy_ratio: f64,
    /// Delay before copying (seconds)
    #[serde(default)]
    pub delay_secs: u64,
}

fn default_copy_ratio() -> f64 {
    0.5
}

#[derive(Debug, Clone, Deserialize)]
pub struct IngesterConfig {
    /// Enable signal ingestion
    #[serde(default)]
    pub enabled: bool,
    /// Telegram userbot configuration
    pub telegram_userbot: Option<TelegramUserbotConfig>,
    /// Telegram bot configuration (for public channels)
    pub telegram_bot: Option<TelegramBotIngesterConfig>,
    /// Twitter/X configuration
    pub twitter: Option<TwitterConfig>,
    /// Signal processing settings
    #[serde(default)]
    pub processing: ProcessingConfig,
    /// Author trust scores
    #[serde(default)]
    pub author_trust: std::collections::HashMap<String, f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TelegramUserbotConfig {
    /// Telegram API ID
    pub api_id: i32,
    /// Telegram API Hash
    pub api_hash: String,
    /// Session file path
    pub session_file: String,
    /// Chat IDs to monitor
    pub watch_chats: Vec<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TelegramBotIngesterConfig {
    /// Bot token (can reuse from telegram config)
    pub bot_token: String,
    /// Channel usernames to monitor
    pub channels: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TwitterConfig {
    /// Twitter API bearer token
    pub bearer_token: Option<String>,
    /// User IDs to monitor
    pub user_ids: Vec<String>,
    /// Keywords to filter
    #[serde(default)]
    pub keywords: Vec<String>,
    /// Nitter instance for RSS fallback
    pub nitter_instance: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ProcessingConfig {
    /// Aggregation window in seconds
    #[serde(default = "default_aggregation_window")]
    pub aggregation_window_secs: i64,
    /// Minimum confidence to process signal
    #[serde(default = "default_min_confidence")]
    pub min_confidence: f64,
    /// Minimum aggregate score to emit
    #[serde(default = "default_min_agg_score")]
    pub min_agg_score: f64,
}

fn default_aggregation_window() -> i64 {
    300 // 5 minutes
}

fn default_min_confidence() -> f64 {
    0.5
}

fn default_min_agg_score() -> f64 {
    0.6
}

#[derive(Debug, Clone, Deserialize)]
pub struct PolymarketConfig {
    /// CLOB API endpoint
    pub clob_url: String,
    /// Gamma API endpoint (market data)
    pub gamma_url: String,
    /// Private key for signing (hex, without 0x prefix)
    pub private_key: String,
    /// Funder address (for proxy wallets)
    pub funder_address: Option<String>,
    /// Chain ID (137 for Polygon mainnet)
    pub chain_id: u64,
    /// Signature type (0=EOA, 1=Magic, 2=Proxy)
    pub signature_type: u8,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StrategyConfig {
    /// Minimum edge (model vs market) to trigger trade
    pub min_edge: Decimal,
    /// Minimum confidence score to trade
    pub min_confidence: Decimal,
    /// Kelly fraction (e.g., 0.25 for quarter Kelly)
    pub kelly_fraction: Decimal,
    /// Market scan interval in seconds
    pub scan_interval_secs: u64,
    /// Model update interval in seconds
    pub model_update_interval_secs: u64,
    /// Enable compound growth strategy
    #[serde(default)]
    pub compound_enabled: bool,
    /// Use sqrt scaling for compound growth (safer)
    #[serde(default = "default_true")]
    pub compound_sqrt_scaling: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RiskConfig {
    /// Maximum position size as fraction of portfolio (e.g., 0.05 = 5%)
    pub max_position_pct: Decimal,
    /// Maximum total exposure as fraction of portfolio
    pub max_exposure_pct: Decimal,
    /// Maximum daily loss as fraction of portfolio
    pub max_daily_loss_pct: Decimal,
    /// Minimum USDC balance to keep (for gas/reserves)
    pub min_balance_reserve: Decimal,
    /// Maximum number of open positions
    pub max_open_positions: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    /// SQLite database path
    pub path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LlmConfig {
    /// LLM provider: deepseek, anthropic, openai, ollama, compatible
    pub provider: String,
    /// API key (not required for ollama)
    #[serde(default)]
    pub api_key: String,
    /// Model name (optional, uses provider default)
    pub model: Option<String>,
    /// Base URL for OpenAI-compatible APIs
    pub base_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TelegramConfig {
    /// Bot token from @BotFather
    pub bot_token: String,
    /// Chat ID to send messages to (your user ID or group ID)
    pub chat_id: String,
    /// Send signal notifications (default: true)
    #[serde(default = "default_true")]
    pub notify_signals: bool,
    /// Send trade execution notifications (default: true)
    #[serde(default = "default_true")]
    pub notify_trades: bool,
    /// Send error notifications (default: true)
    #[serde(default = "default_true")]
    pub notify_errors: bool,
    /// Send daily reports (default: true)
    #[serde(default = "default_true")]
    pub notify_daily: bool,
}

fn default_true() -> bool {
    true
}

impl Config {
    /// Load configuration from file
    pub fn load<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let settings = config::Config::builder()
            .add_source(config::File::with_name(path.as_ref().to_str().unwrap()))
            .add_source(config::Environment::with_prefix("POLYMARKET"))
            .build()?;

        let config: Config = settings.try_deserialize()?;
        Ok(config)
    }

    /// Load from default locations
    pub fn load_default() -> anyhow::Result<Self> {
        // Try loading from current directory or user config
        let paths = ["config.toml", "config.yaml", "~/.config/polymarket-bot/config.toml"];
        
        for path in paths {
            let expanded = shellexpand::tilde(path);
            if Path::new(expanded.as_ref()).exists() {
                return Self::load(expanded.as_ref());
            }
        }

        anyhow::bail!("No configuration file found")
    }
}

impl Default for StrategyConfig {
    fn default() -> Self {
        Self {
            min_edge: Decimal::new(6, 2),        // 6%
            min_confidence: Decimal::new(60, 2), // 60%
            kelly_fraction: Decimal::new(35, 2), // 35% Kelly
            scan_interval_secs: 180,
            model_update_interval_secs: 900,
            compound_enabled: true,
            compound_sqrt_scaling: true,
        }
    }
}

impl Default for RiskConfig {
    fn default() -> Self {
        Self {
            max_position_pct: Decimal::new(5, 2),     // 5%
            max_exposure_pct: Decimal::new(50, 2),   // 50%
            max_daily_loss_pct: Decimal::new(10, 2), // 10%
            min_balance_reserve: Decimal::new(100, 0), // $100
            max_open_positions: 10,
        }
    }
}
