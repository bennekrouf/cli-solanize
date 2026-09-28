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
    #[serde(default)]
    pub api0: Api0Config,
}

/// Lets the api0 gateway call us from anywhere, the way it calls cvenom: it
/// mints a Google OIDC identity token for `oidc_audience` with its service
/// account, and we accept it only when it was minted by `oidc_service_account`.
/// Pinning the account matters — any Google service account can mint a token
/// for any audience. Both unset → the OIDC path is off and only the internal
/// secret is accepted.
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct Api0Config {
    /// Override with SOLANIZE_OIDC_AUDIENCE, e.g. "https://api.ribh.io".
    pub oidc_audience: Option<String>,
    /// Override with SOLANIZE_OIDC_SERVICE_ACCOUNT, the api0 tenant's SA email.
    pub oidc_service_account: Option<String>,
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

        if let Ok(aud) = std::env::var("SOLANIZE_OIDC_AUDIENCE") {
            if !aud.is_empty() {
                config.api0.oidc_audience = Some(aud);
            }
        }
        if let Ok(sa) = std::env::var("SOLANIZE_OIDC_SERVICE_ACCOUNT") {
            if !sa.is_empty() {
                config.api0.oidc_service_account = Some(sa);
            }
        }

        match (&config.api0.oidc_audience, &config.api0.oidc_service_account) {
            (Some(aud), Some(sa)) => app_log!(info, "api0 OIDC auth enabled — audience: {}, service account: {}", aud, sa),
            (None, None) => {}
            _ => app_log!(warn, "api0 OIDC auth needs both SOLANIZE_OIDC_AUDIENCE and SOLANIZE_OIDC_SERVICE_ACCOUNT — disabled"),
        }

        if config.internal.secret == "change-me-in-production" || config.internal.secret.len() < 16 {
            app_log!(warn, "cli-solanize internal secret is weak or default — set CLI_INTERNAL_SECRET");
        }

        app_log!(info, "Config loaded successfully");
        Ok(config)
    }
}
