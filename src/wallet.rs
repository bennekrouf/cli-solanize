use crate::{config::Config, error::SolanaClientError, token};
use anyhow::Result;
// use solana_account_decoder::{UiAccountEncoding, parse_token::UiTokenAccount};
use solana_client::{
    rpc_client::RpcClient,
    // rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig},
    // rpc_filter::{Memcmp, RpcFilterType},
};
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    // system_instruction,
};
use std::fs;
use tracing::{error, info};

#[derive(Debug, Clone)]
pub struct TokenBalance {
    pub mint: String,
    pub symbol: String,
    pub name: String,
    pub balance: f64,
    pub decimals: u8,
    pub ui_amount: Option<f64>,
}

pub async fn generate_wallet(config: &Config) -> Result<()> {
    app_log!(info, "Generating new wallet");

    let keypair = Keypair::new();
    let pubkey = keypair.pubkey();

    // Save keypair to file
    let keypair_bytes = keypair.to_bytes();
    let keypair_json = serde_json::to_string(&keypair_bytes.to_vec())?;

    fs::write(&config.wallet.keypair_path, keypair_json)?;

    app_log!(info, "✅ Wallet generated successfully!");
    app_log!(info, "📍 Public Key: {}", pubkey);
    app_log!(info, "💾 Saved to: {}", config.wallet.keypair_path);

    Ok(())
}

