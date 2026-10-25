use std::{
    collections::VecDeque,
    sync::Arc,
    time::{Duration, Instant},
};

use aml_core::{AmlConfig, CommitmentBytes, VerificationRequest};
use aml_protocol::DemoServerState;
use chrono::Utc;
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

use crate::audit::AuditStore;
use crate::demo::{DemoScenario, ScenarioInfo, USER_COMPLIANT, seed_demo_users, tx_id_for_user};

pub struct AppState {
    pub config: AmlConfig,
    pub demo: DemoServerState,
    pub default_scenario: DemoScenario,
    pub audit: AuditStore,
    pub security: SecurityConfig,
    pub rate_limiter: RateLimiter,
}

impl AppState {
    pub async fn new() -> anyhow::Result<Self> {
        let audit = AuditStore::connect_from_env().await?;
        Self::new_with_audit(audit, SecurityConfig::from_env())
    }

    pub async fn new_with_audit_url(database_url: &str) -> anyhow::Result<Self> {
        let audit = AuditStore::connect(database_url).await?;
        Self::new_with_audit(audit, SecurityConfig::disabled())
    }

    pub async fn new_with_audit_url_and_security(
        database_url: &str,
        security: SecurityConfig,
    ) -> anyhow::Result<Self> {
        let audit = AuditStore::connect(database_url).await?;
        Self::new_with_audit(audit, security)
    }

    fn new_with_audit(audit: AuditStore, security: SecurityConfig) -> anyhow::Result<Self> {
        let mut demo = DemoServerState::new();
        seed_demo_users(&mut demo);
        let rate_limiter = RateLimiter::new(security.rate_limit_per_minute);

        Ok(Self {
            config: AmlConfig::default(),
            demo,
            default_scenario: crate::demo::scenario_compliant(),
            audit,
            security,
            rate_limiter,
        })
    }

    pub fn demo_tx_id(&self, user_key: &str) -> Option<CommitmentBytes> {
        tx_id_for_user(&self.demo, user_key)
    }

    pub fn window_start(&self) -> chrono::DateTime<Utc> {
        aml_crypto_window::SlidingWindow::window_start(&self.config.window, Utc::now())
    }

    pub fn setup_for_scenario(&self, scenario: &DemoScenario) -> SetupResponse {
        let history_cents = crate::demo::history_cents_for(scenario);
        let projected_total_cents = history_cents + scenario.suggested_amount_cents;
        SetupResponse {
            scenario: scenario.id.to_string(),
            user_key: scenario.user_key.to_string(),
            tx_id_commitment: self
                .demo_tx_id(scenario.user_key)
                .expect("demo user seeded"),
            threshold_cents: self.config.window.threshold_cents,
            window_days: self.config.window.window_days,
            suggested_amount_cents: scenario.suggested_amount_cents,
            expected_status: scenario.expected_status.to_string(),
            history_cents,
            projected_total_cents,
            message: format!(
                "Scenario '{}' ready. Expected result: {}.",
                scenario.id, scenario.expected_status
            ),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SecurityConfig {
    pub api_key: Option<String>,
    pub rate_limit_per_minute: Option<usize>,
}

impl SecurityConfig {
    pub fn new(api_key: Option<String>, rate_limit_per_minute: Option<usize>) -> Self {
        Self {
            api_key: api_key
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            rate_limit_per_minute: rate_limit_per_minute.filter(|value| *value > 0),
        }
    }

    fn from_env() -> Self {
        let api_key = std::env::var("AML_API_KEY")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let rate_limit_per_minute = std::env::var("AML_RATE_LIMIT_PER_MINUTE")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|value| *value > 0);

        Self {
            api_key,
            rate_limit_per_minute,
        }
    }

    fn disabled() -> Self {
        Self::new(None, None)
    }

    pub fn api_key_enabled(&self) -> bool {
        self.api_key.is_some()
    }

    pub fn accepts_api_key(&self, candidate: Option<&str>) -> bool {
        match &self.api_key {
            Some(expected) => candidate.is_some_and(|actual| actual == expected),
            None => true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RateLimiter {
    max_per_minute: Option<usize>,
    hits: Arc<Mutex<VecDeque<Instant>>>,
}

impl RateLimiter {
    fn new(max_per_minute: Option<usize>) -> Self {
        Self {
            max_per_minute,
            hits: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    pub async fn allow(&self) -> bool {
        let Some(max_per_minute) = self.max_per_minute else {
            return true;
        };

        let cutoff = Instant::now() - Duration::from_secs(60);
        let mut hits = self.hits.lock().await;
        while hits.front().is_some_and(|instant| *instant < cutoff) {
            hits.pop_front();
        }
        if hits.len() >= max_per_minute {
            return false;
        }
        hits.push_back(Instant::now());
        true
    }
}

pub type SharedState = Arc<RwLock<AppState>>;

#[derive(serde::Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub service: &'static str,
}

#[derive(serde::Serialize)]
pub struct SetupResponse {
    pub scenario: String,
    pub user_key: String,
    pub tx_id_commitment: CommitmentBytes,
    pub threshold_cents: u64,
    pub window_days: u32,
    pub suggested_amount_cents: u64,
    pub expected_status: String,
    pub history_cents: u64,
    pub projected_total_cents: u64,
    pub message: String,
}

#[derive(serde::Deserialize, Default)]
pub struct SetupBody {
    pub scenario: Option<String>,
}

#[derive(serde::Serialize)]
pub struct ScenariosResponse {
    pub threshold_cents: u64,
    pub threshold_display: String,
    pub scenarios: Vec<ScenarioInfo>,
}

#[derive(serde::Deserialize)]
pub struct VerifyBody {
    pub amount_cents: u64,
    pub tx_id_commitment: Option<CommitmentBytes>,
    pub user_key: Option<String>,
    pub scenario: Option<String>,
}

impl VerifyBody {
    pub fn resolve_user_key(&self, state: &AppState) -> String {
        if let Some(key) = &self.user_key {
            return key.clone();
        }
        if let Some(id) = &self.scenario
            && let Some(scenario) = crate::demo::resolve_scenario(id)
        {
            return scenario.user_key.to_string();
        }
        state.default_scenario.user_key.to_string()
    }

    pub fn into_request(self, state: &AppState, user_key: &str) -> VerificationRequest {
        VerificationRequest {
            request_id: Uuid::new_v4(),
            tx_id_commitment: self.tx_id_commitment.unwrap_or_else(|| {
                state
                    .demo_tx_id(user_key)
                    .unwrap_or_else(|| state.demo_tx_id(USER_COMPLIANT).unwrap())
            }),
            amount_cents: self.amount_cents,
            window_start: state.window_start(),
            window_config: state.config.window.clone(),
        }
    }
}

#[derive(serde::Serialize)]
pub struct ConfigResponse {
    pub threshold_cents: u64,
    pub threshold_display: String,
    pub window_days: u32,
    pub max_tx_cents: u64,
    pub api_key_required: bool,
    pub rate_limit_per_minute: Option<usize>,
}

impl ConfigResponse {
    pub fn from_state(state: &AppState) -> Self {
        let dollars = state.config.window.threshold_cents as f64 / 100.0;
        Self {
            threshold_cents: state.config.window.threshold_cents,
            threshold_display: format!("SGD {dollars:.2}"),
            window_days: state.config.window.window_days,
            max_tx_cents: state.config.max_tx_cents,
            api_key_required: state.security.api_key_enabled(),
            rate_limit_per_minute: state.security.rate_limit_per_minute,
        }
    }
}
