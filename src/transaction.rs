use crate::{config::Config, error::SolanaClientError, wallet::load_keypair};
use anyhow::Result;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    message::Message,
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    system_instruction,
    transaction::{Transaction, VersionedTransaction},
};
use std::str::FromStr;
use tracing::{error, info};

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