pub async fn get_wallet_tokens(config: &Config) -> Result<Vec<TokenBalance>> {
    let keypair = load_keypair(config).await?;
    let client = RpcClient::new(&config.solana.rpc_url);

    app_log!(info, "Scanning wallet for SPL tokens");

    let mut token_balances = Vec::new();

    // First, add native SOL balance
    let sol_balance = get_balance(config).await?;
    if sol_balance > 0.0 {
        token_balances.push(TokenBalance {
            mint: config.tokens.sol.clone(),
            symbol: "SOL".to_string(),
            name: "Solana".to_string(),
            balance: sol_balance,
            decimals: 9,
            ui_amount: Some(sol_balance),
        });
    }

    // Get all SPL token accounts owned by this wallet
    let accounts = client.get_token_accounts_by_owner(
        &keypair.pubkey(),
        solana_client::rpc_request::TokenAccountsFilter::ProgramId(spl_token::id()),
    )?;

    app_log!(info, "Found {} token accounts", accounts.len());

    // Process each token account
    for account in accounts {
        if let solana_account_decoder::UiAccountData::Json(token_account) = &account.account.data {
            if let Some(info) = token_account
                .parsed
                .as_object()
                .and_then(|obj| obj.get("info"))
                .and_then(|v| v.as_object())
            {
                let mint = info
                    .get("mint")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let token_amount = info.get("tokenAmount").and_then(|v| v.as_object());

                if let Some(amount_info) = token_amount {
                    let ui_amount = amount_info
                        .get("uiAmount")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0);

                    let decimals = amount_info
                        .get("decimals")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u8;

                    // Skip accounts with zero balance
                    if ui_amount <= 0.0 {
                        continue;
                    }

                    // Try to get token info from Jupiter
                    let (symbol, name) = match token::get_token_info(config, &mint).await {
                        Ok(Some(token_info)) => (token_info.symbol, token_info.name),
                        _ => {
                            // Fallback: use mint address as symbol
                            let short_mint = if mint.len() > 8 {
                                format!("{}..{}", &mint[..4], &mint[mint.len() - 4..])
                            } else {
                                mint.clone()
                            };
                            (
                                short_mint.clone(),
                                format!("Unknown Token ({})", short_mint),
                            )
                        }
                    };

                    token_balances.push(TokenBalance {
                        mint: mint.clone(),
                        symbol,
                        name,
                        balance: ui_amount,
                        decimals,
                        ui_amount: Some(ui_amount),
                    });
                }
            }
        }
    }

    // Sort by balance descending
    token_balances.sort_by(|a, b| {
        b.balance
            .partial_cmp(&a.balance)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(token_balances)
}

pub async fn list_wallet_tokens(config: &Config) -> Result<()> {
    let tokens = get_wallet_tokens(config).await?;

    if tokens.is_empty() {
        app_log!(info, "💸 No tokens found in wallet");
        return Ok(());
    }

    app_log!(info, "🪙 Wallet Token Holdings:");
    app_log!(info, "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    for (i, token) in tokens.iter().enumerate() {
        app_log!(info, 
            "{}. {} ({}) - {} tokens",
            i + 1,
            token.symbol,
            token.name,
            format_balance(token.balance)
        );

        // Show USD value if we can get price
        if let Ok(price) = crate::jupiter::get_token_price(config, &token.symbol).await {
            let usd_value = token.balance * price;
            app_log!(info, "   💲 ~${:.2} (${:.6} per token)", usd_value, price);
        }

        app_log!(info, "   📍 {}", token.mint);
        app_log!(info, );
    }

    Ok(())
}

pub fn format_balance(balance: f64) -> String {
    if balance >= 1_000_000.0 {
        format!("{:.2}M", balance / 1_000_000.0)
    } else if balance >= 1_000.0 {
        format!("{:.2}K", balance / 1_000.0)
    } else if balance >= 1.0 {
        format!("{:.6}", balance)
    } else {
        format!("{:.9}", balance)
    }
}

pub async fn load_keypair(config: &Config) -> Result<Keypair> {
    let keypair_path = &config.wallet.keypair_path;

    if !std::path::Path::new(keypair_path).exists() {
        return Err(SolanaClientError::WalletNotFound {
            path: keypair_path.clone(),
        }
        .into());
    }

    let keypair_json = fs::read_to_string(keypair_path)?;
    let keypair_bytes: Vec<u8> = serde_json::from_str(&keypair_json)?;

    if keypair_bytes.len() != 64 {
        return Err(SolanaClientError::InvalidWalletFormat.into());
    }

    let mut bytes = [0u8; 64];
    bytes.copy_from_slice(&keypair_bytes);

    Ok(Keypair::try_from(&bytes[..])?)
}

pub async fn get_balance(config: &Config) -> Result<f64> {
    let keypair = load_keypair(config).await?;
    let client = RpcClient::new(&config.solana.rpc_url);

    let balance = client.get_balance(&keypair.pubkey())?;
    let sol_balance = balance as f64 / solana_sdk::native_token::LAMPORTS_PER_SOL as f64;

    app_log!(info, "Current balance: {} SOL", sol_balance);
    Ok(sol_balance)
}

pub async fn get_balance_for_pubkey(config: &Config, pubkey: &Pubkey) -> Result<f64> {
    let client = RpcClient::new(&config.solana.rpc_url);

    let balance = client.get_balance(pubkey)?;
    let sol_balance = balance as f64 / solana_sdk::native_token::LAMPORTS_PER_SOL as f64;

    app_log!(info, "Balance for {}: {} SOL", pubkey, sol_balance);
    Ok(sol_balance)
}

pub async fn get_wallet_tokens_for_pubkey(
    config: &Config,
    pubkey: &Pubkey,
) -> Result<Vec<TokenBalance>> {
    let client = RpcClient::new(&config.solana.rpc_url);

    app_log!(info, "Scanning wallet for SPL tokens: {}", pubkey);

    let mut token_balances = Vec::new();

    // First, add native SOL balance
    let sol_balance = get_balance_for_pubkey(config, pubkey).await?;
    if sol_balance > 0.0 {
        token_balances.push(TokenBalance {
            mint: config.tokens.sol.clone(),
            symbol: "SOL".to_string(),
            name: "Solana".to_string(),
            balance: sol_balance,
            decimals: 9,
            ui_amount: Some(sol_balance),
        });
    }

    // Get all SPL token accounts owned by this wallet
    let accounts = client.get_token_accounts_by_owner(
        pubkey,
        solana_client::rpc_request::TokenAccountsFilter::ProgramId(spl_token::id()),
    )?;

    app_log!(info, "Found {} token accounts", accounts.len());

    // Process each token account
    for account in accounts {
        if let solana_account_decoder::UiAccountData::Json(token_account) = &account.account.data {
            if let Some(info) = token_account.parsed.get("info").and_then(|v| v.as_object()) {
                let mint = info
                    .get("mint")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let token_amount = info.get("tokenAmount").and_then(|v| v.as_object());

                if let Some(amount_info) = token_amount {
                    let ui_amount = amount_info
                        .get("uiAmount")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0);

                    let decimals = amount_info
                        .get("decimals")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u8;

                    // Skip accounts with zero balance
                    if ui_amount <= 0.0 {
                        continue;
                    }

                    // Try to get token info from Jupiter
                    let (symbol, name) = match token::get_token_info(config, &mint).await {
                        Ok(Some(token_info)) => (token_info.symbol, token_info.name),
                        _ => {
                            // Fallback: use mint address as symbol
                            let short_mint = if mint.len() > 8 {
                                format!("{}..{}", &mint[..4], &mint[mint.len() - 4..])
                            } else {
                                mint.clone()
                            };
                            (
                                short_mint.clone(),
                                format!("Unknown Token ({})", short_mint),
                            )
                        }
                    };

                    token_balances.push(TokenBalance {
                        mint: mint.clone(),
                        symbol,
                        name,
                        balance: ui_amount,
                        decimals,
                        ui_amount: Some(ui_amount),
                    });
                }
            }
        }
    }

    // Sort by balance descending
    token_balances.sort_by(|a, b| {
        b.balance
            .partial_cmp(&a.balance)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(token_balances)
}

pub async fn request_airdrop(config: &Config, amount: f64) -> Result<()> {
    let keypair = load_keypair(config).await?;
    let client = RpcClient::new(&config.solana.rpc_url);

    let lamports = (amount * solana_sdk::native_token::LAMPORTS_PER_SOL as f64) as u64;

    app_log!(info, "Requesting airdrop of {} SOL", amount);

    match client.request_airdrop(&keypair.pubkey(), lamports) {
        Ok(signature) => {
            app_log!(info, "✅ Airdrop requested successfully!");
            app_log!(info, "🔗 Signature: {}", signature);
            app_log!(info, "⏳ Waiting for confirmation...");

            // Wait for confirmation
            client.confirm_transaction(&signature)?;

            let new_balance = get_balance(config).await?;
            app_log!(info, "💰 New balance: {} SOL", new_balance);
        }
        Err(e) => {
            app_log!(error, "Airdrop failed: {}", e);
            return Err(SolanaClientError::TransactionFailed {
                reason: format!("Airdrop failed: {}", e),
            }
            .into());
        }
    }

    Ok(())
}

