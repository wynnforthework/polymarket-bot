#!/bin/bash
# Polymarket Bot Startup Script

set -e

cd "$(dirname "$0")"

# Load environment variables
if [ -f .env ]; then
    export $(grep -v '^#' .env | xargs)
fi

# Check required variables
if [ -z "$DEEPSEEK_API_KEY" ]; then
    echo "❌ DEEPSEEK_API_KEY not set"
    exit 1
fi

if [ -z "$TELEGRAM_BOT_TOKEN" ] || [ -z "$TELEGRAM_CHAT_ID" ]; then
    echo "⚠️  Telegram not configured, notifications disabled"
fi

# Create data directory
mkdir -p data

# Build if needed
if [ ! -f target/release/polymarket-bot ] || [ Cargo.toml -nt target/release/polymarket-bot ]; then
    echo "🔨 Building..."
    cargo build --release
fi

# Run
echo "🚀 Starting Polymarket Bot..."
RUST_LOG=info ./target/release/polymarket-bot run "$@"
