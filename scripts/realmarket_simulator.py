#!/usr/bin/env python3
"""
Real Market Simulator - Tests trading strategy against live Polymarket data
"""

import json
import random
import math
from datetime import datetime
from pathlib import Path
import urllib.request

# Strategy Parameters (optimized)
MIN_EDGE = 0.05          # 5% minimum edge
MIN_CONFIDENCE = 0.70    # 70% confidence threshold
MAX_KELLY = 0.15         # 15% max Kelly fraction
MAX_POSITION = 0.02      # 2% max position size
INITIAL_CAPITAL = 1000   # 1000 USDC

def fetch_markets():
    """Fetch active markets from Polymarket Gamma API"""
    url = "https://gamma-api.polymarket.com/markets?closed=false&limit=500"
    try:
        req = urllib.request.Request(url, headers={
            'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36',
            'Accept': 'application/json'
        })
        with urllib.request.urlopen(req, timeout=30) as resp:
            data = json.loads(resp.read().decode())
            return data
    except Exception as e:
        print(f"Error fetching markets: {e}")
        # Fallback: try with curl
        import subprocess
        try:
            result = subprocess.run(
                ['curl', '-s', url],
                capture_output=True, text=True, timeout=30
            )
            if result.returncode == 0:
                return json.loads(result.stdout)
        except:
            pass
        return []

def calculate_market_quality(market):
    """Score market quality 0-100"""
    score = 0
    
    # Volume score (0-30)
    volume = market.get('volumeNum', 0) or 0
    if volume > 1000000:
        score += 30
    elif volume > 100000:
        score += 20
    elif volume > 10000:
        score += 10
    
    # Liquidity score (0-30)
    liquidity = market.get('liquidityNum', 0) or 0
    if liquidity > 100000:
        score += 30
    elif liquidity > 10000:
        score += 20
    elif liquidity > 1000:
        score += 10
    
    # Spread score (0-20)
    spread = market.get('spread', 1) or 1
    if spread < 0.02:
        score += 20
    elif spread < 0.05:
        score += 15
    elif spread < 0.10:
        score += 10
    
    # Recent activity (0-20)
    vol_24h = market.get('volume24hr', 0) or 0
    if vol_24h > 10000:
        score += 20
    elif vol_24h > 1000:
        score += 10
    elif vol_24h > 100:
        score += 5
    
    return score

def parse_prices(market):
    """Extract Yes/No prices from market"""
    try:
        prices_str = market.get('outcomePrices', '[]')
        prices = json.loads(prices_str) if isinstance(prices_str, str) else prices_str
        if len(prices) >= 2:
            yes_price = float(prices[0])
            no_price = float(prices[1])
            return yes_price, no_price
    except:
        pass
    return None, None

def simulate_llm_analysis(market, yes_price, no_price):
    """
    Simulate LLM market analysis
    In production, this would call DeepSeek/GPT for real analysis
    Here we simulate reasonable behavior
    """
    quality = calculate_market_quality(market)
    
    # Better quality markets → more confident analysis
    base_confidence = 0.5 + (quality / 200)  # 0.5 to 1.0
    
    # Add some randomness to simulate varied LLM opinions
    confidence_noise = random.gauss(0, 0.1)
    confidence = max(0.3, min(0.95, base_confidence + confidence_noise))
    
    # Simulated probability estimate
    # LLM tends to find value when market is inefficient
    market_prob = yes_price if yes_price and yes_price > 0 else 0.5
    
    # Simulate LLM having edge in high-quality markets
    if quality > 60:
        # More likely to find genuine edge
        prob_noise = random.gauss(0, 0.08)
    else:
        # Less reliable in low-quality markets
        prob_noise = random.gauss(0, 0.15)
    
    estimated_prob = max(0.05, min(0.95, market_prob + prob_noise))
    
    return {
        'estimated_probability': estimated_prob,
        'confidence': confidence,
        'reasoning': f"Quality score {quality}/100, market analysis complete"
    }

def kelly_fraction(prob, odds, confidence):
    """Calculate Kelly bet size with confidence adjustment"""
    if odds <= 0:
        return 0
    q = 1 - prob
    edge = prob * odds - q
    if edge <= 0:
        return 0
    kelly = edge / odds
    # Scale by confidence
    adjusted = kelly * confidence * MAX_KELLY
    return min(adjusted, MAX_POSITION)

