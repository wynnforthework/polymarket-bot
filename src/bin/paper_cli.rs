//! Paper Trading CLI
//!
//! Commands:
//! - status: Show account summary
//! - buy: Simulate buying shares
//! - sell: Simulate selling a position
//! - positions: List open positions
//! - history: Show trade history

use clap::{Parser, Subcommand};
use polymarket_bot::paper::{PaperTrader, PaperTraderConfig, PortfolioSummary, PositionSide};
use polymarket_bot::client::GammaClient;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::str::FromStr;

#[derive(Parser)]
#[command(name = "paper")]
#[command(about = "Paper trading CLI for Polymarket simulation")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Show account status and summary
    Status,
    /// Simulate buying shares in a market
    Buy {
        /// Market ID (numeric) or search query
        #[arg(short, long)]
        market: String,
        /// Use market ID directly instead of searching
        #[arg(long)]
        id: bool,
        /// Side: yes or no
        #[arg(short, long)]
        side: String,
        /// Amount in USD
        #[arg(short, long)]
        amount: f64,
    },
    /// Sell/close a position
    Sell {
        /// Position ID
        #[arg(short, long)]
        position: String,
    },
    /// List open positions
    Positions,
    /// Show trade history
    History {
        /// Number of records to show
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    
    // Initialize trader
    let client = GammaClient::new("https://gamma-api.polymarket.com")?;
    let config = PaperTraderConfig {
        initial_balance: dec!(1000),
        ..Default::default()
    };
    let trader = PaperTrader::new(config, client.clone());
    
    // Try to load existing state
    let state_file = "paper_trading_state.json";
    let _ = trader.load_state(state_file).await;
    
    match cli.command {
        Commands::Status => {
            let summary = trader.get_summary().await;
            print_status(&summary);
        }
        Commands::Buy { market, id, side, amount } => {
            // Parse side
            let position_side = match side.to_lowercase().as_str() {
                "yes" | "y" => PositionSide::Yes,
                "no" | "n" => PositionSide::No,
                _ => {
                    eprintln!("❌ Invalid side: {}. Use 'yes' or 'no'", side);
                    return Ok(());
                }
            };
            
            // Get market by ID or search
            let target_market = if id {
                println!("🔍 Fetching market ID: {}", market);
                match client.get_market(&market).await {
                    Ok(m) => m,
                    Err(e) => {
                        eprintln!("❌ Market not found: {}", e);
                        return Ok(());
                    }
                }
            } else {
                println!("🔍 Searching for market: {}", market);
                let markets = client.search_markets(&market).await?;
                
                if markets.is_empty() {
                    eprintln!("❌ No markets found for: {}", market);
                    return Ok(());
                }
                
                // Find best matching active market with valid prices
                let found = markets.into_iter()
                    .filter(|m| m.active && !m.closed)
                    .filter(|m| m.outcomes.iter().any(|o| o.price > dec!(0) && o.price < dec!(1)))
                    .next();
                
                match found {
                    Some(m) => m,
                    None => {
                        eprintln!("❌ No valid active markets found for: {}", market);
                        return Ok(());
                    }
                }
            };
            
            let target_market = &target_market;
            
            println!("📊 Found: {}", target_market.question);
            
            // Show current prices
            if let Some(yes_outcome) = target_market.outcomes.first() {
                println!("   YES: {:.1}¢", yes_outcome.price * dec!(100));
            }
            if let Some(no_outcome) = target_market.outcomes.get(1) {
                println!("   NO:  {:.1}¢", no_outcome.price * dec!(100));
            }
            
            // Execute buy
            let amount_dec = Decimal::from_str(&amount.to_string()).unwrap_or(dec!(0));
            match trader.buy(target_market, position_side, amount_dec, format!("CLI buy: {}", market)).await {
                Ok(position) => {
                    println!("✅ Bought {} {} @ {:.4}", position.shares.round_dp(2), position.side, position.entry_price);
                    println!("   Cost: ${:.2}", position.cost_basis);
                    println!("   Position ID: {}", position.id);
                }
                Err(e) => {
                    eprintln!("❌ Buy failed: {}", e);
                }
            }
        }
        Commands::Sell { position } => {
            match trader.sell(&position, "CLI sell".to_string()).await {
                Ok(trade) => {
                    let pnl = trade.pnl.unwrap_or(dec!(0));
                    let emoji = if pnl >= dec!(0) { "✅" } else { "❌" };
                    println!("{} Sold {} {} @ {:.4}", emoji, trade.shares.round_dp(2), trade.side, trade.price);
                    println!("   Proceeds: ${:.2}", trade.total_value);
                    println!("   P&L: ${:.2}", pnl);
                }
                Err(e) => {
                    eprintln!("❌ Sell failed: {}", e);
                }
            }
        }
        Commands::Positions => {
            let positions = trader.get_open_positions().await;
            print_positions(&positions);
        }
        Commands::History { limit } => {
            let history = trader.get_history().await;
            print_history(&history, limit);
        }
    }
    
    // Save state after any operation
    let _ = trader.save_state(state_file).await;
    
    Ok(())
}

