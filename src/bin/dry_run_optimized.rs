//! Optimized Dry Run Simulation - 1000 USDC
//! Uses conservative risk parameters for better risk-adjusted returns
//!
//! Optimized Parameters vs Original:
//! - min_edge: 5% (vs 2%)
//! - min_confidence: 70% (vs 50%)
//! - kelly_fraction: 15% (vs 25-35%)
//! - max_position_pct: 2% (vs 5%)
//! - max_total_exposure: 30% (vs 50%)
//! - max_daily_loss: 5% (vs 10%)
//! - stop_loss: 15%
//! - take_profit: 25%
//! - trailing_stop: 10%
//! - max_trades_per_hour: 3

use polymarket_bot::testing::optimized_simulator::EnhancedDryRunSimulator;
use polymarket_bot::config::{StrategyConfig, RiskConfig};
use polymarket_bot::types::{Market, Outcome};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use chrono::{Utc, Duration};
use std::fs::File;
use std::io::Write;
use tracing_subscriber;

fn generate_test_markets() -> Vec<Market> {
    vec![
        Market {
            id: "btc-100k".to_string(),
            question: "Will BTC reach $100k by March 2026?".to_string(),
            description: Some("Bitcoin price prediction".to_string()),
            end_date: Some(Utc::now() + Duration::days(365)),
            outcomes: vec![
                Outcome {
                    outcome: "Yes".to_string(),
                    token_id: "btc-100k-yes".to_string(),
                    price: dec!(0.45),
                },
                Outcome {
                    outcome: "No".to_string(),
                    token_id: "btc-100k-no".to_string(),
                    price: dec!(0.55),
                },
            ],
            volume: dec!(500000),
            liquidity: dec!(100000),
            active: true,
            closed: false,
        },
        Market {
            id: "eth-5k".to_string(),
            question: "Will ETH reach $5k by Feb 2026?".to_string(),
            description: Some("Ethereum price prediction".to_string()),
            end_date: Some(Utc::now() + Duration::days(300)),
            outcomes: vec![
                Outcome {
                    outcome: "Yes".to_string(),
                    token_id: "eth-5k-yes".to_string(),
                    price: dec!(0.35),
                },
                Outcome {
                    outcome: "No".to_string(),
                    token_id: "eth-5k-no".to_string(),
                    price: dec!(0.65),
                },
            ],
            volume: dec!(300000),
            liquidity: dec!(80000),
            active: true,
            closed: false,
        },
        Market {
            id: "fed-rate".to_string(),
            question: "Will Fed cut rates in Q1 2026?".to_string(),
            description: Some("Federal Reserve policy".to_string()),
            end_date: Some(Utc::now() + Duration::days(200)),
            outcomes: vec![
                Outcome {
                    outcome: "Yes".to_string(),
                    token_id: "fed-rate-yes".to_string(),
                    price: dec!(0.60),
                },
                Outcome {
                    outcome: "No".to_string(),
                    token_id: "fed-rate-no".to_string(),
                    price: dec!(0.40),
                },
            ],
            volume: dec!(200000),
            liquidity: dec!(50000),
            active: true,
            closed: false,
        },
        Market {
            id: "trump-approval".to_string(),
            question: "Trump approval above 50% by June 2026?".to_string(),
            description: Some("Political approval rating".to_string()),
            end_date: Some(Utc::now() + Duration::days(400)),
            outcomes: vec![
                Outcome {
                    outcome: "Yes".to_string(),
                    token_id: "trump-approval-yes".to_string(),
                    price: dec!(0.32),
                },
                Outcome {
                    outcome: "No".to_string(),
                    token_id: "trump-approval-no".to_string(),
                    price: dec!(0.68),
                },
            ],
            volume: dec!(450000),
            liquidity: dec!(120000),
            active: true,
            closed: false,
        },
        Market {
            id: "sp500-6k".to_string(),
            question: "Will S&P 500 reach 6000 by Dec 2025?".to_string(),
            description: Some("Stock market prediction".to_string()),
            end_date: Some(Utc::now() + Duration::days(150)),
            outcomes: vec![
                Outcome {
                    outcome: "Yes".to_string(),
                    token_id: "sp500-6k-yes".to_string(),
                    price: dec!(0.72),
                },
                Outcome {
                    outcome: "No".to_string(),
                    token_id: "sp500-6k-no".to_string(),
                    price: dec!(0.28),
                },
            ],
            volume: dec!(180000),
            liquidity: dec!(45000),
            active: true,
            closed: false,
        },
        // Additional markets for more diverse testing
        Market {
            id: "ai-regulation".to_string(),
            question: "Will major AI regulation pass in US by 2026?".to_string(),
            description: Some("AI policy prediction".to_string()),
            end_date: Some(Utc::now() + Duration::days(250)),
            outcomes: vec![
                Outcome {
                    outcome: "Yes".to_string(),
                    token_id: "ai-reg-yes".to_string(),
                    price: dec!(0.40),
                },
                Outcome {
                    outcome: "No".to_string(),
                    token_id: "ai-reg-no".to_string(),
                    price: dec!(0.60),
                },
            ],
            volume: dec!(150000),
            liquidity: dec!(35000),
            active: true,
            closed: false,
        },
        Market {
            id: "sol-500".to_string(),
            question: "Will SOL reach $500 by Dec 2025?".to_string(),
            description: Some("Solana price prediction".to_string()),
            end_date: Some(Utc::now() + Duration::days(180)),
            outcomes: vec![
                Outcome {
                    outcome: "Yes".to_string(),
                    token_id: "sol-500-yes".to_string(),
                    price: dec!(0.25),
                },
                Outcome {
                    outcome: "No".to_string(),
                    token_id: "sol-500-no".to_string(),
                    price: dec!(0.75),
                },
            ],
            volume: dec!(220000),
            liquidity: dec!(55000),
            active: true,
            closed: false,
        },
    ]
}

