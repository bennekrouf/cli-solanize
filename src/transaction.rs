use crate::{config::Config, error::SolanaClientError, wallet::load_keypair};
use anyhow::Result;
use solana_client::rpc_client::GetConfirmedSignaturesForAddress2Config;
use solana_sdk::{
    message::Message,
    // pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    system_instruction,
    transaction::{Transaction, VersionedTransaction},
};
use std::str::FromStr;
use tracing::{error, info};
// use solana_client::rpc_config::RpcSignaturesForAddressConfig;
// use solana_client::rpc_client::GetConfirmedSignaturesForAddress2Config;
use serde::{Deserialize, Serialize};
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use solana_transaction_status::{UiTransactionEncoding, EncodedConfirmedTransactionWithStatusMeta};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TransactionHistory {
    pub signature: String,
    pub status: TransactionStatus,
    pub confirmation_status: ConfirmationStatus,
    pub block_time: Option<i64>,
    pub slot: Option<u64>,
    pub fee: Option<f64>,
    pub amount: Option<f64>,
    pub token_symbol: Option<String>,
    pub transaction_type: TransactionType,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum TransactionStatus {
    Success,
    Failed,
    Pending,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ConfirmationStatus {
    Processed,
    Confirmed,
    Finalized,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum TransactionType {
    Transfer,
    TokenTransfer,
    Swap,
    Unknown,
}

pub async fn create_transaction(config: &Config, to_address: &str, amount: f64) -> Result<String> {
    let from_keypair = load_keypair(config).await?;
    let client = RpcClient::new(&config.solana.rpc_url);

    // Parse recipient address
    let to_pubkey =
        Pubkey::from_str(to_address).map_err(|_| SolanaClientError::InvalidAddress {
            address: to_address.to_string(),
        })?;

    // Check balance
    let current_balance = crate::wallet::get_balance(config).await?;
    if current_balance < amount {
        return Err(SolanaClientError::InsufficientBalance {
            current: current_balance,
            required: amount,
        }
        .into());
    }

    let lamports = (amount * solana_sdk::native_token::LAMPORTS_PER_SOL as f64) as u64;

    info!("Creating transaction: {} SOL to {}", amount, to_address);

    // Create transfer instruction
    let instruction = system_instruction::transfer(&from_keypair.pubkey(), &to_pubkey, lamports);

    // Get recent blockhash
    let recent_blockhash = client.get_latest_blockhash()?;

    // Create transaction
    let message = Message::new(&[instruction], Some(&from_keypair.pubkey()));
    let transaction = Transaction::new(&[&from_keypair], message, recent_blockhash);

    // Serialize transaction for later use
    let serialized_tx = bincode::serialize(&transaction)?;
    let tx_string = bs58::encode(serialized_tx).into_string();

    println!("✅ Transaction created successfully!");
    println!("📦 Transaction data: {}", tx_string);
    println!("💸 Amount: {} SOL", amount);
    println!("📍 To: {}", to_address);

    Ok(tx_string)
}

pub async fn prepare_sol_transfer(
    config: &Config,
    payer_pubkey: &Pubkey,
    to_address: &str,
    amount: f64,
) -> Result<(String, Vec<String>, String)> {
    let client = RpcClient::new(&config.solana.rpc_url);

    // Parse recipient address
    let to_pubkey =
        Pubkey::from_str(to_address).map_err(|_| SolanaClientError::InvalidAddress {
            address: to_address.to_string(),
        })?;

    // Check balance
    let current_balance = crate::wallet::get_balance_for_pubkey(config, payer_pubkey).await?;
    if current_balance < amount {
        return Err(SolanaClientError::InsufficientBalance {
            current: current_balance,
            required: amount,
        }
        .into());
    }

    let lamports = (amount * solana_sdk::native_token::LAMPORTS_PER_SOL as f64) as u64;

    info!(
        "Preparing SOL transfer: {} SOL from {} to {}",
        amount, payer_pubkey, to_address
    );

    // Create transfer instruction
    let instruction = system_instruction::transfer(payer_pubkey, &to_pubkey, lamports);

    // Get recent blockhash
    let recent_blockhash = client.get_latest_blockhash()?;

    // Create unsigned transaction message
    let message = Message::new(&[instruction], Some(payer_pubkey));

    // Create unsigned transaction (with empty signatures)
    let mut transaction = Transaction::new_unsigned(message);
    transaction.message.recent_blockhash = recent_blockhash;

    // Serialize unsigned transaction
    let serialized_tx = bincode::serialize(&transaction)?;
    let unsigned_tx_b64 = base64::encode(serialized_tx);

    // Required signers (just the payer)
    let required_signers = vec![payer_pubkey.to_string()];

    info!("Unsigned transaction prepared");

    Ok((
        unsigned_tx_b64,
        required_signers,
        recent_blockhash.to_string(),
    ))
}

pub async fn submit_signed_transaction(
    config: &Config,
    signed_transaction_b64: &str,
) -> Result<String> {
    let client = RpcClient::new(&config.solana.rpc_url);

    info!("Submitting signed transaction");

    // Decode the signed transaction
    let tx_bytes = base64::decode(signed_transaction_b64)?;

    // Try to deserialize as both legacy and versioned transaction
    let signature = if let Ok(transaction) = bincode::deserialize::<Transaction>(&tx_bytes) {
        // Legacy transaction
        client.send_and_confirm_transaction(&transaction)?
    } else if let Ok(versioned_tx) = bincode::deserialize::<VersionedTransaction>(&tx_bytes) {
        // Versioned transaction
        client.send_and_confirm_transaction(&versioned_tx)?
    } else {
        return Err(SolanaClientError::TransactionFailed {
            reason: "Invalid transaction format".to_string(),
        }
        .into());
    };

    info!("Transaction submitted: {}", signature);
    Ok(signature.to_string())
}

pub async fn send_transaction(config: &Config, tx_data: &str) -> Result<()> {
    let client = RpcClient::new(&config.solana.rpc_url);

    info!("Sending transaction");

    // Deserialize transaction
    let tx_bytes = bs58::decode(tx_data).into_vec()?;
    let transaction: Transaction = bincode::deserialize(&tx_bytes)?;

    // Send transaction
    match client.send_and_confirm_transaction(&transaction) {
        Ok(signature) => {
            println!("✅ Transaction sent successfully!");
            println!("🔗 Signature: {}", signature);

            // Update balance
            let new_balance = crate::wallet::get_balance(config).await?;
            println!("💰 New balance: {} SOL", new_balance);
        }
        Err(e) => {
            error!("Transaction failed: {}", e);
            return Err(SolanaClientError::TransactionFailed {
                reason: format!("Send failed: {}", e),
            }
            .into());
        }
    }

    Ok(())
}

pub async fn create_transaction_with_keypair(
    config: &Config,
    to_address: &str,
    amount: f64,
    keypair: Option<&Keypair>,
) -> Result<String> {
    let from_keypair = match keypair {
        Some(k) => k,
        None => &crate::wallet::load_keypair(config).await?,
    };

    let client = RpcClient::new(&config.solana.rpc_url);

    // Parse recipient address
    let to_pubkey =
        Pubkey::from_str(to_address).map_err(|_| SolanaClientError::InvalidAddress {
            address: to_address.to_string(),
        })?;

    // Check balance
    let current_balance =
        crate::wallet::get_balance_for_pubkey(config, &from_keypair.pubkey()).await?;
    if current_balance < amount {
        return Err(SolanaClientError::InsufficientBalance {
            current: current_balance,
            required: amount,
        }
        .into());
    }

    let lamports = (amount * solana_sdk::native_token::LAMPORTS_PER_SOL as f64) as u64;

    info!(
        "Creating transaction: {} SOL from {} to {}",
        amount,
        from_keypair.pubkey(),
        to_address
    );

    // Create transfer instruction
    let instruction = system_instruction::transfer(&from_keypair.pubkey(), &to_pubkey, lamports);

    // Get recent blockhash
    let recent_blockhash = client.get_latest_blockhash()?;

    // Create transaction
    let message = Message::new(&[instruction], Some(&from_keypair.pubkey()));
    let transaction = Transaction::new(&[&from_keypair], message, recent_blockhash);

    // Serialize transaction for later use
    let serialized_tx = bincode::serialize(&transaction)?;
    let tx_string = bs58::encode(serialized_tx).into_string();

    info!("Transaction created: {}", tx_string);

    Ok(tx_string)
}


/// Core function to get transaction history without web dependencies
pub async fn fetch_transaction_history(
    config: &Config,
    pubkey: &Pubkey,
    limit: Option<usize>,
    before: Option<String>,
) -> Result<Vec<TransactionHistory>> {
    let client = RpcClient::new(&config.solana.rpc_url);
    let limit = limit.unwrap_or(50).min(1000); // Cap at 1000

    info!("Fetching transaction history for {}, limit: {}", pubkey, limit);

    // Get transaction signatures
    // let mut config_params = solana_client::rpc_config::RpcTransactionLogsConfigWrapper {
    //     filter: solana_client::rpc_config::RpcTransactionLogsFilter::All,
    // };

    let signatures = client.get_signatures_for_address_with_config(
        pubkey,
        GetConfirmedSignaturesForAddress2Config {
            before: before.as_deref().map(|s| s.parse().ok()).flatten(),
            until: None,
            limit: Some(limit),
            commitment: Some(solana_sdk::commitment_config::CommitmentConfig::confirmed()),
        },
    )?;

    let mut transactions = Vec::new();

    for sig_info in signatures {
        let signature = sig_info.signature;
        
        // Determine status from signature info
        let status = match sig_info.err {
            None => TransactionStatus::Success,
            Some(_) => TransactionStatus::Failed,
        };

        let confirmation_status = match sig_info.confirmation_status {
            Some(status) => match status {
                solana_transaction_status::TransactionConfirmationStatus::Processed => ConfirmationStatus::Processed,
                solana_transaction_status::TransactionConfirmationStatus::Confirmed => ConfirmationStatus::Confirmed,
                solana_transaction_status::TransactionConfirmationStatus::Finalized => ConfirmationStatus::Finalized,
            },
            None => ConfirmationStatus::Finalized, // Default for older transactions
        };

        // Try to get transaction details for amount/type analysis
        let (amount, token_symbol, tx_type) = match client.get_transaction(
            &signature.parse()?,
            UiTransactionEncoding::JsonParsed,
        ) {
            Ok(tx) => analyze_transaction_details(&tx),
            Err(_) => (None, None, TransactionType::Unknown),
        };

        let fee = None; //sig_info.fee.map(|f| f as f64 / solana_sdk::native_token::LAMPORTS_PER_SOL as f64);

        transactions.push(TransactionHistory {
            signature,
            status,
            confirmation_status,
            block_time: sig_info.block_time,
            slot: Some(sig_info.slot),
            fee,
            amount,
            token_symbol,
            transaction_type: tx_type,
            error: sig_info.err.map(|e| format!("{:?}", e)),
        });
    }

    info!("Found {} transactions", transactions.len());
    Ok(transactions)
}

/// Core function to get pending transactions without web dependencies
pub async fn fetch_pending_transactions(
    config: &Config,
    pubkey: &Pubkey,
) -> Result<Vec<TransactionHistory>> {
    let client = RpcClient::new(&config.solana.rpc_url);

    info!("Fetching pending transactions for {}", pubkey);

    // Get recent signatures with processed commitment to catch pending ones
    let signatures = client.get_signatures_for_address_with_config(
        pubkey,
        GetConfirmedSignaturesForAddress2Config {
            before: None,
            until: None,
            limit: Some(20),
            commitment: Some(solana_sdk::commitment_config::CommitmentConfig::confirmed()),
        },
    )?;

    let mut pending_transactions = Vec::new();

    for sig_info in signatures {
        // Only include transactions that are processed but not confirmed/finalized
        if let Some(status) = &sig_info.confirmation_status {
            if matches!(status, solana_transaction_status::TransactionConfirmationStatus::Processed) {
                let signature = sig_info.signature;
                
                let (amount, token_symbol, tx_type) = match client.get_transaction(
                    &signature.parse()?,
                    UiTransactionEncoding::JsonParsed,
                ) {
                    Ok(tx) => analyze_transaction_details(&tx),
                    Err(_) => (None, None, TransactionType::Unknown),
                };

                let fee = None; // sig_info.fee.map(|f| f as f64 / solana_sdk::native_token::LAMPORTS_PER_SOL as f64);

                pending_transactions.push(TransactionHistory {
                    signature,
                    status: TransactionStatus::Pending,
                    confirmation_status: ConfirmationStatus::Processed,
                    block_time: sig_info.block_time,
                    slot: Some(sig_info.slot),
                    fee,
                    amount,
                    token_symbol,
                    transaction_type: tx_type,
                    error: None,
                });
            }
        }
    }

    info!("Found {} pending transactions", pending_transactions.len());
    Ok(pending_transactions)
}

fn analyze_transaction_details(
    tx: &EncodedConfirmedTransactionWithStatusMeta,
) -> (Option<f64>, Option<String>, TransactionType) {
    // Basic transaction analysis - can be expanded
    let mut amount = None;
    let mut token_symbol = None;
    let mut tx_type = TransactionType::Unknown;

    // Try to extract SOL transfer amount from pre/post balances
    if let Some(meta) = &tx.transaction.meta {
        if let (pre_balances, post_balances) = (&meta.pre_balances, &meta.post_balances) {
            // Look for significant balance changes (excluding fees)
            for (i, (pre, post)) in pre_balances.iter().zip(post_balances.iter()).enumerate() {
                let diff = (*post as i64) - (*pre as i64);
                if diff.abs() > 1000000 { // More than 0.001 SOL
                    amount = Some(diff.abs() as f64 / solana_sdk::native_token::LAMPORTS_PER_SOL as f64);
                    token_symbol = Some("SOL".to_string());
                    tx_type = TransactionType::Transfer;
                    break;
                }
            }
        }
    }

    // TODO: Add more sophisticated analysis for:
    // - Token transfers (SPL)
    // - Jupiter swaps
    // - Other program interactions

    (amount, token_symbol, tx_type)
}
