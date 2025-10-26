use crate::{config::Config, jupiter, token, transaction, wallet};
use anyhow::Result;
use dialoguer::{Confirm, Input, Select, theme::ColorfulTheme};
use solana_sdk::signature::Signer;
use tracing::error;

pub struct InteractiveMenu {
    config: Config,
}

impl InteractiveMenu {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub async fn run(&self) -> Result<()> {
        println!("\n🚀 Solana CLI Client - Interactive Mode");
        println!("=====================================\n");

        loop {
            let options = vec![
                "🔑 Generate Wallet",
                "💰 Check Balance",
                "🎁 Request Airdrop (Testnet)",
                "📤 Create Transaction",
                "🚀 Send Transaction",
                "🔄 Swap Tokens (Jupiter)",
                "💲 Get Token Price",
                "🔍 Search Tokens",
                "🪙 List Wallet Tokens",
                "📜 Transaction History",
                "⏳ Pending Transactions",
                "⚙️  Show Config",
                "❌ Exit",
            ];

            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt("Choose an operation")
                .items(&options)
                .default(0)
                .interact()?;

            match selection {
                0 => self.handle_generate_wallet().await?,
                1 => self.handle_check_balance().await?,
                2 => self.handle_airdrop().await?,
                3 => self.handle_create_transaction().await?,
                4 => self.handle_send_transaction().await?,
                5 => self.handle_swap_tokens().await?,
                6 => self.handle_get_price().await?,
                7 => self.handle_search_tokens().await?,
                8 => self.handle_list_wallet_tokens().await?,
                9 => self.handle_transaction_history().await?, // Add this line
                10 => self.handle_pending_transactions().await?, // Add this line
                11 => self.handle_show_config()?,              // Update: was 9
                12 => {
                    // Update: was 10
                    println!("👋 Goodbye!");
                    break;
                }
                _ => unreachable!(),
            }

            println!("\n{}\n", "=".repeat(50));
        }

        Ok(())
    }

    async fn handle_list_wallet_tokens(&self) -> Result<()> {
        match wallet::list_wallet_tokens(&self.config).await {
            Ok(_) => {}
            Err(e) => {
                error!("Failed to list wallet tokens: {}", e);
                println!("❌ Error: {}", e);
            }
        }

        Ok(())
    }

    async fn handle_generate_wallet(&self) -> Result<()> {
        let confirm = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("This will overwrite existing wallet. Continue?")
            .default(false)
            .interact()?;

        if confirm {
            wallet::generate_wallet(&self.config).await?;
        } else {
            println!("Operation cancelled.");
        }

        Ok(())
    }

    async fn handle_check_balance(&self) -> Result<()> {
        match wallet::get_balance(&self.config).await {
            Ok(balance) => {
                println!("💰 Current Balance: {} SOL", balance);
            }
            Err(e) => {
                error!("Failed to get balance: {}", e);
                println!("❌ Error: {}", e);
            }
        }

        Ok(())
    }

    async fn handle_airdrop(&self) -> Result<()> {
        let amount: f64 = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Amount (SOL)")
            .default(self.config.faucet.airdrop_amount)
            .interact()?;

        if amount <= 0.0 {
            println!("❌ Amount must be positive");
            return Ok(());
        }

        match wallet::request_airdrop(&self.config, amount).await {
            Ok(_) => println!("✅ Airdrop completed successfully!"),
            Err(e) => {
                error!("Airdrop failed: {}", e);
                println!("❌ Error: {}", e);
            }
        }

        Ok(())
    }

    async fn handle_create_transaction(&self) -> Result<()> {
        let to_address: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Recipient address")
            .interact()?;

        let amount: f64 = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Amount (SOL)")
            .interact()?;

        if amount <= 0.0 {
            println!("❌ Amount must be positive");
            return Ok(());
        }

        match transaction::create_transaction(&self.config, &to_address, amount).await {
            Ok(tx_data) => {
                println!("✅ Transaction created successfully!");
                println!("📋 Copy this transaction data to send later:");
                println!("{}", tx_data);
            }
            Err(e) => {
                error!("Transaction creation failed: {}", e);
                println!("❌ Error: {}", e);
            }
        }

        Ok(())
    }

