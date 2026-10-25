//! HTTP integration tests for POST `/api/v1/verify`.

use aml_api::{
    SecurityConfig, SharedState, build_api_router, new_shared_state_with_audit_url,
    new_shared_state_with_audit_url_and_security,
};
use aml_core::{VerificationStatus, VerifyApiResponse};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use tower::ServiceExt;

struct TestApp {
    state: SharedState,
}

impl TestApp {
    async fn new() -> Self {
        Self {
            state: new_shared_state_with_audit_url("sqlite::memory:")
                .await
                .unwrap(),
        }
    }

    async fn new_with_security(security: SecurityConfig) -> Self {
        Self {
            state: new_shared_state_with_audit_url_and_security("sqlite::memory:", security)
                .await
                .unwrap(),
        }
    }

    fn router(&self) -> axum::Router {
        build_api_router(self.state.clone())
    }

    async fn post(&self, path: &str, body: &str) -> (StatusCode, Vec<u8>) {
        self.post_with_key(path, body, None).await
    }

    async fn post_with_key(
        &self,
        path: &str,
        body: &str,
        api_key: Option<&str>,
    ) -> (StatusCode, Vec<u8>) {
        let mut builder = Request::builder()
            .method("POST")
            .uri(path)
            .header("content-type", "application/json");
        if let Some(api_key) = api_key {
            builder = builder.header("x-api-key", api_key);
        }
        let response = self
            .router()
            .oneshot(builder.body(Body::from(body.to_string())).unwrap())
            .await
            .unwrap();

        let status = response.status();
        let body = response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec();
        (status, body)
    }

    async fn get(&self, path: &str) -> (StatusCode, Vec<u8>) {
        self.get_with_key(path, None).await
    }

    async fn get_with_key(&self, path: &str, api_key: Option<&str>) -> (StatusCode, Vec<u8>) {
        let mut builder = Request::builder().uri(path);
        if let Some(api_key) = api_key {
            builder = builder.header("x-api-key", api_key);
        }
        let response = self
            .router()
            .oneshot(builder.body(Body::empty()).unwrap())
            .await
            .unwrap();

        let status = response.status();
        let body = response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec();
        (status, body)
    }

    async fn setup_scenario(&self, scenario: &str) -> serde_json::Value {
        let (status, body) = self
            .post(
                "/api/v1/demo/setup",
                &format!(r#"{{"scenario":"{scenario}"}}"#),
            )
            .await;
        assert_eq!(status, StatusCode::OK);
        serde_json::from_slice(&body).unwrap()
    }

    async fn verify(&self, scenario: &str, amount_cents: u64) -> (StatusCode, VerifyApiResponse) {
        let (status, body) = self
            .post(
                "/api/v1/verify",
                &format!(r#"{{"amount_cents":{amount_cents},"scenario":"{scenario}"}}"#),
            )
            .await;
        let parsed: VerifyApiResponse = serde_json::from_slice(&body).unwrap();
        (status, parsed)
    }
}

/// Compliant scenario: history 1.2M + 200k = 1.4M < 2M threshold.
#[tokio::test]
async fn scenario_compliant_approves() {
    let app = TestApp::new().await;
    app.setup_scenario("compliant").await;
    let (status, body) = app.verify("compliant", 200_000).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.status, VerificationStatus::Approved);
    assert!(body.threshold_compliant);
    assert!(body.audit_recorded);
    assert!(body.proof_audit_hash.is_some());
    assert!(!body.replay_warning);
    assert!(body.crypto_proof_metadata.zk_mnnp_threshold.is_some());
    assert!(body.crypto_proof_metadata.ibc_identity_equality.is_some());
    assert!(body.crypto_proof_metadata.verifiable_shuffle.is_some());
}

/// Exceeded scenario: history 1.9M + 500k = 2.4M > 2M threshold.
#[tokio::test]
async fn scenario_exceeded_rejects() {
    let app = TestApp::new().await;
    app.setup_scenario("exceeded").await;
    let (status, body) = app.verify("exceeded", 500_000).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.status, VerificationStatus::Rejected);
    assert!(!body.threshold_compliant);
    assert!(body.audit_recorded);
    assert!(body.proof_audit_hash.is_some());
}

