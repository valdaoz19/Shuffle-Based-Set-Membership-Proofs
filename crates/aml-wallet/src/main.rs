use std::{net::SocketAddr, sync::Arc};

use aml_three::{
    AlipayPaymentRequest, DEFAULT_ALIPAY_PORT, DEFAULT_PAYPAY_PORT, DEFAULT_WALLET_PORT,
    MetricsReport, MetricsRunRequest, ThreeServerDecisionResponse, WalletPaymentRequest,
    WalletSetupResponse, load_dotenv_defaults, post_json,
};
use axum::{
    Json, Router,
    extract::State,
    response::{Html, IntoResponse},
    routing::{get, post},
};
use tokio::sync::RwLock;
use tower_http::trace::TraceLayer;

type SharedState = Arc<RwLock<WalletState>>;

#[derive(Clone)]
struct WalletState {
    alipay_base_url: String,
    paypay_base_url: String,
    latest_result: Option<ThreeServerDecisionResponse>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    load_dotenv_defaults(".env")?;
    tracing_subscriber::fmt::init();
    let alipay_base_url = std::env::var("ALIPAY_BASE_URL")
        .unwrap_or_else(|_| format!("http://localhost:{DEFAULT_ALIPAY_PORT}"));
    let paypay_base_url = std::env::var("PAYPAY_BASE_URL")
        .unwrap_or_else(|_| format!("http://localhost:{DEFAULT_PAYPAY_PORT}"));
    let state = Arc::new(RwLock::new(WalletState {
        alipay_base_url,
        paypay_base_url,
        latest_result: None,
    }));
    let app = Router::new()
        .route("/", get(index))
        .route("/api/v1/wallet/health", get(health))
        .route("/api/v1/wallet/payments", post(payments))
        .route("/api/v1/wallet/metrics/run", post(metrics_run))
        .route("/api/v1/wallet/result/latest", get(result_latest))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = bind_addr("WALLET_PORT", DEFAULT_WALLET_PORT)?;
    tracing::info!("Wallet dummy owner listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status":"ok","service":"aml-wallet"}))
}

async fn payments(
    State(state): State<SharedState>,
    Json(body): Json<WalletPaymentRequest>,
) -> Result<Json<ThreeServerDecisionResponse>, WalletError> {
    let (alipay_base, paypay_base) = {
        let state = state.read().await;
        (state.alipay_base_url.clone(), state.paypay_base_url.clone())
    };

    let setup: WalletSetupResponse = post_json(
        &paypay_base,
        "/api/v1/paypay/demo/setup",
        &serde_json::json!({ "scenario": body.scenario }),
    )?;
    let tx_id_commitment = body.tx_id_commitment.unwrap_or(setup.tx_id_commitment);
    let decision: ThreeServerDecisionResponse = post_json(
        &alipay_base,
        "/api/v1/payments",
        &AlipayPaymentRequest {
            amount_cents: body.amount_cents,
            scenario: setup.scenario,
            user_key: setup.user_key,
            tx_id_commitment,
        },
    )?;

    let mut state = state.write().await;
    state.latest_result = Some(decision.clone());
    Ok(Json(decision))
}

async fn result_latest(
    State(state): State<SharedState>,
) -> Json<Option<ThreeServerDecisionResponse>> {
    let state = state.read().await;
    Json(state.latest_result.clone())
}

async fn metrics_run(
    State(state): State<SharedState>,
    Json(body): Json<MetricsRunRequest>,
) -> Result<Json<MetricsReport>, WalletError> {
    let alipay_base = {
        let state = state.read().await;
        state.alipay_base_url.clone()
    };
    let report: MetricsReport = post_json(&alipay_base, "/api/v1/metrics/run", &body)?;
    Ok(Json(report))
}

#[derive(Debug)]
struct WalletError(anyhow::Error);

impl<E> From<E> for WalletError
where
    E: Into<anyhow::Error>,
{
    fn from(value: E) -> Self {
        Self(value.into())
    }
}

