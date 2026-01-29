#!/usr/bin/env python3
"""Analyze Polymarket traders via their public data"""

import requests
import json
from datetime import datetime

TRADERS = [
    {
        "name": "livebreathevolatility",
        "address": None,  # Need to discover
    },
    {
        "name": "0xf247584e41117bbBe4Cc06E4d2C95741792a5216-1742469835200",
        "address": "0xf247584e41117bbBe4Cc06E4d2C95741792a5216",
    }
]

DATA_API = "https://data-api.polymarket.com"
GAMMA_API = "https://gamma-api.polymarket.com"

def get_positions(address):
    """Get user positions from Data API"""
    url = f"{DATA_API}/positions?user={address}"
    print(f"Fetching: {url}")
    try:
        resp = requests.get(url, timeout=30)
        print(f"Status: {resp.status_code}")
        if resp.ok:
            return resp.json()
        print(f"Error: {resp.text}")
    except Exception as e:
        print(f"Error: {e}")
    return None

def get_activity(address, limit=100):
    """Get user activity (trades) from Data API"""
    url = f"{DATA_API}/activity?user={address}&limit={limit}"
    print(f"Fetching: {url}")
    try:
        resp = requests.get(url, timeout=30)
        print(f"Status: {resp.status_code}")
        if resp.ok:
            return resp.json()
        print(f"Error: {resp.text}")
    except Exception as e:
        print(f"Error: {e}")
    return None

def get_profit(address):
    """Get user PnL from Data API"""
    url = f"{DATA_API}/profit?user={address}"
    print(f"Fetching: {url}")
    try:
        resp = requests.get(url, timeout=30)
        print(f"Status: {resp.status_code}")
        if resp.ok:
            return resp.json()
        print(f"Error: {resp.text}")
    except Exception as e:
        print(f"Error: {e}")
    return None

def get_rankings():
    """Get leaderboard"""
    url = f"{DATA_API}/rankings?limit=100"
    print(f"Fetching leaderboard: {url}")
    try:
        resp = requests.get(url, timeout=30)
        if resp.ok:
            return resp.json()
        print(f"Leaderboard error: {resp.text}")
    except Exception as e:
        print(f"Error: {e}")
    return None

def search_user(query):
    """Search for user"""
    url = f"{DATA_API}/users/search?query={query}"
    try:
        resp = requests.get(url, timeout=30)
        if resp.ok:
            return resp.json()
    except:
        pass
    return None

def main():
    print("=" * 60)
    print("Polymarket Trader Analysis")
    print("=" * 60)
    
    # First, try to get leaderboard to find addresses
    print("\n📊 Fetching leaderboard...")
    rankings = get_rankings()
    if rankings:
        print(f"Found {len(rankings)} traders on leaderboard")
        for r in rankings[:20]:
            print(f"  {r.get('name', r.get('username', 'unknown'))}: ${r.get('pnl', 0):,.2f}")
    
    # Now analyze each trader
    for trader in TRADERS:
        print("\n" + "=" * 60)
        print(f"🎯 Analyzing: {trader['name']}")
        print("=" * 60)
        
        address = trader.get('address')
        
        if not address:
            print(f"No address for {trader['name']}, trying to search...")
            result = search_user(trader['name'])
            if result:
                print(f"Search result: {json.dumps(result, indent=2)}")
            continue
        
        # Get positions
        print("\n📈 Current Positions:")
        positions = get_positions(address)
        if positions:
            total_value = 0
            for pos in positions[:10]:
                size = float(pos.get('size', 0))
                value = float(pos.get('currentValue', 0))
                pnl = float(pos.get('cashPnl', 0))
                title = pos.get('title', 'Unknown')[:50]
                outcome = pos.get('outcome', '?')
                total_value += value
                print(f"  {outcome} {title}: ${value:,.2f} (PnL: ${pnl:+,.2f})")
            print(f"  TOTAL VALUE: ${total_value:,.2f}")
        
        # Get activity
        print("\n📜 Recent Activity:")
        activity = get_activity(address, 20)
        if activity:
            for act in activity[:10]:
                side = act.get('side', '?')
                price = float(act.get('price', 0))
                size = float(act.get('size', 0))
                title = act.get('title', 'Unknown')[:40]
                ts = act.get('timestamp', '')[:10]
                print(f"  [{ts}] {side.upper()} ${size:,.2f} @ {price:.2f} - {title}")
        
        # Get profit
        print("\n💰 Profit Summary:")
        profit = get_profit(address)
        if profit:
            print(json.dumps(profit, indent=2))

if __name__ == "__main__":
    main()
