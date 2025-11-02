use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use tracing::info;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub solana: SolanaConfig,
    pub wallet: WalletConfig,
    pub faucet: FaucetConfig,
    pub logging: LoggingConfig,
    pub jupiter: JupiterConfig,
    pub tokens: TokensConfig,
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
        let config: Config = serde_yaml::from_str(&content)?;
        app_log!(info, "Config loaded successfully");
        Ok(config)
    }
}
