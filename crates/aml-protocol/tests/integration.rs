//! End-to-end integration tests for approve / reject scenarios.

use aml_core::{VerificationRequest, VerificationStatus, VerifyApiResponse, WindowConfig};
use aml_crypto_ibc::IbcScheme;
use aml_crypto_window::SlidingWindow;
use aml_protocol::{DemoServerState, ProtocolOrchestrator};
use chrono::Utc;
use uuid::Uuid;

fn run_verify(state: &mut DemoServerState, user_key: &str, amount_cents: u64) -> VerifyApiResponse {
    let tx_id = state.users.get(user_key).unwrap().tx_id_commitment;
    let tx_encoded = IbcScheme::encode(&tx_id).unwrap();

    let request = VerificationRequest {
        request_id: Uuid::new_v4(),
        tx_id_commitment: aml_core::CommitmentBytes::new(tx_encoded),
        amount_cents,
        window_start: SlidingWindow::window_start(&WindowConfig::default(), Utc::now()),
        window_config: WindowConfig::default(),
    };

    let result = ProtocolOrchestrator::new()
        .verify_payment(state, user_key, request)
        .expect("pipeline verify");

    VerifyApiResponse::from_result(result)
}

#[test]
fn scenario_a_pipeline_approves_under_threshold() {
    let mut state = DemoServerState::new();
    state
        .setup_user("approve_user", 1, &[(800_000, 10), (600_000, 20)])
        .unwrap();

    let response = run_verify(&mut state, "approve_user", 300_000);
    assert_eq!(response.status, VerificationStatus::Approved);
    assert!(response.threshold_compliant);
    assert!(
        response.execution_time_ms > 0
            || response.crypto_proof_metadata.zk_mnnp_threshold.is_some()
    );
}

#[test]
fn scenario_b_pipeline_rejects_over_threshold() {
    let mut state = DemoServerState::new();
    state
        .setup_user("reject_user", 2, &[(1_500_000, 5), (400_000, 15)])
        .unwrap();

    let response = run_verify(&mut state, "reject_user", 500_000);
    assert_eq!(response.status, VerificationStatus::Rejected);
    assert!(!response.threshold_compliant);
}

#[test]
fn full_pipeline_integration() {
    let mut state = DemoServerState::new();
    state
        .setup_user(
            "integration_user",
            1001,
            &[(800_000, 15), (600_000, 40), (200_000, 100)],
        )
        .expect("setup");

    let response = run_verify(&mut state, "integration_user", 300_000);
    assert_eq!(response.status, VerificationStatus::Approved);
    assert!(response.crypto_proof_metadata.set_membership.is_some());
    assert!(
        response
            .crypto_proof_metadata
            .ibc_identity_equality
            .is_some()
    );
    assert!(response.crypto_proof_metadata.verifiable_shuffle.is_some());
    assert!(response.crypto_proof_metadata.zk_mnnp_threshold.is_some());
}
