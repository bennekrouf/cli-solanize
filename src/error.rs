use thiserror::Error;

#[derive(Error, Debug)]
pub enum SolanaClientError {
    #[error("Wallet not found at path: {path}")]
    WalletNotFound { path: String },

    #[error("Invalid wallet format")]
    InvalidWalletFormat,

    #[error("Network connection failed: {source}")]
    NetworkError {
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("Transaction failed: {reason}")]
    TransactionFailed { reason: String },

    #[error("Insufficient balance: have {current}, need {required}")]
    InsufficientBalance { current: f64, required: f64 },

    #[error("Invalid address: {address}")]
    InvalidAddress { address: String },

    #[error("Config error: {message}")]
    ConfigError { message: String },
}
