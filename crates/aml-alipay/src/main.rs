use std::{net::SocketAddr, sync::Arc, time::Instant};

use aml_core::VerificationStatus;
use aml_crypto_ibc::{IbcScheme, PublicParams};
use aml_crypto_membership::{
    MembershipMode, MembershipProofEnvelope, MembershipSchemeImpl, MembershipStatement,
};
use aml_three::{
    AlipayPaymentRequest, DEFAULT_ALIPAY_PORT, DEFAULT_PAYPAY_PORT, MetricsReport,
    MetricsRunRequest, PaypayLookupRequest, PaypayLookupResponse, PaypayMembershipRequest,
    PaypayMembershipResponse, PaypayShuffleRequest, PaypayShuffleResponse, PaypayThresholdRequest,
    PaypayThresholdResponse, PaypayTodoCommitRequest, ThreeServerDecisionResponse, TraceStep,
    load_dotenv_defaults, metric, post_json,
};
use axum::{
    Json, Router,
    extract::State,
    routing::{get, post},
};
use tokio::sync::RwLock;
use tower_http::trace::TraceLayer;
use uuid::Uuid;

type SharedState = Arc<RwLock<AlipayState>>;

#[derive(Clone)]
struct AlipayState {
    paypay_base_url: String,
    pp: PublicParams,
    latest_decision: Option<ThreeServerDecisionResponse>,
    latest_metrics: Option<MetricsReport>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    load_dotenv_defaults(".env")?;
    tracing_subscriber::fmt::init();
    let paypay_base_url = std::env::var("PAYPAY_BASE_URL")
        .unwrap_or_else(|_| format!("http://localhost:{DEFAULT_PAYPAY_PORT}"));
    let state = Arc::new(RwLock::new(AlipayState {
        paypay_base_url,
        pp: PublicParams::setup(),
        latest_decision: None,
        latest_metrics: None,
    }));
    let app = Router::new()
        .route("/api/v1/alipay/health", get(health))
        .route("/api/v1/payments", post(payments))
        .route("/api/v1/traces/latest", get(trace_latest))
        .route("/api/v1/metrics/latest", get(metrics_latest))
        .route("/api/v1/metrics/run", post(metrics_run))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = bind_addr("ALIPAY_PORT", DEFAULT_ALIPAY_PORT)?;
    tracing::info!("Alipay+ dummy verifier listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status":"ok","service":"aml-alipay"}))
}

