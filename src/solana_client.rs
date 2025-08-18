use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthChallenge {
    pub wallet_address: String,
    pub challenge: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthVerification {
    pub wallet_address: String,
    pub signature: String,
    pub challenge: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionRequest {
    pub from: String,
    pub to: String,
    pub amount: f64,
}

pub struct SolanaApiClient {
    client: Client,
    base_url: String,
}

impl SolanaApiClient {
    pub fn new(base_url: String) -> Self {
        Self {
            client: Client::new(),
            base_url,
        }
    }

    // Future API implementations
    pub async fn request_challenge(&self, _wallet_address: &str) -> Result<AuthChallenge> {
        // TODO: Implement POST /api/v1/auth/challenge/{wallet_address}
        unimplemented!("API endpoints not implemented yet")
    }

    pub async fn verify_auth(&self, _verification: AuthVerification) -> Result<String> {
        // TODO: Implement POST /api/v1/auth/verify
        unimplemented!("API endpoints not implemented yet")
    }

    pub async fn refresh_token(&self, _token: &str) -> Result<String> {
        // TODO: Implement POST /api/v1/auth/refresh
        unimplemented!("API endpoints not implemented yet")
    }
}
