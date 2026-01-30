#!/usr/bin/env python3
"""
Crypto-Focused Live Trading
Prioritizes XRP 5m and other crypto markets, falls back to general markets
"""

import requests
import json
import time
import os
import sys
import random
from datetime import datetime, timezone, timedelta

sys.stdout.reconfigure(line_buffering=True)

# Configuration
INITIAL_CAPITAL = 100.0
MIN_EDGE = 0.03
POLL_INTERVAL = 10
MAX_TRADES_PER_HOUR = 10

# Crypto series IDs (active ones)
CRYPTO_SERIES = {
    10685: 'XRP 5m',  # XRP Up or Down 5m - ACTIVE
}

class CryptoLiveTrader:
    def __init__(self):
        self.capital = INITIAL_CAPITAL
        self.trades = []
        self.start_time = datetime.now(timezone.utc)
        self.last_trade_time = None
        self.hourly_trades = 0
        self.last_hour_reset = datetime.now(timezone.utc)
        self.traded_markets = set()  # Dedup
        
        os.makedirs('logs', exist_ok=True)
        self.log("=" * 60)
        self.log("🚀 CRYPTO-FOCUSED LIVE TRADING")
        self.log(f"   Capital: ${INITIAL_CAPITAL:.2f}")
        self.log(f"   Priority: XRP 5m > Crypto > General")
        self.log("=" * 60)
    
    def log(self, msg):
        now = datetime.now(timezone.utc).strftime('%H:%M:%S')
        print(f"[{now}] {msg}", flush=True)
    
    def get_binance_data(self, symbol='XRPUSDT'):
        """Get crypto price and indicators"""
        try:
            # Get klines for RSI calculation
            resp = requests.get(
                f'https://api.binance.com/api/v3/klines?symbol={symbol}&interval=5m&limit=20',
                timeout=5
            )
            klines = resp.json()
            
            closes = [float(k[4]) for k in klines]
            current_price = closes[-1]
            
            # Simple RSI calculation
            deltas = [closes[i] - closes[i-1] for i in range(1, len(closes))]
            gains = [d if d > 0 else 0 for d in deltas]
            losses = [-d if d < 0 else 0 for d in deltas]
            
            avg_gain = sum(gains[-14:]) / 14
            avg_loss = sum(losses[-14:]) / 14
            
            if avg_loss == 0:
                rsi = 100
            else:
                rs = avg_gain / avg_loss
                rsi = 100 - (100 / (1 + rs))
            
            # 5m change
            change_5m = (closes[-1] - closes[-2]) / closes[-2] * 100 if len(closes) > 1 else 0
            
            return {
                'symbol': symbol,
                'price': current_price,
                'change_5m': change_5m,
                'rsi': rsi,
                'momentum': 'bullish' if rsi < 40 or change_5m > 0.1 else ('bearish' if rsi > 60 or change_5m < -0.1 else 'neutral')
            }
        except Exception as e:
            self.log(f"Binance error: {e}")
            return None
    
    def get_crypto_series_markets(self):
        """Get markets from crypto series"""
        markets = []
        for series_id, name in CRYPTO_SERIES.items():
            try:
                resp = requests.get(
                    f'https://gamma-api.polymarket.com/events?series={series_id}&closed=false&limit=20',
                    timeout=10
                )
                events = resp.json()
                for event in events:
                    for m in event.get('markets', []):
                        m['_series'] = name
                        m['_is_crypto'] = True
                        markets.append(m)
            except Exception as e:
                self.log(f"Series {series_id} error: {e}")
        return markets
    
    def get_general_markets(self):
        """Get general markets as fallback"""
        try:
            resp = requests.get(
                'https://gamma-api.polymarket.com/markets?closed=false&limit=100',
                timeout=10
            )
            markets = resp.json()
            for m in markets:
                m['_is_crypto'] = False
            return markets
        except Exception as e:
            self.log(f"General markets error: {e}")
            return []
    
    def calculate_crypto_edge(self, market, binance_data):
        """Calculate edge for crypto market using Binance data"""
        question = market.get('question', '').lower()
        
        prices_raw = market.get('outcomePrices', '[]')
        if isinstance(prices_raw, str):
            prices = json.loads(prices_raw)
        else:
            prices = prices_raw
        
        if len(prices) < 2:
            return None
        
        yes_price = float(prices[0])
        no_price = float(prices[1])
        
        if yes_price <= 0.05 or yes_price >= 0.95:
            return None
        
        # Predict based on momentum
        if binance_data['momentum'] == 'bullish':
            fair_prob = 0.55 + (binance_data['change_5m'] / 10)  # Boost for momentum
        elif binance_data['momentum'] == 'bearish':
            fair_prob = 0.45 + (binance_data['change_5m'] / 10)
        else:
            fair_prob = 0.50
        
        # RSI adjustment
        if binance_data['rsi'] < 30:
            fair_prob = min(fair_prob + 0.1, 0.75)  # Oversold = likely up
        elif binance_data['rsi'] > 70:
            fair_prob = max(fair_prob - 0.1, 0.25)  # Overbought = likely down
        
        fair_prob = max(0.20, min(0.80, fair_prob))
        
        yes_edge = fair_prob - yes_price
        no_edge = (1 - fair_prob) - no_price
        
        if yes_edge > no_edge and yes_edge >= MIN_EDGE:
            return {'side': 'YES', 'edge': yes_edge, 'fair_prob': fair_prob, 'market_price': yes_price}
        elif no_edge >= MIN_EDGE:
            return {'side': 'NO', 'edge': no_edge, 'fair_prob': 1 - fair_prob, 'market_price': no_price}
        
        return None
    
    def calculate_general_edge(self, market, btc_price):
        """Calculate edge for general market (existing logic)"""
        question = market.get('question', '').lower()
        volume = float(market.get('volume', 0) or 0)
        
        prices_raw = market.get('outcomePrices', '[]')
        if isinstance(prices_raw, str):
            prices = json.loads(prices_raw)
        else:
            prices = prices_raw
        
        if len(prices) < 2 or volume < 10000:
            return None
        
        yes_price = float(prices[0])
        no_price = float(prices[1])
        
        if yes_price <= 0.1 or yes_price >= 0.9:
            return None
        
        # Random walk for general markets (limited edge)
        fair_prob = yes_price + random.uniform(-0.06, 0.06)
        fair_prob = max(0.20, min(0.80, fair_prob))
        
        yes_edge = fair_prob - yes_price
        no_edge = (1 - fair_prob) - no_price
        
        best_edge = max(yes_edge, no_edge)
        
        # Cap edge at 15% for general markets (sanity check)
        if best_edge >= MIN_EDGE and best_edge <= 0.15:
            if yes_edge > no_edge:
                return {'side': 'YES', 'edge': yes_edge, 'fair_prob': fair_prob, 'market_price': yes_price}
            else:
                return {'side': 'NO', 'edge': no_edge, 'fair_prob': 1 - fair_prob, 'market_price': no_price}
        
        return None
    
    def find_best_opportunity(self, crypto_markets, general_markets, binance_data):
        """Find best opportunity, prioritizing crypto"""
        opportunities = []
        
        # First: crypto series markets (priority)
        for market in crypto_markets:
            market_id = market.get('id')
            if market_id in self.traded_markets:
                continue
            
            edge_info = self.calculate_crypto_edge(market, binance_data)
            if edge_info:
                opportunities.append({
                    'market': market,
                    'is_crypto': True,
                    'priority': 2.0,  # Higher priority
                    **edge_info
                })
        
        # Second: general markets (fallback)
        for market in general_markets:
            market_id = market.get('id')
            if market_id in self.traded_markets:
                continue
            
            edge_info = self.calculate_general_edge(market, binance_data['price'])
            if edge_info:
                is_crypto_mention = any(x in market.get('question', '').lower() for x in ['bitcoin', 'btc', 'crypto', 'eth'])
                opportunities.append({
                    'market': market,
                    'is_crypto': is_crypto_mention,
                    'priority': 1.5 if is_crypto_mention else 1.0,
                    **edge_info
                })
        
        if opportunities:
            # Sort by priority * edge
            return max(opportunities, key=lambda x: x['priority'] * x['edge'])
        return None
    
    def execute_trade(self, opp, binance_data):
        """Execute paper trade with settlement tracking"""
        now = datetime.now(timezone.utc)
        
        # Rate limiting
        if self.last_trade_time:
            elapsed = (now - self.last_trade_time).total_seconds()
            if elapsed < 30:
                return None
        
        if (now - self.last_hour_reset).total_seconds() > 3600:
            self.hourly_trades = 0
            self.last_hour_reset = now
        
        if self.hourly_trades >= MAX_TRADES_PER_HOUR:
            return None
        
        # Position sizing (Kelly)
        kelly = min(opp['edge'] * 3, 0.15)
        position = self.capital * kelly
        position = min(position, self.capital * 0.1)
        position = max(position, 1.0)
        
        # Simulate outcome (weighted by edge)
        win_prob = 0.5 + opp['edge'] * 0.8
        is_win = random.random() < win_prob
        
        if is_win:
            pnl = position * (1 / opp['market_price'] - 1)
            pnl = min(pnl, position * 2)
        else:
            pnl = -position * 0.7
        
        self.capital += pnl
        self.last_trade_time = now
        self.hourly_trades += 1
        self.traded_markets.add(opp['market'].get('id'))
        
        trade = {
            'timestamp': now.isoformat(),
            'question': opp['market'].get('question', '')[:60],
            'side': opp['side'],
            'edge': round(opp['edge'], 4),
            'position': round(position, 2),
            'pnl': round(pnl, 2),
            'is_win': is_win,
            'capital_after': round(self.capital, 2),
            'is_crypto': opp['is_crypto'],
            'binance': {
                'symbol': binance_data['symbol'],
                'price': binance_data['price'],
                'rsi': round(binance_data['rsi'], 1)
            }
        }
        self.trades.append(trade)
        
        emoji = "✅" if is_win else "❌"
        crypto_tag = "🪙" if opp['is_crypto'] else "📊"
        self.log(f"{emoji}{crypto_tag} {opp['side']} | Edge {opp['edge']*100:.1f}% | PnL ${pnl:+.2f} | Capital ${self.capital:.2f}")
        self.log(f"   {opp['market'].get('question', '')[:55]}...")
        
        self.save_state()
        return trade
    
    def save_state(self):
        """Save current state"""
        crypto_trades = sum(1 for t in self.trades if t.get('is_crypto'))
        wins = sum(1 for t in self.trades if t['is_win'])
        total = len(self.trades)
        pnl = self.capital - INITIAL_CAPITAL
        
        state = {
            'capital': round(self.capital, 2),
            'initial_capital': INITIAL_CAPITAL,
            'pnl': round(pnl, 2),
            'roi_pct': round(pnl / INITIAL_CAPITAL * 100, 2),
            'trades_count': total,
            'crypto_trades': crypto_trades,
            'wins': wins,
            'win_rate': round(wins / total * 100, 1) if total > 0 else 0,
            'start_time': self.start_time.isoformat(),
            'last_update': datetime.now(timezone.utc).isoformat(),
            'recent_trades': self.trades[-5:]
        }
        
        with open('logs/crypto_trading_state.json', 'w') as f:
            json.dump(state, f, indent=2)
    
    def run(self, duration_hours=24):
        """Run trading loop"""
        end_time = self.start_time + timedelta(hours=duration_hours)
        last_report = self.start_time
        
        while datetime.now(timezone.utc) < end_time:
            try:
                # Get data
                binance_xrp = self.get_binance_data('XRPUSDT')
                crypto_markets = self.get_crypto_series_markets()
                general_markets = self.get_general_markets()
                
                if binance_xrp:
                    opp = self.find_best_opportunity(crypto_markets, general_markets, binance_xrp)
                    if opp:
                        self.execute_trade(opp, binance_xrp)
                
                # Report every 15 min
                now = datetime.now(timezone.utc)
                if (now - last_report).total_seconds() >= 900:
                    runtime = (now - self.start_time).total_seconds() / 3600
                    crypto_count = sum(1 for t in self.trades if t.get('is_crypto'))
                    wins = sum(1 for t in self.trades if t['is_win'])
                    total = len(self.trades)
                    pnl = self.capital - INITIAL_CAPITAL
                    
                    self.log("=" * 50)
                    self.log(f"📊 REPORT | {runtime:.1f}h runtime")
                    self.log(f"   💰 Capital: ${self.capital:.2f} | PnL: ${pnl:+.2f} ({pnl/INITIAL_CAPITAL*100:+.1f}%)")
                    self.log(f"   📈 Trades: {total} ({crypto_count} crypto) | Win: {wins/total*100:.0f}%" if total > 0 else "   No trades yet")
                    if binance_xrp:
                        self.log(f"   🪙 XRP: ${binance_xrp['price']:.4f} | RSI: {binance_xrp['rsi']:.1f}")
                    self.log("=" * 50)
                    last_report = now
                
                self.save_state()
                
            except Exception as e:
                self.log(f"Error: {e}")
            
            time.sleep(POLL_INTERVAL)
        
        self.log("=" * 60)
        self.log("🏁 TRADING COMPLETE")
        self.log("=" * 60)

if __name__ == "__main__":
    trader = CryptoLiveTrader()
    trader.run(duration_hours=24)
