//! Core domain types for the ZKP AML cross-border PoC.

pub mod config;
pub mod error;
pub mod types;

pub use config::{AmlConfig, WindowConfig};
pub use error::{AmlError, AmlResult};
pub use types::{
    CommitmentBytes, CryptoProofMetadata, IdentityId, ProofBundle, Transaction, TransactionId,
    VerificationDecision, VerificationRequest, VerificationResult, VerificationStatus,
    VerifyApiResponse,
};
