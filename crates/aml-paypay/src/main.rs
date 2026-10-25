use std::{net::SocketAddr, sync::Arc, time::Instant};

use aml_core::{
    CommitmentBytes, VerificationDecision, VerificationRequest, VerificationStatus, WindowConfig,
};
use aml_crypto_ibc::IbcScheme;
use aml_crypto_membership::{
    MembershipSchemeImpl, MembershipStatement, finalize_membership_envelope,
    membership_mode_from_env,
};
use aml_crypto_shuffle::VerifiableShuffle;
use aml_crypto_window::SlidingWindow;
use aml_protocol::{DemoServerState, ProtocolOrchestrator};
use aml_three::{
    DEFAULT_PAYPAY_PORT, MembershipSecurityProfile, MetricsReport, MetricsRunRequest,
    PaypayLookupRequest, PaypayLookupResponse, PaypayMembershipRequest, PaypayMembershipResponse,
    PaypayShuffleRequest, PaypayShuffleResponse, PaypayThresholdRequest, PaypayThresholdResponse,
    PaypayTodoCommitRequest, PaypayTodoCommitResponse, WalletSetupResponse, load_dotenv_defaults,
};
use axum::{
    Json, Router,
    extract::State,
    routing::{get, post},
};
use rand::rngs::OsRng;
use tokio::sync::RwLock;
use tower_http::trace::TraceLayer;
use uuid::Uuid;

type SharedState = Arc<RwLock<PaypayState>>;

struct PaypayState {
    demo: DemoServerState,
    todo_log: Vec<PaypayTodoCommitRequest>,
    latest_metrics: Option<MetricsReport>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    load_dotenv_defaults(".env")?;
    tracing_subscriber::fmt::init();
    let state = Arc::new(RwLock::new(PaypayState {
        demo: seeded_demo_state(),
        todo_log: Vec::new(),
        latest_metrics: None,
    }));
    let app = Router::new()
        .route("/api/v1/paypay/health", get(health))
        .route("/api/v1/paypay/demo/setup", post(demo_setup))
        .route("/api/v1/paypay/lookup", post(lookup))
        .route("/api/v1/paypay/proofs/membership", post(membership))
        .route("/api/v1/paypay/proofs/shuffle", post(shuffle))
        .route("/api/v1/paypay/proofs/threshold", post(threshold))
        .route("/api/v1/paypay/todo/commit", post(todo_commit))
        .route("/api/v1/metrics/latest", get(metrics_latest))
        .route("/api/v1/metrics/run", post(metrics_run))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = bind_addr("PAYPAY_PORT", DEFAULT_PAYPAY_PORT)?;
    tracing::info!("PayPay dummy prover listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status":"ok","service":"aml-paypay"}))
}