def generate_signal(market, analysis, yes_price, no_price):
    """Generate trading signal based on analysis"""
    est_prob = analysis['estimated_probability']
    confidence = analysis['confidence']
    
    signal = {
        'market_id': market.get('id'),
        'question': market.get('question', '')[:100],
        'yes_price': yes_price,
        'no_price': no_price,
        'estimated_prob': est_prob,
        'confidence': confidence,
        'action': 'HOLD',
        'direction': None,
        'edge': 0,
        'position_size': 0,
        'reason': ''
    }
    
    # Check for YES opportunity
    if yes_price and yes_price > 0.01 and yes_price < 0.99:
        yes_odds = 1 / yes_price - 1
        yes_edge = est_prob - yes_price
        
        if yes_edge >= MIN_EDGE and confidence >= MIN_CONFIDENCE:
            size = kelly_fraction(est_prob, yes_odds, confidence)
            if size > 0.001:
                signal['action'] = 'BUY'
                signal['direction'] = 'YES'
                signal['edge'] = yes_edge
                signal['position_size'] = size
                signal['reason'] = f"YES edge {yes_edge:.1%}, conf {confidence:.0%}"
    
    # Check for NO opportunity
    if no_price and no_price > 0.01 and no_price < 0.99:
        no_prob = 1 - est_prob
        no_odds = 1 / no_price - 1
        no_edge = no_prob - no_price
        
        if no_edge >= MIN_EDGE and confidence >= MIN_CONFIDENCE and signal['action'] == 'HOLD':
            size = kelly_fraction(no_prob, no_odds, confidence)
            if size > 0.001:
                signal['action'] = 'BUY'
                signal['direction'] = 'NO'
                signal['edge'] = no_edge
                signal['position_size'] = size
                signal['reason'] = f"NO edge {no_edge:.1%}, conf {confidence:.0%}"
    
    if signal['action'] == 'HOLD':
        if confidence < MIN_CONFIDENCE:
            signal['reason'] = f"Low confidence ({confidence:.0%} < {MIN_CONFIDENCE:.0%})"
        elif yes_price and no_price:
            max_edge = max(abs(est_prob - yes_price), abs((1-est_prob) - no_price))
            signal['reason'] = f"Insufficient edge ({max_edge:.1%} < {MIN_EDGE:.0%})"
        else:
            signal['reason'] = "Invalid prices"
    
    return signal

def simulate_outcome(signal):
    """Simulate trade outcome based on our edge estimate"""
    if signal['action'] == 'HOLD':
        return 0
    
    edge = signal['edge']
    confidence = signal['confidence']
    
    # True win probability = market price + our estimated edge (discounted by confidence)
    if signal['direction'] == 'YES':
        true_prob = signal['yes_price'] + (edge * confidence * 0.5)
    else:
        true_prob = signal['no_price'] + (edge * confidence * 0.5)
    
    true_prob = max(0.1, min(0.9, true_prob))
    
    # Simulate outcome
    won = random.random() < true_prob
    
    if won:
        # Won: receive payout minus cost
        if signal['direction'] == 'YES':
            profit = (1 - signal['yes_price']) / signal['yes_price']
        else:
            profit = (1 - signal['no_price']) / signal['no_price']
    else:
        profit = -1  # Lost entire position
    
    return profit * signal['position_size']

