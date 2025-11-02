use crate::app_log;
use crate::{config::Config, jupiter, token, transaction, wallet};
use anyhow::Result;
use dialoguer::{Confirm, Input, Select, theme::ColorfulTheme};
use solana_sdk::signature::Signer;

pub struct InteractiveMenu {
    config: Config,
}

impl InteractiveMenu {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub async fn run(&self) -> Result<()> {
        app_log!(info, "\n🚀 Solana CLI Client - Interactive Mode");
        app_log!(info, "=====================================\n");

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
                    app_log!(info, "👋 Goodbye!");
                    break;
                }
                _ => unreachable!(),
            }

            app_log!(info, "\n{}\n", "=".repeat(50));
        }

        Ok(())
    }

    async fn handle_list_wallet_tokens(&self) -> Result<()> {
        match wallet::list_wallet_tokens(&self.config).await {
            Ok(_) => {}
            Err(e) => {
                app_log!(error, "Failed to list wallet tokens: {}", e);
                app_log!(info, "❌ Error: {}", e);
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
            app_log!(info, "Operation cancelled.");
        }

        Ok(())
    }

    async fn handle_check_balance(&self) -> Result<()> {
        match wallet::get_balance(&self.config).await {
            Ok(balance) => {
                app_log!(info, "💰 Current Balance: {} SOL", balance);
            }
            Err(e) => {
                app_log!(error, "Failed to get balance: {}", e);
                app_log!(info, "❌ Error: {}", e);
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
            app_log!(info, "❌ Amount must be positive");
            return Ok(());
        }

        match wallet::request_airdrop(&self.config, amount).await {
            Ok(_) => app_log!(info, "✅ Airdrop completed successfully!"),
            Err(e) => {
                app_log!(error, "Airdrop failed: {}", e);
                app_log!(info, "❌ Error: {}", e);
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
            app_log!(info, "❌ Amount must be positive");
            return Ok(());
        }

        match transaction::create_transaction(&self.config, &to_address, amount).await {
            Ok(tx_data) => {
                app_log!(info, "✅ Transaction created successfully!");
                app_log!(info, "📋 Copy this transaction data to send later:");
                app_log!(info, "{}", tx_data);
            }
            Err(e) => {
                app_log!(error, "Transaction creation failed: {}", e);
                app_log!(info, "❌ Error: {}", e);
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
            app_log!(info, "Transaction cancelled.");
            return Ok(());
        }

        match transaction::send_transaction(&self.config, &tx_data).await {
            Ok(_) => app_log!(info, "✅ Transaction sent successfully!"),
            Err(e) => {
                app_log!(error, "Transaction send failed: {}", e);
                app_log!(info, "❌ Error: {}", e);
            }
        }

        Ok(())
    }

    async fn handle_swap_tokens(&self) -> Result<()> {
        app_log!(info, "🔄 Token Swap");

        // First, show available tokens in wallet
        let wallet_tokens = wallet::get_wallet_tokens(&self.config).await?;

        if wallet_tokens.is_empty() {
            app_log!(info, "❌ No tokens found in wallet. Get some tokens first!");
            return Ok(());
        }

        app_log!(info, "\n💼 Available tokens in your wallet:");
        for (i, token) in wallet_tokens.iter().enumerate().take(10) {
            app_log!(
                info,
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
            app_log!(info, "❌ Amount must be positive");
            return Ok(());
        }

        // Show price preview first
        match jupiter::get_token_price(&self.config, &from_token).await {
            Ok(price) => {
                let estimated_value = amount * price;
                app_log!(
                    info,
                    "💲 Current {} price: ${:.6}",
                    from_token.to_uppercase(),
                    price
                );
                app_log!(info, "💰 Estimated value: ${:.2}", estimated_value);
            }
            Err(_) => app_log!(info, "⚠️  Could not fetch current price"),
        }

        let confirm = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Proceed with swap?")
            .default(false)
            .interact()?;

        if !confirm {
            app_log!(info, "Swap cancelled.");
            return Ok(());
        }

        match jupiter::swap_tokens(&self.config, &from_token, &to_token, amount).await {
            Ok(_) => app_log!(info, "✅ Swap completed successfully!"),
            Err(e) => {
                app_log!(error, "Swap failed: {}", e);
                app_log!(info, "❌ Error: {}", e);
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
                app_log!(info, "💲 {} price: ${:.6}", token.to_uppercase(), price);

                // Also show token info if available
                if let Ok(Some(token_info)) = token::get_token_info(&self.config, &token).await {
                    app_log!(
                        info,
                        "📝 Token: {} ({})",
                        token_info.name,
                        token_info.symbol
                    );
                    app_log!(info, "📍 Address: {}", token_info.address);
                    app_log!(info, "🔢 Decimals: {}", token_info.decimals);
                }
            }
            Err(e) => {
                app_log!(error, "Failed to get price: {}", e);
                app_log!(info, "❌ Error: {}", e);
            }
        }

        Ok(())
    }

    async fn handle_search_tokens(&self) -> Result<()> {
        let query: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Search tokens (symbol, name, or address)")
            .interact()?;

        if query.trim().is_empty() {
            app_log!(info, "❌ Search query cannot be empty");
            return Ok(());
        }

        match token::search_tokens(&self.config, &query).await {
            Ok(tokens) => {
                if tokens.is_empty() {
                    app_log!(info, "🔍 No tokens found for '{}'", query);
                } else {
                    app_log!(info, "\n📋 Search Results:");
                    for (i, token) in tokens.iter().enumerate() {
                        app_log!(
                            info,
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
                            app_log!(info, "   💲 Price: ${:.6}", price);
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
                app_log!(error, "Token search failed: {}", e);
                app_log!(info, "❌ Error: {}", e);
            }
        }

        Ok(())
    }

    async fn show_token_details(&self, token: &token::TokenInfo) -> Result<()> {
        app_log!(info, "\n🪙 Token Details:");
        app_log!(info, "━━━━━━━━━━━━━━━━━━━━━━");
        app_log!(info, "📛 Symbol: {}", token.symbol);
        app_log!(info, "📝 Name: {}", token.name);
        app_log!(info, "📍 Address: {}", token.address);
        app_log!(info, "🔢 Decimals: {}", token.decimals);

        if !token.tags.is_empty() {
            app_log!(info, "🏷️  Tags: {}", token.tags.join(", "));
        }

        // Get current price
        match jupiter::get_token_price(&self.config, &token.symbol).await {
            Ok(price) => app_log!(info, "💲 Current Price: ${:.6}", price),
            Err(_) => app_log!(info, "💲 Price: Not available"),
        }

        if let Some(logo) = &token.logo_uri {
            app_log!(info, "🖼️  Logo: {}", logo);
        }

        Ok(())
    }

    fn handle_show_config(&self) -> Result<()> {
        app_log!(info, "⚙️  Current Configuration:");
        app_log!(info, "Network: {}", self.config.solana.network);
        app_log!(info, "RPC URL: {}", self.config.solana.rpc_url);
        app_log!(info, "Wallet Path: {}", self.config.wallet.keypair_path);
        app_log!(info, "Log Level: {}", self.config.logging.level);
        app_log!(info, "Jupiter API: {}", self.config.jupiter.api_url);
        app_log!(info, "Slippage: {}bps", self.config.jupiter.slippage_bps);

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
                    app_log!(info, "No transactions found");
                } else {
                    app_log!(info, "\nTransaction History:");
                    for (i, tx) in history.iter().enumerate() {
                        app_log!(info, "{}. {} | {:?}", i + 1, &tx.signature[..8], tx.status);
                        if let Some(amount) = tx.amount {
                            app_log!(
                                info,
                                "   Amount: {} {}",
                                amount,
                                tx.token_symbol.as_deref().unwrap_or("Unknown")
                            );
                        }
                    }
                }
            }
            Err(e) => {
                app_log!(error, "Failed to get transaction history: {}", e);
                app_log!(info, "Error: {}", e);
            }
        }
        Ok(())
    }

    async fn handle_pending_transactions(&self) -> Result<()> {
        let keypair = wallet::load_keypair(&self.config).await?;

        match transaction::fetch_pending_transactions(&self.config, &keypair.pubkey()).await {
            Ok(pending) => {
                if pending.is_empty() {
                    app_log!(info, "No pending transactions");
                } else {
                    app_log!(info, "\nPending Transactions:");
                    for (i, tx) in pending.iter().enumerate() {
                        app_log!(info, "{}. {} | {:?}", i + 1, &tx.signature[..8], tx.status);
                    }
                }
            }
            Err(e) => {
                app_log!(error, "Failed to get pending transactions: {}", e);
                app_log!(info, "Error: {}", e);
            }
        }
        Ok(())
    }
}