    async fn handle_send_transaction(&self) -> Result<()> {
        let tx_data: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Transaction data")
            .interact()?;

        let confirm = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Confirm sending transaction?")
            .default(false)
            .interact()?;

        if !confirm {
            println!("Transaction cancelled.");
            return Ok(());
        }

        match transaction::send_transaction(&self.config, &tx_data).await {
            Ok(_) => println!("✅ Transaction sent successfully!"),
            Err(e) => {
                error!("Transaction send failed: {}", e);
                println!("❌ Error: {}", e);
            }
        }

        Ok(())
    }

    async fn handle_swap_tokens(&self) -> Result<()> {
        println!("🔄 Token Swap");

        // First, show available tokens in wallet
        let wallet_tokens = wallet::get_wallet_tokens(&self.config).await?;

        if wallet_tokens.is_empty() {
            println!("❌ No tokens found in wallet. Get some tokens first!");
            return Ok(());
        }

        println!("\n💼 Available tokens in your wallet:");
        for (i, token) in wallet_tokens.iter().enumerate().take(10) {
            println!(
                "{}. {} - {} tokens",
                i + 1,
                token.symbol,
                wallet::format_balance(token.balance)
            );
        }

        let from_token: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("From token (symbol or address)")
            .interact()?;

        let to_token: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("To token (symbol or address)")
            .interact()?;

        let amount: f64 = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Amount")
            .interact()?;

        if amount <= 0.0 {
            println!("❌ Amount must be positive");
            return Ok(());
        }

        // Show price preview first
        match jupiter::get_token_price(&self.config, &from_token).await {
            Ok(price) => {
                let estimated_value = amount * price;
                println!(
                    "💲 Current {} price: ${:.6}",
                    from_token.to_uppercase(),
                    price
                );
                println!("💰 Estimated value: ${:.2}", estimated_value);
            }
            Err(_) => println!("⚠️  Could not fetch current price"),
        }

        let confirm = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Proceed with swap?")
            .default(false)
            .interact()?;

        if !confirm {
            println!("Swap cancelled.");
            return Ok(());
        }

        match jupiter::swap_tokens(&self.config, &from_token, &to_token, amount).await {
            Ok(_) => println!("✅ Swap completed successfully!"),
            Err(e) => {
                error!("Swap failed: {}", e);
                println!("❌ Error: {}", e);
            }
        }

        Ok(())
    }

    async fn handle_get_price(&self) -> Result<()> {
        let token: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Token symbol or address")
            .default("SOL".to_string())
            .interact()?;

        match jupiter::get_token_price(&self.config, &token).await {
            Ok(price) => {
                println!("💲 {} price: ${:.6}", token.to_uppercase(), price);

                // Also show token info if available
                if let Ok(Some(token_info)) = token::get_token_info(&self.config, &token).await {
                    println!("📝 Token: {} ({})", token_info.name, token_info.symbol);
                    println!("📍 Address: {}", token_info.address);
                    println!("🔢 Decimals: {}", token_info.decimals);
                }
            }
            Err(e) => {
                error!("Failed to get price: {}", e);
                println!("❌ Error: {}", e);
            }
        }

        Ok(())
    }