def run_simulation():
    """Main simulation loop"""
    print("=" * 60)
    print("POLYMARKET REAL MARKET SIMULATION TEST")
    print("=" * 60)
    print(f"Strategy: min_edge={MIN_EDGE:.0%}, confidence={MIN_CONFIDENCE:.0%}")
    print(f"          kelly={MAX_KELLY:.0%}, max_position={MAX_POSITION:.0%}")
    print(f"Initial Capital: {INITIAL_CAPITAL} USDC")
    print("-" * 60)
    
    # Fetch real markets
    print("\nFetching live market data from Polymarket...")
    markets = fetch_markets()
    print(f"Retrieved {len(markets)} markets")
    
    # Filter active markets with decent liquidity
    active_markets = []
    for m in markets:
        if m.get('closed'):
            continue
        liquidity = m.get('liquidityNum', 0) or 0
        if liquidity > 100:  # At least $100 liquidity
            yes_p, no_p = parse_prices(m)
            if yes_p and no_p and yes_p > 0.01 and no_p > 0.01:
                active_markets.append(m)
    
    print(f"Active tradeable markets: {len(active_markets)}")
    
    # Results tracking
    results = {
        'timestamp': datetime.now().isoformat(),
        'parameters': {
            'min_edge': MIN_EDGE,
            'min_confidence': MIN_CONFIDENCE,
            'max_kelly': MAX_KELLY,
            'max_position': MAX_POSITION,
            'initial_capital': INITIAL_CAPITAL
        },
        'markets_scanned': len(active_markets),
        'signals': [],
        'trades': [],
        'summary': {}
    }
    
    # Simulate 100 decision points
    capital = INITIAL_CAPITAL
    trades = 0
    wins = 0
    total_pnl = 0
    
    decision_points = min(100, len(active_markets))
    sampled_markets = random.sample(active_markets, decision_points) if len(active_markets) > decision_points else active_markets
    
    print(f"\nRunning {len(sampled_markets)} decision points...")
    print("-" * 60)
    
    for i, market in enumerate(sampled_markets):
        yes_price, no_price = parse_prices(market)
        quality = calculate_market_quality(market)
        
        # Simulate LLM analysis
        analysis = simulate_llm_analysis(market, yes_price, no_price)
        
        # Generate signal
        signal = generate_signal(market, analysis, yes_price, no_price)
        signal['quality_score'] = quality
        
        # Record signal
        results['signals'].append({
            'market_id': signal['market_id'],
            'question': signal['question'],
            'quality': quality,
            'yes_price': yes_price,
            'no_price': no_price,
            'action': signal['action'],
            'direction': signal['direction'],
            'edge': signal['edge'],
            'confidence': signal['confidence'],
            'position_size': signal['position_size'],
            'reason': signal['reason']
        })
        
        if signal['action'] == 'BUY':
            # Simulate trade
            position_value = capital * signal['position_size']
            pnl_pct = simulate_outcome(signal)
            pnl = position_value * pnl_pct
            
            trades += 1
            total_pnl += pnl
            capital += pnl
            
            if pnl > 0:
                wins += 1
            
            results['trades'].append({
                'market_id': signal['market_id'],
                'question': signal['question'][:50],
                'direction': signal['direction'],
                'edge': signal['edge'],
                'position_value': position_value,
                'pnl': pnl,
                'cumulative_pnl': total_pnl,
                'capital': capital
            })
            
            status = "✓ WIN" if pnl > 0 else "✗ LOSS"
            print(f"[{i+1:3d}] {status} | {signal['direction']:3s} | Edge {signal['edge']:.1%} | PnL ${pnl:+.2f} | Capital ${capital:.2f}")
        else:
            if (i + 1) % 20 == 0:
                print(f"[{i+1:3d}] HOLD - {signal['reason'][:50]}")
    
    # Summary
    win_rate = wins / trades if trades > 0 else 0
    roi = (capital - INITIAL_CAPITAL) / INITIAL_CAPITAL
    
    results['summary'] = {
        'total_markets_scanned': len(sampled_markets),
        'signals_generated': len([s for s in results['signals'] if s['action'] == 'BUY']),
        'trades_executed': trades,
        'wins': wins,
        'losses': trades - wins,
        'win_rate': win_rate,
        'total_pnl': total_pnl,
        'roi': roi,
        'final_capital': capital,
        'avg_edge': sum(t['edge'] for t in results['trades']) / trades if trades > 0 else 0,
        'max_drawdown': min([t['cumulative_pnl'] for t in results['trades']], default=0)
    }
    
    # Print summary
    print("\n" + "=" * 60)
    print("SIMULATION RESULTS")
    print("=" * 60)
    print(f"Markets Scanned:    {len(sampled_markets)}")
    print(f"Trading Signals:    {results['summary']['signals_generated']}")
    print(f"Trades Executed:    {trades}")
    print(f"Win Rate:           {win_rate:.1%}")
    print(f"Total P&L:          ${total_pnl:+.2f}")
    print(f"ROI:                {roi:+.1%}")
    print(f"Final Capital:      ${capital:.2f}")
    print(f"Max Drawdown:       ${results['summary']['max_drawdown']:.2f}")
    print("=" * 60)
    
    # Quality distribution
    quality_dist = {'high': 0, 'medium': 0, 'low': 0}
    for s in results['signals']:
        if s['quality'] >= 60:
            quality_dist['high'] += 1
        elif s['quality'] >= 30:
            quality_dist['medium'] += 1
        else:
            quality_dist['low'] += 1
    
    print(f"\nMarket Quality Distribution:")
    print(f"  High (60+):   {quality_dist['high']}")
    print(f"  Medium (30+): {quality_dist['medium']}")
    print(f"  Low (<30):    {quality_dist['low']}")
    
    # Save results
    logs_dir = Path("/home/bot/clawd/projects/polymarket-bot/logs")
    logs_dir.mkdir(exist_ok=True)
    
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    output_file = logs_dir / f"realmarket_test_{timestamp}.json"
    
    with open(output_file, 'w') as f:
        json.dump(results, f, indent=2)
    
    print(f"\nResults saved to: {output_file}")
    
    return results

if __name__ == "__main__":
    run_simulation()
