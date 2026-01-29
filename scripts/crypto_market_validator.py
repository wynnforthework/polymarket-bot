#!/usr/bin/env python3
"""
Crypto Market Validator
Scans Polymarket for crypto-related markets and validates against real prices

Purpose:
- Find BTC/ETH related markets on Polymarket
- Get real-time prices from Binance
- Analyze market settlement periods
- Report findings and validation plan
"""

import json
import requests
from datetime import datetime, timedelta
from pathlib import Path
import re

# Configuration
POLYMARKET_API = "https://gamma-api.polymarket.com"
BINANCE_API = "https://api.binance.com/api/v3"
LOG_DIR = Path(__file__).parent.parent / "logs"

# Crypto keywords to search for
CRYPTO_KEYWORDS = [
    'bitcoin', 'btc', 'ethereum', 'eth', 'crypto', 'cryptocurrency',
    'solana', 'sol', 'dogecoin', 'doge', 'xrp', 'cardano', 'ada'
]

# Price-related keywords
PRICE_KEYWORDS = [
    'price', '$', 'k', '000', 'above', 'below', 'reach', 'hit', 'fall',
    'rise', 'drop', 'surge', 'crash', 'ath', 'all-time high'
]


def get_binance_prices():
    """Get current BTC and ETH prices from Binance"""
    try:
        url = f"{BINANCE_API}/ticker/price"
        params = {"symbols": '["BTCUSDT","ETHUSDT"]'}
        response = requests.get(url, params=params, timeout=10)
        response.raise_for_status()
        data = response.json()
        
        prices = {}
        for item in data:
            symbol = item['symbol'].replace('USDT', '')
            prices[symbol] = float(item['price'])
        return prices
    except Exception as e:
        print(f"Error fetching Binance prices: {e}")
        return {}


def get_polymarket_markets(limit=500):
    """Fetch all open markets from Polymarket"""
    all_markets = []
    offset = 0
    
    while True:
        try:
            url = f"{POLYMARKET_API}/markets"
            params = {
                "closed": "false",
                "limit": 100,
                "offset": offset
            }
            response = requests.get(url, params=params, timeout=30)
            response.raise_for_status()
            markets = response.json()
            
            if not markets:
                break
                
            all_markets.extend(markets)
            offset += 100
            
            if offset >= limit:
                break
                
        except Exception as e:
            print(f"Error fetching markets: {e}")
            break
    
    return all_markets


def is_crypto_related(market):
    """Check if a market is crypto-related"""
    question = market.get('question', '').lower()
    description = market.get('description', '').lower()
    slug = market.get('slug', '').lower()
    
    text = f"{question} {description} {slug}"
    
    for keyword in CRYPTO_KEYWORDS:
        if keyword in text:
            return True
    return False


def is_price_market(market):
    """Check if market is about price predictions"""
    question = market.get('question', '').lower()
    description = market.get('description', '').lower()
    
    text = f"{question} {description}"
    
    for keyword in PRICE_KEYWORDS:
        if keyword in text:
            return True
    return False


def calculate_time_to_settlement(market):
    """Calculate time remaining until market settlement"""
    end_date_str = market.get('endDate')
    if not end_date_str:
        return None
    
    try:
        # Parse ISO format date
        end_date = datetime.fromisoformat(end_date_str.replace('Z', '+00:00'))
        now = datetime.now(end_date.tzinfo)
        
        delta = end_date - now
        return {
            'end_date': end_date_str,
            'days_remaining': delta.days,
            'hours_remaining': delta.total_seconds() / 3600,
            'minutes_remaining': delta.total_seconds() / 60
        }
    except Exception as e:
        return None


def categorize_market_timeframe(time_info):
    """Categorize market by settlement timeframe"""
    if not time_info:
        return "unknown"
    
    minutes = time_info.get('minutes_remaining', float('inf'))
    
    if minutes <= 15:
        return "15min"
    elif minutes <= 60:
        return "1hour"
    elif minutes <= 60 * 24:
        return "1day"
    elif minutes <= 60 * 24 * 7:
        return "1week"
    elif minutes <= 60 * 24 * 30:
        return "1month"
    else:
        return "long_term"


