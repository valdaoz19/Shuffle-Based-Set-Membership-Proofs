use serde::{Deserialize, Serialize};

/// Serializable sequential commitment state from ZKNNP v2.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CommitmentState {
    pub c_l: String,
    pub r_l: String,
    pub c_u: String,
    pub r_u: String,
    pub lower: u64,
    pub upper: u64,
    /// Remaining headroom in cents for sequential proofs.
    pub remaining: u64,
}

pub trait ValueVerifier: Send + Sync {
    fn init_bounds(
        &self,
        lower: u64,
        upper: u64,
    ) -> Result<CommitmentState, crate::PersephoneError>;

    fn prove_transaction(
        &self,
        lower: u64,
        amount: u64,
        upper: u64,
        state: &CommitmentState,
    ) -> Result<(bool, CommitmentState), crate::PersephoneError>;

    fn prove_threshold_headroom(
        &self,
        threshold: u64,
        window_total: u64,
        new_amount: u64,
        state: &CommitmentState,
    ) -> Result<(bool, CommitmentState), crate::PersephoneError>;
}
