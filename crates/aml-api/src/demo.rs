//! Built-in demo scenarios for presentations (no code changes needed at demo time).

use aml_core::CommitmentBytes;
use aml_protocol::DemoServerState;

pub const SCENARIO_COMPLIANT: &str = "compliant";
pub const SCENARIO_EXCEEDED: &str = "exceeded";

pub const USER_COMPLIANT: &str = "scenario_compliant";
pub const USER_EXCEEDED: &str = "scenario_exceeded";

/// Suggested payment for compliant scenario (SGD 2,000).
pub const COMPLIANT_AMOUNT_CENTS: u64 = 200_000;
/// Suggested payment for exceeded scenario (SGD 5,000).
pub const EXCEEDED_AMOUNT_CENTS: u64 = 500_000;

/// Historical spend in sliding window for compliant user (SGD 12,000).
pub const COMPLIANT_HISTORY_CENTS: u64 = 1_200_000;
/// Historical spend in sliding window for exceeded user (SGD 19,000).
pub const EXCEEDED_HISTORY_CENTS: u64 = 1_900_000;

#[derive(Debug, Clone)]
pub struct DemoScenario {
    pub id: &'static str,
    pub user_key: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub expected_status: &'static str,
    pub suggested_amount_cents: u64,
}

pub fn all_scenarios() -> Vec<DemoScenario> {
    vec![scenario_compliant(), scenario_exceeded()]
}

pub fn scenario_compliant() -> DemoScenario {
    DemoScenario {
        id: SCENARIO_COMPLIANT,
        user_key: USER_COMPLIANT,
        label: "Compliant — within AML limit",
        description: "Turis Singapore: riwayat SGD 12,000 + transaksi baru SGD 2,000 = SGD 14,000 (< limit SGD 20,000).",
        expected_status: "APPROVED",
        suggested_amount_cents: COMPLIANT_AMOUNT_CENTS,
    }
}

pub fn scenario_exceeded() -> DemoScenario {
    DemoScenario {
        id: SCENARIO_EXCEEDED,
        user_key: USER_EXCEEDED,
        label: "Exceeded — over AML limit",
        description: "Turis Singapore: riwayat SGD 19,000 + transaksi baru SGD 5,000 = SGD 24,000 (> limit SGD 20,000).",
        expected_status: "REJECTED",
        suggested_amount_cents: EXCEEDED_AMOUNT_CENTS,
    }
}

pub fn resolve_scenario(id: &str) -> Option<DemoScenario> {
    match id {
        SCENARIO_COMPLIANT | "approved" | "approve" => Some(scenario_compliant()),
        SCENARIO_EXCEEDED | "rejected" | "reject" => Some(scenario_exceeded()),
        _ => None,
    }
}

pub fn seed_demo_users(demo: &mut DemoServerState) {
    // Compliant: 5k + 4k + 3k SGD history = 12k in window
    demo.setup_user(
        USER_COMPLIANT,
        101,
        &[(500_000, 30), (400_000, 45), (300_000, 90)],
    )
    .expect("compliant demo user");

    // Exceeded: 15k + 4k SGD history = 19k in window; +5k new tx exceeds 20k limit
    demo.setup_user(USER_EXCEEDED, 102, &[(1_500_000, 10), (400_000, 20)])
        .expect("exceeded demo user");
}

pub fn tx_id_for_user(demo: &DemoServerState, user_key: &str) -> Option<CommitmentBytes> {
    let user = demo.users.get(user_key)?;
    aml_crypto_ibc::IbcScheme::encode(&user.tx_id_commitment)
        .ok()
        .map(CommitmentBytes::new)
}

pub fn history_cents_for(scenario: &DemoScenario) -> u64 {
    match scenario.id {
        SCENARIO_COMPLIANT => COMPLIANT_HISTORY_CENTS,
        SCENARIO_EXCEEDED => EXCEEDED_HISTORY_CENTS,
        _ => 0,
    }
}

pub fn format_sgd(cents: u64) -> String {
    format!("SGD {:.2}", cents as f64 / 100.0)
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ScenarioInfo {
    pub id: String,
    pub user_key: String,
    pub label: String,
    pub description: String,
    pub expected_status: String,
    pub suggested_amount_cents: u64,
    pub suggested_amount_display: String,
    pub history_cents: u64,
    pub history_display: String,
    pub projected_total_cents: u64,
    pub projected_total_display: String,
}

impl From<DemoScenario> for ScenarioInfo {
    fn from(s: DemoScenario) -> Self {
        let history_cents = history_cents_for(&s);
        let projected_total_cents = history_cents + s.suggested_amount_cents;
        Self {
            id: s.id.to_string(),
            user_key: s.user_key.to_string(),
            label: s.label.to_string(),
            description: s.description.to_string(),
            expected_status: s.expected_status.to_string(),
            suggested_amount_cents: s.suggested_amount_cents,
            suggested_amount_display: format_sgd(s.suggested_amount_cents),
            history_cents,
            history_display: format_sgd(history_cents),
            projected_total_cents,
            projected_total_display: format_sgd(projected_total_cents),
        }
    }
}