def analyze_markets():
    """Main analysis function"""
    print("=" * 60)
    print("POLYMARKET CRYPTO MARKET VALIDATION ANALYSIS")
    print("=" * 60)
    print(f"Timestamp: {datetime.utcnow().isoformat()}Z")
    print()
    
    # Get current crypto prices
    print("Fetching current crypto prices from Binance...")
    prices = get_binance_prices()
    print(f"BTC: ${prices.get('BTC', 'N/A'):,.2f}")
    print(f"ETH: ${prices.get('ETH', 'N/A'):,.2f}")
    print()
    
    # Fetch markets
    print("Fetching Polymarket markets...")
    markets = get_polymarket_markets(limit=500)
    print(f"Total markets fetched: {len(markets)}")
    print()
    
    # Filter crypto-related markets
    crypto_markets = []
    for market in markets:
        if is_crypto_related(market):
            crypto_markets.append(market)
    
    print(f"Crypto-related markets found: {len(crypto_markets)}")
    print()
    
    # Analyze crypto markets
    results = {
        'timestamp': datetime.utcnow().isoformat() + 'Z',
        'binance_prices': prices,
        'total_markets': len(markets),
        'crypto_markets_count': len(crypto_markets),
        'crypto_markets': [],
        'timeframe_distribution': {
            '15min': [],
            '1hour': [],
            '1day': [],
            '1week': [],
            '1month': [],
            'long_term': [],
            'unknown': []
        },
        'price_markets': [],
        'verifiable_markets': []
    }
    
    print("-" * 60)
    print("CRYPTO MARKET ANALYSIS")
    print("-" * 60)
    
    for market in crypto_markets:
        time_info = calculate_time_to_settlement(market)
        timeframe = categorize_market_timeframe(time_info)
        is_price = is_price_market(market)
        
        market_data = {
            'id': market.get('id'),
            'question': market.get('question'),
            'slug': market.get('slug'),
            'endDate': market.get('endDate'),
            'timeframe': timeframe,
            'is_price_market': is_price,
            'yes_price': market.get('outcomePrices', '[]'),
            'volume': market.get('volume', 0),
            'liquidity': market.get('liquidity', 0),
            'time_info': time_info
        }
        
        results['crypto_markets'].append(market_data)
        results['timeframe_distribution'][timeframe].append(market_data['question'])
        
        if is_price:
            results['price_markets'].append(market_data)
        
        # Check if verifiable (short-term with price data)
        if timeframe in ['15min', '1hour', '1day'] and is_price:
            results['verifiable_markets'].append(market_data)
    
    # Print summary
    print("\nTIMEFRAME DISTRIBUTION:")
    print("-" * 40)
    for tf, markets_list in results['timeframe_distribution'].items():
        count = len(markets_list)
        if count > 0:
            print(f"  {tf}: {count} markets")
    
    print("\nPRICE-RELATED MARKETS:")
    print("-" * 40)
    if results['price_markets']:
        for pm in results['price_markets'][:10]:  # Show first 10
            print(f"  - {pm['question'][:60]}...")
            print(f"    Timeframe: {pm['timeframe']}, Volume: ${float(pm.get('volume', 0) or 0):,.0f}")
    else:
        print("  No price-related markets found!")
    
    print("\n15-MINUTE / SHORT-TERM VERIFIABLE MARKETS:")
    print("-" * 40)
    if results['verifiable_markets']:
        for vm in results['verifiable_markets']:
            print(f"  - {vm['question']}")
            print(f"    Settlement: {vm['endDate']}")
    else:
        print("  ⚠️  NO 15-MINUTE MARKETS FOUND!")
        print("  Polymarket primarily offers long-term prediction markets.")
        print("  Shortest available crypto markets are typically daily/weekly.")
    
    # Save results
    LOG_DIR.mkdir(exist_ok=True)
    timestamp = datetime.utcnow().strftime('%Y%m%d_%H%M%S')
    log_file = LOG_DIR / f"crypto_validation_{timestamp}.json"
    
    with open(log_file, 'w') as f:
        json.dump(results, f, indent=2, default=str)
    
    print(f"\n✅ Results saved to: {log_file}")
    
    # Print recommendations
    print("\n" + "=" * 60)
    print("RECOMMENDATIONS")
    print("=" * 60)
    
    shortest_markets = []
    for market in results['crypto_markets']:
        ti = market.get('time_info')
        if ti and ti.get('hours_remaining'):
            shortest_markets.append((ti['hours_remaining'], market))
    
    shortest_markets.sort(key=lambda x: x[0])
    
    if shortest_markets:
        print("\n📊 SHORTEST AVAILABLE CRYPTO MARKETS:")
        for hours, m in shortest_markets[:5]:
            days = hours / 24
            print(f"\n  Market: {m['question'][:70]}...")
            print(f"  Time to settlement: {days:.1f} days ({hours:.0f} hours)")
            print(f"  Volume: ${float(m.get('volume', 0) or 0):,.0f}")
    
    print("\n📝 VALIDATION PLAN:")
    print("-" * 40)
    print("""
1. NO 15-MINUTE MARKETS AVAILABLE
   Polymarket focuses on event prediction, not short-term trading.
   
2. SHORTEST VERIFIABLE MARKETS:
   - Most crypto markets have settlement periods of days to months
   - MicroStrategy Bitcoin holdings markets are popular
   - Crypto exchange IPO markets (Kraken, etc.)
   
3. ALTERNATIVE APPROACHES:
   a) For 15-min trading validation, use:
      - Direct Binance/exchange trading
      - DEX protocols (Uniswap, etc.)
      - Perpetual futures markets
      
   b) For Polymarket validation:
      - Track daily/weekly crypto markets
      - Monitor price threshold markets
      - Verify against settlement outcomes
      
4. CURRENT BOT FOCUS:
   Since 15-min markets don't exist on Polymarket,
   the bot should focus on longer-term crypto predictions
   or use alternative platforms for short-term validation.
""")
    
    return results


if __name__ == "__main__":
    results = analyze_markets()