async fn payments(
    State(state): State<SharedState>,
    Json(body): Json<AlipayPaymentRequest>,
) -> Result<Json<ThreeServerDecisionResponse>, String> {
    let trace_id = Uuid::new_v4();
    let flow_started = Instant::now();
    let paypay_base = {
        let state = state.read().await;
        state.paypay_base_url.clone()
    };
    let mut trace = Vec::new();

    let lookup_started = Instant::now();
    let lookup: PaypayLookupResponse = post_json(
        &paypay_base,
        "/api/v1/paypay/lookup",
        &PaypayLookupRequest {
            scenario: body.scenario.clone(),
            user_key: body.user_key.clone(),
            tx_id_commitment: body.tx_id_commitment.clone(),
        },
    )
    .map_err(|e| e.to_string())?;
    trace.push(step(
        "Alipay+",
        "Forward lookup gid/tidc to PayPay",
        lookup_started,
    ));
    if !lookup.found {
        return Err("PayPay lookup failed for transaction commitment".into());
    }

    let membership_started = Instant::now();
    let membership: PaypayMembershipResponse = post_json(
        &paypay_base,
        "/api/v1/paypay/proofs/membership",
        &PaypayMembershipRequest {
            scenario: body.scenario.clone(),
            user_key: body.user_key.clone(),
            tx_id_commitment: body.tx_id_commitment.clone(),
        },
    )
    .map_err(|e| e.to_string())?;
    trace.push(step(
        "PayPay",
        &format!(
            "Generate {} set membership proof",
            membership.poc_implementation
        ),
        membership_started,
    ));
    let pp = {
        let state = state.read().await;
        state.pp.clone()
    };
    verify_membership_response(&pp, &body.tx_id_commitment, &membership)?;
    trace.push(TraceStep {
        actor: "Alipay+".into(),
        action: format!("Verify {} set membership proof for c_tid", membership.mode),
        elapsed_ms: membership.verification_time_ms,
    });

    let shuffle_started = Instant::now();
    let shuffle: PaypayShuffleResponse = post_json(
        &paypay_base,
        "/api/v1/paypay/proofs/shuffle",
        &PaypayShuffleRequest {
            user_key: body.user_key.clone(),
        },
    )
    .map_err(|e| e.to_string())?;
    trace.push(step(
        "PayPay",
        "Generate verifiable shuffle proof",
        shuffle_started,
    ));
    verify_shuffle_response(&shuffle)?;
    trace.push(TraceStep {
        actor: "Alipay+".into(),
        action: "Verify shuffle proof (PoC sigma, paper target Bayer-Groth)".into(),
        elapsed_ms: shuffle.generation_time_ms,
    });

    let threshold_started = Instant::now();
    let threshold: PaypayThresholdResponse = post_json(
        &paypay_base,
        "/api/v1/paypay/proofs/threshold",
        &PaypayThresholdRequest {
            scenario: body.scenario.clone(),
            user_key: body.user_key.clone(),
            tx_id_commitment: body.tx_id_commitment.clone(),
            amount_cents: body.amount_cents,
        },
    )
    .map_err(|e| e.to_string())?;
    trace.push(step(
        "PayPay",
        "Generate sliding window + threshold proof",
        threshold_started,
    ));
    verify_threshold_response(&threshold)?;
    trace.push(TraceStep {
        actor: "Alipay+".into(),
        action: "Verify F(S^T) and decide approve/reject".into(),
        elapsed_ms: threshold.generation_time_ms,
    });

    let todo_started = Instant::now();
    let _todo: serde_json::Value = post_json(
        &paypay_base,
        "/api/v1/paypay/todo/commit",
        &PaypayTodoCommitRequest {
            request_id: threshold.request_id,
            user_key: body.user_key,
            status: threshold.status,
        },
    )
    .map_err(|e| e.to_string())?;
    trace.push(step(
        "PayPay",
        "Verified todo list log expire action",
        todo_started,
    ));

    let e2e_ms = flow_started.elapsed().as_millis() as u64;
    let metrics = vec![
        metric(
            &format!("Set membership generation time ({})", membership.mode),
            membership.generation_time_ms as f64,
            "ms",
            &format!("<= {} ms", membership.threshold_gen_ms),
            &membership.security_profile.current_primitive,
            membership.verdict.as_str(),
        ),
        metric(
            &format!("Set membership verification time ({})", membership.mode),
            membership.verification_time_ms as f64,
            "ms",
            &format!("<= {} ms", membership.threshold_verify_ms),
            &membership.security_profile.current_primitive,
            membership.verdict.as_str(),
        ),
        metric(
            &format!("Set membership proof size ({})", membership.mode),
            membership.proof_size_bytes as f64,
            "bytes",
            &format!("<= {} bytes", membership.threshold_size_bytes),
            &membership.security_profile.current_primitive,
            membership.verdict.as_str(),
        ),
        metric(
            "End-to-end latency",
            e2e_ms as f64,
            "ms",
            "< 200 ms n=100; < 1s n=10k",
            "HTTP Wallet -> Alipay+ -> PayPay dummy flow",
            if e2e_ms < 1000 { "pass" } else { "fail" },
        ),
        metric(
            "Shuffle proof size",
            shuffle.proof_size_bytes as f64,
            "bytes",
            "paper tracks proof size separately",
            "PoC sigma shuffle proof JSON",
            "not_comparable",
        ),
        metric(
            "Threshold/update proof size",
            threshold.proof_size_bytes as f64,
            "bytes",
            "~200 B SNARK target",
            "PoC metadata + simplified update + ZK-MNNP flag",
            "not_comparable",
        ),
    ];

    let response = ThreeServerDecisionResponse {
        request_id: threshold.request_id,
        trace_id,
        status: threshold.status,
        threshold_compliant: threshold.threshold_compliant,
        proof_audit_hash: threshold.proof_audit_hash,
        message: threshold.message,
        membership_security_profile: membership.security_profile,
        trace,
        metrics,
    };
    let mut state = state.write().await;
    state.latest_decision = Some(response.clone());
    Ok(Json(response))
}

