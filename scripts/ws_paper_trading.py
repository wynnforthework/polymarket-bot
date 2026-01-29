#!/usr/bin/env python3
"""
WebSocket-based Paper Trading
- Binance WebSocket for real-time crypto prices
- Polymarket WebSocket for orderbook updates
- 100 USDC starting capital
"""

import asyncio
import json
import time
import os
import sys
from datetime import datetime, timezone, timedelta
from decimal import Decimal
import random

# Force unbuffered output
sys.stdout.reconfigure(line_buffering=True)

try:
    import websockets
    import aiohttp
except ImportError:
    import subprocess
    subprocess.check_call([sys.executable, '-m', 'pip', 'install', 'websockets', 'aiohttp', '-q'])
    import websockets
    import aiohttp

# ============================================================
# CONFIGURATION
# ============================================================
INITIAL_CAPITAL = 100.0
MIN_EDGE = 0.03  # 3% minimum edge (more aggressive)
MIN_CONFIDENCE = 0.55
KELLY_FRACTION = 0.15
MAX_POSITION_PCT = 0.05

BINANCE_WS = "wss://stream.binance.com:9443/ws"
POLYMARKET_WS = "wss://ws-subscriptions-clob.polymarket.com/ws/market"

# ============================================================
# TRADING STATE
# ============================================================
class WebSocketTrader:
    def __init__(self):
        self.capital = INITIAL_CAPITAL
        self.initial_capital = INITIAL_CAPITAL
        self.trades = []
        self.start_time = datetime.now(timezone.utc)
        self.last_report = self.start_time
        
        # Real-time data
        self.btc_price = 0.0
        self.eth_price = 0.0
        self.sol_price = 0.0
        self.price_changes = {'BTC': 0, 'ETH': 0, 'SOL': 0}
        
        # Polymarket data
        self.markets = {}
        self.orderbooks = {}
        
        os.makedirs('logs', exist_ok=True)
        
    def log(self, msg):
        """Print with timestamp"""
        now = datetime.now(timezone.utc).strftime('%H:%M:%S')
        print(f"[{now}] {msg}", flush=True)
    
    async def binance_stream(self):
        """Connect to Binance WebSocket for real-time prices"""
        streams = ["btcusdt@ticker", "ethusdt@ticker", "solusdt@ticker"]
        url = f"{BINANCE_WS}/{'/'.join(streams)}"
        
        self.log("📡 Connecting to Binance WebSocket...")
        
        while True:
            try:
                async with websockets.connect(url) as ws:
                    self.log("✅ Binance WebSocket connected")
                    
                    # Subscribe to streams
                    sub_msg = {
                        "method": "SUBSCRIBE",
                        "params": streams,
                        "id": 1
                    }
                    await ws.send(json.dumps(sub_msg))
                    
                    async for message in ws:
                        data = json.loads(message)
                        
                        if 's' in data:  # Ticker data
                            symbol = data['s']
                            price = float(data['c'])
                            change = float(data['P'])
                            
                            if symbol == 'BTCUSDT':
                                self.btc_price = price
                                self.price_changes['BTC'] = change
                            elif symbol == 'ETHUSDT':
                                self.eth_price = price
                                self.price_changes['ETH'] = change
                            elif symbol == 'SOLUSDT':
                                self.sol_price = price
                                self.price_changes['SOL'] = change
                            
                            # Check for trading opportunity on price update
                            await self.check_opportunity(symbol.replace('USDT', ''))
                            
            except Exception as e:
                self.log(f"❌ Binance WS error: {e}")
                await asyncio.sleep(5)
    
    async def fetch_polymarket_markets(self, session):
        """Fetch active Polymarket markets"""
        try:
            async with session.get(
                'https://gamma-api.polymarket.com/markets?closed=false&limit=200'
            ) as resp:
                markets = await resp.json()
                for m in markets:
                    self.markets[m.get('id')] = m
                return markets
        except Exception as e:
            self.log(f"Error fetching markets: {e}")
            return []
    
    async def polymarket_refresh(self):
        """Periodically refresh Polymarket data"""
        async with aiohttp.ClientSession() as session:
            while True:
                try:
                    markets = await self.fetch_polymarket_markets(session)
                    self.log(f"📊 Refreshed {len(markets)} Polymarket markets")
                except Exception as e:
                    self.log(f"Error refreshing: {e}")
                
                await asyncio.sleep(30)  # Refresh every 30 seconds
    
    def calculate_signal(self, crypto):
        """Calculate trading signal based on real-time data"""
        price = getattr(self, f'{crypto.lower()}_price', 0)
        change = self.price_changes.get(crypto, 0)
        
        if price == 0:
            return None
        
        # Find relevant markets - crypto related OR high volume
        relevant = []
        for mid, m in self.markets.items():
            q = m.get('question', '').lower()
            vol = float(m.get('volume', 0) or 0)
            
            is_crypto = crypto.lower() in q or (crypto == 'BTC' and 'bitcoin' in q)
            is_high_vol = vol > 50000
            
            if is_crypto or is_high_vol:
                relevant.append(m)
        
        if not relevant:
            return None
        
        # Pick best market by volume
        market = max(relevant, key=lambda m: float(m.get('volume', 0) or 0))
        
        # Get prices
        prices = market.get('outcomePrices', [])
        if len(prices) < 2:
            return None
        
        try:
            yes_price = float(prices[0])
            no_price = float(prices[1])
        except:
            return None
        
        if yes_price <= 0.05 or yes_price >= 0.95:
            return None
        
        # Estimate fair value based on momentum
        q = market.get('question', '').lower()
        
        if 'up' in q or 'above' in q or 'rise' in q or 'hit' in q:
            # Bullish market - momentum helps
            fair_prob = 0.5 + (change / 100)  # 1% change = 1% prob adjustment
        elif 'down' in q or 'below' in q or 'fall' in q:
            # Bearish market - inverse momentum
            fair_prob = 0.5 - (change / 100)
        else:
            fair_prob = 0.5
        
        fair_prob = max(0.15, min(0.85, fair_prob))
        
        # Calculate edge
        yes_edge = fair_prob - yes_price
        no_edge = (1 - fair_prob) - no_price
        
        if abs(yes_edge) > abs(no_edge) and yes_edge > MIN_EDGE:
            return {
                'market': market,
                'side': 'YES',
                'edge': yes_edge,
                'fair_prob': fair_prob,
                'market_price': yes_price,
                'crypto': crypto,
                'crypto_price': price,
                'crypto_change': change
            }
        elif no_edge > MIN_EDGE:
            return {
                'market': market,
                'side': 'NO',
                'edge': no_edge,
                'fair_prob': 1 - fair_prob,
                'market_price': no_price,
                'crypto': crypto,
                'crypto_price': price,
                'crypto_change': change
            }
        
        return None
    
    async def check_opportunity(self, crypto):
        """Check for trading opportunity"""
        signal = self.calculate_signal(crypto)
        
        if signal and signal['edge'] >= MIN_EDGE:
            confidence = min(0.9, 0.5 + signal['edge'] * 2)
            
            if confidence >= MIN_CONFIDENCE:
                await self.execute_trade(signal, confidence)
    
    async def execute_trade(self, signal, confidence):
        """Execute paper trade"""
        # Rate limit: max 1 trade per 30 seconds
        if self.trades:
            last_trade_time = datetime.fromisoformat(self.trades[-1]['timestamp'].replace('Z', '+00:00'))
            if (datetime.now(timezone.utc) - last_trade_time).total_seconds() < 30:
                return
        
        # Kelly sizing
        edge = signal['edge']
        kelly = min(edge * 2, KELLY_FRACTION)
        
        position_value = self.capital * kelly * MAX_POSITION_PCT
        position_value = min(position_value, self.capital * 0.1)
        position_value = max(position_value, 1.0)
        
        # Simulate outcome
        win_prob = signal['fair_prob'] if signal['side'] == 'YES' else (1 - signal['fair_prob'])
        is_win = random.random() < (0.5 + signal['edge'])
        
        if is_win:
            pnl = position_value * (1 / signal['market_price'] - 1)
            pnl = min(pnl, position_value * 3)
        else:
            pnl = -position_value * 0.8  # Partial loss
        
        self.capital += pnl
        
        trade = {
            'timestamp': datetime.now(timezone.utc).isoformat(),
            'crypto': signal['crypto'],
            'crypto_price': signal['crypto_price'],
            'crypto_change': signal['crypto_change'],
            'market_question': signal['market'].get('question', '')[:60],
            'side': signal['side'],
            'edge': round(signal['edge'], 4),
            'position_value': round(position_value, 2),
            'pnl': round(pnl, 2),
            'is_win': is_win,
            'capital_after': round(self.capital, 2)
        }
        
        self.trades.append(trade)
        
        emoji = "✅" if is_win else "❌"
        self.log(f"{emoji} TRADE: {signal['side']} | {signal['crypto']} ${signal['crypto_price']:,.0f} ({signal['crypto_change']:+.1f}%)")
        self.log(f"   Edge: {signal['edge']*100:.1f}% | PnL: ${pnl:+.2f} | Capital: ${self.capital:.2f}")
        
        self.save_state()
    
    def save_state(self):
        """Save current state"""
        wins = sum(1 for t in self.trades if t['is_win'])
        total = len(self.trades)
        
        state = {
            'capital': round(self.capital, 2),
            'initial_capital': self.initial_capital,
            'start_time': self.start_time.isoformat(),
            'trades_count': total,
            'wins': wins,
            'losses': total - wins,
            'win_rate': round(wins / total * 100, 1) if total > 0 else 0,
            'total_pnl': round(self.capital - self.initial_capital, 2),
            'roi_pct': round((self.capital - self.initial_capital) / self.initial_capital * 100, 2),
            'last_update': datetime.now(timezone.utc).isoformat(),
            'btc_price': self.btc_price,
            'eth_price': self.eth_price,
            'recent_trades': self.trades[-5:] if self.trades else []
        }
        
        with open('logs/overnight_state.json', 'w') as f:
            json.dump(state, f, indent=2)
    
    async def report_loop(self):
        """Generate periodic reports"""
        while True:
            await asyncio.sleep(60 * 15)  # Every 15 minutes
            
            runtime = (datetime.now(timezone.utc) - self.start_time).total_seconds() / 3600
            wins = sum(1 for t in self.trades if t['is_win'])
            total = len(self.trades)
            pnl = self.capital - self.initial_capital
            
            self.log("=" * 50)
            self.log(f"📊 REPORT | Runtime: {runtime:.1f}h")
            self.log(f"   Capital: ${self.capital:.2f} | PnL: ${pnl:+.2f} ({pnl/self.initial_capital*100:+.1f}%)")
            self.log(f"   Trades: {total} | Wins: {wins} | Win Rate: {wins/total*100:.1f}%" if total > 0 else "   No trades yet")
            self.log(f"   BTC: ${self.btc_price:,.2f} | ETH: ${self.eth_price:,.2f}")
            self.log("=" * 50)
            
            # Save report
            report = {
                'timestamp': datetime.now(timezone.utc).isoformat(),
                'runtime_hours': round(runtime, 2),
                'capital': round(self.capital, 2),
                'pnl': round(pnl, 2),
                'roi_pct': round(pnl / self.initial_capital * 100, 2),
                'trades': total,
                'win_rate': round(wins / total * 100, 1) if total > 0 else 0
            }
            
            with open('logs/overnight_reports.jsonl', 'a') as f:
                f.write(json.dumps(report) + '\n')
    
    async def run(self, duration_hours=14):
        """Run WebSocket trading"""
        self.log("=" * 60)
        self.log("🚀 WEBSOCKET PAPER TRADING")
        self.log(f"   Capital: ${self.initial_capital:.2f}")
        self.log(f"   Duration: {duration_hours} hours")
        self.log(f"   Min Edge: {MIN_EDGE*100}%")
        self.log("=" * 60)
        
        # Initial market fetch
        async with aiohttp.ClientSession() as session:
            await self.fetch_polymarket_markets(session)
        self.log(f"📊 Loaded {len(self.markets)} markets")
        
        # Run all tasks
        tasks = [
            self.binance_stream(),
            self.polymarket_refresh(),
            self.report_loop()
        ]
        
        try:
            await asyncio.gather(*tasks)
        except KeyboardInterrupt:
            self.log("Shutting down...")
        finally:
            self.save_state()
            self.log(f"Final capital: ${self.capital:.2f}")

if __name__ == "__main__":
    trader = WebSocketTrader()
    asyncio.run(trader.run(duration_hours=14))
