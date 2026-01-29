# Polymarket Trading Bot 🎲

A Rust-based automated trading system for Polymarket prediction markets.

## Features

- **Probability Modeling**: Uses Claude/GPT to analyze markets and estimate "true" probabilities
- **Signal Generation**: Compares model predictions vs market prices to find edge
- **Kelly Criterion**: Optimal position sizing based on edge and confidence
- **Risk Management**: Daily loss limits, position limits, exposure caps
- **Real-time Data**: WebSocket streaming for live price updates
- **Trade Execution**: Full CLOB integration for order placement
- **Performance Tracking**: SQLite storage for trade history and analytics

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                   Polymarket Trading Bot                    │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐     │
│  │   Data      │───>│  Strategy   │───>│  Executor   │     │
│  │  (Client)   │    │  (Model)    │    │  (Risk)     │     │
│  └─────────────┘    └─────────────┘    └─────────────┘     │
└─────────────────────────────────────────────────────────────┘
```

## Installation

```bash
# Clone the repo
git clone https://github.com/yourusername/polymarket-bot
cd polymarket-bot

# Build
cargo build --release

# Copy and edit config
cp config.example.toml config.toml
# Edit config.toml with your keys
```

## Configuration

Create `config.toml`:

```toml
[polymarket]
clob_url = "https://clob.polymarket.com"
gamma_url = "https://gamma-api.polymarket.com"
private_key = "YOUR_PRIVATE_KEY"  # Without 0x prefix
chain_id = 137  # Polygon mainnet

[strategy]
min_edge = 0.10        # 10% minimum edge
min_confidence = 0.60  # 60% confidence threshold
kelly_fraction = 0.25  # Quarter Kelly

[risk]
max_position_pct = 0.05    # 5% max per position
max_exposure_pct = 0.50    # 50% max total exposure
max_daily_loss_pct = 0.10  # 10% daily loss limit

[database]
path = "data/polymarket.db"

[llm]
provider = "anthropic"
api_key = "YOUR_ANTHROPIC_KEY"
model = "claude-sonnet-4-20250514"
```

## Usage

```bash
# Run the trading bot
./polymarket-bot run

# Run in dry-run mode (no actual trades)
./polymarket-bot run --dry-run

# View top markets
./polymarket-bot markets --limit 20

# Analyze a specific market
./polymarket-bot analyze <market_id>

# Check account status
./polymarket-bot status
```

## Project Structure

```
src/
├── main.rs          # CLI entry point
├── lib.rs           # Library exports
├── types.rs         # Core types (Market, Order, Signal)
├── error.rs         # Error definitions
├── config.rs        # Configuration management
├── client/          # Polymarket API clients
│   ├── auth.rs      # EIP-712 signing
│   ├── clob.rs      # Order book API
│   ├── gamma.rs     # Market data API
│   └── websocket.rs # Real-time streaming
├── model/           # Probability models
│   ├── llm.rs       # Claude/GPT analysis
│   └── sentiment.rs # Sentiment analysis
├── strategy/        # Trading strategy
│   └── mod.rs       # Signal generation + Kelly
├── executor/        # Trade execution
│   └── mod.rs       # Risk management
├── monitor/         # Performance tracking
└── storage/         # SQLite persistence
```

## How It Works

1. **Scan Markets**: Fetch active markets from Gamma API
2. **Predict Probabilities**: Use LLM to estimate "true" probability
3. **Find Edge**: Compare model vs market price
4. **Generate Signal**: If edge > threshold, create trade signal
5. **Size Position**: Use Kelly criterion for optimal sizing
6. **Execute Trade**: Place order via CLOB API
7. **Monitor**: Track performance and enforce risk limits

## Risk Warning ⚠️

This bot trades real money. Use at your own risk.

- Start with small amounts
- Use dry-run mode extensively
- Monitor the bot closely
- Set conservative risk limits
- The model can be wrong

## License

MIT