impl IntoResponse for WalletError {
    fn into_response(self) -> axum::response::Response {
        (
            axum::http::StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({
                "error": self.0.to_string()
            })),
        )
            .into_response()
    }
}

fn bind_addr(port_env: &str, default_port: u16) -> anyhow::Result<SocketAddr> {
    let port = std::env::var(port_env)
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(default_port);
    Ok(SocketAddr::from(([0, 0, 0, 0], port)))
}

const INDEX_HTML: &str = r##"<!doctype html>
<html lang="id">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Wallet Owner - Three Server ZKP AML Demo</title>
  <style>
    :root { color-scheme: light; font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; background:#f7f8fa; color:#1d2733; }
    body { margin:0; }
    main { max-width:1120px; margin:0 auto; padding:28px 20px 48px; }
    header { display:flex; align-items:flex-end; justify-content:space-between; gap:16px; margin-bottom:20px; }
    h1 { font-size:28px; line-height:1.15; margin:0; }
    .subtle { color:#5c6673; font-size:14px; }
    .grid { display:grid; grid-template-columns:340px 1fr; gap:18px; align-items:start; }
    .stack { display:grid; gap:18px; }
    section, form { background:#fff; border:1px solid #d9dee7; border-radius:8px; padding:18px; box-shadow:0 1px 2px rgba(16,24,40,.04); }
    label { display:block; font-size:13px; font-weight:650; margin:14px 0 6px; }
    select, input { width:100%; box-sizing:border-box; border:1px solid #b9c2d0; border-radius:6px; padding:10px 11px; font:inherit; }
    button { margin-top:16px; width:100%; border:0; border-radius:6px; padding:11px 14px; background:#1769aa; color:white; font-weight:700; cursor:pointer; }
    button:disabled { opacity:.65; cursor:wait; }
    .status { display:inline-flex; align-items:center; min-height:28px; padding:0 10px; border-radius:999px; font-weight:800; font-size:13px; }
    .APPROVED { color:#08603b; background:#daf5e8; }
    .REJECTED { color:#8a1f11; background:#ffe1da; }
    .trace { display:grid; gap:10px; margin-top:14px; }
    .step { border:1px solid #e4e8ef; border-radius:6px; padding:10px 12px; display:grid; grid-template-columns:96px 1fr 72px; gap:10px; align-items:center; }
    .actor { font-weight:750; }
    .metric-table { width:100%; border-collapse:collapse; font-size:13px; margin-top:14px; }
    .comparison-table { width:100%; border-collapse:collapse; font-size:13px; margin-top:12px; }
    .raw-title { display:none; margin:16px 0 8px; font-size:13px; font-weight:750; color:#344054; }
    th, td { border-bottom:1px solid #e4e8ef; padding:8px; text-align:left; vertical-align:top; }
    th:nth-child(2), td:nth-child(2) { text-align:right; }
    .comparison-table th:nth-child(2), .comparison-table td:nth-child(2),
    .comparison-table th:nth-child(3), .comparison-table td:nth-child(3),
    .comparison-table th:nth-child(4), .comparison-table td:nth-child(4) { text-align:right; }
    .pill { display:inline-flex; align-items:center; min-height:24px; padding:0 8px; border-radius:999px; background:#eef2f7; color:#344054; font-weight:750; font-size:12px; }
    .pass { color:#08603b; background:#daf5e8; }
    .fail, .fail_timeout { color:#8a1f11; background:#ffe1da; }
    .pass_warn, .scale_report { color:#7a4d00; background:#fff0c2; }
    .not_comparable { color:#344054; background:#eef2f7; }
    .profile { margin:14px 0; padding:12px; border:1px solid #d7dee8; border-radius:6px; background:#fbfcfe; }
    .profile-grid { display:grid; grid-template-columns:160px 1fr; gap:8px 12px; margin-top:10px; }
    .profile-key { font-weight:700; color:#344054; }
    pre { white-space:pre-wrap; background:#111827; color:#f9fafb; padding:14px; border-radius:6px; overflow:auto; max-height:260px; }
    @media (max-width: 820px) { .grid { grid-template-columns:1fr; } header { display:block; } .step, .profile-grid { grid-template-columns:1fr; } }
  </style>
</head>
<body>
  <main>
    <header>
      <div>
        <h1>User Wallet Owner</h1>
        <div class="subtle">Dummy integration: Wallet -> Alipay+ verifier -> PayPay prover</div>
      </div>
      <div class="subtle">Wallet :8891 | Alipay+ :8892 | PayPay :8893</div>
    </header>
    <div class="grid">
      <form id="payment-form">
        <label for="scenario">Scenario</label>
        <select id="scenario">
          <option value="compliant">Compliant - expected APPROVED</option>
          <option value="exceeded">Exceeded - expected REJECTED</option>
        </select>
        <label for="amount">Amount cents</label>
        <input id="amount" type="number" min="1" step="1" value="200000" />
        <label for="tidc">Transaction ID commitment override</label>
        <input id="tidc" placeholder="optional, normally generated by PayPay setup" />
        <button id="submit" type="submit">Run Payment Flow</button>
        <p class="subtle">Default amount follows the selected research scenario. The override field is only for replay/negative demos.</p>
      </form>
      <div class="stack">
        <section>
          <div id="summary" class="subtle">No payment flow has run yet.</div>
          <div id="security-profile" class="profile" style="display:none"></div>
          <div id="trace" class="trace"></div>
          <table id="metrics" class="metric-table" style="display:none">
            <thead><tr><th>Metric</th><th>Measured</th><th>Target</th><th>Verdict</th></tr></thead>
            <tbody></tbody>
          </table>
          <div id="raw-title" class="raw-title">Raw ThreeServerDecisionResponse payload</div>
          <pre id="raw" style="display:none"></pre>
        </section>
        <section>
          <div style="display:flex;justify-content:space-between;gap:12px;align-items:center">
            <div>
              <strong>Set Membership Mode Comparison</strong>
              <div class="subtle">Runs all membership modes with the same dummy dataset via Alipay+ metrics endpoint.</div>
            </div>
            <span class="pill">n=100</span>
          </div>
          <button id="run-comparison" type="button">Run Membership Comparison</button>
          <table id="comparison" class="comparison-table" style="display:none">
            <thead><tr><th>Mode</th><th>Gen ms</th><th>Verify ms</th><th>Size</th><th>Targets</th><th>Verdict</th><th>Formal Target</th></tr></thead>
            <tbody></tbody>
          </table>
          <div id="comparison-raw-title" class="raw-title">Raw MetricsReport payload</div>
          <pre id="comparison-raw" style="display:none"></pre>
        </section>
      </div>
    </div>
  </main>
  <script>
    const scenario = document.querySelector("#scenario");
    const amount = document.querySelector("#amount");
    const form = document.querySelector("#payment-form");
    const submit = document.querySelector("#submit");
    const summary = document.querySelector("#summary");
    const securityProfile = document.querySelector("#security-profile");
    const trace = document.querySelector("#trace");
    const metrics = document.querySelector("#metrics");
    const metricsBody = document.querySelector("#metrics tbody");
    const rawTitle = document.querySelector("#raw-title");
    const raw = document.querySelector("#raw");
    const runComparison = document.querySelector("#run-comparison");
    const comparison = document.querySelector("#comparison");
    const comparisonBody = document.querySelector("#comparison tbody");
    const comparisonRawTitle = document.querySelector("#comparison-raw-title");
    const comparisonRaw = document.querySelector("#comparison-raw");
    scenario.addEventListener("change", () => {
      amount.value = scenario.value === "compliant" ? "200000" : "500000";
    });
    form.addEventListener("submit", async (event) => {
      event.preventDefault();
      submit.disabled = true;
      summary.textContent = "Running three-server flow...";
      trace.innerHTML = "";
      metrics.style.display = "none";
      rawTitle.style.display = "none";
      raw.style.display = "none";
      securityProfile.style.display = "none";
      const tidc = document.querySelector("#tidc").value.trim();
      try {
        const response = await fetch("/api/v1/wallet/payments", {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({
            scenario: scenario.value,
            amount_cents: Number(amount.value),
            tx_id_commitment: tidc ? tidc : null
          })
        });
        const data = await response.json();
        if (!response.ok) throw new Error(data.error || "payment failed");
        summary.innerHTML = `<span class="status ${data.status}">${data.status}</span> request_id ${data.request_id}<br><span class="subtle">${data.message}</span>`;
        securityProfile.innerHTML = renderSecurityProfile(data.membership_security_profile);
        securityProfile.style.display = "";
        trace.innerHTML = data.trace.map((step) => `<div class="step"><div class="actor">${step.actor}</div><div>${step.action}</div><div>${step.elapsed_ms} ms</div></div>`).join("");
        metricsBody.innerHTML = data.metrics.map((metric) => `<tr><td>${metric.name}</td><td>${formatNumber(metric.measured_value)} ${metric.unit}</td><td>${metric.paper_target}</td><td>${verdictPill(metric.pass_fail_or_not_comparable)}</td></tr>`).join("");
        metrics.style.display = "";
        rawTitle.style.display = "block";
        raw.textContent = JSON.stringify(data, null, 2);
        raw.style.display = "";
      } catch (error) {
        summary.textContent = error.message;
      } finally {
        submit.disabled = false;
      }
    });
    runComparison.addEventListener("click", async () => {
      runComparison.disabled = true;
      comparison.style.display = "none";
      comparisonRawTitle.style.display = "none";
      comparisonRaw.style.display = "none";
      try {
        const response = await fetch("/api/v1/wallet/metrics/run", {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({ runs: 100, n: 100, modes: "explicit,implicit,hybrid" })
        });
        const data = await response.json();
        if (!response.ok) throw new Error(data.error || "metrics failed");
        comparisonBody.innerHTML = data.membership_comparison.map((row) => `<tr><td><strong>${row.mode}</strong><br><span class="subtle">${row.current_primitive}</span></td><td>${formatNumber(row.proof_generation_ms)}</td><td>${formatNumber(row.verification_ms)}</td><td>${row.proof_size_bytes} bytes</td><td>gen <= ${row.generation_target_ms} ms<br>verify <= ${row.verification_target_ms} ms<br>size <= ${row.size_target_bytes} bytes</td><td>${verdictPill(row.verdict)}<br><span class="subtle">${row.claim_level}</span></td><td>${row.formal_target}<br><span class="subtle">${row.formal_gap}</span></td></tr>`).join("");
        comparison.style.display = "";
        comparisonRawTitle.style.display = "block";
        comparisonRaw.textContent = JSON.stringify(data, null, 2);
        comparisonRaw.style.display = "";
      } catch (error) {
        comparisonBody.innerHTML = `<tr><td colspan="7">${error.message}</td></tr>`;
        comparison.style.display = "";
      } finally {
        runComparison.disabled = false;
      }
    });
    function formatNumber(value) {
      return Number(value).toFixed(3);
    }
    function verdictPill(verdict) {
      return `<span class="pill ${verdict}">${verdict}</span>`;
    }
    function renderSecurityProfile(profile) {
      if (!profile) return "";
      return `<strong>Set Membership Security Profile</strong>
        <div class="profile-grid">
          <div class="profile-key">Claim level</div><div>${profile.claim_level}</div>
          <div class="profile-key">Current primitive</div><div>${profile.current_primitive}</div>
          <div class="profile-key">Formal target</div><div>${profile.formal_target}</div>
          <div class="profile-key">Formal gap</div><div>${profile.formal_gap}</div>
          <div class="profile-key">Current trust</div><div>${profile.trust_anchor_current}</div>
          <div class="profile-key">Ideal trust</div><div>${profile.trust_anchor_ideal}</div>
        </div>`;
    }
  </script>
</body>
</html>"##;