fn print_status(summary: &PortfolioSummary) {
    println!("╔══════════════════════════════════════╗");
    println!("║       📊 Paper Trading Status        ║");
    println!("╠══════════════════════════════════════╣");
    println!("║ Initial Balance:    ${:>15.2} ║", summary.initial_balance);
    println!("║ Cash Balance:       ${:>15.2} ║", summary.cash_balance);
    println!("║ Positions Value:    ${:>15.2} ║", summary.positions_value);
    println!("║ ─────────────────────────────────── ║");
    println!("║ Total Value:        ${:>15.2} ║", summary.total_value);
    println!("║ Total P&L:          ${:>15.2} ║", summary.total_pnl);
    println!("║ ROI:                {:>15.1}% ║", summary.roi_percent);
    println!("║ ─────────────────────────────────── ║");
    println!("║ Trades:             {:>15} ║", summary.trade_count);
    println!("║ Win Rate:           {:>14.1}% ║", summary.win_rate * dec!(100));
    println!("║ Open Positions:     {:>15} ║", summary.open_positions);
    println!("╚══════════════════════════════════════╝");
}

fn print_positions(positions: &[polymarket_bot::paper::Position]) {
    if positions.is_empty() {
        println!("📭 No open positions");
        return;
    }
    
    println!("📈 Open Positions ({}):", positions.len());
    println!("─────────────────────────────────────────────────────────────");
    for pos in positions {
        let pnl_emoji = if pos.unrealized_pnl >= dec!(0) { "🟢" } else { "🔴" };
        println!(
            "{} {} | {} @ {:.4} | Value: ${:.2} | PnL: ${:.2} ({:.1}%)",
            pnl_emoji,
            pos.side,
            &pos.question[..pos.question.len().min(30)],
            pos.entry_price,
            pos.current_value,
            pos.unrealized_pnl,
            pos.unrealized_pnl_pct
        );
    }
}

fn print_history(history: &[polymarket_bot::paper::TradeRecord], limit: usize) {
    if history.is_empty() {
        println!("📭 No trade history");
        return;
    }
    
    let recent: Vec<_> = history.iter().rev().take(limit).collect();
    println!("📜 Trade History (last {}):", recent.len());
    println!("─────────────────────────────────────────────────────────────");
    for trade in recent {
        let action_emoji = match trade.action {
            polymarket_bot::paper::TradeAction::Buy => "🟢 BUY",
            polymarket_bot::paper::TradeAction::Sell => "🔴 SELL",
        };
        let pnl_str = trade.pnl
            .map(|p| format!(" PnL: ${:.2}", p))
            .unwrap_or_default();
        println!(
            "{} {} {} @ {:.4} = ${:.2}{}",
            trade.timestamp.format("%m-%d %H:%M"),
            action_emoji,
            trade.side,
            trade.price,
            trade.total_value,
            pnl_str
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn test_cli_help() {
        // Verify CLI parses correctly
        Cli::command().debug_assert();
    }

    #[test]
    fn test_cli_status_parses() {
        let cli = Cli::parse_from(["paper", "status"]);
        assert!(matches!(cli.command, Commands::Status));
    }

    #[test]
    fn test_cli_buy_parses() {
        let cli = Cli::parse_from([
            "paper", "buy", 
            "--market", "btc-100k",
            "--side", "yes",
            "--amount", "50"
        ]);
        if let Commands::Buy { market, side, amount, id } = cli.command {
            assert_eq!(market, "btc-100k");
            assert_eq!(side, "yes");
            assert!((amount - 50.0).abs() < 0.01);
            assert!(!id); // Default is false
        } else {
            panic!("Expected Buy command");
        }
    }

    #[test]
    fn test_cli_buy_with_id_flag() {
        let cli = Cli::parse_from([
            "paper", "buy", 
            "--id",
            "--market", "517310",
            "--side", "yes",
            "--amount", "50"
        ]);
        if let Commands::Buy { market, id, .. } = cli.command {
            assert_eq!(market, "517310");
            assert!(id);
        } else {
            panic!("Expected Buy command");
        }
    }

    #[test]
    fn test_cli_sell_parses() {
        let cli = Cli::parse_from([
            "paper", "sell",
            "--position", "pos123"
        ]);
        if let Commands::Sell { position } = cli.command {
            assert_eq!(position, "pos123");
        } else {
            panic!("Expected Sell command");
        }
    }

    #[test]
    fn test_cli_positions_parses() {
        let cli = Cli::parse_from(["paper", "positions"]);
        assert!(matches!(cli.command, Commands::Positions));
    }

    #[test]
    fn test_cli_history_parses() {
        let cli = Cli::parse_from(["paper", "history", "--limit", "20"]);
        if let Commands::History { limit } = cli.command {
            assert_eq!(limit, 20);
        } else {
            panic!("Expected History command");
        }
    }

    #[test]
    fn test_cli_history_default_limit() {
        let cli = Cli::parse_from(["paper", "history"]);
        if let Commands::History { limit } = cli.command {
            assert_eq!(limit, 10);
        } else {
            panic!("Expected History command");
        }
    }
}
