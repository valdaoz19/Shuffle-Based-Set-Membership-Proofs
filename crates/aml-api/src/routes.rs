use aml_core::VerifyApiResponse;
use aml_protocol::ProtocolOrchestrator;
use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
};

use crate::audit::AuditInsert;
use crate::demo::{ScenarioInfo, all_scenarios, format_sgd, resolve_scenario};
use crate::state::{
    ConfigResponse, HealthResponse, ScenariosResponse, SetupBody, SetupResponse, SharedState,
    VerifyBody,
};

pub fn api_routes() -> Router<SharedState> {
    Router::new()
        .route("/api/v1/health", get(health))
        .route("/api/v1/config", get(config))
        .route("/api/v1/demo/scenarios", get(demo_scenarios))
        .route("/api/v1/demo/setup", post(demo_setup))
        .route("/api/v1/verify", post(verify))
        .route("/api/v1/proofs/{request_id}", get(get_proof))
        .route("/api/v1/proofs/{request_id}/verify", post(verify_proof))
        .route("/api/v1/ledger/commitments", get(commitment_ledger))
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: "aml-zkp-poc",
    })
}

async fn config(State(state): State<SharedState>) -> Json<ConfigResponse> {
    let state = state.read().await;
    Json(ConfigResponse::from_state(&state))
}

async fn demo_scenarios(State(state): State<SharedState>) -> Json<ScenariosResponse> {
    let state = state.read().await;
    let threshold_cents = state.config.window.threshold_cents;
    Json(ScenariosResponse {
        threshold_display: format_sgd(threshold_cents),
        threshold_cents,
        scenarios: all_scenarios()
            .into_iter()
            .map(ScenarioInfo::from)
            .collect(),
    })
}

async fn demo_setup(
    State(state): State<SharedState>,
    body: Option<Json<SetupBody>>,
) -> Result<Json<SetupResponse>, ApiError> {
    let state = state.read().await;
    let scenario_id = body
        .and_then(|Json(b)| b.scenario)
        .unwrap_or_else(|| state.default_scenario.id.to_string());

    let scenario = resolve_scenario(&scenario_id).ok_or_else(|| {
        ApiError::bad_request(format!(
            "Unknown scenario '{scenario_id}'. Use 'compliant' or 'exceeded'."
        ))
    })?;

    Ok(Json(state.setup_for_scenario(&scenario)))
}

async fn verify(
    headers: HeaderMap,
    State(state): State<SharedState>,
    Json(body): Json<VerifyBody>,
) -> Result<Json<VerifyApiResponse>, ApiError> {
    enforce_api_access(&state, &headers).await?;

    let scenario = body.scenario.clone();
    let amount_cents = body.amount_cents;

    let (user_key, request, audit, threshold_cents, window_days) = {
        let s = state.read().await;
        if body.amount_cents == 0 {
            return Err(ApiError::bad_request(
                "amount_cents must be greater than zero".to_string(),
            ));
        }
        if body.amount_cents > s.config.max_tx_cents {
            return Err(ApiError::bad_request(format!(
                "amount_cents exceeds max_tx_cents ({})",
                s.config.max_tx_cents
            )));
        }
        let user_key = body.resolve_user_key(&s);
        if !s.demo.users.contains_key(&user_key) {
            return Err(ApiError::bad_request(format!(
                "Unknown demo user '{user_key}'"
            )));
        }
        let request = body.into_request(&s, &user_key);
        (
            user_key,
            request,
            s.audit.clone(),
            s.config.window.threshold_cents,
            s.config.window.window_days,
        )
    };

    let replay_warning = audit
        .has_seen_tx_commitment(&request.tx_id_commitment)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let tx_id_commitment = request.tx_id_commitment.clone();

    let result = {
        let mut s = state.write().await;
        ProtocolOrchestrator::new()
            .verify_payment(&mut s.demo, &user_key, request)
            .map_err(|e| ApiError::bad_request(e.to_string()))?
    };

    let mut response = VerifyApiResponse::from_result(result);
    response.replay_warning = replay_warning;
    if replay_warning {
        response.message = format!(
            "{} Replay warning: transaction ID commitment has been seen before.",
            response.message
        );
    }

    match audit
        .record_verification(AuditInsert {
            user_key,
            scenario,
            amount_cents,
            tx_id_commitment,
            threshold_cents,
            window_days,
            response: response.clone(),
        })
        .await
    {
        Ok(hash) => {
            response.proof_audit_hash = Some(hash);
            response.audit_recorded = true;
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to record audit proof");
            response.audit_recorded = false;
        }
    }

    Ok(Json(response))
}

async fn get_proof(
    headers: HeaderMap,
    State(state): State<SharedState>,
    Path(request_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    enforce_api_access(&state, &headers).await?;

    let audit = {
        let state = state.read().await;
        state.audit.clone()
    };
    let Some(record) = audit
        .get_proof(&request_id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
    else {
        return Err(ApiError::not_found(format!(
            "No audit proof found for request_id '{request_id}'"
        )));
    };
    Ok(Json(record))
}

async fn verify_proof(
    headers: HeaderMap,
    State(state): State<SharedState>,
    Path(request_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    enforce_api_access(&state, &headers).await?;

    let audit = {
        let state = state.read().await;
        state.audit.clone()
    };
    let Some(result) = audit
        .verify_proof_hash(&request_id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
    else {
        return Err(ApiError::not_found(format!(
            "No audit proof found for request_id '{request_id}'"
        )));
    };
    Ok(Json(result))
}

async fn commitment_ledger(
    headers: HeaderMap,
    State(state): State<SharedState>,
) -> Result<impl IntoResponse, ApiError> {
    enforce_api_access(&state, &headers).await?;

    let audit = {
        let state = state.read().await;
        state.audit.clone()
    };
    let ledger = audit
        .list_commitments()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(ledger))
}

async fn enforce_api_access(state: &SharedState, headers: &HeaderMap) -> Result<(), ApiError> {
    let (security, rate_limiter) = {
        let state = state.read().await;
        (state.security.clone(), state.rate_limiter.clone())
    };

    let api_key = headers
        .get("x-api-key")
        .and_then(|value| value.to_str().ok());
    if !security.accepts_api_key(api_key) {
        return Err(ApiError::unauthorized(
            "Missing or invalid x-api-key header".to_string(),
        ));
    }

    if !rate_limiter.allow().await {
        return Err(ApiError::too_many_requests(
            "Rate limit exceeded; retry later.".to_string(),
        ));
    }

    Ok(())
}

struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad_request(message: String) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message,
        }
    }

    fn unauthorized(message: String) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message,
        }
    }

    fn too_many_requests(message: String) -> Self {
        Self {
            status: StatusCode::TOO_MANY_REQUESTS,
            message,
        }
    }

    fn not_found(message: String) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message,
        }
    }

    fn internal(message: String) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let body = serde_json::json!({ "error": self.message });
        (self.status, Json(body)).into_response()
    }
}
