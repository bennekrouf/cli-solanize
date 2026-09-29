use crate::app_log;
use crate::{config::Config, error::SolanaClientError, token, wallet::load_keypair};
use anyhow::{Result, anyhow, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use reqwest::{Client, Method, Response, StatusCode};
use serde_json::{Value, json};
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signature::Signer, transaction::VersionedTransaction};
use std::str::FromStr;
use std::sync::LazyLock;
use std::time::Duration;

// ── Jupiter HTTP ──────────────────────────────────────────────────────────────
// Every Jupiter call goes through here, so the tier (keyed or keyless), the
// key header and the error wording live in one place.

static CLIENT: LazyLock<Client> = LazyLock::new(Client::new);

pub async fn get(config: &Config, path: &str, query: &[(&str, &str)]) -> Result<Response> {
    send(config, Method::GET, path, query, None).await
}

async fn post(config: &Config, path: &str, body: &Value) -> Result<Response> {
    send(config, Method::POST, path, &[], Some(body)).await
}

async fn send(
    config: &Config,
    method: Method,
    path: &str,
    query: &[(&str, &str)],
    body: Option<&Value>,
) -> Result<Response> {
    if !config.has_jupiter() {
        bail!("Jupiter only runs on mainnet — not available on {}", config.solana.network);
    }

    let url = format!("{}{}", config.jupiter.base(), path);
    let mut request = CLIENT
        .request(method, &url)
        .query(query)
        .timeout(Duration::from_secs(15));
    if let Some(key) = &config.jupiter.api_key {
        request = request.header("x-api-key", key);
    }
    if let Some(body) = body {
        request = request.json(body);
    }

    let response = request.send().await?;
    match response.status() {
        s if s.is_success() => Ok(response),
        StatusCode::TOO_MANY_REQUESTS => Err(anyhow!(
            "Jupiter rate limit reached{}",
            if config.jupiter.api_key.is_some() {
                " for this API key"
            } else {
                " — the keyless tier is shared; set JUPITER_API_KEY"
            }
        )),
        s => {
            let text: String = response.text().await.unwrap_or_default().chars().take(300).collect();
            Err(SolanaClientError::NetworkError {
                source: format!("Jupiter {} on {}: {}", s, path, text).into(),
            }
            .into())
        }
    }
}

// ── Swap ──────────────────────────────────────────────────────────────────────

/// Swaps accept SOL, USDC or a mint address — never a looked-up symbol, since
/// symbols are not unique and a wrong guess here moves real money.
pub fn get_token_mint(config: &Config, symbol: &str) -> Result<String> {
    match symbol.trim().to_uppercase().as_str() {
        "SOL" => Ok(config.tokens.sol.clone()),
        "USDC" => Ok(config.tokens.usdc.clone()),
        _ if Pubkey::from_str(symbol.trim()).is_ok() => Ok(symbol.trim().to_string()),
        _ => Err(SolanaClientError::InvalidAddress {
            address: format!(
                "Unknown token '{}': use SOL, USDC or a mint address (find one with tokens/search)",
                symbol
            ),
        }
        .into()),
    }
}

/// An unsigned swap transaction built by Jupiter, plus what the caller shows.
struct BuiltSwap {
    transaction: VersionedTransaction,
    unsigned_base64: String,
    expected_output: f64,
    price_impact: f64,
    route_steps: usize,
}

async fn build_swap(
    config: &Config,
    from_symbol: &str,
    to_symbol: &str,
    amount: f64,
    payer: &Pubkey,
) -> Result<BuiltSwap> {
    let input_mint = get_token_mint(config, from_symbol)?;
    let output_mint = get_token_mint(config, to_symbol)?;
    if input_mint == output_mint {
        bail!("Cannot swap a token for itself");
    }

    // Real decimals for both sides — one lookup, not a guess
    let tokens = token::lookup_mints(config, &[input_mint.clone(), output_mint.clone()]).await?;
    let decimals = |mint: &str, label: &str| {
        tokens
            .get(mint)
            .map(|t| t.decimals as i32)
            .ok_or_else(|| anyhow!("Jupiter does not know token '{}'", label))
    };
    let in_decimals = decimals(&input_mint, from_symbol)?;
    let out_decimals = decimals(&output_mint, to_symbol)?;

    let amount_units = (amount * 10_f64.powi(in_decimals)).round() as u64;
    if amount_units == 0 {
        bail!("Amount {} {} is below the token's smallest unit", amount, from_symbol);
    }

    app_log!(info, "Preparing swap: {} {} -> {} (payer: {})", amount, from_symbol, to_symbol, payer);

    // Kept as raw JSON: /swap wants the quote back exactly as /quote sent it
    let quote: Value = get(
        config,
        "/swap/v1/quote",
        &[
            ("inputMint", input_mint.as_str()),
            ("outputMint", output_mint.as_str()),
            ("amount", &amount_units.to_string()),
            ("slippageBps", &config.jupiter.slippage_bps.to_string()),
        ],
    )
    .await?
    .json()
    .await?;

    let out_units: u64 = quote["outAmount"]
        .as_str()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| anyhow!("Jupiter quote has no outAmount"))?;
    let expected_output = out_units as f64 / 10_f64.powi(out_decimals);
    let price_impact = quote["priceImpactPct"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let route_steps = quote["routePlan"].as_array().map_or(0, |r| r.len());

    app_log!(
        info,
        "Quote: {} {} -> {:.6} {}, price impact: {}, {} route steps",
        amount, from_symbol, expected_output, to_symbol, price_impact, route_steps
    );

    let swap: Value = post(
        config,
        "/swap/v1/swap",
        &json!({
            "quoteResponse": quote,
            "userPublicKey": payer.to_string(),
            "wrapAndUnwrapSol": true,
            "dynamicComputeUnitLimit": true,
        }),
    )
    .await?
    .json()
    .await?;

    if let Some(error) = swap.get("simulationError").filter(|e| !e.is_null()) {
        return Err(SolanaClientError::TransactionFailed {
            reason: format!("Simulation failed: {}", error),
        }
        .into());
    }

    let unsigned_base64 = swap["swapTransaction"]
        .as_str()
        .ok_or_else(|| anyhow!("Jupiter swap response has no swapTransaction"))?
        .to_string();
    let transaction: VersionedTransaction = bincode::deserialize(&BASE64.decode(&unsigned_base64)?)?;

    Ok(BuiltSwap { transaction, unsigned_base64, expected_output, price_impact, route_steps })
}

