#!/usr/bin/env python3
"""
Live Paper Trading - Real-time market data, simulated trades
Connects to Polymarket and Binance for live data
"""

import asyncio
import json
import time
from datetime import datetime, timezone
from decimal import Decimal
import aiohttp
import os

# Configuration
INITIAL_CAPITAL = 1000.0
MIN_EDGE = 0.05  # 5% minimum edge
MIN_CONFIDENCE = 0.70
KELLY_FRACTION = 0.15
MAX_POSITION_PCT = 0.02

class LivePaperTrader:
    def __init__(self):
        self.capital = INITIAL_CAPITAL
        self.positions = {}
        self.trades = []
        self.start_time = datetime.now(timezone.utc)
        
    async def get_binance_price(self, session, symbol="BTCUSDT"):
        """Get real-time price from Binance"""
        url = f"https://api.binance.com/api/v3/ticker/price?symbol={symbol}"
        async with session.get(url) as resp:
            data = await resp.json()
            return float(data['price'])
    
    async def get_binance_kline(self, session, symbol="BTCUSDT", interval="1h"):
        """Get current kline data"""
        url = f"https://api.binance.com/api/v3/klines?symbol={symbol}&interval={interval}&limit=2"
        async with session.get(url) as resp:
            data = await resp.json()
            if len(data) >= 2:
                current = data[-1]
                return {
                    'open': float(current[1]),
                    'high': float(current[2]),
                    'low': float(current[3]),
                    'close': float(current[4]),
                    'volume': float(current[5]),
                    'open_time': current[0],
                    'close_time': current[6]
                }
        return None
    
    async def get_polymarket_markets(self, session):
        """Get active Polymarket markets"""
        url = "https://gamma-api.polymarket.com/markets?closed=false&limit=200"
        async with session.get(url) as resp:
            return await resp.json()
    
    def calculate_signal(self, market, btc_price, kline):
        """Generate trading signal based on market and price data"""
        question = market.get('question', '').lower()
        outcomes = market.get('outcomes', [])
        
        # Skip non-crypto markets for this test
        if not any(x in question for x in ['bitcoin', 'btc', 'crypto', 'ethereum', 'eth']):
            return None
        
        # Get current prices
        yes_price = None
        no_price = None
        for outcome in outcomes:
            if outcome.lower() == 'yes':
                yes_price = float(market.get('outcomePrices', ['0.5', '0.5'])[0])
            elif outcome.lower() == 'no':
                no_price = float(market.get('outcomePrices', ['0.5', '0.5'])[1])
        
        if yes_price is None:
            return None
        
        # Simple momentum signal based on BTC trend
        if kline:
            price_change = (kline['close'] - kline['open']) / kline['open']
            
            # Estimate fair probability based on momentum
            if 'up' in question or 'above' in question or 'rise' in question:
                fair_prob = 0.5 + (price_change * 10)  # Momentum adjustment
            elif 'down' in question or 'below' in question or 'fall' in question:
                fair_prob = 0.5 - (price_change * 10)
            else:
                fair_prob = 0.5
            
            fair_prob = max(0.1, min(0.9, fair_prob))
            
            # Calculate edge
            edge = fair_prob - yes_price
            
            if abs(edge) >= MIN_EDGE:
                return {
                    'market_id': market.get('id'),
                    'question': market.get('question', '')[:60],
                    'side': 'YES' if edge > 0 else 'NO',
                    'edge': abs(edge),
                    'fair_prob': fair_prob,
                    'market_price': yes_price if edge > 0 else no_price,
                    'confidence': min(0.9, 0.6 + abs(edge)),
                    'btc_price': btc_price,
                    'btc_change': price_change
                }
        
        return None
    
    def execute_paper_trade(self, signal):
        """Execute a paper trade"""
        if signal['confidence'] < MIN_CONFIDENCE:
            return None
        
        # Kelly sizing
        edge = signal['edge']
        win_prob = signal['fair_prob'] if signal['side'] == 'YES' else (1 - signal['fair_prob'])
        kelly = (win_prob * (1 + edge) - 1) / edge if edge > 0 else 0
        kelly = min(kelly, KELLY_FRACTION)
        
        position_size = self.capital * kelly * MAX_POSITION_PCT / signal['market_price']
        position_value = position_size * signal['market_price']
        
        if position_value < 1:  # Min $1 trade
            return None
        
        trade = {
            'timestamp': datetime.now(timezone.utc).isoformat(),
            'market_id': signal['market_id'],
            'question': signal['question'],
            'side': signal['side'],
            'size': position_size,
            'entry_price': signal['market_price'],
            'edge': signal['edge'],
            'confidence': signal['confidence'],
            'btc_price': signal['btc_price'],
            'position_value': position_value
        }
        
        self.trades.append(trade)
        return trade
    
    async def run_live_session(self, duration_minutes=5):
        """Run live paper trading session"""
        print("=" * 70)
        print("🔴 LIVE PAPER TRADING SESSION")
        print(f"   Initial Capital: ${self.capital:,.2f}")
        print(f"   Duration: {duration_minutes} minutes")
        print(f"   Strategy: Edge>{MIN_EDGE*100}%, Confidence>{MIN_CONFIDENCE*100}%")
        print("=" * 70)
        print()
        
        end_time = time.time() + (duration_minutes * 60)
        scan_count = 0
        
        async with aiohttp.ClientSession() as session:
            while time.time() < end_time:
                scan_count += 1
                now = datetime.now(timezone.utc)
                
                try:
                    # Get live data
                    btc_price = await self.get_binance_price(session, "BTCUSDT")
                    kline = await self.get_binance_kline(session, "BTCUSDT", "1h")
                    markets = await self.get_polymarket_markets(session)
                    
                    print(f"\n[{now.strftime('%H:%M:%S')}] Scan #{scan_count}")
                    print(f"   BTC: ${btc_price:,.2f} | 1H Change: {((kline['close']-kline['open'])/kline['open']*100):+.2f}%")
                    print(f"   Markets scanned: {len(markets)}")
                    
                    # Find signals
                    signals = []
                    for market in markets:
                        signal = self.calculate_signal(market, btc_price, kline)
                        if signal:
                            signals.append(signal)
                    
                    print(f"   Signals found: {len(signals)}")
                    
                    # Execute best signal
                    if signals:
                        best = max(signals, key=lambda x: x['edge'] * x['confidence'])
                        trade = self.execute_paper_trade(best)
                        if trade:
                            print(f"\n   📈 PAPER TRADE EXECUTED:")
                            print(f"      {trade['question']}")
                            print(f"      Side: {trade['side']} | Edge: {trade['edge']*100:.1f}%")
                            print(f"      Size: ${trade['position_value']:.2f}")
                        else:
                            print(f"   ⏸️  Signal below threshold (conf: {best['confidence']*100:.1f}%)")
                    
                except Exception as e:
                    print(f"   ❌ Error: {e}")
                
                # Wait before next scan
                remaining = end_time - time.time()
                if remaining > 30:
                    await asyncio.sleep(30)
                elif remaining > 0:
                    await asyncio.sleep(remaining)
        
        # Summary
        print("\n" + "=" * 70)
        print("📊 SESSION SUMMARY")
        print("=" * 70)
        print(f"Duration: {duration_minutes} minutes")
        print(f"Scans: {scan_count}")
        print(f"Paper Trades: {len(self.trades)}")
        
        if self.trades:
            print("\nTrades:")
            for t in self.trades:
                print(f"  - {t['side']} {t['question'][:40]}...")
                print(f"    Edge: {t['edge']*100:.1f}% | Value: ${t['position_value']:.2f}")
        
        # Save results
        results = {
            'session_start': self.start_time.isoformat(),
            'session_end': datetime.now(timezone.utc).isoformat(),
            'initial_capital': INITIAL_CAPITAL,
            'scans': scan_count,
            'trades': self.trades,
            'config': {
                'min_edge': MIN_EDGE,
                'min_confidence': MIN_CONFIDENCE,
                'kelly_fraction': KELLY_FRACTION
            }
        }
        
        os.makedirs('logs', exist_ok=True)
        filename = f"logs/paper_trading_{datetime.now().strftime('%Y%m%d_%H%M%S')}.json"
        with open(filename, 'w') as f:
            json.dump(results, f, indent=2, default=str)
        print(f"\n📁 Results saved: {filename}")
        
        return results

if __name__ == "__main__":
    trader = LivePaperTrader()
    asyncio.run(trader.run_live_session(duration_minutes=3))
