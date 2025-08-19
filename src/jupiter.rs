use crate::{config::Config, error::SolanaClientError, wallet::load_keypair};
use anyhow::Result;
use base64;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use solana_sdk::signature::Signer;
use solana_sdk::transaction::VersionedTransaction;
use solana_sdk::{pubkey::Pubkey, signature::Keypair, transaction::Transaction};
use std::str::FromStr;
use tracing::{error, info};

#[derive(Debug, Serialize, Deserialize)]
pub struct QuoteResponse {
    #[serde(rename = "inputMint")]
    pub input_mint: String,
    #[serde(rename = "inAmount")]
    pub in_amount: String,
    #[serde(rename = "outputMint")]
    pub output_mint: String,
    #[serde(rename = "outAmount")]
    pub out_amount: String,
    #[serde(rename = "otherAmountThreshold")]
    pub other_amount_threshold: String,
    #[serde(rename = "swapMode")]
    pub swap_mode: String,
    #[serde(rename = "slippageBps")]
    pub slippage_bps: u16,
    #[serde(rename = "platformFee")]
    pub platform_fee: Option<PlatformFee>,
    #[serde(rename = "priceImpactPct")]
    pub price_impact_pct: String,
    #[serde(rename = "routePlan")]
    pub route_plan: Vec<RoutePlan>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PlatformFee {
    pub amount: String,
    pub fee_bps: u16,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RoutePlan {
    #[serde(rename = "swapInfo")]
    pub swap_info: SwapInfo,
    pub percent: u8,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SwapInfo {
    #[serde(rename = "ammKey")]
    pub amm_key: String,
    pub label: String,
    #[serde(rename = "inputMint")]
    pub input_mint: String,
    #[serde(rename = "outputMint")]
    pub output_mint: String,
    #[serde(rename = "inAmount")]
    pub in_amount: String,
    #[serde(rename = "outAmount")]
    pub out_amount: String,
    #[serde(rename = "feeAmount")]
    pub fee_amount: String,
    #[serde(rename = "feeMint")]
    pub fee_mint: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SwapRequest {
    #[serde(rename = "quoteResponse")]
    pub quote_response: QuoteResponse,
    #[serde(rename = "userPublicKey")]
    pub user_public_key: String,
    #[serde(rename = "wrapAndUnwrapSol")]
    pub wrap_and_unwrap_sol: bool,
    #[serde(rename = "useSharedAccounts")]
    pub use_shared_accounts: bool,
    #[serde(rename = "feeAccount")]
    pub fee_account: Option<String>,
    #[serde(rename = "trackingAccount")]
    pub tracking_account: Option<String>,
    #[serde(rename = "computeUnitPriceMicroLamports")]
    pub compute_unit_price_micro_lamports: Option<u64>,
    #[serde(rename = "prioritizationFeeLamports")]
    pub prioritization_fee_lamports: Option<u64>,
    #[serde(rename = "asLegacyTransaction")]
    pub as_legacy_transaction: bool,
    #[serde(rename = "useTokenLedger")]
    pub use_token_ledger: bool,
    #[serde(rename = "destinationTokenAccount")]
    pub destination_token_account: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SwapResponse {
    #[serde(rename = "swapTransaction")]
    pub swap_transaction: String,
    #[serde(rename = "lastValidBlockHeight")]
    pub last_valid_block_height: u64,
    #[serde(rename = "prioritizationFeeLamports")]
    pub prioritization_fee_lamports: Option<u64>,
    #[serde(rename = "computeUnitLimit")]
    pub compute_unit_limit: Option<u64>,
    #[serde(rename = "dynamicSlippageReport")]
    pub dynamic_slippage_report: Option<String>,
    #[serde(rename = "simulationError")]
    pub simulation_error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PriceResponseV3 {
    // V3 returns direct token mapping
}

#[derive(Debug, Deserialize)]
pub struct PriceDataV3 {
    #[serde(rename = "usdPrice")]
    pub usd_price: f64,
    #[serde(rename = "blockId")]
    pub block_id: u64,
    pub decimals: u8,
    #[serde(rename = "priceChange24h")]
    pub price_change_24h: f64,
}

pub async fn get_token_mint(config: &Config, symbol: &str) -> Result<String> {
    let symbol_upper = symbol.to_uppercase();

    match symbol_upper.as_str() {
        "SOL" => Ok(config.tokens.sol.clone()),
        "USDC" => Ok(config.tokens.usdc.clone()),
        _ => {
            // Try to parse as direct mint address
            if let Ok(_) = Pubkey::from_str(symbol) {
                Ok(symbol.to_string())
            } else {
                Err(SolanaClientError::InvalidAddress {
                    address: format!("Unknown token: {}", symbol),
                }
                .into())
            }
        }
    }
}

pub async fn prepare_swap_transaction(
    config: &Config,
    from_symbol: &str,
    to_symbol: &str,
    amount: f64,
    payer_pubkey: &Pubkey,
) -> Result<(String, crate::web::QuoteInfo, Vec<String>, String)> {
    // Get token mints
    let input_mint = get_token_mint(config, from_symbol).await?;
    let output_mint = get_token_mint(config, to_symbol).await?;

    // Convert amount to smallest unit
    let decimals = if from_symbol.to_uppercase() == "SOL" {
        9
    } else {
        6
    };
    let amount_units = (amount * 10_f64.powi(decimals)) as u64;

    info!(
        "Preparing swap: {} {} for {} (payer: {})",
        amount,
        from_symbol.to_uppercase(),
        to_symbol.to_uppercase(),
        payer_pubkey
    );

    // Get quote
    let quote = get_quote(config, &input_mint, &output_mint, amount_units).await?;

    let out_amount_f64 = quote.out_amount.parse::<u64>()? as f64
        / 10_f64.powi(if to_symbol.to_uppercase() == "SOL" {
            9
        } else {
            6
        });
    let price_impact = quote.price_impact_pct.parse::<f64>()?;

    info!(
        "Quote: {} {} -> {:.6} {}, price impact: {:.4}%",
        amount,
        from_symbol.to_uppercase(),
        out_amount_f64,
        to_symbol.to_uppercase(),
        price_impact
    );

    // Get swap transaction (unsigned)
    let swap_response = get_swap_transaction(config, quote, payer_pubkey).await?;

    // Decode versioned transaction to extract info
    let tx_bytes = base64::decode(&swap_response.swap_transaction)?;
    let versioned_tx: VersionedTransaction = bincode::deserialize(&tx_bytes)?;

    // Extract required signers from the transaction
    let required_signers = match &versioned_tx.message {
        solana_sdk::message::VersionedMessage::Legacy(legacy_msg) => legacy_msg
            .account_keys
            .iter()
            .take(legacy_msg.header.num_required_signatures as usize)
            .map(|key| key.to_string())
            .collect(),
        solana_sdk::message::VersionedMessage::V0(v0_msg) => v0_msg
            .account_keys
            .iter()
            .take(v0_msg.header.num_required_signatures as usize)
            .map(|key| key.to_string())
            .collect(),
    };

    // Get recent blockhash
    let client = solana_client::rpc_client::RpcClient::new(&config.solana.rpc_url);
    let recent_blockhash = client.get_latest_blockhash()?.to_string();

    let quote_info = crate::web::QuoteInfo {
        expected_output: out_amount_f64,
        price_impact,
        route_steps: 1, // Simplified for now
    };

    Ok((
        swap_response.swap_transaction, // Return unsigned transaction as-is
        quote_info,
        required_signers,
        recent_blockhash,
    ))
}

pub async fn get_quote(
    config: &Config,
    input_mint: &str,
    output_mint: &str,
    amount: u64,
) -> Result<QuoteResponse> {
    let client = Client::new();
    let url = format!("{}/quote", config.jupiter.api_url);

    info!(
        "Getting quote from Jupiter: {} -> {}",
        input_mint, output_mint
    );

    let response = client
        .get(&url)
        .query(&[
            ("inputMint", input_mint),
            ("outputMint", output_mint),
            ("amount", &amount.to_string()),
            ("slippageBps", &config.jupiter.slippage_bps.to_string()),
        ])
        .send()
        .await?;

    if !response.status().is_success() {
        let error_text = response.text().await?;
        return Err(SolanaClientError::NetworkError {
            source: error_text.into(),
        }
        .into());
    }

    let quote: QuoteResponse = response.json().await?;
    Ok(quote)
}

pub async fn get_swap_transaction(
    config: &Config,
    quote: QuoteResponse,
    user_pubkey: &Pubkey,
) -> Result<SwapResponse> {
    let client = Client::new();
    let url = format!("{}/swap", config.jupiter.api_url);

    let request = SwapRequest {
        quote_response: quote,
        user_public_key: user_pubkey.to_string(),
        wrap_and_unwrap_sol: true,
        use_shared_accounts: true,
        fee_account: None,
        tracking_account: None,
        compute_unit_price_micro_lamports: Some(1000),
        prioritization_fee_lamports: Some(1000),
        as_legacy_transaction: false,
        use_token_ledger: false,
        destination_token_account: None,
    };

    info!("Getting swap transaction from Jupiter");

    let response = client.post(&url).json(&request).send().await?;

    if !response.status().is_success() {
        let error_text = response.text().await?;
        return Err(SolanaClientError::NetworkError {
            source: error_text.into(),
        }
        .into());
    }

    let swap_response: SwapResponse = response.json().await?;

    if let Some(error) = &swap_response.simulation_error {
        return Err(SolanaClientError::TransactionFailed {
            reason: format!("Simulation failed: {}", error),
        }
        .into());
    }

    Ok(swap_response)
}

pub async fn swap_tokens(
    config: &Config,
    from_symbol: &str,
    to_symbol: &str,
    amount: f64,
) -> Result<()> {
    let keypair = load_keypair(config).await?;

    // Get token mints
    let input_mint = get_token_mint(config, from_symbol).await?;
    let output_mint = get_token_mint(config, to_symbol).await?;

    // Convert amount to smallest unit
    let decimals = if from_symbol.to_uppercase() == "SOL" {
        9
    } else {
        6
    }; // USDC has 6 decimals
    let amount_units = (amount * 10_f64.powi(decimals)) as u64;

    println!(
        "🔄 Swapping {} {} for {}...",
        amount,
        from_symbol.to_uppercase(),
        to_symbol.to_uppercase()
    );

    // Get quote
    let quote = get_quote(config, &input_mint, &output_mint, amount_units).await?;

    let out_amount_f64 = quote.out_amount.parse::<u64>()? as f64
        / 10_f64.powi(if to_symbol.to_uppercase() == "SOL" {
            9
        } else {
            6
        });
    let price_impact = quote.price_impact_pct.parse::<f64>()?;

    println!("📊 Quote received:");
    println!(
        "   Expected output: {:.6} {}",
        out_amount_f64,
        to_symbol.to_uppercase()
    );
    println!("   Price impact: {:.4}%", price_impact);
    println!("   Route: {} steps", quote.route_plan.len());

    // Get swap transaction
    let swap_response = get_swap_transaction(config, quote, &keypair.pubkey()).await?;

    // Decode and sign transaction
    let tx_bytes = bs58::decode(&swap_response.swap_transaction).into_vec()?;
    let mut transaction: Transaction = bincode::deserialize(&tx_bytes)?;

    // Sign transaction
    transaction.sign(&[&keypair], transaction.message.recent_blockhash);

    // Send transaction
    let client = solana_client::rpc_client::RpcClient::new(&config.solana.rpc_url);

    match client.send_and_confirm_transaction(&transaction) {
        Ok(signature) => {
            println!("✅ Swap completed successfully!");
            println!("🔗 Signature: {}", signature);
            println!(
                "💰 Swapped {} {} for ~{:.6} {}",
                amount,
                from_symbol.to_uppercase(),
                out_amount_f64,
                to_symbol.to_uppercase()
            );
        }
        Err(e) => {
            error!("Swap failed: {}", e);
            return Err(SolanaClientError::TransactionFailed {
                reason: format!("Swap failed: {}", e),
            }
            .into());
        }
    }

    Ok(())
}

pub async fn swap_tokens_with_keypair(
    config: &Config,
    from_symbol: &str,
    to_symbol: &str,
    amount: f64,
    keypair: Option<&Keypair>,
) -> Result<String> {
    let kp = match keypair {
        Some(k) => k,
        None => &crate::wallet::load_keypair(config).await?,
    };

    // Get token mints
    let input_mint = get_token_mint(config, from_symbol).await?;
    let output_mint = get_token_mint(config, to_symbol).await?;

    // Convert amount to smallest unit
    let decimals = if from_symbol.to_uppercase() == "SOL" {
        9
    } else {
        6
    }; // USDC has 6 decimals
    let amount_units = (amount * 10_f64.powi(decimals)) as u64;

    info!(
        "Swapping {} {} for {} with keypair {}",
        amount,
        from_symbol.to_uppercase(),
        to_symbol.to_uppercase(),
        kp.pubkey()
    );

    // Get quote
    let quote = get_quote(config, &input_mint, &output_mint, amount_units).await?;

    let out_amount_f64 = quote.out_amount.parse::<u64>()? as f64
        / 10_f64.powi(if to_symbol.to_uppercase() == "SOL" {
            9
        } else {
            6
        });
    let price_impact = quote.price_impact_pct.parse::<f64>()?;

    info!(
        "Quote: {} {} -> {:.6} {}, price impact: {:.4}%",
        amount,
        from_symbol.to_uppercase(),
        out_amount_f64,
        to_symbol.to_uppercase(),
        price_impact
    );

    // Get swap transaction
    let swap_response = get_swap_transaction(config, quote, &kp.pubkey()).await?;

    // Decode versioned transaction
    let tx_bytes = base64::decode(&swap_response.swap_transaction)?;
    let versioned_tx: VersionedTransaction = bincode::deserialize(&tx_bytes)?;

    // Sign versioned transaction by creating a new one with signers
    let signed_tx = VersionedTransaction::try_new(versioned_tx.message, &[&kp])?;

    // Send signed transaction
    let client = solana_client::rpc_client::RpcClient::new(&config.solana.rpc_url);

    match client.send_and_confirm_transaction(&signed_tx) {
        Ok(signature) => {
            info!("Swap completed: {}", signature);
            Ok(signature.to_string())
        }
        Err(e) => {
            error!("Swap failed: {}", e);
            Err(crate::error::SolanaClientError::TransactionFailed {
                reason: format!("Swap failed: {}", e),
            }
            .into())
        }
    }
}

pub async fn get_token_price(config: &Config, symbol: &str) -> Result<f64> {
    let client = Client::new();

    // Get token mint
    let mint = get_token_mint(config, symbol).await?;

    let url = format!("{}?ids={}", config.jupiter.price_api_url, mint);

    info!("Getting price for token: {}", symbol);

    let response = client.get(&url).send().await?;

    if !response.status().is_success() {
        return Err(SolanaClientError::NetworkError {
            source: "Failed to fetch price".into(),
        }
        .into());
    }

    // V3 API returns direct mapping: { "mint_address": { "usdPrice": 123.45, ... } }
    let price_response: std::collections::HashMap<String, PriceDataV3> = response.json().await?;

    if let Some(price_data) = price_response.get(&mint) {
        Ok(price_data.usd_price)
    } else {
        Err(SolanaClientError::InvalidAddress {
            address: format!("Price not found for: {}", symbol),
        }
        .into())
    }
}
