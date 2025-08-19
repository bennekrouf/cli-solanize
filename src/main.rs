use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing::info;

mod cli;
mod config;
mod error;
mod jupiter;
mod solana_client;
mod token;
mod transaction;
mod wallet;
mod web;

use cli::InteractiveMenu;
use config::Config;

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
    WebServer {
        #[arg(short, long, default_value = "8000")]
        port: u16,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize config
    let config = Config::load(&cli.config)?;

    // Initialize logging
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(&config.logging.level)
        .with_target(false);

    match config.logging.format.as_str() {
        "json" => {
            #[cfg(feature = "json")]
            subscriber.json().init();
            #[cfg(not(feature = "json"))]
            {
                eprintln!("JSON logging not available, falling back to pretty format");
                subscriber.pretty().init();
            }
        }
        _ => subscriber.pretty().init(),
    }

    info!("Starting Solana CLI client");

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
            println!("Balance: {} SOL", balance);
        }
        Some(Commands::Faucet { amount }) => {
            wallet::request_airdrop(&config, amount).await?;
        }
        Some(Commands::CreateTx { to, amount }) => {
            let tx = transaction::create_transaction(&config, &to, amount).await?;
            println!("Transaction created: {}", tx);
        }
        Some(Commands::SendTx { signature }) => {
            transaction::send_transaction(&config, &signature).await?;
        }
        Some(Commands::Swap { from, to, amount }) => {
            jupiter::swap_tokens(&config, &from, &to, amount).await?;
        }
        Some(Commands::Price { token }) => {
            let price = jupiter::get_token_price(&config, &token).await?;
            println!("Price for {}: ${}", token, price);
        }
        Some(Commands::Search { query }) => {
            let tokens = token::search_tokens(&config, &query).await?;
            for token in tokens {
                println!("{}: {} ({})", token.symbol, token.name, token.address);
            }
        }
        Some(Commands::ListTokens) => {
            wallet::list_wallet_tokens(&config).await?;
        }
        Some(Commands::WebServer { port }) => {
            info!("Starting web server on port {}", port);
            web::start_server(config, port).await?;
        }
    }

    Ok(())
}

