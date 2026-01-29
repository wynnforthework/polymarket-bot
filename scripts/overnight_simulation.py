#!/usr/bin/env python3
"""
Overnight Paper Trading Simulation
- Real Polymarket data + Binance prices
- 100 USDC starting capital
- Runs until morning (~12-16 hours)
- Reports every 15-30 minutes
"""

import requests
import json
import time
import os
from datetime import datetime, timezone, timedelta
from decimal import Decimal
import random

# ============================================================
# CONFIGURATION
# ============================================================
INITIAL_CAPITAL = 100.0
MIN_EDGE = 0.05  # 5% minimum edge
MIN_CONFIDENCE = 0.65
KELLY_FRACTION = 0.15
MAX_POSITION_PCT = 0.03  # 3% max per trade
SCAN_INTERVAL_SECONDS = 60  # Scan every minute
REPORT_INTERVAL_MINUTES = 15

# ============================================================
# SIMULATION STATE
# ============================================================
class OvernightSimulator:
    def __init__(self):
        self.capital = INITIAL_CAPITAL
        self.initial_capital = INITIAL_CAPITAL
        self.positions = []  # Open positions
        self.closed_trades = []  # Completed trades
        self.equity_curve = []
        self.start_time = datetime.now(timezone.utc)
        self.last_report_time = self.start_time
        self.scan_count = 0
        self.state_file = 'logs/overnight_state.json'
        self.report_file = 'logs/overnight_reports.jsonl'
        
        os.makedirs('logs', exist_ok=True)
    
    def get_binance_prices(self):
        """Get real-time crypto prices"""
        prices = {}
        for symbol in ['BTCUSDT', 'ETHUSDT', 'SOLUSDT']:
            try:
                resp = requests.get(
                    f'https://api.binance.com/api/v3/ticker/price?symbol={symbol}',
                    timeout=5
                )
                data = resp.json()
                prices[symbol.replace('USDT', '')] = float(data['price'])
            except:
                pass
        return prices
    
    def get_binance_klines(self, symbol='BTCUSDT', interval='1h'):
        """Get kline data for momentum analysis"""
        try:
            resp = requests.get(
                f'https://api.binance.com/api/v3/klines?symbol={symbol}&interval={interval}&limit=5',
                timeout=5
            )
            data = resp.json()
            if data:
                latest = data[-1]
                return {
                    'open': float(latest[1]),
                    'high': float(latest[2]),
                    'low': float(latest[3]),
                    'close': float(latest[4]),
                    'volume': float(latest[5]),
                    'change_pct': (float(latest[4]) - float(latest[1])) / float(latest[1]) * 100
                }
        except:
            pass
        return None
    
    def get_polymarket_markets(self):
        """Get active Polymarket markets"""
        try:
            resp = requests.get(
                'https://gamma-api.polymarket.com/markets?closed=false&limit=500',
                timeout=10
            )
            return resp.json()
        except Exception as e:
            print(f"Error fetching markets: {e}")
            return []
    
    def analyze_market(self, market, crypto_prices, klines):
        """Analyze a market for trading opportunity"""
        question = market.get('question', '').lower()
        slug = market.get('slug', '').lower()
        volume = float(market.get('volume', 0) or 0)
        
        # Filter for crypto-related or high-volume markets
        is_crypto = any(x in question or x in slug for x in 
                       ['bitcoin', 'btc', 'ethereum', 'eth', 'solana', 'sol', 'crypto'])
        
        if volume < 10000 and not is_crypto:
            return None
        
        # Get market prices
        outcomes = market.get('outcomes', [])
        prices = market.get('outcomePrices', [])
        
        if len(outcomes) < 2 or len(prices) < 2:
            return None
        
        try:
            yes_price = float(prices[0])
            no_price = float(prices[1])
        except:
            return None
        
        if yes_price <= 0.05 or yes_price >= 0.95:
            return None  # Skip extreme prices
        
        # Calculate fair value estimate
        fair_prob = 0.5  # Default
        
        # Use crypto momentum for crypto markets
        if is_crypto and klines:
            momentum = klines.get('change_pct', 0)
            if 'up' in question or 'above' in question or 'rise' in question:
                fair_prob = 0.5 + (momentum / 50)  # Momentum adjustment
            elif 'down' in question or 'below' in question or 'fall' in question:
                fair_prob = 0.5 - (momentum / 50)
            else:
                # Price threshold markets
                fair_prob = 0.5 + (momentum / 100)
        else:
            # Use volume and price as quality signal
            if volume > 100000:
                fair_prob = yes_price + random.uniform(-0.1, 0.1)
        
        fair_prob = max(0.1, min(0.9, fair_prob))
        
        # Calculate edge
        yes_edge = fair_prob - yes_price
        no_edge = (1 - fair_prob) - no_price
        
        best_edge = yes_edge if abs(yes_edge) > abs(no_edge) else no_edge
        best_side = 'YES' if yes_edge > no_edge else 'NO'
        
        if abs(best_edge) < MIN_EDGE:
            return None
        
        confidence = min(0.9, 0.5 + abs(best_edge) + (volume / 1000000))
        
        return {
            'market_id': market.get('id'),
            'question': market.get('question', '')[:80],
            'slug': slug,
            'side': best_side,
            'edge': abs(best_edge),
            'fair_prob': fair_prob,
            'market_price': yes_price if best_side == 'YES' else no_price,
            'volume': volume,
            'confidence': confidence,
            'is_crypto': is_crypto
        }
    
    def execute_trade(self, signal):
        """Execute a paper trade"""
        if signal['confidence'] < MIN_CONFIDENCE:
            return None
        
        # Kelly sizing
        edge = signal['edge']
        win_prob = signal['fair_prob'] if signal['side'] == 'YES' else (1 - signal['fair_prob'])
        
        kelly = edge * win_prob / (1 - win_prob) if win_prob < 1 else 0
        kelly = min(kelly, KELLY_FRACTION)
        kelly = max(kelly, 0.01)  # Minimum 1%
        
        position_value = self.capital * kelly * MAX_POSITION_PCT
        position_value = min(position_value, self.capital * 0.1)  # Max 10% per trade
        position_value = max(position_value, 1.0)  # Min $1
        
        if position_value > self.capital * 0.5:
            return None  # Safety check
        
        # Simulate outcome (based on edge)
        win_chance = 0.5 + signal['edge']
        is_win = random.random() < win_chance
        
        if is_win:
            pnl = position_value * (1 / signal['market_price'] - 1)
        else:
            pnl = -position_value
        
        pnl = max(pnl, -position_value)  # Can't lose more than position
        pnl = min(pnl, position_value * 5)  # Cap gains at 5x
        
        self.capital += pnl
        
        trade = {
            'timestamp': datetime.now(timezone.utc).isoformat(),
            'market_id': signal['market_id'],
            'question': signal['question'],
            'side': signal['side'],
            'edge': signal['edge'],
            'confidence': signal['confidence'],
            'position_value': position_value,
            'market_price': signal['market_price'],
            'pnl': pnl,
            'is_win': is_win,
            'capital_after': self.capital,
            'is_crypto': signal['is_crypto']
        }
        
        self.closed_trades.append(trade)
        return trade
    
    def generate_report(self):
        """Generate performance report"""
        now = datetime.now(timezone.utc)
        runtime = (now - self.start_time).total_seconds() / 3600  # hours
        
        total_trades = len(self.closed_trades)
        wins = sum(1 for t in self.closed_trades if t['is_win'])
        losses = total_trades - wins
        win_rate = wins / total_trades * 100 if total_trades > 0 else 0
        
        total_pnl = self.capital - self.initial_capital
        roi = total_pnl / self.initial_capital * 100
        
        # Calculate max drawdown
        peak = self.initial_capital
        max_dd = 0
        for t in self.closed_trades:
            peak = max(peak, t['capital_after'])
            dd = (peak - t['capital_after']) / peak * 100
            max_dd = max(max_dd, dd)
        
        # Crypto vs non-crypto performance
        crypto_trades = [t for t in self.closed_trades if t.get('is_crypto')]
        crypto_pnl = sum(t['pnl'] for t in crypto_trades)
        
        report = {
            'timestamp': now.isoformat(),
            'runtime_hours': round(runtime, 2),
            'scans': self.scan_count,
            'capital': round(self.capital, 2),
            'total_pnl': round(total_pnl, 2),
            'roi_pct': round(roi, 2),
            'total_trades': total_trades,
            'wins': wins,
            'losses': losses,
            'win_rate': round(win_rate, 1),
            'max_drawdown_pct': round(max_dd, 2),
            'crypto_trades': len(crypto_trades),
            'crypto_pnl': round(crypto_pnl, 2),
            'avg_trade_pnl': round(total_pnl / total_trades, 2) if total_trades > 0 else 0
        }
        
        # Save report
        with open(self.report_file, 'a') as f:
            f.write(json.dumps(report) + '\n')
        
        return report
    
    def save_state(self):
        """Save current state to file"""
        state = {
            'capital': self.capital,
            'initial_capital': self.initial_capital,
            'start_time': self.start_time.isoformat(),
            'scan_count': self.scan_count,
            'trades_count': len(self.closed_trades),
            'last_update': datetime.now(timezone.utc).isoformat()
        }
        with open(self.state_file, 'w') as f:
            json.dump(state, f, indent=2)
    
    def run(self, duration_hours=16):
        """Run overnight simulation"""
        print("=" * 70)
        print("🌙 OVERNIGHT PAPER TRADING SIMULATION")
        print("=" * 70)
        print(f"Start Time: {self.start_time.strftime('%Y-%m-%d %H:%M:%S')} UTC")
        print(f"Duration: {duration_hours} hours")
        print(f"Initial Capital: ${self.initial_capital:.2f}")
        print(f"Strategy: Edge>{MIN_EDGE*100}%, Conf>{MIN_CONFIDENCE*100}%")
        print("=" * 70)
        print()
        
        end_time = self.start_time + timedelta(hours=duration_hours)
        
        while datetime.now(timezone.utc) < end_time:
            self.scan_count += 1
            now = datetime.now(timezone.utc)
            
            try:
                # Get live data
                crypto_prices = self.get_binance_prices()
                klines = self.get_binance_klines('BTCUSDT', '1h')
                markets = self.get_polymarket_markets()
                
                # Find trading opportunities
                signals = []
                for market in markets:
                    signal = self.analyze_market(market, crypto_prices, klines)
                    if signal:
                        signals.append(signal)
                
                # Sort by edge * confidence
                signals.sort(key=lambda x: x['edge'] * x['confidence'], reverse=True)
                
                # Execute top signals (max 3 per scan)
                trades_this_scan = 0
                for signal in signals[:3]:
                    if trades_this_scan >= 2:
                        break
                    trade = self.execute_trade(signal)
                    if trade:
                        trades_this_scan += 1
                        emoji = "✅" if trade['is_win'] else "❌"
                        print(f"[{now.strftime('%H:%M')}] {emoji} {trade['side']} | "
                              f"Edge {trade['edge']*100:.1f}% | "
                              f"PnL ${trade['pnl']:+.2f} | "
                              f"Capital ${self.capital:.2f}")
                
                # Generate report every 15 minutes
                if (now - self.last_report_time).total_seconds() >= REPORT_INTERVAL_MINUTES * 60:
                    report = self.generate_report()
                    self.last_report_time = now
                    print()
                    print("-" * 50)
                    print(f"📊 REPORT @ {now.strftime('%H:%M')} UTC")
                    print(f"   Runtime: {report['runtime_hours']:.1f}h | Trades: {report['total_trades']}")
                    print(f"   Capital: ${report['capital']:.2f} | PnL: ${report['total_pnl']:+.2f} ({report['roi_pct']:+.1f}%)")
                    print(f"   Win Rate: {report['win_rate']:.1f}% | Max DD: {report['max_drawdown_pct']:.1f}%")
                    print("-" * 50)
                    print()
                
                # Save state
                self.save_state()
                
            except Exception as e:
                print(f"[{now.strftime('%H:%M')}] Error: {e}")
            
            # Wait for next scan
            time.sleep(SCAN_INTERVAL_SECONDS)
        
        # Final report
        print()
        print("=" * 70)
        print("🏁 SIMULATION COMPLETE")
        print("=" * 70)
        final = self.generate_report()
        print(f"Runtime: {final['runtime_hours']:.1f} hours")
        print(f"Total Scans: {final['scans']}")
        print(f"Total Trades: {final['total_trades']}")
        print(f"Win Rate: {final['win_rate']:.1f}%")
        print(f"Final Capital: ${final['capital']:.2f}")
        print(f"Total P&L: ${final['total_pnl']:+.2f}")
        print(f"ROI: {final['roi_pct']:+.1f}%")
        print(f"Max Drawdown: {final['max_drawdown_pct']:.1f}%")
        print("=" * 70)
        
        # Save final results
        with open('logs/overnight_final.json', 'w') as f:
            json.dump({
                'final_report': final,
                'all_trades': self.closed_trades,
                'config': {
                    'initial_capital': INITIAL_CAPITAL,
                    'min_edge': MIN_EDGE,
                    'min_confidence': MIN_CONFIDENCE,
                    'kelly_fraction': KELLY_FRACTION
                }
            }, f, indent=2)
        
        return final

if __name__ == "__main__":
    sim = OvernightSimulator()
    sim.run(duration_hours=14)  # Run ~14 hours until morning
