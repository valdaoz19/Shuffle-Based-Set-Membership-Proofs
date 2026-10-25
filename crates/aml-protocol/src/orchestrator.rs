use chrono::Utc;
use rand::rngs::OsRng;
use std::time::Instant;
use uuid::Uuid;

use aml_core::{
    AmlError, AmlResult, CommitmentBytes, ProofBundle, VerificationDecision, VerificationRequest,
    VerificationResult,
};
use aml_crypto_ibc::{IbcScheme, IdEqProver, IdEqVerifier, PublicParams, VVerProver, VVerVerifier};
use aml_crypto_membership::{SetMembershipProver, SetMembershipVerifier};
use aml_crypto_shuffle::VerifiableShuffle;
use aml_crypto_update::ObliviousUpdate;
use aml_crypto_window::{SlidingWindow, VerifiedTodoList, WindowProof};
use aml_persephone::{PersephoneVerifier, ValueVerifier};

use crate::demo_state::{DemoServerState, DemoUserRecord};

pub struct ProtocolOrchestrator {
    verifier: PersephoneVerifier,
}

impl Default for ProtocolOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}

impl ProtocolOrchestrator {
    pub fn new() -> Self {
        Self {
            verifier: PersephoneVerifier::new(),
        }
    }

    pub fn verify_payment(
        &self,
        state: &mut DemoServerState,
        user_key: &str,
        request: VerificationRequest,
    ) -> AmlResult<VerificationResult> {
        let pp = state.pp.clone();
        let user = state
            .users
            .get_mut(user_key)
            .ok_or_else(|| AmlError::InvalidInput(format!("unknown user: {user_key}")))?;

        self.run_pipeline(&pp, user, request)
    }

    fn run_pipeline(
        &self,
        pp: &PublicParams,
        user: &mut DemoUserRecord,
        request: VerificationRequest,
    ) -> AmlResult<VerificationResult> {
        let mut rng = OsRng;
        let threshold = request.window_config.threshold_cents;

        let tx_commitment = IbcScheme::decode(&request.tx_id_commitment.0)?;
        let membership_proof = match SetMembershipProver::prove(
            pp,
            &user.membership_set_commitments,
            &tx_commitment,
            &mut rng,
        ) {
            Ok(proof) => proof,
            Err(_) => {
                return Ok(rejected(
                    request.request_id,
                    "set membership proof failed: transaction commitment is not in PayPay set",
                    ProofBundle::default(),
                ));
            }
        };
        if !SetMembershipVerifier::verify(
            pp,
            &user.membership_set_commitments,
            &tx_commitment,
            &membership_proof,
        ) {
            return Ok(rejected(
                request.request_id,
                "set membership verification failed",
                ProofBundle::default(),
            ));
        }

        if tx_commitment != user.tx_id_commitment {
            return Ok(rejected(
                request.request_id,
                "transaction ID commitment mismatch",
                ProofBundle::default(),
            ));
        }

        if user.commitments.is_empty() {
            return Err(AmlError::InvalidInput(
                "user has no historical transactions".into(),
            ));
        }

        let hist_opening = &user.openings[0];
        let hist_commitment = user.commitments[0];

        // Phase 2: ID Equality Proof
        let id_eq_proof = IdEqProver::prove(
            pp,
            &user.tx_id_opening,
            hist_opening,
            &user.tx_id_commitment,
            &hist_commitment,
            &mut rng,
        );
        if !IdEqVerifier::verify(pp, &user.tx_id_commitment, &hist_commitment, &id_eq_proof) {
            return Ok(rejected(
                request.request_id,
                "ID equality proof failed",
                ProofBundle::default(),
            ));
        }

        // Phase 3a: Value Verification (Π.VVer)
        let (incoming_c, incoming_o) =
            IbcScheme::commit(pp, user.user_id_hash, request.amount_cents, &mut rng);
        let vver_proof =
            VVerProver::prove(pp, &incoming_o, &incoming_c, request.amount_cents, &mut rng);
        if !VVerVerifier::verify(pp, &incoming_c, request.amount_cents, &vver_proof) {
            return Ok(rejected(
                request.request_id,
                "value verification failed",
                ProofBundle::default(),
            ));
        }

        // Phase 3b: persephone ZKNNP per historical tx (fresh state each verification)
        let mut nnp_state = self
            .verifier
            .init_bounds(0, threshold)
            .map_err(|e| AmlError::Internal(e.to_string()))?;
        for opening in &user.openings {
            let amount = opening.value;
            let (ok, new_state) = self
                .verifier
                .prove_transaction(0, amount, threshold, &nnp_state)
                .map_err(|e| AmlError::Internal(e.to_string()))?;
            if !ok {
                return Ok(rejected(
                    request.request_id,
                    "sequential value proof failed",
                    ProofBundle::default(),
                ));
            }
            nnp_state = new_state;
        }

        // Phase 4: Verifiable Shuffle
        let (shuffle_proof, shuffled_openings) =
            VerifiableShuffle::shuffle_and_prove(pp, &user.commitments, &user.openings, &mut rng);
        if !VerifiableShuffle::verify(pp, &user.commitments, &shuffle_proof) {
            return Ok(rejected(
                request.request_id,
                "shuffle verification failed",
                ProofBundle::default(),
            ));
        }

        // Phase 5: Sliding Window
        let filter_result = SlidingWindow::filter_with_new(
            &user.transactions,
            request.window_start,
            request.amount_cents,
            Utc::now(),
        );

        let mut todo_list = VerifiedTodoList::new();
        for tx in SlidingWindow::expiring_transactions(&user.transactions, request.window_start, 7)
        {
            todo_list.schedule_expiry(&tx, tx.timestamp + chrono::Duration::days(365));
        }
        let window_proof = WindowProof::build(&filter_result, todo_list);

        // Phase 6: Oblivious Update
        let mut shuffled_commitments = shuffle_proof.outputs.clone();
        let mut shuffled_openings = shuffled_openings;
        let update_proof = ObliviousUpdate::apply(
            pp,
            &mut shuffled_commitments,
            &mut shuffled_openings,
            0,
            request.amount_cents,
            &mut rng,
        );

        // Phase 7: Threshold Check via ZK-MNNP
        let threshold_started = Instant::now();
        let (threshold_ok, final_state) = self
            .verifier
            .prove_threshold_headroom(
                threshold,
                filter_result.total_cents,
                request.amount_cents,
                &nnp_state,
            )
            .map_err(|e| AmlError::Internal(e.to_string()))?;
        let zk_mnnp_execution_time_ms = threshold_started.elapsed().as_millis() as u64;
        let _final_state = final_state;

        let decision = if threshold_ok {
            VerificationDecision::Approved
        } else {
            VerificationDecision::Rejected
        };

        let id_eq_json = serde_json::to_string(&id_eq_proof)
            .map_err(|e| AmlError::Serialization(e.to_string()))?;
        let vver_json = serde_json::to_string(&vver_proof)
            .map_err(|e| AmlError::Serialization(e.to_string()))?;

        let proof_bundle = ProofBundle {
            set_membership_proof: Some(membership_proof.to_json()?),
            id_eq_proof: Some(id_eq_json),
            value_verification_proof: Some(vver_json),
            shuffle_proof: Some(shuffle_proof.to_json()?),
            window_proof: Some(window_proof.to_json()?),
            update_proof: Some(update_proof.to_json()?),
            threshold_proof: Some(format!("zk-mnnp:ok={threshold_ok}")),
            commitments: shuffled_commitments
                .iter()
                .map(|c| {
                    CommitmentBytes::new(IbcScheme::encode(c).unwrap_or_else(|_| "invalid".into()))
                })
                .collect(),
        };

        let message = match decision {
            VerificationDecision::Approved => {
                "Payment approved: within AML threshold for sliding window".into()
            }
            VerificationDecision::Rejected => {
                "Payment rejected: exceeds AML threshold or proof failure".into()
            }
        };

        Ok(VerificationResult {
            request_id: request.request_id,
            decision,
            threshold_compliant: threshold_ok,
            proof_bundle,
            message,
            zk_mnnp_execution_time_ms,
        })
    }
}

