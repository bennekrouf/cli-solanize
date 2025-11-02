mod cli;
mod config;
mod error;
mod jupiter;
mod token;
mod transaction;
mod wallet;
mod web;

use anyhow::Result;
use clap::{Parser, Subcommand};
use graflog::app_log;
use graflog::init_logging;
use solana_sdk::signature::Signer;
use std::str::FromStr;

use crate::cli::InteractiveMenu;
use crate::config::Config;

#[derive(Parser)]
#[command(name = "solana-cli-client")]
#[command(about = "A CLI client for basic Solana operations")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(short, long, default_value = "config.yaml")]
    config: String,
}

#[derive(Subcommand)]
enum Commands {
    /// Interactive menu mode
    Menu,
    /// Generate a new wallet
    GenerateWallet,
    /// Get wallet balance
    Balance,
    /// Request SOL from faucet (testnet/devnet only)
    Faucet {
        #[arg(short, long, default_value = "1.0")]
        amount: f64,
    },
    /// Create a transaction
    CreateTx {
        #[arg(short, long)]
        to: String,
        #[arg(short, long)]
        amount: f64,
    },
    /// Send a transaction
    SendTx {
        #[arg(short, long)]
        signature: String,
    },
    /// Swap tokens using Jupiter
    Swap {
        #[arg(short, long)]
        from: String,
        #[arg(short, long)]
        to: String,
        #[arg(short, long)]
        amount: f64,
    },
    /// Get token price
    Price {
        #[arg(short, long)]
        token: String,
    },
    /// Search token by symbol or address
    Search {
        #[arg(short, long)]
        query: String,
    },
    /// List all tokens in wallet
    ListTokens,
    /// Start web server
    Server,
    /// Get transaction history for wallet
    History {
        #[arg(short, long, default_value = "50")]
        limit: usize,
        #[arg(short, long)]
        before: Option<String>,
        #[arg(short, long)]
        pubkey: Option<String>, // Optional: check other wallet
    },
    /// Get pending transactions
    Pending {
        #[arg(short, long)]
        pubkey: Option<String>, // Optional: check other wallet
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize config
    let config = Config::load(&cli.config)?;

    init_logging!("/var/log/solanize.log", "solanize", "cli");
    app_log!(info, "Starting Solana CLI client");

    match cli.command {
        Some(Commands::Menu) | None => {
            let menu = InteractiveMenu::new(config);
            menu.run().await?;
        }
        Some(Commands::GenerateWallet) => {
            wallet::generate_wallet(&config).await?;
        }
        Some(Commands::Balance) => {
            let balance = wallet::get_balance(&config).await?;
            app_log!(info, "Balance: {} SOL", balance);
        }
        Some(Commands::Faucet { amount }) => {
            wallet::request_airdrop(&config, amount).await?;
        }
        Some(Commands::CreateTx { to, amount }) => {
            let tx = transaction::create_transaction(&config, &to, amount).await?;
            app_log!(info, "Transaction created: {}", tx);
        }
        Some(Commands::SendTx { signature }) => {
            transaction::send_transaction(&config, &signature).await?;
        }
        Some(Commands::Swap { from, to, amount }) => {
            jupiter::swap_tokens(&config, &from, &to, amount).await?;
        }
        Some(Commands::Price { token }) => {
            let price = jupiter::get_token_price(&config, &token).await?;
            app_log!(info, "Price for {}: ${}", token, price);
        }
        Some(Commands::Search { query }) => {
            let tokens = token::search_tokens(&config, &query).await?;
            for token in tokens {
                app_log!(info, "{}: {} ({})", token.symbol, token.name, token.address);
            }
        }
        Some(Commands::ListTokens) => {
            wallet::list_wallet_tokens(&config).await?;
        }
        Some(Commands::Server) => {
            let port = std::env::var("ROCKET_PORT")
                .map_err(|_| anyhow::anyhow!("ROCKET_PORT environment variable is required"))?
                .parse::<u16>()
                .map_err(|_| anyhow::anyhow!("ROCKET_PORT must be a valid port number"))?;

            app_log!(info, "Starting web server on port {}", port);
            web::start_server(config, port).await?;
        }
        Some(Commands::History {
            limit,
            before,
            pubkey,
        }) => {
            let target_pubkey = if let Some(pk) = pubkey {
                solana_sdk::pubkey::Pubkey::from_str(&pk)?
            } else {
                let keypair = wallet::load_keypair(&config).await?;
                keypair.pubkey()
            };

            let history = transaction::fetch_transaction_history(
                &config,
                &target_pubkey,
                Some(limit),
                before,
            )
            .await?;

            if history.is_empty() {
                app_log!(info, "No transactions found");
            } else {
                app_log!(info, "Transaction History for {}", target_pubkey);
                app_log!(info, "{}", "=".repeat(80));

                for (i, tx) in history.iter().enumerate() {
                    app_log!(
                        info,
                        "{}. {} | {} | {:?}",
                        i + 1,
                        &tx.signature[..8],
                        format_tx_type(&tx.transaction_type),
                        tx.status
                    );

                    if let Some(amount) = tx.amount {
                        let symbol = tx.token_symbol.as_deref().unwrap_or("Unknown");
                        app_log!(info, "   Amount: {} {}", amount, symbol);
                    }

                    if let Some(fee) = tx.fee {
                        app_log!(info, "   Fee: {} SOL", fee);
                    }

                    if let Some(block_time) = tx.block_time {
                        let dt = chrono::DateTime::from_timestamp(block_time, 0)
                            .unwrap_or_else(|| chrono::Utc::now());
                        app_log!(info, "   Time: {}", dt.format("%Y-%m-%d %H:%M:%S UTC"));
                    }

                    match tx.confirmation_status {
                        transaction::ConfirmationStatus::Finalized => {
                            app_log!(info, "   Status: Finalized")
                        }
                        transaction::ConfirmationStatus::Confirmed => {
                            app_log!(info, "   Status: Confirmed")
                        }
                        transaction::ConfirmationStatus::Processed => {
                            app_log!(info, "   Status: Processed")
                        }
                    }

                    if let Some(error) = &tx.error {
                        app_log!(info, "   Error: {}", error);
                    }

                    app_log!(info,);
                }
            }
        }

        Some(Commands::Pending { pubkey }) => {
            let target_pubkey = if let Some(pk) = pubkey {
                solana_sdk::pubkey::Pubkey::from_str(&pk)?
            } else {
                let keypair = wallet::load_keypair(&config).await?;
                keypair.pubkey()
            };

            let pending = transaction::fetch_pending_transactions(&config, &target_pubkey).await?;

            if pending.is_empty() {
                app_log!(info, "No pending transactions");
            } else {
                app_log!(info, "Pending Transactions for {}", target_pubkey);
                app_log!(info, "{}", "=".repeat(50));

                for (i, tx) in pending.iter().enumerate() {
                    app_log!(
                        info,
                        "{}. {} | {} | {:?}",
                        i + 1,
                        &tx.signature[..8],
                        format_tx_type(&tx.transaction_type),
                        tx.status
                    );

                    if let Some(amount) = tx.amount {
                        let symbol = tx.token_symbol.as_deref().unwrap_or("Unknown");
                        app_log!(info, "   Amount: {} {}", amount, symbol);
                    }

                    app_log!(info, "   Status: Pending confirmation");
                    app_log!(info,);
                }
            }
        }
    }

    Ok(())
}

fn format_tx_type(tx_type: &transaction::TransactionType) -> &'static str {
    match tx_type {
        transaction::TransactionType::Transfer => "SOL Transfer",
        transaction::TransactionType::TokenTransfer => "Token Transfer",
        transaction::TransactionType::Swap => "Token Swap",
        transaction::TransactionType::Unknown => "Unknown",
    }
}
