#!/usr/bin/env python3
"""
Real API Dry Run - Uses real Polymarket data, simulates trades
NO REAL MONEY - just validates API integration and strategy logic
"""

import requests
import json
import time
import os
import sys
from datetime import datetime, timezone, timedelta
from typing import Optional, Dict, List

sys.stdout.reconfigure(line_buffering=True)

# Configuration
INITIAL_CAPITAL = 100.0
MIN_EDGE = 0.03  # 3%
POLL_INTERVAL = 30  # seconds (slower for dry run)
MAX_TRADES_PER_HOUR = 5

POLYMARKET_API = "https://gamma-api.polymarket.com"
CLOB_API = "https://clob.polymarket.com"

class RealAPIDryRun:
    def __init__(self):
        self.capital = INITIAL_CAPITAL
        self.trades = []
        self.pending_trades = []  # Trades waiting for settlement
        self.start_time = datetime.now(timezone.utc)
        self.last_trade_time = None
        self.hourly_trades = 0
        self.last_hour_reset = datetime.now(timezone.utc)
        self.traded_markets = set()
        
        os.makedirs('logs', exist_ok=True)
        self.log_file = f'logs/dryrun_{self.start_time.strftime("%Y%m%d_%H%M%S")}.jsonl'
        
        self.log("=" * 60)
        self.log("🧪 REAL API DRY RUN - NO REAL MONEY")
        self.log(f"   Simulated Capital: ${INITIAL_CAPITAL:.2f}")
        self.log(f"   Min Edge: {MIN_EDGE*100:.0f}%")
        self.log(f"   Using REAL Polymarket API data")
        self.log("=" * 60)
    
    def log(self, msg: str):
        now = datetime.now(timezone.utc).strftime('%H:%M:%S')
        print(f"[{now}] {msg}", flush=True)
    
    def log_trade(self, trade: dict):
        """Log trade to JSONL file"""
        with open(self.log_file, 'a') as f:
            f.write(json.dumps(trade) + '\n')
    
    def get_markets(self, limit: int = 100) -> List[dict]:
        """Get active markets from Polymarket"""
        try:
            resp = requests.get(
                f"{POLYMARKET_API}/markets",
                params={"closed": "false", "limit": limit},
                timeout=15
            )
            resp.raise_for_status()
            return resp.json()
        except Exception as e:
            self.log(f"❌ API Error: {e}")
            return []
    
    def get_orderbook(self, token_id: str) -> Optional[dict]:
        """Get orderbook from CLOB API"""
        try:
            resp = requests.get(
                f"{CLOB_API}/book",
                params={"token_id": token_id},
                timeout=10
            )
            if resp.status_code == 200:
                return resp.json()
        except:
            pass
        return None
    
    def parse_prices(self, prices_raw) -> Optional[tuple]:
        """Parse outcome prices"""
        try:
            if isinstance(prices_raw, str):
                prices = json.loads(prices_raw)
            else:
                prices = prices_raw
            
            if len(prices) >= 2:
                return float(prices[0]), float(prices[1])
        except:
            pass
        return None
    
    def calculate_edge(self, market: dict) -> Optional[dict]:
        """Calculate trading edge for a market"""
        question = market.get('question', '').lower()
        volume = float(market.get('volume', 0) or 0)
        liquidity = float(market.get('liquidity', 0) or 0)
        
        # Filter: need decent volume
        if volume < 50000:
            return None
        
        prices = self.parse_prices(market.get('outcomePrices'))
        if not prices:
            return None
        
        yes_price, no_price = prices
        
        # Filter: avoid extreme prices
        if yes_price <= 0.05 or yes_price >= 0.95:
            return None
        
        # Simple edge calculation based on market inefficiency
        # In real trading, this would use more sophisticated signals
        spread = abs(yes_price + no_price - 1.0)
        
        # Look for mispriced markets
        # If yes + no != 1.0, there's potential edge
        if yes_price + no_price < 0.98:
            # Market underpriced - potential buy opportunity
            edge = (1.0 - yes_price - no_price) / 2
            side = 'YES' if yes_price < 0.5 else 'NO'
            price = yes_price if side == 'YES' else no_price
        elif yes_price + no_price > 1.02:
            # Market overpriced - no clear edge
            return None
        else:
            # Normal pricing - use volume momentum heuristic
            # Higher volume markets tend to be more efficient
            # Look for recent volume spikes as signal
            edge = 0.02 + (liquidity / volume * 0.05) if volume > 0 else 0
            edge = min(edge, 0.08)
            
            if edge < MIN_EDGE:
                return None
            
            # Slight preference for higher probability outcomes
            if yes_price > 0.6:
                side = 'YES'
                price = yes_price
            elif no_price > 0.6:
                side = 'NO'
                price = no_price
            else:
                side = 'YES' if yes_price <= no_price else 'NO'
                price = yes_price if side == 'YES' else no_price
        
        if edge < MIN_EDGE:
            return None
        
        return {
            'side': side,
            'edge': min(edge, 0.15),  # Cap at 15%
            'price': price,
            'spread': spread,
            'volume': volume,
            'liquidity': liquidity
        }
    
    def find_opportunity(self, markets: List[dict]) -> Optional[dict]:
        """Find best trading opportunity"""
        opportunities = []
        
        for market in markets:
            market_id = market.get('id')
            if market_id in self.traded_markets:
                continue
            
            edge_info = self.calculate_edge(market)
            if edge_info:
                opportunities.append({
                    'market': market,
                    **edge_info
                })
        
        if not opportunities:
            return None
        
        # Sort by edge * sqrt(volume) for balanced selection
        return max(opportunities, key=lambda x: x['edge'] * (x['volume'] ** 0.3))
    
    def simulate_trade(self, opp: dict) -> Optional[dict]:
        """Simulate a trade (DRY RUN - no real execution)"""
        now = datetime.now(timezone.utc)
        
        # Rate limiting
        if self.last_trade_time:
            elapsed = (now - self.last_trade_time).total_seconds()
            if elapsed < 60:  # Min 60s between trades for dry run
                return None
        
        # Reset hourly counter
        if (now - self.last_hour_reset).total_seconds() > 3600:
            self.hourly_trades = 0
            self.last_hour_reset = now
        
        if self.hourly_trades >= MAX_TRADES_PER_HOUR:
            return None
        
        # Position sizing (Kelly)
        kelly = min(opp['edge'] * 2.5, 0.10)
        position = self.capital * kelly
        position = max(5.0, min(position, self.capital * 0.15))
        
        if position > self.capital:
            return None
        
        market = opp['market']
        
        trade = {
            'id': f"DRY_{now.strftime('%Y%m%d_%H%M%S')}",
            'timestamp': now.isoformat(),
            'market_id': market.get('id'),
            'question': market.get('question', '')[:80],
            'slug': market.get('slug'),
            'side': opp['side'],
            'entry_price': opp['price'],
            'position_size': round(position, 2),
            'shares': round(position / opp['price'], 4),
            'edge': round(opp['edge'], 4),
            'volume': opp['volume'],
            'liquidity': opp['liquidity'],
            'status': 'SIMULATED',
            'real_execution': False
        }
        
        # Deduct from capital (simulated)
        self.capital -= position
        self.last_trade_time = now
        self.hourly_trades += 1
        self.traded_markets.add(market.get('id'))
        self.pending_trades.append(trade)
        self.trades.append(trade)
        
        # Log
        self.log("=" * 50)
        self.log(f"🧪 DRY RUN TRADE: {trade['id']}")
        self.log(f"   {opp['side']} @ ${opp['price']:.4f}")
        self.log(f"   Position: ${position:.2f} ({trade['shares']:.2f} shares)")
        self.log(f"   Edge: {opp['edge']*100:.1f}%")
        self.log(f"   Market: {market.get('question', '')[:50]}...")
        self.log(f"   Volume: ${opp['volume']:,.0f} | Liquidity: ${opp['liquidity']:,.0f}")
        self.log(f"   Remaining Capital: ${self.capital:.2f}")
        self.log("=" * 50)
        
        self.log_trade(trade)
        self.save_state()
        
        return trade
    
    def save_state(self):
        """Save current state"""
        state = {
            'mode': 'DRY_RUN',
            'capital': round(self.capital, 2),
            'initial_capital': INITIAL_CAPITAL,
            'invested': round(INITIAL_CAPITAL - self.capital, 2),
            'trades_count': len(self.trades),
            'pending_count': len(self.pending_trades),
            'start_time': self.start_time.isoformat(),
            'last_update': datetime.now(timezone.utc).isoformat(),
            'recent_trades': self.trades[-5:] if self.trades else []
        }
        
        with open('logs/dryrun_state.json', 'w') as f:
            json.dump(state, f, indent=2)
    
    def run(self, duration_minutes: int = 60):
        """Run dry run test"""
        end_time = self.start_time + timedelta(minutes=duration_minutes)
        last_report = self.start_time
        scan_count = 0
        
        self.log(f"🏃 Running for {duration_minutes} minutes...")
        
        while datetime.now(timezone.utc) < end_time:
            try:
                scan_count += 1
                
                # Get real market data
                markets = self.get_markets(limit=100)
                
                if markets:
                    self.log(f"📡 Scan #{scan_count}: {len(markets)} markets from API")
                    
                    # Find opportunity
                    opp = self.find_opportunity(markets)
                    
                    if opp:
                        self.simulate_trade(opp)
                    else:
                        self.log(f"   No opportunities meeting criteria")
                else:
                    self.log(f"⚠️ Failed to fetch markets")
                
                # Report every 5 minutes
                now = datetime.now(timezone.utc)
                if (now - last_report).total_seconds() >= 300:
                    runtime = (now - self.start_time).total_seconds() / 60
                    invested = INITIAL_CAPITAL - self.capital
                    
                    self.log("")
                    self.log("=" * 50)
                    self.log(f"📊 DRY RUN STATUS | {runtime:.0f} min")
                    self.log(f"   Trades: {len(self.trades)}")
                    self.log(f"   Invested: ${invested:.2f}")
                    self.log(f"   Remaining: ${self.capital:.2f}")
                    self.log("=" * 50)
                    self.log("")
                    last_report = now
                
                self.save_state()
                time.sleep(POLL_INTERVAL)
                
            except KeyboardInterrupt:
                self.log("⏹️ Interrupted by user")
                break
            except Exception as e:
                self.log(f"❌ Error: {e}")
                time.sleep(POLL_INTERVAL)
        
        # Final summary
        self.log("")
        self.log("=" * 60)
        self.log("🏁 DRY RUN COMPLETE")
        self.log(f"   Duration: {duration_minutes} minutes")
        self.log(f"   Total Trades: {len(self.trades)}")
        self.log(f"   Capital Deployed: ${INITIAL_CAPITAL - self.capital:.2f}")
        self.log(f"   Remaining: ${self.capital:.2f}")
        self.log("")
        self.log("   📝 Trade log: " + self.log_file)
        self.log("   📊 State: logs/dryrun_state.json")
        self.log("=" * 60)
        
        return self.trades

if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument('--minutes', type=int, default=30, help='Duration in minutes')
    args = parser.parse_args()
    
    runner = RealAPIDryRun()
    runner.run(duration_minutes=args.minutes)