async fn demo_setup(
    State(state): State<SharedState>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<WalletSetupResponse>, String> {
    let scenario = body
        .get("scenario")
        .and_then(|value| value.as_str())
        .unwrap_or("compliant");
    let (user_key, amount_cents, expected_status) = scenario_defaults(scenario)?;
    let state = state.read().await;
    let user = state
        .demo
        .users
        .get(user_key)
        .ok_or_else(|| format!("unknown scenario user: {user_key}"))?;
    Ok(Json(WalletSetupResponse {
        scenario: scenario.to_string(),
        user_key: user_key.to_string(),
        tx_id_commitment: CommitmentBytes::new(
            IbcScheme::encode(&user.tx_id_commitment).map_err(|e| e.to_string())?,
        ),
        amount_cents,
        expected_status: expected_status.to_string(),
    }))
}

async fn lookup(
    State(state): State<SharedState>,
    Json(body): Json<PaypayLookupRequest>,
) -> Json<PaypayLookupResponse> {
    let state = state.read().await;
    let found = state
        .demo
        .users
        .get(&body.user_key)
        .and_then(|user| IbcScheme::encode(&user.tx_id_commitment).ok())
        .is_some_and(|encoded| encoded == body.tx_id_commitment.0);
    Json(PaypayLookupResponse {
        found,
        gid: if found {
            format!("gid:{}", body.user_key)
        } else {
            "unknown".into()
        },
        user_key: body.user_key,
        tx_id_commitment: body.tx_id_commitment,
    })
}

async fn membership(
    State(state): State<SharedState>,
    Json(body): Json<PaypayMembershipRequest>,
) -> Result<Json<PaypayMembershipResponse>, String> {
    let state = state.read().await;
    let user = state
        .demo
        .users
        .get(&body.user_key)
        .ok_or_else(|| format!("unknown user: {}", body.user_key))?;
    let target = IbcScheme::decode(&body.tx_id_commitment.0).map_err(|e| e.to_string())?;
    let statement = MembershipStatement {
        set_commitments: user.membership_set_commitments.clone(),
        target,
        set_id: body.user_key.clone(),
    };
    let mode = membership_mode_from_env();
    let scheme = MembershipSchemeImpl::for_mode(mode);
    let mut rng = OsRng;
    let mut envelope = scheme
        .prove(&state.demo.pp, &statement, &mut rng)
        .map_err(|e| e.to_string())?;
    let verify_started = Instant::now();
    let ok = scheme
        .verify(&state.demo.pp, &statement, &envelope)
        .map_err(|e| e.to_string())?;
    finalize_membership_envelope(&mut envelope, verify_started.elapsed().as_millis() as u64);
    let set_commitments = user
        .membership_set_commitments
        .iter()
        .map(|commitment| CommitmentBytes::new(IbcScheme::encode(commitment).unwrap_or_default()))
        .collect();
    Ok(Json(PaypayMembershipResponse {
        ok,
        mode: envelope.mode.to_string(),
        set_commitments,
        set_size: user.membership_set_commitments.len(),
        proof_size_bytes: envelope.proof_size_bytes,
        proof_json: envelope.proof_json,
        generation_time_ms: envelope.generation_time_ms,
        verification_time_ms: envelope.verification_time_ms,
        threshold_gen_ms: envelope.threshold_gen_ms,
        threshold_verify_ms: envelope.threshold_verify_ms,
        threshold_size_bytes: envelope.threshold_size_bytes,
        warn_timeout_ms: envelope.warn_timeout_ms,
        hard_timeout_ms: envelope.hard_timeout_ms,
        verdict: envelope.verdict,
        poc_implementation: envelope.implementation,
        security_profile: MembershipSecurityProfile {
            current_primitive: envelope.current_primitive,
            formal_target: envelope.formal_target,
            formal_gap: envelope.formal_gap,
            trust_anchor_current: envelope.trust_anchor_current,
            trust_anchor_ideal: envelope.trust_anchor_ideal,
            claim_level: envelope.claim_level,
        },
    }))
}

async fn shuffle(
    State(state): State<SharedState>,
    Json(body): Json<PaypayShuffleRequest>,
) -> Result<Json<PaypayShuffleResponse>, String> {
    let state = state.read().await;
    let user = state
        .demo
        .users
        .get(&body.user_key)
        .ok_or_else(|| format!("unknown user: {}", body.user_key))?;
    let mut rng = OsRng;
    let started = Instant::now();
    let (proof, _) = VerifiableShuffle::shuffle_and_prove(
        &state.demo.pp,
        &user.commitments,
        &user.openings,
        &mut rng,
    );
    let generation_time_ms = started.elapsed().as_millis() as u64;
    let proof_json = proof.to_json().map_err(|e| e.to_string())?;
    let output_commitments = proof
        .outputs
        .iter()
        .map(|commitment| CommitmentBytes::new(IbcScheme::encode(commitment).unwrap_or_default()))
        .collect();
    Ok(Json(PaypayShuffleResponse {
        ok: VerifiableShuffle::verify(&state.demo.pp, &user.commitments, &proof),
        proof_size_bytes: proof_json.len(),
        proof_json,
        output_commitments,
        generation_time_ms,
    }))
}

async fn threshold(
    State(state): State<SharedState>,
    Json(body): Json<PaypayThresholdRequest>,
) -> Result<Json<PaypayThresholdResponse>, String> {
    let started = Instant::now();
    let mut state = state.write().await;
    let request_id = Uuid::new_v4();
    let request = VerificationRequest {
        request_id,
        tx_id_commitment: body.tx_id_commitment,
        amount_cents: body.amount_cents,
        window_start: SlidingWindow::window_start(&WindowConfig::default(), chrono::Utc::now()),
        window_config: WindowConfig::default(),
    };
    let result = ProtocolOrchestrator::new()
        .verify_payment(&mut state.demo, &body.user_key, request)
        .map_err(|e| e.to_string())?;
    let proof_metadata = result.proof_bundle.clone().into();
    let status = match result.decision {
        VerificationDecision::Approved => VerificationStatus::Approved,
        VerificationDecision::Rejected => VerificationStatus::Rejected,
    };
    let proof_json = serde_json::to_string(&proof_metadata).map_err(|e| e.to_string())?;
    Ok(Json(PaypayThresholdResponse {
        request_id,
        status,
        threshold_compliant: result.threshold_compliant,
        proof_audit_hash: format!("paypay-proof-{request_id}"),
        proof_size_bytes: proof_json.len(),
        proof_metadata,
        generation_time_ms: started.elapsed().as_millis() as u64,
        message: result.message,
    }))
}

async fn todo_commit(
    State(state): State<SharedState>,
    Json(body): Json<PaypayTodoCommitRequest>,
) -> Json<PaypayTodoCommitResponse> {
    let mut state = state.write().await;
    state.todo_log.push(body);
    Json(PaypayTodoCommitResponse {
        committed: true,
        log_size: state.todo_log.len(),
    })
}

async fn metrics_latest(State(state): State<SharedState>) -> Json<Option<MetricsReport>> {
    let state = state.read().await;
    Json(state.latest_metrics.clone())
}

async fn metrics_run(
    State(state): State<SharedState>,
    Json(body): Json<MetricsRunRequest>,
) -> Result<Json<MetricsReport>, String> {
    let runs = body.runs.unwrap_or(100);
    let n = body.n.unwrap_or(100);
    let report = match body.modes.or(body.mode) {
        Some(modes) => aml_metrics::generate_report_for_modes(
            runs,
            n,
            aml_crypto_membership::parse_membership_modes(&modes).map_err(|e| e.to_string())?,
        ),
        None => aml_metrics::generate_report(runs, n),
    }
    .map_err(|e| e.to_string())?;
    let mut state = state.write().await;
    state.latest_metrics = Some(report.clone());
    Ok(Json(report))
}

fn seeded_demo_state() -> DemoServerState {
    let mut demo = DemoServerState::new();
    demo.setup_user(
        "scenario_compliant",
        101,
        &[(500_000, 30), (400_000, 45), (300_000, 90)],
    )
    .expect("seed compliant user");
    demo.setup_user("scenario_exceeded", 102, &[(1_500_000, 10), (400_000, 20)])
        .expect("seed exceeded user");
    demo
}

fn scenario_defaults(scenario: &str) -> Result<(&'static str, u64, &'static str), String> {
    match scenario {
        "compliant" => Ok(("scenario_compliant", 200_000, "APPROVED")),
        "exceeded" => Ok(("scenario_exceeded", 500_000, "REJECTED")),
        _ => Err(format!("unknown scenario: {scenario}")),
    }
}

fn bind_addr(port_env: &str, default_port: u16) -> anyhow::Result<SocketAddr> {
    let port = std::env::var(port_env)
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(default_port);
    Ok(SocketAddr::from(([0, 0, 0, 0], port)))
}
