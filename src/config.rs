use crate::app_log;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub solana: SolanaConfig,
    pub wallet: WalletConfig,
    pub faucet: FaucetConfig,
    pub logging: LoggingConfig,
    pub jupiter: JupiterConfig,
    pub tokens: TokensConfig,
    pub internal: InternalConfig,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct InternalConfig {
    /// Shared secret used by the gateway to authenticate requests.
    /// Override at runtime via CLI_INTERNAL_SECRET env var.
    pub secret: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SolanaConfig {
    pub network: String,
    pub rpc_url: String,
    pub commitment: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct WalletConfig {
    pub keypair_path: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FaucetConfig {
    pub airdrop_amount: f64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LoggingConfig {
    pub level: String,
    pub format: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct JupiterConfig {
    pub api_url: String,
    pub price_api_url: String,
    pub slippage_bps: u16,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TokensConfig {
    pub sol: String,
    pub usdc: String,
}

impl Config {
    pub fn load(path: &str) -> Result<Self> {
        app_log!(info, "Loading config from: {}", path);
        let content = fs::read_to_string(path)?;
        let mut config: Config = serde_yaml::from_str(&content)?;

        // Allow overriding the internal secret via environment variable
        // (so config.yaml can stay on disk with a placeholder)
        if let Ok(secret) = std::env::var("CLI_INTERNAL_SECRET") {
            if !secret.is_empty() {
                config.internal.secret = secret;
            }
        }

        if config.internal.secret == "change-me-in-production" || config.internal.secret.len() < 16 {
            app_log!(warn, "cli-solanize internal secret is weak or default — set CLI_INTERNAL_SECRET");
        }

        app_log!(info, "Config loaded successfully");
        Ok(config)
    }
}
