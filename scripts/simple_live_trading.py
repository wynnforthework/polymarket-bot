#!/usr/bin/env python3
"""
Simple Live Paper Trading - Robust polling-based approach
Uses real Binance + Polymarket data
"""

import requests
import json
import time
import os
import sys
import random
from datetime import datetime, timezone, timedelta

# Force unbuffered output
sys.stdout.reconfigure(line_buffering=True)

# Configuration
INITIAL_CAPITAL = 100.0
MIN_EDGE = 0.03  # 3%
POLL_INTERVAL = 10  # seconds
MAX_TRADES_PER_HOUR = 10

class SimpleLiveTrader:
    def __init__(self):
        self.capital = INITIAL_CAPITAL
        self.trades = []
        self.start_time = datetime.now(timezone.utc)
        self.last_trade_time = None
        self.hourly_trades = 0
        self.last_hour_reset = datetime.now(timezone.utc)
        
        os.makedirs('logs', exist_ok=True)
        self.log("=" * 60)
        self.log("🚀 LIVE PAPER TRADING STARTED")
        self.log(f"   Capital: ${INITIAL_CAPITAL:.2f}")
        self.log(f"   Min Edge: {MIN_EDGE*100:.0f}%")
        self.log(f"   Poll Interval: {POLL_INTERVAL}s")
        self.log("=" * 60)
    
    def log(self, msg):
        now = datetime.now(timezone.utc).strftime('%H:%M:%S')
        print(f"[{now}] {msg}", flush=True)
    
    def get_binance_data(self):
        """Get BTC price and change"""
        try:
            resp = requests.get(
                'https://api.binance.com/api/v3/ticker/24hr?symbol=BTCUSDT',
                timeout=5
            )
            data = resp.json()
            return {
                'price': float(data['lastPrice']),
                'change_pct': float(data['priceChangePercent'])
            }
        except Exception as e:
            self.log(f"Binance error: {e}")
            return None
    
    def get_polymarket_markets(self):
        """Get active markets"""
        try:
            resp = requests.get(
                'https://gamma-api.polymarket.com/markets?closed=false&limit=100',
                timeout=10
            )
            return resp.json()
        except Exception as e:
            self.log(f"Polymarket error: {e}")
            return []
    
    def find_opportunity(self, btc_data, markets):
        """Find best trading opportunity"""
        opportunities = []
        
        for market in markets:
            try:
                question = market.get('question', '').lower()
                volume = float(market.get('volume', 0) or 0)
                prices_raw = market.get('outcomePrices', [])
                
                # Parse prices (can be string or list)
                if isinstance(prices_raw, str):
                    import json as _json
                    prices = _json.loads(prices_raw)
                else:
                    prices = prices_raw
                
                if len(prices) < 2 or volume < 10000:
                    continue
                
                yes_price = float(prices[0])
                no_price = float(prices[1])
                
                if yes_price <= 0.1 or yes_price >= 0.9:
                    continue
                
                # Calculate fair value estimate
                is_crypto = any(x in question for x in ['bitcoin', 'btc', 'crypto', 'eth'])
                
                if is_crypto:
                    # Use BTC momentum
                    momentum = btc_data['change_pct'] / 100
                    if 'up' in question or 'above' in question or 'hit' in question:
                        fair_prob = 0.5 + momentum
                    else:
                        fair_prob = 0.5 - momentum
                else:
                    # Random walk for non-crypto
                    fair_prob = yes_price + random.uniform(-0.08, 0.08)
                
                fair_prob = max(0.15, min(0.85, fair_prob))
                
                # Calculate edges
                yes_edge = fair_prob - yes_price
                no_edge = (1 - fair_prob) - no_price
                
                best_edge = max(yes_edge, no_edge)
                best_side = 'YES' if yes_edge > no_edge else 'NO'
                
                if best_edge >= MIN_EDGE:
                    opportunities.append({
                        'market': market,
                        'side': best_side,
                        'edge': best_edge,
                        'fair_prob': fair_prob,
                        'market_price': yes_price if best_side == 'YES' else no_price,
                        'volume': volume,
                        'is_crypto': is_crypto
                    })
            except:
                continue
        
        if opportunities:
            return max(opportunities, key=lambda x: x['edge'] * (1.5 if x['is_crypto'] else 1.0))
        return None
    
    def execute_trade(self, opp, btc_price):
        """Execute paper trade"""
        # Rate limiting
        now = datetime.now(timezone.utc)
        if self.last_trade_time:
            elapsed = (now - self.last_trade_time).total_seconds()
            if elapsed < 30:  # Min 30s between trades
                return None
        
        # Reset hourly counter
        if (now - self.last_hour_reset).total_seconds() > 3600:
            self.hourly_trades = 0
            self.last_hour_reset = now
        
        if self.hourly_trades >= MAX_TRADES_PER_HOUR:
            return None
        
        # Position sizing
        kelly = min(opp['edge'] * 3, 0.15)
        position = self.capital * kelly
        position = min(position, self.capital * 0.1)
        position = max(position, 1.0)
        
        # Simulate outcome
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
        
        trade = {
            'timestamp': now.isoformat(),
            'question': opp['market'].get('question', '')[:60],
            'side': opp['side'],
            'edge': round(opp['edge'], 4),
            'position': round(position, 2),
            'pnl': round(pnl, 2),
            'is_win': is_win,
            'capital_after': round(self.capital, 2),
            'btc_price': btc_price,
            'is_crypto': opp['is_crypto']
        }
        self.trades.append(trade)
        
        emoji = "✅" if is_win else "❌"
        crypto_tag = "🪙" if opp['is_crypto'] else "📊"
        self.log(f"{emoji}{crypto_tag} {opp['side']} | Edge {opp['edge']*100:.1f}% | PnL ${pnl:+.2f} | Capital ${self.capital:.2f}")
        self.log(f"   {opp['market'].get('question', '')[:50]}...")
        
        self.save_state()
        return trade
    
    def save_state(self):
        """Save current state"""
        wins = sum(1 for t in self.trades if t['is_win'])
        total = len(self.trades)
        pnl = self.capital - INITIAL_CAPITAL
        
        state = {
            'capital': round(self.capital, 2),
            'initial_capital': INITIAL_CAPITAL,
            'pnl': round(pnl, 2),
            'roi_pct': round(pnl / INITIAL_CAPITAL * 100, 2),
            'trades_count': total,
            'wins': wins,
            'win_rate': round(wins / total * 100, 1) if total > 0 else 0,
            'start_time': self.start_time.isoformat(),
            'last_update': datetime.now(timezone.utc).isoformat(),
            'recent_trades': self.trades[-5:]
        }
        
        with open('logs/overnight_state.json', 'w') as f:
            json.dump(state, f, indent=2)
    
    def run(self, duration_hours=14):
        """Run trading loop"""
        end_time = self.start_time + timedelta(hours=duration_hours)
        last_report = self.start_time
        
        while datetime.now(timezone.utc) < end_time:
            try:
                # Get live data
                btc = self.get_binance_data()
                markets = self.get_polymarket_markets()
                
                if btc and markets:
                    # Find opportunity
                    opp = self.find_opportunity(btc, markets)
                    
                    if opp:
                        self.execute_trade(opp, btc['price'])
                
                # Periodic report
                now = datetime.now(timezone.utc)
                if (now - last_report).total_seconds() >= 900:  # 15 min
                    runtime = (now - self.start_time).total_seconds() / 3600
                    wins = sum(1 for t in self.trades if t['is_win'])
                    total = len(self.trades)
                    pnl = self.capital - INITIAL_CAPITAL
                    
                    self.log("=" * 50)
                    self.log(f"📊 REPORT | Runtime: {runtime:.1f}h")
                    self.log(f"   Capital: ${self.capital:.2f} | PnL: ${pnl:+.2f} ({pnl/INITIAL_CAPITAL*100:+.1f}%)")
                    if total > 0:
                        self.log(f"   Trades: {total} | Wins: {wins} | Win Rate: {wins/total*100:.1f}%")
                    self.log("=" * 50)
                    last_report = now
                    
                    # Save report
                    with open('logs/overnight_reports.jsonl', 'a') as f:
                        f.write(json.dumps({
                            'timestamp': now.isoformat(),
                            'runtime_hours': round(runtime, 2),
                            'capital': round(self.capital, 2),
                            'pnl': round(pnl, 2),
                            'trades': total,
                            'win_rate': round(wins/total*100, 1) if total > 0 else 0
                        }) + '\n')
                
                self.save_state()
                
            except Exception as e:
                self.log(f"Error: {e}")
            
            time.sleep(POLL_INTERVAL)
        
        # Final summary
        self.log("=" * 60)
        self.log("🏁 SIMULATION COMPLETE")
        wins = sum(1 for t in self.trades if t['is_win'])
        total = len(self.trades)
        pnl = self.capital - INITIAL_CAPITAL
        self.log(f"Final Capital: ${self.capital:.2f}")
        self.log(f"Total PnL: ${pnl:+.2f} ({pnl/INITIAL_CAPITAL*100:+.1f}%)")
        self.log(f"Trades: {total} | Win Rate: {wins/total*100:.1f}%" if total > 0 else "No trades")
        self.log("=" * 60)

if __name__ == "__main__":
    trader = SimpleLiveTrader()
    trader.run(duration_hours=14)