    async fn handle_search_tokens(&self) -> Result<()> {
        let query: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Search tokens (symbol, name, or address)")
            .interact()?;

        if query.trim().is_empty() {
            println!("❌ Search query cannot be empty");
            return Ok(());
        }

        match token::search_tokens(&self.config, &query).await {
            Ok(tokens) => {
                if tokens.is_empty() {
                    println!("🔍 No tokens found for '{}'", query);
                } else {
                    println!("\n📋 Search Results:");
                    for (i, token) in tokens.iter().enumerate() {
                        println!(
                            "{}. {} ({}) - {}",
                            i + 1,
                            token.symbol,
                            token.name,
                            &token.address[..8]
                        );

                        // Show price if available
                        if let Ok(price) =
                            jupiter::get_token_price(&self.config, &token.symbol).await
                        {
                            println!("   💲 Price: ${:.6}", price);
                        }
                    }

                    // Option to get more details
                    let mut options: Vec<String> = tokens
                        .iter()
                        .map(|t| format!("{} ({})", t.symbol, t.name))
                        .collect();
                    options.push("Skip".to_string());

                    let selection = Select::with_theme(&ColorfulTheme::default())
                        .with_prompt("Get details for token")
                        .items(&options)
                        .default(options.len() - 1)
                        .interact()?;

                    if selection < tokens.len() {
                        let selected_token = &tokens[selection];
                        self.show_token_details(selected_token).await?;
                    }
                }
            }
            Err(e) => {
                error!("Token search failed: {}", e);
                println!("❌ Error: {}", e);
            }
        }

        Ok(())
    }

    async fn show_token_details(&self, token: &token::TokenInfo) -> Result<()> {
        println!("\n🪙 Token Details:");
        println!("━━━━━━━━━━━━━━━━━━━━━━");
        println!("📛 Symbol: {}", token.symbol);
        println!("📝 Name: {}", token.name);
        println!("📍 Address: {}", token.address);
        println!("🔢 Decimals: {}", token.decimals);

        if !token.tags.is_empty() {
            println!("🏷️  Tags: {}", token.tags.join(", "));
        }

        // Get current price
        match jupiter::get_token_price(&self.config, &token.symbol).await {
            Ok(price) => println!("💲 Current Price: ${:.6}", price),
            Err(_) => println!("💲 Price: Not available"),
        }

        if let Some(logo) = &token.logo_uri {
            println!("🖼️  Logo: {}", logo);
        }

        Ok(())
    }

    fn handle_show_config(&self) -> Result<()> {
        println!("⚙️  Current Configuration:");
        println!("Network: {}", self.config.solana.network);
        println!("RPC URL: {}", self.config.solana.rpc_url);
        println!("Wallet Path: {}", self.config.wallet.keypair_path);
        println!("Log Level: {}", self.config.logging.level);
        println!("Jupiter API: {}", self.config.jupiter.api_url);
        println!("Slippage: {}bps", self.config.jupiter.slippage_bps);

        Ok(())
    }

    async fn handle_transaction_history(&self) -> Result<()> {
        let limit: usize = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Number of transactions to fetch")
            .default(20)
            .interact()?;

        let keypair = wallet::load_keypair(&self.config).await?;

        match transaction::fetch_transaction_history(
            &self.config,
            &keypair.pubkey(),
            Some(limit),
            None,
        )
        .await
        {
            Ok(history) => {
                if history.is_empty() {
                    println!("No transactions found");
                } else {
                    println!("\nTransaction History:");
                    for (i, tx) in history.iter().enumerate() {
                        println!("{}. {} | {:?}", i + 1, &tx.signature[..8], tx.status);
                        if let Some(amount) = tx.amount {
                            println!(
                                "   Amount: {} {}",
                                amount,
                                tx.token_symbol.as_deref().unwrap_or("Unknown")
                            );
                        }
                    }
                }
            }
            Err(e) => {
                error!("Failed to get transaction history: {}", e);
                println!("Error: {}", e);
            }
        }
        Ok(())
    }

    async fn handle_pending_transactions(&self) -> Result<()> {
        let keypair = wallet::load_keypair(&self.config).await?;

        match transaction::fetch_pending_transactions(&self.config, &keypair.pubkey()).await {
            Ok(pending) => {
                if pending.is_empty() {
                    println!("No pending transactions");
                } else {
                    println!("\nPending Transactions:");
                    for (i, tx) in pending.iter().enumerate() {
                        println!("{}. {} | {:?}", i + 1, &tx.signature[..8], tx.status);
                    }
                }
            }
            Err(e) => {
                error!("Failed to get pending transactions: {}", e);
                println!("Error: {}", e);
            }
        }
        Ok(())
    }
}
