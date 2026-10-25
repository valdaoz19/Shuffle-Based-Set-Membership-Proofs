use thiserror::Error;

#[derive(Debug, Error)]
pub enum AmlError {
    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("cryptographic verification failed: {0}")]
    CryptoVerifyFailed(String),

    #[error("threshold exceeded: total {total} exceeds limit {threshold}")]
    ThresholdExceeded { total: u64, threshold: u64 },

    #[error("protocol error: {0}")]
    Protocol(String),

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("internal error: {0}")]
    Internal(String),
}

pub type AmlResult<T> = Result<T, AmlError>;