fn rejected(request_id: Uuid, message: &str, proof_bundle: ProofBundle) -> VerificationResult {
    VerificationResult {
        request_id,
        decision: VerificationDecision::Rejected,
        threshold_compliant: false,
        proof_bundle,
        message: message.to_string(),
        zk_mnnp_execution_time_ms: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aml_core::WindowConfig;

    #[test]
    fn approves_within_threshold() {
        let mut state = DemoServerState::new();
        state
            .setup_user("user_42", 42, &[(500_000, 30), (300_000, 60)])
            .unwrap();

        let tx_id = state.users.get("user_42").unwrap().tx_id_commitment;
        let tx_encoded = IbcScheme::encode(&tx_id).unwrap();

        let orch = ProtocolOrchestrator::new();
        let request = VerificationRequest {
            request_id: Uuid::new_v4(),
            tx_id_commitment: CommitmentBytes::new(tx_encoded),
            amount_cents: 200_000,
            window_start: SlidingWindow::window_start(&WindowConfig::default(), Utc::now()),
            window_config: WindowConfig::default(),
        };

        let result = orch.verify_payment(&mut state, "user_42", request).unwrap();
        assert_eq!(result.decision, VerificationDecision::Approved);
        assert!(result.threshold_compliant);
    }

    #[test]
    fn rejects_over_threshold() {
        let mut state = DemoServerState::new();
        state
            .setup_user("user_99", 99, &[(1_500_000, 10), (400_000, 20)])
            .unwrap();

        let tx_id = state.users.get("user_99").unwrap().tx_id_commitment;
        let tx_encoded = IbcScheme::encode(&tx_id).unwrap();

        let orch = ProtocolOrchestrator::new();
        let request = VerificationRequest {
            request_id: Uuid::new_v4(),
            tx_id_commitment: CommitmentBytes::new(tx_encoded),
            amount_cents: 500_000,
            window_start: SlidingWindow::window_start(&WindowConfig::default(), Utc::now()),
            window_config: WindowConfig::default(),
        };

        let result = orch.verify_payment(&mut state, "user_99", request).unwrap();
        assert_eq!(result.decision, VerificationDecision::Rejected);
    }

    #[test]
    fn repeated_verify_on_same_user_succeeds() {
        let mut state = DemoServerState::new();
        state
            .setup_user("user_42", 42, &[(500_000, 30), (300_000, 60)])
            .unwrap();

        let tx_id = state.users.get("user_42").unwrap().tx_id_commitment;
        let tx_encoded = IbcScheme::encode(&tx_id).unwrap();
        let orch = ProtocolOrchestrator::new();

        for _ in 0..3 {
            let request = VerificationRequest {
                request_id: Uuid::new_v4(),
                tx_id_commitment: CommitmentBytes::new(tx_encoded.clone()),
                amount_cents: 200_000,
                window_start: SlidingWindow::window_start(&WindowConfig::default(), Utc::now()),
                window_config: WindowConfig::default(),
            };
            let result = orch.verify_payment(&mut state, "user_42", request).unwrap();
            assert_eq!(result.decision, VerificationDecision::Approved);
        }
    }
}
