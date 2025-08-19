use anyhow::Result;
use rocket::{State, get, post, routes, serde::json::Json};
use serde::{Deserialize, Serialize};
use solana_sdk::{pubkey::Pubkey, signer::Signer};
use std::str::FromStr;
use tracing::{error, info};

use crate::{config::Config, jupiter, token, transaction, wallet};

#[derive(Deserialize)]
pub struct BalanceRequest {
    pub pubkey: String, // Public key to check balance for
}

#[derive(Deserialize)]
pub struct PrepareSwapRequest {
    pub payer_pubkey: String, // Who pays fees
    pub from_token: String,
    pub to_token: String,
    pub amount: f64,
}

#[derive(Deserialize)]
pub struct PrepareTransactionRequest {
    pub payer_pubkey: String, // Who pays fees and sends
    pub to_address: String,
    pub amount: f64,
}

#[derive(Deserialize)]
pub struct SubmitSignedRequest {
    pub signed_transaction: String, // Base64 encoded signed transaction
}

#[derive(Deserialize)]
pub struct PriceRequest {
    pub token: String, // Token symbol or mint address
}

#[derive(Deserialize)]
pub struct SearchRequest {
    pub query: String, // Search term
}

#[derive(Deserialize)]
pub struct WalletTokensRequest {
    pub pubkey: String, // Public key to get tokens for
}

#[derive(Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct BalanceResponse {
    pub pubkey: String,
    pub balance: f64,
    pub token: String,
}

#[derive(Serialize)]
pub struct PrepareSwapResponse {
    pub unsigned_transaction: String, // Base64 encoded unsigned transaction
    pub quote_info: QuoteInfo,
    pub required_signers: Vec<String>,
    pub recent_blockhash: String,
}

#[derive(Serialize)]
pub struct PrepareTransactionResponse {
    pub unsigned_transaction: String, // Base64 encoded unsigned transaction
    pub from: String,
    pub to: String,
    pub amount: f64,
    pub required_signers: Vec<String>,
    pub recent_blockhash: String,
}

#[derive(Serialize)]
pub struct QuoteInfo {
    pub expected_output: f64,
    pub price_impact: f64,
    pub route_steps: usize,
}

#[derive(Serialize)]
pub struct SubmitResponse {
    pub signature: String,
    pub status: String,
}

#[derive(Serialize)]
pub struct PriceResponse {
    pub token: String,
    pub price: f64,
    pub currency: String,
}

#[derive(Serialize)]
pub struct TokenSearchResponse {
    pub tokens: Vec<TokenInfo>,
    pub count: usize,
}

#[derive(Serialize)]
pub struct TokenInfo {
    pub symbol: String,
    pub name: String,
    pub address: String,
    pub decimals: u8,
}

#[derive(Serialize)]
pub struct WalletTokensResponse {
    pub pubkey: String,
    pub tokens: Vec<WalletTokenInfo>,
    pub total_tokens: usize,
}

#[derive(Serialize)]
pub struct WalletTokenInfo {
    pub symbol: String,
    pub name: String,
    pub mint: String,
    pub balance: f64,
    pub decimals: u8,
    pub usd_value: Option<f64>,
}

// Helper function to parse public key
fn parse_public_key(pubkey: &str) -> Result<Pubkey> {
    Ok(Pubkey::from_str(pubkey)?)
}

#[get("/health")]
pub fn health() -> Json<ApiResponse<String>> {
    Json(ApiResponse {
        success: true,
        data: Some("OK".to_string()),
        error: None,
    })
}

#[post("/balance", data = "<request>")]
pub async fn get_balance(
    request: Json<BalanceRequest>,
    config: &State<Config>,
) -> Json<ApiResponse<BalanceResponse>> {
    info!("Balance request for pubkey: {}", request.pubkey);

    match parse_public_key(&request.pubkey) {
        Ok(pubkey) => match wallet::get_balance_for_pubkey(config, &pubkey).await {
            Ok(balance) => Json(ApiResponse {
                success: true,
                data: Some(BalanceResponse {
                    pubkey: request.pubkey.clone(),
                    balance,
                    token: "SOL".to_string(),
                }),
                error: None,
            }),
            Err(e) => {
                error!("Failed to get balance: {}", e);
                Json(ApiResponse {
                    success: false,
                    data: None,
                    error: Some(format!("Failed to get balance: {}", e)),
                })
            }
        },
        Err(e) => Json(ApiResponse {
            success: false,
            data: None,
            error: Some(format!("Invalid public key: {}", e)),
        }),
    }
}