/// Same user can be verified repeatedly without state corruption.
#[tokio::test]
async fn repeated_verify_on_same_user_succeeds() {
    let app = TestApp::new().await;
    app.setup_scenario("compliant").await;

    let (status, first) = app.verify("compliant", 200_000).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first.status, VerificationStatus::Approved);
    assert!(!first.replay_warning);

    let (status, second) = app.verify("compliant", 200_000).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(second.status, VerificationStatus::Approved);
    assert!(second.replay_warning);
    assert!(second.message.contains("Replay warning"));
}

#[tokio::test]
async fn demo_scenarios_endpoint_lists_both_scenarios() {
    let app = TestApp::new().await;
    let (status, body) = app.get("/api/v1/demo/scenarios").await;

    assert_eq!(status, StatusCode::OK);
    let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(parsed["scenarios"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn proof_lookup_and_integrity_verify_succeeds() {
    let app = TestApp::new().await;
    app.setup_scenario("compliant").await;
    let (_, verify_body) = app.verify("compliant", 200_000).await;

    let proof_path = format!("/api/v1/proofs/{}", verify_body.request_id);
    let (status, proof_body) = app.get(&proof_path).await;
    assert_eq!(status, StatusCode::OK);
    let proof: serde_json::Value = serde_json::from_slice(&proof_body).unwrap();
    assert_eq!(proof["request_id"], verify_body.request_id.to_string());
    assert_eq!(
        proof["proof_audit_hash"],
        verify_body.proof_audit_hash.unwrap()
    );

    let verify_path = format!("/api/v1/proofs/{}/verify", verify_body.request_id);
    let (status, integrity_body) = app.post(&verify_path, "{}").await;
    assert_eq!(status, StatusCode::OK);
    let integrity: serde_json::Value = serde_json::from_slice(&integrity_body).unwrap();
    assert_eq!(integrity["valid"], true);
}

#[tokio::test]
async fn commitment_ledger_lists_entries_after_verify() {
    let app = TestApp::new().await;
    app.setup_scenario("compliant").await;
    let (_, verify_body) = app.verify("compliant", 200_000).await;

    let (status, body) = app.get("/api/v1/ledger/commitments").await;
    assert_eq!(status, StatusCode::OK);
    let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let entries = parsed["entries"].as_array().unwrap();
    assert!(!entries.is_empty());
    assert!(
        entries
            .iter()
            .any(|entry| entry["request_id"] == verify_body.request_id.to_string())
    );
}

#[tokio::test]
async fn amount_zero_is_rejected() {
    let app = TestApp::new().await;
    let (status, body) = app.post("/api/v1/verify", r#"{"amount_cents":0}"#).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(
        parsed["error"]
            .as_str()
            .unwrap()
            .contains("greater than zero")
    );
}

#[tokio::test]
async fn amount_above_max_is_rejected() {
    let app = TestApp::new().await;
    let (status, body) = app
        .post("/api/v1/verify", r#"{"amount_cents":5000001}"#)
        .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(parsed["error"].as_str().unwrap().contains("max_tx_cents"));
}

#[tokio::test]
async fn api_key_protects_audit_endpoints_when_enabled() {
    let app = TestApp::new_with_security(SecurityConfig::new(Some("secret".into()), None)).await;

    let (status, _) = app.get("/api/v1/ledger/commitments").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, body) = app
        .get_with_key("/api/v1/ledger/commitments", Some("secret"))
        .await;
    assert_eq!(status, StatusCode::OK);
    let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(parsed["entries"].as_array().is_some());
}

#[tokio::test]
async fn rate_limit_returns_429_when_limit_is_exceeded() {
    let app = TestApp::new_with_security(SecurityConfig::new(Some("secret".into()), Some(1))).await;

    let (status, _) = app
        .get_with_key("/api/v1/ledger/commitments", Some("secret"))
        .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = app
        .get_with_key("/api/v1/ledger/commitments", Some("secret"))
        .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(parsed["error"].as_str().unwrap().contains("Rate limit"));
}