/// Create optimized strategy config
fn optimized_strategy_config() -> StrategyConfig {
    StrategyConfig {
        min_edge: dec!(0.05),         // 5% (was 2%)
        min_confidence: dec!(0.70),   // 70% (was 50%)
        kelly_fraction: dec!(0.15),   // 15% (was 25-35%)
        scan_interval_secs: 180,
        model_update_interval_secs: 900,
        compound_enabled: true,
        compound_sqrt_scaling: true,
    }
}

/// Create optimized risk config
fn optimized_risk_config() -> RiskConfig {
    RiskConfig {
        max_position_pct: dec!(0.02),     // 2% (was 5%)
        max_exposure_pct: dec!(0.30),     // 30% (was 50%)
        max_daily_loss_pct: dec!(0.05),   // 5% (was 10%)
        min_balance_reserve: dec!(100),
        max_open_positions: 5,            // Reduced from default
    }
}

/// Original/aggressive strategy config for comparison
fn original_strategy_config() -> StrategyConfig {
    StrategyConfig {
        min_edge: dec!(0.02),         // 2%
        min_confidence: dec!(0.50),   // 50%
        kelly_fraction: dec!(0.25),   // 25%
        scan_interval_secs: 180,
        model_update_interval_secs: 900,
        compound_enabled: true,
        compound_sqrt_scaling: true,
    }
}

