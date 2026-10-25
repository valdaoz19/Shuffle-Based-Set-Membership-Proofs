use serde::{Deserialize, Serialize};

use aml_crypto_ibc::IbcCommitment;

use crate::filter::WindowFilterResult;
use crate::todo::VerifiedTodoList;

/// Homomorphic aggregation metadata over window transactions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowAggregate {
    pub transaction_count: usize,
    pub total_cents: u64,
    pub total_with_new_cents: u64,
    pub commitment_encodings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowProof {
    pub aggregate: WindowAggregate,
    pub todo_list: VerifiedTodoList,
}

impl WindowProof {
    pub fn build(filter_result: &WindowFilterResult, todo_list: VerifiedTodoList) -> Self {
        let commitment_encodings: Vec<String> = filter_result
            .transactions
            .iter()
            .map(|tx| tx.commitment.0.clone())
            .collect();

        Self {
            aggregate: WindowAggregate {
                transaction_count: filter_result.transactions.len(),
                total_cents: filter_result.total_cents,
                total_with_new_cents: filter_result.total_with_new_cents,
                commitment_encodings,
            },
            todo_list,
        }
    }

    pub fn to_json(&self) -> aml_core::AmlResult<String> {
        serde_json::to_string(self).map_err(|e| aml_core::AmlError::Serialization(e.to_string()))
    }
}

/// Sum commitment points homomorphically (same identity required for sound aggregation).
#[allow(dead_code)]
pub fn homomorphic_sum(commitments: &[IbcCommitment]) -> Option<IbcCommitment> {
    if commitments.is_empty() {
        return None;
    }
    let sum = commitments
        .iter()
        .fold(commitments[0].0, |acc, c| acc + c.0);
    Some(IbcCommitment(sum))
}

#[cfg(test)]
mod tests {
    use super::*;
    use aml_crypto_ibc::{IbcScheme, PublicParams};
    use rand::rngs::OsRng;

    #[test]
    fn homomorphic_sum_commits() {
        let pp = PublicParams::setup();
        let mut rng = OsRng;
        let (c1, _) = IbcScheme::commit(&pp, 1, 100, &mut rng);
        let (c2, _) = IbcScheme::commit(&pp, 1, 200, &mut rng);
        let sum = homomorphic_sum(&[c1, c2]).unwrap();
        assert_ne!(sum, c1);
        let _ = sum;
    }
}
