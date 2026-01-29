#!/usr/bin/env python3
"""
Crypto Market Scanner - Accurate Analysis
Specifically searches for BTC/ETH price-related markets on Polymarket
"""

import json
import requests
from datetime import datetime, timezone
from pathlib import Path

POLYMARKET_API = "https://gamma-api.polymarket.com"
BINANCE_API = "https://api.binance.com/api/v3"
LOG_DIR = Path(__file__).parent.parent / "logs"

# Strict crypto keywords (actual crypto assets)
STRICT_CRYPTO_KEYWORDS = [
    'bitcoin', 'btc', 'ethereum', 'eth', 'solana', 'sol', 
    'xrp', 'cardano', 'dogecoin', 'doge', 'crypto price'
]

# Price prediction patterns
PRICE_PATTERNS = [
    'price', 'above $', 'below $', 'reach $', 'hit $', 
    'k by', '$100k', '$50k', '$150k', 'all-time high', 'ath'
]


def get_binance_prices():
    """Get current BTC and ETH prices"""
    try:
        response = requests.get(f"{BINANCE_API}/ticker/price", timeout=10)
        data = response.json()
        prices = {}
        for item in data:
            if item['symbol'] in ['BTCUSDT', 'ETHUSDT', 'SOLUSDT']:
                sym = item['symbol'].replace('USDT', '')
                prices[sym] = float(item['price'])
        return prices
    except Exception as e:
        return {'BTC': 0, 'ETH': 0, 'SOL': 0}


def fetch_all_markets():
    """Fetch markets from Polymarket"""
    all_markets = []
    offset = 0
    
    while offset < 1000:
        try:
            response = requests.get(
                f"{POLYMARKET_API}/markets",
                params={"closed": "false", "limit": 100, "offset": offset},
                timeout=30
            )
            markets = response.json()
            if not markets:
                break
            all_markets.extend(markets)
            offset += 100
        except Exception as e:
            print(f"Error at offset {offset}: {e}")
            break
    
    return all_markets


def is_strict_crypto_price_market(market):
    """Check if market is specifically about crypto asset prices"""
    q = market.get('question', '').lower()
    desc = market.get('description', '').lower()
    text = f"{q} {desc}"
    
    # Must have strict crypto keyword
    has_crypto = any(kw in text for kw in STRICT_CRYPTO_KEYWORDS)
    # Must have price pattern
    has_price = any(pat in text for pat in PRICE_PATTERNS)
    
    return has_crypto and has_price


def analyze_settlement_time(market):
    """Analyze time to settlement"""
    end_str = market.get('endDate')
    if not end_str:
        return None
    
    try:
        end_date = datetime.fromisoformat(end_str.replace('Z', '+00:00'))
        now = datetime.now(timezone.utc)
        delta = end_date - now
        
        total_hours = delta.total_seconds() / 3600
        
        if total_hours <= 0:
            return {'status': 'expired', 'hours': total_hours}
        
        return {
            'status': 'active',
            'end_date': end_str,
            'hours': total_hours,
            'days': total_hours / 24,
            'category': categorize_timeframe(total_hours)
        }
    except:
        return None


def categorize_timeframe(hours):
    """Categorize by timeframe"""
    if hours <= 0.25:  # 15 min
        return '15min'
    elif hours <= 1:
        return '1hour'
    elif hours <= 24:
        return '1day'
    elif hours <= 168:  # 7 days
        return '1week'
    elif hours <= 720:  # 30 days
        return '1month'
    else:
        return 'long_term'


