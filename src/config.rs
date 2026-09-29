use crate::app_log;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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
    /// The active network. At load time this is the default one; a web request
    /// can pick another through `Config::for_network`.
    pub network: String,
    /// The active network's RPC. Filled from `networks` when that is set.
    #[serde(default)]
    pub rpc_url: String,
    pub commitment: String,
    /// Every network a request may choose, by name. Empty → only `network`.
    #[serde(default)]
    pub networks: BTreeMap<String, NetworkProfile>,
}

/// What changes from one Solana cluster to another.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct NetworkProfile {
    /// Override with SOLANIZE_RPC_URL_<NAME> (e.g. SOLANIZE_RPC_URL_MAINNET),
    /// so a paid RPC key stays out of config.yaml.
    pub rpc_url: String,
    /// USDC mint on this cluster; devnet's differs from mainnet's.
    pub usdc: String,
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
    /// Keyless tier, used when no API key is set.
    pub base_url: String,
    /// Keyed tier; limits are per key. Used when `api_key` is set.
    pub keyed_base_url: String,
    /// Set with JUPITER_API_KEY, never in config.yaml. One key for the whole
    /// backend — it is solanize's quota, not a user's (see README).
    #[serde(default, skip_serializing)]
    pub api_key: Option<String>,
    pub slippage_bps: u16,
}

impl JupiterConfig {
    pub fn base(&self) -> &str {
        if self.api_key.is_some() { &self.keyed_base_url } else { &self.base_url }
    }
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

        for (name, profile) in config.solana.networks.iter_mut() {
            let var = format!("SOLANIZE_RPC_URL_{}", name.to_uppercase().replace('-', "_"));
            if let Ok(url) = std::env::var(&var) {
                if !url.is_empty() {
                    profile.rpc_url = url;
                }
            }
        }
        if !config.solana.networks.is_empty() {
            let default = config.solana.network.clone();
            config = config
                .for_network(Some(&default))
                .map_err(|e| anyhow::anyhow!("solana.network: {}", e))?;
        }
        if config.solana.rpc_url.is_empty() {
            anyhow::bail!("solana.rpc_url is empty and solana.networks has no '{}'", config.solana.network);
        }
        app_log!(info, "Solana networks: {} (default: {})", config.network_names().join(", "), config.solana.network);

        if let Ok(key) = std::env::var("JUPITER_API_KEY") {
            if !key.trim().is_empty() {
                config.jupiter.api_key = Some(key.trim().to_string());
            }
        }
        app_log!(
            info,
            "Jupiter: {} ({})",
            config.jupiter.base(),
            if config.jupiter.api_key.is_some() { "API key" } else { "keyless — set JUPITER_API_KEY for higher limits" }
        );

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

impl Config {
    /// A copy of this config pointed at `network`, or at the default when
    /// `None`. Everything downstream reads `solana.rpc_url` and `tokens`, so
    /// swapping them here is all it takes to serve another cluster.
    pub fn for_network(&self, network: Option<&str>) -> std::result::Result<Config, String> {
        let name = match network.map(str::trim) {
            None | Some("") => return Ok(self.clone()),
            Some(n) => n.to_lowercase(),
        };

        if self.solana.networks.is_empty() {
            return if name == self.solana.network {
                Ok(self.clone())
            } else {
                Err(format!("unknown network '{}'; available: {}", name, self.solana.network))
            };
        }

        let profile = self.solana.networks.get(&name).ok_or_else(|| {
            format!("unknown network '{}'; available: {}", name, self.network_names().join(", "))
        })?;

        let mut config = self.clone();
        config.solana.network = name;
        config.solana.rpc_url = profile.rpc_url.clone();
        config.tokens.usdc = profile.usdc.clone();
        Ok(config)
    }

    pub fn network_names(&self) -> Vec<String> {
        if self.solana.networks.is_empty() {
            vec![self.solana.network.clone()]
        } else {
            self.solana.networks.keys().cloned().collect()
        }
    }

    /// Jupiter (swaps, quotes, prices, token list) only exists on mainnet.
    pub fn has_jupiter(&self) -> bool {
        self.solana.network == "mainnet"
    }
}
