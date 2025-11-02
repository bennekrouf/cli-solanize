use crate::app_log;
use crate::{config::Config, error::SolanaClientError};
use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TokenInfo {
    pub address: String,
    pub symbol: String,
    pub name: String,
    pub decimals: u8,
    #[serde(rename = "logoURI")]
    pub logo_uri: Option<String>,
    pub tags: Vec<String>,
    pub daily_volume: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct TokenListResponse {
    pub name: String,
    #[serde(rename = "logoURI")]
    pub logo_uri: String,
    pub keywords: Vec<String>,
    pub tags: std::collections::HashMap<String, serde_json::Value>,
    pub timestamp: String,
    pub tokens: Vec<TokenInfo>,
    pub version: Version,
}

#[derive(Debug, Deserialize)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

pub async fn get_all_tokens(_config: &Config) -> Result<Vec<TokenInfo>> {
    let client = Client::new();
    let url = "https://token.jup.ag/all";

    app_log!(info, "Fetching all tokens from Jupiter");

    let response = client.get(url).send().await?;

    if !response.status().is_success() {
        return Err(SolanaClientError::NetworkError {
            source: "Failed to fetch token list".into(),
        }
        .into());
    }

    let tokens: Vec<TokenInfo> = response.json().await?;
    Ok(tokens)
}

pub async fn search_tokens(config: &Config, query: &str) -> Result<Vec<TokenInfo>> {
    let all_tokens = get_all_tokens(config).await?;
    let query_lower = query.to_lowercase();

    app_log!(info, "Searching tokens for: {}", query);

    let mut matches: Vec<TokenInfo> = all_tokens
        .into_iter()
        .filter(|token| {
            // Match by symbol, name, or address
            token.symbol.to_lowercase().contains(&query_lower)
                || token.name.to_lowercase().contains(&query_lower)
                || token.address.to_lowercase().contains(&query_lower)
        })
        .collect();

    // Sort by relevance (exact symbol matches first, then symbol starts with, then name matches)
    matches.sort_by(|a, b| {
        let a_symbol_exact = a.symbol.to_lowercase() == query_lower;
        let b_symbol_exact = b.symbol.to_lowercase() == query_lower;

        if a_symbol_exact && !b_symbol_exact {
            return std::cmp::Ordering::Less;
        }
        if !a_symbol_exact && b_symbol_exact {
            return std::cmp::Ordering::Greater;
        }

        let a_symbol_starts = a.symbol.to_lowercase().starts_with(&query_lower);
        let b_symbol_starts = b.symbol.to_lowercase().starts_with(&query_lower);

        if a_symbol_starts && !b_symbol_starts {
            return std::cmp::Ordering::Less;
        }
        if !a_symbol_starts && b_symbol_starts {
            return std::cmp::Ordering::Greater;
        }

        // Finally sort by symbol alphabetically
        a.symbol.cmp(&b.symbol)
    });

    // Limit results to top 20
    matches.truncate(20);

    app_log!(
        info,
        "🔍 Found {} tokens matching '{}':",
        matches.len(),
        query
    );

    Ok(matches)
}

pub async fn get_token_info(config: &Config, query: &str) -> Result<Option<TokenInfo>> {
    let tokens = search_tokens(config, query).await?;

    // Return exact symbol match if found, otherwise first result
    for token in &tokens {
        if token.symbol.to_lowercase() == query.to_lowercase() {
            return Ok(Some(token.clone()));
        }
    }

    // If no exact symbol match, check for exact address match
    for token in &tokens {
        if token.address.to_lowercase() == query.to_lowercase() {
            return Ok(Some(token.clone()));
        }
    }

    // Return first result if any
    Ok(tokens.into_iter().next())
}

pub async fn get_popular_tokens(config: &Config) -> Result<Vec<TokenInfo>> {
    let all_tokens = get_all_tokens(config).await?;

    // Define popular token symbols
    let popular_symbols = vec![
        "SOL", "USDC", "USDT", "BTC", "ETH", "RAY", "SRM", "FTT", "STEP", "ORCA", "SAMO", "GRAPE",
        "COPE", "FIDA", "KIN", "MAPS", "MEDIA", "ROPE", "SBR", "SLRS",
    ];

    let mut popular_tokens: Vec<TokenInfo> = all_tokens
        .into_iter()
        .filter(|token| popular_symbols.contains(&token.symbol.as_str()))
        .collect();

    // Sort by the order in popular_symbols
    popular_tokens.sort_by(|a, b| {
        let a_pos = popular_symbols
            .iter()
            .position(|&s| s == a.symbol)
            .unwrap_or(usize::MAX);
        let b_pos = popular_symbols
            .iter()
            .position(|&s| s == b.symbol)
            .unwrap_or(usize::MAX);
        a_pos.cmp(&b_pos)
    });

    Ok(popular_tokens)
}