#[post("/swap/prepare", data = "<request>")]
pub async fn prepare_swap(
    request: Json<PrepareSwapRequest>,
    config: &State<Config>,
) -> Json<ApiResponse<PrepareSwapResponse>> {
    info!(
        "Prepare swap request: {} {} -> {} for {}",
        request.amount, request.from_token, request.to_token, request.payer_pubkey
    );

    match parse_public_key(&request.payer_pubkey) {
        Ok(payer_pubkey) => {
            match jupiter::prepare_swap_transaction(
                config,
                &request.from_token,
                &request.to_token,
                request.amount,
                &payer_pubkey,
            )
            .await
            {
                Ok((unsigned_tx, quote_info, signers, blockhash)) => Json(ApiResponse {
                    success: true,
                    data: Some(PrepareSwapResponse {
                        unsigned_transaction: unsigned_tx,
                        quote_info,
                        required_signers: signers,
                        recent_blockhash: blockhash,
                    }),
                    error: None,
                }),
                Err(e) => {
                    error!("Swap preparation failed: {}", e);
                    Json(ApiResponse {
                        success: false,
                        data: None,
                        error: Some(format!("Swap preparation failed: {}", e)),
                    })
                }
            }
        }
        Err(e) => Json(ApiResponse {
            success: false,
            data: None,
            error: Some(format!("Invalid payer public key: {}", e)),
        }),
    }
}

#[post("/transaction/prepare", data = "<request>")]
pub async fn prepare_transaction(
    request: Json<PrepareTransactionRequest>,
    config: &State<Config>,
) -> Json<ApiResponse<PrepareTransactionResponse>> {
    info!(
        "Prepare transaction request: {} SOL from {} to {}",
        request.amount, request.payer_pubkey, request.to_address
    );

    match parse_public_key(&request.payer_pubkey) {
        Ok(payer_pubkey) => {
            match transaction::prepare_sol_transfer(
                config,
                &payer_pubkey,
                &request.to_address,
                request.amount,
            )
            .await
            {
                Ok((unsigned_tx, signers, blockhash)) => Json(ApiResponse {
                    success: true,
                    data: Some(PrepareTransactionResponse {
                        unsigned_transaction: unsigned_tx,
                        from: request.payer_pubkey.clone(),
                        to: request.to_address.clone(),
                        amount: request.amount,
                        required_signers: signers,
                        recent_blockhash: blockhash,
                    }),
                    error: None,
                }),
                Err(e) => {
                    error!("Transaction preparation failed: {}", e);
                    Json(ApiResponse {
                        success: false,
                        data: None,
                        error: Some(format!("Transaction preparation failed: {}", e)),
                    })
                }
            }
        }
        Err(e) => Json(ApiResponse {
            success: false,
            data: None,
            error: Some(format!("Invalid payer public key: {}", e)),
        }),
    }
}

#[post("/transaction/submit", data = "<request>")]
pub async fn submit_signed_transaction(
    request: Json<SubmitSignedRequest>,
    config: &State<Config>,
) -> Json<ApiResponse<SubmitResponse>> {
    info!("Submit signed transaction request");

    match transaction::submit_signed_transaction(config, &request.signed_transaction).await {
        Ok(signature) => Json(ApiResponse {
            success: true,
            data: Some(SubmitResponse {
                signature,
                status: "submitted".to_string(),
            }),
            error: None,
        }),
        Err(e) => {
            error!("Transaction submission failed: {}", e);
            Json(ApiResponse {
                success: false,
                data: None,
                error: Some(format!("Transaction submission failed: {}", e)),
            })
        }
    }
}