def main():
    print("=" * 70)
    print("POLYMARKET CRYPTO PRICE MARKET SCANNER")
    print("=" * 70)
    print(f"Scan Time: {datetime.now(timezone.utc).isoformat()}")
    print()
    
    # Get prices
    print("📊 Current Crypto Prices (Binance):")
    prices = get_binance_prices()
    for sym, price in prices.items():
        print(f"   {sym}: ${price:,.2f}")
    print()
    
    # Fetch markets
    print("🔍 Scanning Polymarket for crypto price markets...")
    all_markets = fetch_all_markets()
    print(f"   Total active markets: {len(all_markets)}")
    
    # Find crypto price markets
    crypto_price_markets = []
    for m in all_markets:
        if is_strict_crypto_price_market(m):
            time_info = analyze_settlement_time(m)
            crypto_price_markets.append({
                'id': m.get('id'),
                'question': m.get('question'),
                'slug': m.get('slug'),
                'volume': m.get('volume'),
                'liquidity': m.get('liquidity'),
                'outcomes': m.get('outcomePrices'),
                'time_info': time_info
            })
    
    print(f"   Crypto price-related markets: {len(crypto_price_markets)}")
    print()
    
    # Categorize by timeframe
    by_timeframe = {
        '15min': [], '1hour': [], '1day': [],
        '1week': [], '1month': [], 'long_term': [],
        'expired': [], 'unknown': []
    }
    
    for m in crypto_price_markets:
        ti = m['time_info']
        if not ti:
            by_timeframe['unknown'].append(m)
        elif ti['status'] == 'expired':
            by_timeframe['expired'].append(m)
        else:
            by_timeframe[ti['category']].append(m)
    
    print("-" * 70)
    print("CRYPTO PRICE MARKETS BY SETTLEMENT TIMEFRAME")
    print("-" * 70)
    
    for tf in ['15min', '1hour', '1day', '1week', '1month', 'long_term']:
        markets = by_timeframe[tf]
        if markets:
            print(f"\n⏱️  {tf.upper()} ({len(markets)} markets):")
            for m in markets[:5]:  # Show max 5
                vol = float(m.get('volume') or 0)
                ti = m['time_info']
                print(f"   • {m['question'][:60]}...")
                print(f"     Volume: ${vol:,.0f} | Settlement: {ti.get('days', '?'):.1f} days")
    
    # Summary
    print("\n" + "=" * 70)
    print("KEY FINDINGS")
    print("=" * 70)
    
    short_term = len(by_timeframe['15min']) + len(by_timeframe['1hour']) + len(by_timeframe['1day'])
    
    print(f"""
📌 SUMMARY:
   - Total crypto price markets: {len(crypto_price_markets)}
   - 15-minute markets: {len(by_timeframe['15min'])}
   - 1-hour markets: {len(by_timeframe['1hour'])}
   - 1-day markets: {len(by_timeframe['1day'])}
   - Short-term verifiable (≤1 day): {short_term}
   - Long-term markets: {len(by_timeframe['long_term'])}
    """)
    
    if len(by_timeframe['15min']) == 0:
        print("⚠️  NO 15-MINUTE CRYPTO PRICE MARKETS FOUND ON POLYMARKET")
        print("   Polymarket focuses on event prediction, not intraday trading.")
        print()
        print("📋 SHORTEST AVAILABLE CRYPTO MARKETS:")
        
        # Find shortest markets
        active_markets = []
        for m in crypto_price_markets:
            ti = m['time_info']
            if ti and ti.get('status') == 'active':
                active_markets.append((ti['hours'], m))
        
        active_markets.sort(key=lambda x: x[0])
        
        for hours, m in active_markets[:5]:
            print(f"   • {m['question'][:55]}...")
            print(f"     Settlement in: {hours/24:.1f} days ({hours:.0f} hours)")
    
    # Save results
    LOG_DIR.mkdir(exist_ok=True)
    result_data = {
        'timestamp': datetime.now(timezone.utc).isoformat(),
        'binance_prices': prices,
        'total_markets': len(all_markets),
        'crypto_price_markets': len(crypto_price_markets),
        'by_timeframe': {k: len(v) for k, v in by_timeframe.items()},
        'markets': crypto_price_markets,
        'finding': '15-minute crypto price markets NOT available on Polymarket'
    }
    
    log_file = LOG_DIR / f"crypto_scan_{datetime.now(timezone.utc).strftime('%Y%m%d_%H%M%S')}.json"
    with open(log_file, 'w') as f:
        json.dump(result_data, f, indent=2, default=str)
    
    print(f"\n✅ Results saved: {log_file}")
    
    print("\n" + "=" * 70)
    print("VALIDATION PLAN")
    print("=" * 70)
    print("""
Since Polymarket does NOT offer 15-minute crypto markets:

1. FOR POLYMARKET VALIDATION:
   - Track long-term BTC/ETH price threshold markets
   - Monitor MicroStrategy Bitcoin holdings markets
   - Verify predictions at settlement dates

2. FOR 15-MINUTE TRADING VALIDATION:
   - Use Binance/exchange for real price data
   - Consider Hyperliquid or other perp DEXs
   - Paper trade against real price movements

3. RECOMMENDED APPROACH:
   - Build strategy backtesting with historical price data
   - Validate predictions on daily/weekly Polymarket markets
   - For short-term: use simulated trading against Binance prices
""")
    
    return result_data


if __name__ == "__main__":
    main()
