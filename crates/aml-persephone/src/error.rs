use thiserror::Error;

#[derive(Debug, Error)]
pub enum PersephoneError {
    #[error("proof verification failed")]
    VerifyFailed,

    #[error("invalid state: {0}")]
    InvalidState(String),

    #[error("range error: {0}")]
    RangeError(String),

    #[error("encoding error: {0}")]
    Encoding(String),
}
