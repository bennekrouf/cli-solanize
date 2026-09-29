use crate::app_log;
use crate::{config::Config, jupiter};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Token metadata from Jupiter's token API (mainnet only). Search and mint
// lookup are the same endpoint: a query that is a comma-separated list of
// mints returns exactly those tokens, with decimals and a USD price.

const MAX_MINTS_PER_LOOKUP: usize = 100;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TokenInfo {
    #[serde(rename = "id")]
    pub address: String,
    pub symbol: String,
    pub name: String,
    pub decimals: u8,
    #[serde(rename = "icon", default)]
    pub logo_uri: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(rename = "isVerified", default)]
    pub verified: bool,
    #[serde(rename = "usdPrice", default)]
    pub usd_price: Option<f64>,
}

/// Up to 20 tokens matching a symbol, name or mint, as Jupiter ranks them.
pub async fn search_tokens(config: &Config, query: &str) -> Result<Vec<TokenInfo>> {
    app_log!(info, "Searching tokens for: {}", query);
    let tokens: Vec<TokenInfo> = jupiter::get(config, "/tokens/v2/search", &[("query", query.trim())])
        .await?
        .json()
        .await?;
    app_log!(info, "Found {} tokens matching '{}'", tokens.len(), query);
    Ok(tokens)
}

pub async fn get_token_info(config: &Config, query: &str) -> Result<Option<TokenInfo>> {
    let tokens = search_tokens(config, query).await?;
    let q = query.to_lowercase();

    // Exact mint, then exact symbol (verified first), then Jupiter's top hit
    let pick = tokens
        .iter()
        .position(|t| t.address.to_lowercase() == q)
        .or_else(|| tokens.iter().position(|t| t.symbol.to_lowercase() == q && t.verified))
        .or_else(|| tokens.iter().position(|t| t.symbol.to_lowercase() == q))
        .unwrap_or(0);
    Ok(tokens.into_iter().nth(pick))
}

/// Metadata for many mints in as few calls as possible, keyed by mint.
/// Mints Jupiter does not know are simply absent from the map.
pub async fn lookup_mints(config: &Config, mints: &[String]) -> Result<HashMap<String, TokenInfo>> {
    let mut found = HashMap::new();
    for chunk in mints.chunks(MAX_MINTS_PER_LOOKUP) {
        let query = chunk.join(",");
        let tokens: Vec<TokenInfo> = jupiter::get(config, "/tokens/v2/search", &[("query", query.as_str())])
            .await?
            .json()
            .await?;
        found.extend(tokens.into_iter().map(|t| (t.address.clone(), t)));
    }
    Ok(found)
}