#[post("/price", data = "<request>")]
pub async fn get_token_price(
    request: Json<PriceRequest>,
    config: &State<Config>,
) -> Json<ApiResponse<PriceResponse>> {
    info!("Price request for token: {}", request.token);

    match jupiter::get_token_price(config, &request.token).await {
        Ok(price) => Json(ApiResponse {
            success: true,
            data: Some(PriceResponse {
                token: request.token.clone(),
                price,
                currency: "USD".to_string(),
            }),
            error: None,
        }),
        Err(e) => {
            error!("Price fetch failed: {}", e);
            Json(ApiResponse {
                success: false,
                data: None,
                error: Some(format!("Price fetch failed: {}", e)),
            })
        }
    }
}

#[post("/tokens/search", data = "<request>")]
pub async fn search_tokens(
    request: Json<SearchRequest>,
    config: &State<Config>,
) -> Json<ApiResponse<TokenSearchResponse>> {
    info!("Token search request: {}", request.query);

    match token::search_tokens(config, &request.query).await {
        Ok(tokens) => {
            let token_infos: Vec<TokenInfo> = tokens
                .into_iter()
                .map(|t| TokenInfo {
                    symbol: t.symbol,
                    name: t.name,
                    address: t.address,
                    decimals: t.decimals,
                })
                .collect();

            let count = token_infos.len();

            Json(ApiResponse {
                success: true,
                data: Some(TokenSearchResponse {
                    tokens: token_infos,
                    count,
                }),
                error: None,
            })
        }
        Err(e) => {
            error!("Token search failed: {}", e);
            Json(ApiResponse {
                success: false,
                data: None,
                error: Some(format!("Token search failed: {}", e)),
            })
        }
    }
}

#[post("/wallet/tokens", data = "<request>")]
pub async fn get_wallet_tokens(
    request: Json<WalletTokensRequest>,
    config: &State<Config>,
) -> Json<ApiResponse<WalletTokensResponse>> {
    info!("Wallet tokens request for pubkey: {}", request.pubkey);

    match parse_public_key(&request.pubkey) {
        Ok(pubkey) => {
            match wallet::get_wallet_tokens_for_pubkey(config, &pubkey).await {
                Ok(tokens) => {
                    let mut wallet_tokens = Vec::new();

                    for token in tokens {
                        // Try to get USD value
                        let usd_value = if let Ok(price) =
                            jupiter::get_token_price(config, &token.symbol).await
                        {
                            Some(token.balance * price)
                        } else {
                            None
                        };

                        wallet_tokens.push(WalletTokenInfo {
                            symbol: token.symbol,
                            name: token.name,
                            mint: token.mint,
                            balance: token.balance,
                            decimals: token.decimals,
                            usd_value,
                        });
                    }

                    let total_tokens = wallet_tokens.len();

                    Json(ApiResponse {
                        success: true,
                        data: Some(WalletTokensResponse {
                            pubkey: request.pubkey.clone(),
                            tokens: wallet_tokens,
                            total_tokens,
                        }),
                        error: None,
                    })
                }
                Err(e) => {
                    error!("Failed to get wallet tokens: {}", e);
                    Json(ApiResponse {
                        success: false,
                        data: None,
                        error: Some(format!("Failed to get wallet tokens: {}", e)),
                    })
                }
            }
        }
        Err(e) => Json(ApiResponse {
            success: false,
            data: None,
            error: Some(format!("Invalid public key: {}", e)),
        }),
    }
}

pub async fn start_server(config: Config, port: u16) -> Result<()> {
    let figment = rocket::Config::figment()
        .merge(("port", port))
        .merge(("address", "0.0.0.0"));

    let rocket = rocket::custom(figment).manage(config).mount(
        "/api/v1",
        routes![
            health,
            get_balance,
            prepare_swap,
            prepare_transaction,
            submit_signed_transaction,
            get_token_price,
            search_tokens,
            get_wallet_tokens,
        ],
    );

    info!("🚀 Starting Solana API server on http://0.0.0.0:{}", port);
    info!("📋 Available endpoints:");
    info!("  GET  /api/v1/health");
    info!("  POST /api/v1/balance");
    info!("  POST /api/v1/swap/prepare");
    info!("  POST /api/v1/transaction/prepare");
    info!("  POST /api/v1/transaction/submit");
    info!("  POST /api/v1/price");
    info!("  POST /api/v1/tokens/search");
    info!("  POST /api/v1/wallet/tokens");

    let _ = rocket.launch().await?;

    Ok(())
}