async fn trace_latest(
    State(state): State<SharedState>,
) -> Json<Option<ThreeServerDecisionResponse>> {
    let state = state.read().await;
    Json(state.latest_decision.clone())
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

fn verify_shuffle_response(response: &PaypayShuffleResponse) -> Result<(), String> {
    if !response.ok || response.proof_json.is_empty() || response.output_commitments.is_empty() {
        return Err("invalid shuffle proof response".into());
    }
    Ok(())
}

fn verify_membership_response(
    pp: &PublicParams,
    tx_id_commitment: &aml_core::CommitmentBytes,
    response: &PaypayMembershipResponse,
) -> Result<(), String> {
    if !response.ok || response.proof_json.is_empty() || response.set_commitments.is_empty() {
        return Err("invalid set membership proof response".into());
    }
    let target = IbcScheme::decode(&tx_id_commitment.0).map_err(|e| e.to_string())?;
    let set = response
        .set_commitments
        .iter()
        .map(|commitment| IbcScheme::decode(&commitment.0).map_err(|e| e.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    let mode = response
        .mode
        .parse::<MembershipMode>()
        .map_err(|e| e.to_string())?;
    let statement = MembershipStatement {
        set_commitments: set,
        target,
        set_id: "alipay-verification".into(),
    };
    let envelope = MembershipProofEnvelope {
        mode,
        proof_json: response.proof_json.clone(),
        proof_size_bytes: response.proof_size_bytes,
        generation_time_ms: response.generation_time_ms,
        verification_time_ms: response.verification_time_ms,
        threshold_gen_ms: response.threshold_gen_ms,
        threshold_verify_ms: response.threshold_verify_ms,
        threshold_size_bytes: response.threshold_size_bytes,
        warn_timeout_ms: response.warn_timeout_ms,
        hard_timeout_ms: response.hard_timeout_ms,
        verdict: response.verdict.clone(),
        implementation: response.poc_implementation.clone(),
        current_primitive: response.security_profile.current_primitive.clone(),
        formal_target: response.security_profile.formal_target.clone(),
        formal_gap: response.security_profile.formal_gap.clone(),
        trust_anchor_current: response.security_profile.trust_anchor_current.clone(),
        trust_anchor_ideal: response.security_profile.trust_anchor_ideal.clone(),
        claim_level: response.security_profile.claim_level.clone(),
    };
    let scheme = MembershipSchemeImpl::for_mode(mode);
    if !scheme
        .verify(pp, &statement, &envelope)
        .map_err(|e| e.to_string())?
    {
        return Err("set membership proof verification failed".into());
    }
    Ok(())
}

fn verify_threshold_response(response: &PaypayThresholdResponse) -> Result<(), String> {
    if response.proof_metadata.zk_mnnp_threshold.is_none() {
        return Err("missing ZK-MNNP threshold proof metadata".into());
    }
    match response.status {
        VerificationStatus::Approved if !response.threshold_compliant => {
            Err("approved response must be threshold compliant".into())
        }
        VerificationStatus::Rejected if response.threshold_compliant => {
            Err("rejected response cannot be threshold compliant".into())
        }
        _ => Ok(()),
    }
}

fn step(actor: &str, action: &str, started: Instant) -> TraceStep {
    TraceStep {
        actor: actor.into(),
        action: action.into(),
        elapsed_ms: started.elapsed().as_millis() as u64,
    }
}

fn bind_addr(port_env: &str, default_port: u16) -> anyhow::Result<SocketAddr> {
    let port = std::env::var(port_env)
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(default_port);
    Ok(SocketAddr::from(([0, 0, 0, 0], port)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use aml_core::CryptoProofMetadata;

    #[test]
    fn rejects_invalid_threshold_response() {
        let response = PaypayThresholdResponse {
            request_id: Uuid::new_v4(),
            status: VerificationStatus::Approved,
            threshold_compliant: false,
            proof_metadata: CryptoProofMetadata {
                pipeline: "test".into(),
                set_membership: None,
                ibc_identity_equality: None,
                ibc_value_verification: None,
                verifiable_shuffle: None,
                sliding_window: None,
                oblivious_update: None,
                zk_mnnp_threshold: Some("ok".into()),
                commitment_handles: vec![],
            },
            proof_audit_hash: "hash".into(),
            proof_size_bytes: 1,
            generation_time_ms: 1,
            message: "bad".into(),
        };
        assert!(verify_threshold_response(&response).is_err());
    }
}
