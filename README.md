# Polymarket Trading Bot 🎲

A Rust-based automated trading system for Polymarket prediction markets with signal analysis, copy trading, and compound growth strategies.

## Features

### Core Trading
- **Probability Modeling**: Uses DeepSeek/Claude/GPT to analyze markets and estimate probabilities
- **Signal Generation**: Compares model predictions vs market prices to find edge
- **Kelly Criterion**: Dynamic position sizing based on edge, confidence, and recent performance
- **Risk Management**: Daily loss limits, position limits, exposure caps, drawdown protection

### Signal Ingestion
- **Telegram Monitoring**: Monitor alpha channels for market signals
- **Twitter/X Integration**: Follow KOLs for market insights
- **LLM Signal Extraction**: Automatically extract trading signals from text

### Copy Trading
- **Follow Top Traders**: Automatically copy positions from successful traders
- **Configurable Ratio**: Copy 10-100% of their position size
- **Delay Execution**: Avoid detection with configurable delays

### Compound Growth
- **Dynamic Kelly**: Increase sizing on win streaks (up to 2x), reduce on losses (down to 0.5x)
- **Sqrt Scaling**: Balance growth with risk (4x balance → 2x sizing)
- **Drawdown Protection**: Auto-reduce positions at -10% and -20% drawdown

### Analysis
- **Pattern Recognition**: Identify successful trading patterns
- **Trader Profiling**: Categorize traders by style (Contrarian, Scalper, Whale, etc.)
- **Strategy Extraction**: Learn from high performers

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                   Polymarket Trading Bot                    │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  Ingester (TG/X)  ──→  LLM Processor  ──┐                  │
│                                          ↓                  │
│  Copy Trader  ──────────────────────→  Strategy            │
│                                          ↓                  │
│  Market Scanner  ──→  LLM Analyzer  ──→  Signal Gen        │
│                                          ↓                  │
│                                      Executor ──→ Notify   │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

## Quick Start

```bash
# Build
cargo build --release

# Configure
cp .env.example .env
# Edit .env with your keys

# Run
./start.sh
```

## Configuration

### Environment Variables (.env)

```bash
DEEPSEEK_API_KEY=sk-xxx
TELEGRAM_BOT_TOKEN=123456:ABC-xxx
TELEGRAM_CHAT_ID=your_chat_id
POLYMARKET_PRIVATE_KEY=your_wallet_private_key
```

### Main Config (config.toml)

```toml
[llm]
provider = "deepseek"
api_key = "${DEEPSEEK_API_KEY}"
model = "deepseek-chat"

[strategy]
min_edge = 0.06          # 6% minimum edge
min_confidence = 0.60    # 60% confidence threshold
kelly_fraction = 0.35    # 35% Kelly
compound_enabled = true  # Enable compound growth

[risk]
max_position_pct = 0.08  # 8% max per position
max_exposure_pct = 0.60  # 60% max total
max_daily_loss_pct = 0.12

[copy_trade]
enabled = true
follow_users = ["CRYINGLITTLEBABY"]
copy_ratio = 0.5         # 50% of their size
delay_secs = 30

[telegram]
bot_token = "${TELEGRAM_BOT_TOKEN}"
chat_id = "${TELEGRAM_CHAT_ID}"
```

## Usage

```bash
# Run trading bot
./target/release/polymarket-bot run

# Dry run mode
./target/release/polymarket-bot run --dry-run

# View markets
./target/release/polymarket-bot markets

# Analyze a market
./target/release/polymarket-bot analyze <market_id>

# Check status
./target/release/polymarket-bot status
```

## Project Structure

```
src/
├── main.rs              # CLI entry point
├── lib.rs               # Library exports
├── config.rs            # Configuration
├── client/              # Polymarket API
│   ├── clob.rs          # Order book
│   ├── gamma.rs         # Market data
│   └── websocket.rs     # Streaming
├── model/               # Probability models
│   └── llm.rs           # Multi-provider LLM
├── strategy/            # Trading strategies
│   ├── compound.rs      # Compound growth
│   ├── copy_trade.rs    # Copy trading
│   └── mod.rs           # Signal generation
├── ingester/            # Signal collection
│   ├── telegram.rs      # TG monitoring
│   ├── twitter.rs       # X monitoring
│   └── processor.rs     # LLM extraction
├── analysis/            # Pattern recognition
│   ├── pattern.rs       # Trading patterns
│   └── trader_profile.rs
├── executor/            # Trade execution
├── notify/              # Notifications
└── storage/             # Database
```

## Risk Warning ⚠️

This bot trades real money. Use at your own risk.

- Start with small amounts
- Use dry-run mode first
- Monitor closely
- Set conservative limits

## License

MIT