/// Unsigned swap for a wallet to sign: (transaction, quote, signers, blockhash).
pub async fn prepare_swap_transaction(
    config: &Config,
    from_symbol: &str,
    to_symbol: &str,
    amount: f64,
    payer_pubkey: &Pubkey,
) -> Result<(String, crate::web::QuoteInfo, Vec<String>, String)> {
    let built = build_swap(config, from_symbol, to_symbol, amount, payer_pubkey).await?;

    let message = &built.transaction.message;
    let required_signers = message
        .static_account_keys()
        .iter()
        .take(message.header().num_required_signatures as usize)
        .map(|key| key.to_string())
        .collect();
    // The transaction's own blockhash — it expires with this one, not a fresher one
    let recent_blockhash = message.recent_blockhash().to_string();

    let quote_info = crate::web::QuoteInfo {
        expected_output: built.expected_output,
        price_impact: built.price_impact,
        route_steps: built.route_steps,
    };

    Ok((built.unsigned_base64, quote_info, required_signers, recent_blockhash))
}

/// Terminal CLI: swap with the local wallet file and send it.
pub async fn swap_tokens(config: &Config, from_symbol: &str, to_symbol: &str, amount: f64) -> Result<()> {
    let keypair = load_keypair(config).await?;
    let signature = swap_tokens_with_keypair(config, from_symbol, to_symbol, amount, Some(&keypair)).await?;
    app_log!(info, "✅ Swap completed: {}", signature);
    Ok(())
}

pub async fn swap_tokens_with_keypair(
    config: &Config,
    from_symbol: &str,
    to_symbol: &str,
    amount: f64,
    keypair: Option<&Keypair>,
) -> Result<String> {
    let loaded;
    let kp = match keypair {
        Some(k) => k,
        None => {
            loaded = load_keypair(config).await?;
            &loaded
        }
    };

    let built = build_swap(config, from_symbol, to_symbol, amount, &kp.pubkey()).await?;
    let signed = VersionedTransaction::try_new(built.transaction.message, &[kp])?;

    let client = solana_client::rpc_client::RpcClient::new(&config.solana.rpc_url);
    match client.send_and_confirm_transaction(&signed) {
        Ok(signature) => Ok(signature.to_string()),
        Err(e) => {
            app_log!(error, "Swap failed: {}", e);
            Err(SolanaClientError::TransactionFailed {
                reason: format!("Swap failed: {}", e),
            }
            .into())
        }
    }
}

// ── Price ─────────────────────────────────────────────────────────────────────

/// USD price of SOL, USDC, a mint, or any symbol Jupiter can find. Read-only,
/// so unlike swaps a symbol lookup is fine here.
pub async fn get_token_price(config: &Config, symbol: &str) -> Result<f64> {
    let mint = match get_token_mint(config, symbol) {
        Ok(mint) => mint,
        Err(_) => token::get_token_info(config, symbol)
            .await?
            .map(|t| t.address)
            .ok_or_else(|| anyhow!("No token found for '{}'", symbol))?,
    };

    app_log!(info, "Getting price for token: {} ({})", symbol, mint);

    // { "<mint>": { "usdPrice": 123.45, ... } } — or null for an unpriced mint
    let prices: std::collections::HashMap<String, Option<Value>> =
        get(config, "/price/v3", &[("ids", mint.as_str())]).await?.json().await?;

    prices
        .get(&mint)
        .and_then(|p| p.as_ref())
        .and_then(|p| p["usdPrice"].as_f64())
        .ok_or_else(|| anyhow!("No price for '{}'", symbol))
}