fn original_risk_config() -> RiskConfig {
    RiskConfig {
        max_position_pct: dec!(0.05),     // 5%
        max_exposure_pct: dec!(0.50),     // 50%
        max_daily_loss_pct: dec!(0.10),   // 10%
        min_balance_reserve: dec!(100),
        max_open_positions: 10,
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Init tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();
    
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║    🧪 OPTIMIZED vs ORIGINAL STRATEGY COMPARISON               ║");
    println!("║    💰 Initial Balance: 1000 USDC | Steps: 100                 ║");
    println!("╚═══════════════════════════════════════════════════════════════╝\n");
    
    let markets = generate_test_markets();
    let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
    
    // ══════════════════════════════════════════════════════════════════
    // Run ORIGINAL Strategy
    // ══════════════════════════════════════════════════════════════════
    println!("📊 Running ORIGINAL Strategy...");
    let mut original_sim = EnhancedDryRunSimulator::new(
        dec!(1000),
        original_strategy_config(),
        original_risk_config(),
    ).with_markets(markets.clone())
     .with_stop_loss(dec!(0.20))      // 20% stop loss (looser)
     .with_take_profit(dec!(0.30))    // 30% take profit
     .with_trailing_stop(dec!(0.15)); // 15% trailing stop
    
    original_sim.run_for(100, 0).await?;
    let original_result = original_sim.get_results().await?;
    
    // ══════════════════════════════════════════════════════════════════
    // Run OPTIMIZED Strategy  
    // ══════════════════════════════════════════════════════════════════
    println!("📊 Running OPTIMIZED Strategy...");
    let mut optimized_sim = EnhancedDryRunSimulator::new(
        dec!(1000),
        optimized_strategy_config(),
        optimized_risk_config(),
    ).with_markets(markets.clone())
     .with_stop_loss(dec!(0.15))      // 15% stop loss (tighter)
     .with_take_profit(dec!(0.25))    // 25% take profit
     .with_trailing_stop(dec!(0.10))  // 10% trailing stop
     .with_max_trades_per_hour(3);    // Rate limit
    
    optimized_sim.run_for(100, 0).await?;
    let optimized_result = optimized_sim.get_results().await?;
    
    // ══════════════════════════════════════════════════════════════════
    // COMPARISON REPORT
    // ══════════════════════════════════════════════════════════════════
    println!("\n╔═══════════════════════════════════════════════════════════════╗");
    println!("║               📈 STRATEGY COMPARISON RESULTS                  ║");
    println!("╚═══════════════════════════════════════════════════════════════╝\n");
    
    println!("┌─────────────────────────┬───────────────┬───────────────┬──────────┐");
    println!("│ Metric                  │   Original    │   Optimized   │  Better  │");
    println!("├─────────────────────────┼───────────────┼───────────────┼──────────┤");
    
    // Final Balance
    let orig_balance = original_result.final_balance;
    let opt_balance = optimized_result.final_balance;
    let better_balance = if opt_balance >= orig_balance { "✅ OPT" } else { "❌ ORIG" };
    println!("│ Final Balance           │ ${:>11.2} │ ${:>11.2} │ {:>8} │", 
        orig_balance, opt_balance, better_balance);
    
    // Total P&L
    let orig_pnl = original_result.total_pnl;
    let opt_pnl = optimized_result.total_pnl;
    let better_pnl = if opt_pnl >= orig_pnl { "✅ OPT" } else { "❌ ORIG" };
    println!("│ Total P&L               │ ${:>11.2} │ ${:>11.2} │ {:>8} │", 
        orig_pnl, opt_pnl, better_pnl);
    
    // P&L %
    let orig_pnl_pct = original_result.pnl_pct;
    let opt_pnl_pct = optimized_result.pnl_pct;
    let better_pct = if opt_pnl_pct >= orig_pnl_pct { "✅ OPT" } else { "❌ ORIG" };
    println!("│ Return %                │ {:>12.2}% │ {:>12.2}% │ {:>8} │", 
        orig_pnl_pct, opt_pnl_pct, better_pct);
    
    // Total Trades
    let orig_trades = original_result.total_trades;
    let opt_trades = optimized_result.total_trades;
    println!("│ Total Trades            │ {:>13} │ {:>13} │    ---   │", 
        orig_trades, opt_trades);
    
    // Win Rate
    let orig_wr = original_result.win_rate;
    let opt_wr = optimized_result.win_rate;
    let better_wr = if opt_wr >= orig_wr { "✅ OPT" } else { "❌ ORIG" };
    println!("│ Win Rate                │ {:>12.1}% │ {:>12.1}% │ {:>8} │", 
        orig_wr, opt_wr, better_wr);
    
    // Max Drawdown (lower is better)
    let orig_dd = original_result.max_drawdown;
    let opt_dd = optimized_result.max_drawdown;
    let better_dd = if opt_dd <= orig_dd { "✅ OPT" } else { "❌ ORIG" };
    println!("│ Max Drawdown            │ {:>12.2}% │ {:>12.2}% │ {:>8} │", 
        orig_dd, opt_dd, better_dd);
    
    // Sharpe Ratio
    let orig_sharpe = original_result.sharpe_ratio;
    let opt_sharpe = optimized_result.sharpe_ratio;
    let better_sharpe = if opt_sharpe >= orig_sharpe { "✅ OPT" } else { "❌ ORIG" };
    println!("│ Sharpe Ratio            │ {:>13.2} │ {:>13.2} │ {:>8} │", 
        orig_sharpe, opt_sharpe, better_sharpe);
    
    // Avg Win
    let orig_avg_win = original_result.avg_win;
    let opt_avg_win = optimized_result.avg_win;
    println!("│ Avg Win                 │ ${:>11.2} │ ${:>11.2} │    ---   │", 
        orig_avg_win, opt_avg_win);
    
    // Avg Loss
    let orig_avg_loss = original_result.avg_loss;
    let opt_avg_loss = optimized_result.avg_loss;
    println!("│ Avg Loss                │ ${:>11.2} │ ${:>11.2} │    ---   │", 
        orig_avg_loss, opt_avg_loss);
    
    // Profit Factor (wins/losses ratio)
    let orig_pf = if orig_avg_loss > Decimal::ZERO { orig_avg_win / orig_avg_loss } else { dec!(0) };
    let opt_pf = if opt_avg_loss > Decimal::ZERO { opt_avg_win / opt_avg_loss } else { dec!(0) };
    let better_pf = if opt_pf >= orig_pf { "✅ OPT" } else { "❌ ORIG" };
    println!("│ Profit Factor           │ {:>13.2} │ {:>13.2} │ {:>8} │", 
        orig_pf, opt_pf, better_pf);
    
    println!("└─────────────────────────┴───────────────┴───────────────┴──────────┘");
    
    // Risk-adjusted return
    println!("\n📐 RISK-ADJUSTED METRICS:");
    let orig_risk_adj = if orig_dd > Decimal::ZERO { orig_pnl_pct / orig_dd } else { dec!(0) };
    let opt_risk_adj = if opt_dd > Decimal::ZERO { opt_pnl_pct / opt_dd } else { dec!(0) };
    println!("   Return/Drawdown Ratio: Original={:.3}, Optimized={:.3}", orig_risk_adj, opt_risk_adj);
    
    // Save results
    std::fs::create_dir_all("logs")?;
    
    // Save optimized results
    let opt_file = format!("logs/dry_run_optimized_{}.json", timestamp);
    let mut file = File::create(&opt_file)?;
    writeln!(file, "{}", serde_json::to_string_pretty(&optimized_result)?)?;
    println!("\n📁 Optimized results saved to: {}", opt_file);
    
    // Save comparison summary
    let comparison = serde_json::json!({
        "timestamp": Utc::now().to_rfc3339(),
        "simulation_steps": 100,
        "initial_balance": "1000",
        "original_strategy": {
            "params": {
                "min_edge": "2%",
                "min_confidence": "50%",
                "kelly_fraction": "25%",
                "max_position_pct": "5%",
                "max_exposure_pct": "50%",
                "max_daily_loss_pct": "10%",
                "stop_loss": "20%",
                "take_profit": "30%"
            },
            "results": {
                "final_balance": original_result.final_balance.to_string(),
                "total_pnl": original_result.total_pnl.to_string(),
                "pnl_pct": original_result.pnl_pct.to_string(),
                "total_trades": original_result.total_trades,
                "win_rate": original_result.win_rate.to_string(),
                "max_drawdown": original_result.max_drawdown.to_string(),
                "sharpe_ratio": original_result.sharpe_ratio.to_string()
            }
        },
        "optimized_strategy": {
            "params": {
                "min_edge": "5%",
                "min_confidence": "70%",
                "kelly_fraction": "15%",
                "max_position_pct": "2%",
                "max_exposure_pct": "30%",
                "max_daily_loss_pct": "5%",
                "stop_loss": "15%",
                "take_profit": "25%",
                "trailing_stop": "10%",
                "max_trades_per_hour": 3
            },
            "results": {
                "final_balance": optimized_result.final_balance.to_string(),
                "total_pnl": optimized_result.total_pnl.to_string(),
                "pnl_pct": optimized_result.pnl_pct.to_string(),
                "total_trades": optimized_result.total_trades,
                "win_rate": optimized_result.win_rate.to_string(),
                "max_drawdown": optimized_result.max_drawdown.to_string(),
                "sharpe_ratio": optimized_result.sharpe_ratio.to_string()
            }
        },
        "winner": if opt_risk_adj > orig_risk_adj { "OPTIMIZED" } else { "ORIGINAL" }
    });
    
    let comp_file = format!("logs/comparison_{}.json", timestamp);
    let mut comp = File::create(&comp_file)?;
    writeln!(comp, "{}", serde_json::to_string_pretty(&comparison)?)?;
    println!("📁 Comparison saved to: {}", comp_file);
    
    // Final verdict
    println!("\n╔═══════════════════════════════════════════════════════════════╗");
    if opt_risk_adj > orig_risk_adj {
        println!("║  🏆 WINNER: OPTIMIZED STRATEGY                                ║");
        println!("║  Better risk-adjusted returns with lower drawdown             ║");
    } else if orig_pnl > opt_pnl && orig_dd < opt_dd * dec!(1.5) {
        println!("║  🏆 WINNER: ORIGINAL STRATEGY                                 ║");
        println!("║  Higher absolute returns without excessive drawdown           ║");
    } else {
        println!("║  🤝 TIE: Trade-offs between strategies                        ║");
        println!("║  Consider your risk tolerance when choosing                   ║");
    }
    println!("╚═══════════════════════════════════════════════════════════════╝");
    
    println!("\n✅ Simulation complete!");
    
    Ok(())
}
