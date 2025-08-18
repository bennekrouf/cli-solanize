use crate::{config::Config, error::SolanaClientError};
use anyhow::Result;
use solana_client::rpc_client::RpcClient;
use solana_sdk::signature::{Keypair, Signer};
use std::fs;
use tracing::{error, info};

pub async fn generate_wallet(config: &Config) -> Result<()> {
    info!("Generating new wallet");

    let keypair = Keypair::new();
    let pubkey = keypair.pubkey();

    // Save keypair to file
    let keypair_bytes = keypair.to_bytes();
    let keypair_json = serde_json::to_string(&keypair_bytes.to_vec())?;

    fs::write(&config.wallet.keypair_path, keypair_json)?;

    println!("✅ Wallet generated successfully!");
    println!("📍 Public Key: {}", pubkey);
    println!("💾 Saved to: {}", config.wallet.keypair_path);

    Ok(())
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

    Ok(Keypair::from_bytes(&bytes)?)
}

pub async fn get_balance(config: &Config) -> Result<f64> {
    let keypair = load_keypair(config).await?;
    let client = RpcClient::new(&config.solana.rpc_url);

    let balance = client.get_balance(&keypair.pubkey())?;
    let sol_balance = balance as f64 / solana_sdk::native_token::LAMPORTS_PER_SOL as f64;

    info!("Current balance: {} SOL", sol_balance);
    Ok(sol_balance)
}

pub async fn request_airdrop(config: &Config, amount: f64) -> Result<()> {
    let keypair = load_keypair(config).await?;
    let client = RpcClient::new(&config.solana.rpc_url);

    let lamports = (amount * solana_sdk::native_token::LAMPORTS_PER_SOL as f64) as u64;

    info!("Requesting airdrop of {} SOL", amount);

    match client.request_airdrop(&keypair.pubkey(), lamports) {
        Ok(signature) => {
            println!("✅ Airdrop requested successfully!");
            println!("🔗 Signature: {}", signature);
            println!("⏳ Waiting for confirmation...");

            // Wait for confirmation
            client.confirm_transaction(&signature)?;

            let new_balance = get_balance(config).await?;
            println!("💰 New balance: {} SOL", new_balance);
        }
        Err(e) => {
            error!("Airdrop failed: {}", e);
            return Err(SolanaClientError::TransactionFailed {
                reason: format!("Airdrop failed: {}", e),
            }
            .into());
        }
    }

    Ok(())
}
