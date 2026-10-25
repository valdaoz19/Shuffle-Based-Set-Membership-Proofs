use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::config::WindowConfig;

/// Opaque identity handle (never exposed in public verification output).
pub type IdentityId = String;

/// Transaction identifier (may be an IBC commitment encoding).
pub type TransactionId = String;

/// Base64-encoded Ristretto commitment bytes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(transparent)]
pub struct CommitmentBytes(pub String);

impl CommitmentBytes {
    pub fn new(encoded: impl Into<String>) -> Self {
        Self(encoded.into())
    }
}

/// A single approved transaction in server history.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Transaction {
    pub data_id: TransactionId,
    pub group_id: IdentityId,
    pub amount_cents: u64,
    pub timestamp: DateTime<Utc>,
    /// IBC commitment to (group_id, amount_cents).
    pub commitment: CommitmentBytes,
}

/// Client payment verification request (x = (c_tid, v, T, f)).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VerificationRequest {
    pub request_id: Uuid,
    /// Transaction ID commitment c_tid.
    pub tx_id_commitment: CommitmentBytes,
    /// Requested payment amount in SGD cents.
    pub amount_cents: u64,
    /// Window start timestamp T (transactions with t > T are included).
    pub window_start: DateTime<Utc>,
    pub window_config: WindowConfig,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VerificationDecision {
    Approved,
    Rejected,
}

/// Aggregated proof artifacts returned to the public verifier.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ProofBundle {
    pub set_membership_proof: Option<String>,
    pub id_eq_proof: Option<String>,
    pub value_verification_proof: Option<String>,
    pub shuffle_proof: Option<String>,
    pub window_proof: Option<String>,
    pub update_proof: Option<String>,
    pub threshold_proof: Option<String>,
    pub commitments: Vec<CommitmentBytes>,
}

/// Public verification outcome (no raw identity or exact amounts).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VerificationResult {
    pub request_id: Uuid,
    pub decision: VerificationDecision,
    /// Whether threshold check passed (without revealing totals).
    pub threshold_compliant: bool,
    pub proof_bundle: ProofBundle,
    pub message: String,
    /// Wall-clock time for the ZK-MNNP threshold verification phase (milliseconds).
    pub zk_mnnp_execution_time_ms: u64,
}

/// Official API compliance status for `/api/v1/verify`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VerificationStatus {
    Approved,
    Rejected,
}

impl From<VerificationDecision> for VerificationStatus {
    fn from(decision: VerificationDecision) -> Self {
        match decision {
            VerificationDecision::Approved => Self::Approved,
            VerificationDecision::Rejected => Self::Rejected,
        }
    }
}

/// Serialized cryptographic artifacts for academic / audit presentation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CryptoProofMetadata {
    pub pipeline: String,
    pub set_membership: Option<String>,
    pub ibc_identity_equality: Option<String>,
    pub ibc_value_verification: Option<String>,
    pub verifiable_shuffle: Option<String>,
    pub sliding_window: Option<String>,
    pub oblivious_update: Option<String>,
    pub zk_mnnp_threshold: Option<String>,
    pub commitment_handles: Vec<CommitmentBytes>,
}

impl From<ProofBundle> for CryptoProofMetadata {
    fn from(bundle: ProofBundle) -> Self {
        Self {
            pipeline:
                "Set Membership → IBC → Verifiable Shuffle → Sliding Window → ZK-MNNP Threshold"
                    .into(),
            set_membership: bundle.set_membership_proof,
            ibc_identity_equality: bundle.id_eq_proof,
            ibc_value_verification: bundle.value_verification_proof,
            verifiable_shuffle: bundle.shuffle_proof,
            sliding_window: bundle.window_proof,
            oblivious_update: bundle.update_proof,
            zk_mnnp_threshold: bundle.threshold_proof,
            commitment_handles: bundle.commitments,
        }
    }
}

/// Structured JSON response for POST `/api/v1/verify`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VerifyApiResponse {
    pub status: VerificationStatus,
    pub execution_time_ms: u64,
    pub crypto_proof_metadata: CryptoProofMetadata,
    pub request_id: Uuid,
    pub message: String,
    pub threshold_compliant: bool,
    pub proof_audit_hash: Option<String>,
    pub replay_warning: bool,
    pub audit_recorded: bool,
}

impl VerifyApiResponse {
    pub fn from_result(result: VerificationResult) -> Self {
        Self {
            status: result.decision.into(),
            execution_time_ms: result.zk_mnnp_execution_time_ms,
            crypto_proof_metadata: result.proof_bundle.clone().into(),
            request_id: result.request_id,
            message: result.message,
            threshold_compliant: result.threshold_compliant,
            proof_audit_hash: None,
            replay_warning: false,
            audit_recorded: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_api_response_maps_approved_status() {
        let result = VerificationResult {
            request_id: Uuid::new_v4(),
            decision: VerificationDecision::Approved,
            threshold_compliant: true,
            proof_bundle: ProofBundle {
                id_eq_proof: Some(r#"{"t":"abc"}"#.into()),
                threshold_proof: Some("zk-mnnp:ok=true".into()),
                ..Default::default()
            },
            message: "ok".into(),
            zk_mnnp_execution_time_ms: 42,
        };

        let api = VerifyApiResponse::from_result(result);
        assert_eq!(api.status, VerificationStatus::Approved);
        assert_eq!(api.execution_time_ms, 42);
        assert!(api.crypto_proof_metadata.ibc_identity_equality.is_some());
    }

    #[test]
    fn verify_api_response_maps_rejected_status() {
        let result = VerificationResult {
            request_id: Uuid::new_v4(),
            decision: VerificationDecision::Rejected,
            threshold_compliant: false,
            proof_bundle: ProofBundle::default(),
            message: "threshold exceeded".into(),
            zk_mnnp_execution_time_ms: 7,
        };

        let api = VerifyApiResponse::from_result(result);
        assert_eq!(api.status, VerificationStatus::Rejected);
        assert!(!api.threshold_compliant);
    }
}
