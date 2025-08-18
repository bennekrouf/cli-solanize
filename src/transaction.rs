use crate::{config::Config, error::SolanaClientError, wallet::load_keypair};
use anyhow::Result;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    message::Message, pubkey::Pubkey, signer::Signer, system_instruction, transaction::Transaction,
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
