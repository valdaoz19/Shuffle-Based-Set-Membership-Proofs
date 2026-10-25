use std::{
    fs,
    io::{Read, Write},
    net::TcpStream,
    path::Path,
    time::Duration,
};

use aml_core::{CommitmentBytes, CryptoProofMetadata, VerificationStatus};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use uuid::Uuid;

pub const DEFAULT_WALLET_PORT: u16 = 8891;
pub const DEFAULT_ALIPAY_PORT: u16 = 8892;
pub const DEFAULT_PAYPAY_PORT: u16 = 8893;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WalletPaymentRequest {
    pub amount_cents: u64,
    pub scenario: String,
    pub tx_id_commitment: Option<CommitmentBytes>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WalletSetupResponse {
    pub scenario: String,
    pub user_key: String,
    pub tx_id_commitment: CommitmentBytes,
    pub amount_cents: u64,
    pub expected_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AlipayPaymentRequest {
    pub amount_cents: u64,
    pub scenario: String,
    pub user_key: String,
    pub tx_id_commitment: CommitmentBytes,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaypayLookupRequest {
    pub scenario: String,
    pub user_key: String,
    pub tx_id_commitment: CommitmentBytes,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaypayLookupResponse {
    pub found: bool,
    pub gid: String,
    pub user_key: String,
    pub tx_id_commitment: CommitmentBytes,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaypayMembershipRequest {
    pub scenario: String,
    pub user_key: String,
    pub tx_id_commitment: CommitmentBytes,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaypayMembershipResponse {
    pub ok: bool,
    pub mode: String,
    pub proof_json: String,
    pub set_commitments: Vec<CommitmentBytes>,
    pub set_size: usize,
    pub proof_size_bytes: usize,
    pub generation_time_ms: u64,
    pub verification_time_ms: u64,
    pub threshold_gen_ms: u64,
    pub threshold_verify_ms: u64,
    pub threshold_size_bytes: usize,
    pub warn_timeout_ms: u64,
    pub hard_timeout_ms: u64,
    pub verdict: String,
    pub poc_implementation: String,
    pub security_profile: MembershipSecurityProfile,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaypayShuffleRequest {
    pub user_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaypayShuffleResponse {
    pub ok: bool,
    pub proof_json: String,
    pub output_commitments: Vec<CommitmentBytes>,
    pub proof_size_bytes: usize,
    pub generation_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaypayThresholdRequest {
    pub scenario: String,
    pub user_key: String,
    pub tx_id_commitment: CommitmentBytes,
    pub amount_cents: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaypayThresholdResponse {
    pub request_id: Uuid,
    pub status: VerificationStatus,
    pub threshold_compliant: bool,
    pub proof_metadata: CryptoProofMetadata,
    pub proof_audit_hash: String,
    pub proof_size_bytes: usize,
    pub generation_time_ms: u64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaypayTodoCommitRequest {
    pub request_id: Uuid,
    pub user_key: String,
    pub status: VerificationStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaypayTodoCommitResponse {
    pub committed: bool,
    pub log_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThreeServerDecisionResponse {
    pub request_id: Uuid,
    pub trace_id: Uuid,
    pub status: VerificationStatus,
    pub threshold_compliant: bool,
    pub proof_audit_hash: String,
    pub message: String,
    pub membership_security_profile: MembershipSecurityProfile,
    pub trace: Vec<TraceStep>,
    pub metrics: Vec<MetricResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MembershipSecurityProfile {
    pub current_primitive: String,
    pub formal_target: String,
    pub formal_gap: String,
    pub trust_anchor_current: String,
    pub trust_anchor_ideal: String,
    pub claim_level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TraceStep {
    pub actor: String,
    pub action: String,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MetricResult {
    pub name: String,
    pub category: String,
    pub measured_value: f64,
    pub unit: String,
    pub paper_target: String,
    pub poc_implementation: String,
    pub pass_fail_or_not_comparable: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MetricsReport {
    pub generated_at: DateTime<Utc>,
    pub runs: usize,
    pub n: usize,
    pub metrics: Vec<MetricResult>,
    pub membership_comparison: Vec<MembershipModeMetric>,
    pub markdown: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MetricsRunRequest {
    pub runs: Option<usize>,
    pub n: Option<usize>,
    pub mode: Option<String>,
    pub modes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MembershipModeMetric {
    pub mode: String,
    pub proof_generation_ms: f64,
    pub verification_ms: f64,
    pub proof_size_bytes: usize,
    pub generation_target_ms: u64,
    pub verification_target_ms: u64,
    pub size_target_bytes: usize,
    pub warn_timeout_ms: u64,
    pub hard_timeout_ms: u64,
    pub verdict: String,
    pub poc_implementation: String,
    pub current_primitive: String,
    pub formal_target: String,
    pub formal_gap: String,
    pub claim_level: String,
}

pub fn post_json<T, R>(base_url: &str, path: &str, body: &T) -> anyhow::Result<R>
where
    T: Serialize,
    R: DeserializeOwned,
{
    let body = serde_json::to_vec(body)?;
    let response = request("POST", base_url, path, Some(&body))?;
    Ok(serde_json::from_slice(&response)?)
}

pub fn get_json<R>(base_url: &str, path: &str) -> anyhow::Result<R>
where
    R: DeserializeOwned,
{
    let response = request("GET", base_url, path, None)?;
    Ok(serde_json::from_slice(&response)?)
}

fn request(
    method: &str,
    base_url: &str,
    path: &str,
    body: Option<&[u8]>,
) -> anyhow::Result<Vec<u8>> {
    let (host, port, base_path) = parse_http_url(base_url)?;
    let full_path = format!("{base_path}{path}");
    let mut stream = TcpStream::connect((host.as_str(), port))?;
    stream.set_read_timeout(Some(Duration::from_secs(120)))?;
    stream.set_write_timeout(Some(Duration::from_secs(30)))?;

    let body = body.unwrap_or_default();
    let request = format!(
        "{method} {full_path} HTTP/1.1\r\nHost: {host}:{port}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(request.as_bytes())?;
    stream.write_all(body)?;

    let mut response = Vec::new();
    stream.read_to_end(&mut response)?;
    let split = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| anyhow::anyhow!("invalid HTTP response"))?;
    let (head, body) = response.split_at(split + 4);
    let status_line = std::str::from_utf8(head)?
        .lines()
        .next()
        .ok_or_else(|| anyhow::anyhow!("missing HTTP status"))?;
    if !status_line.contains(" 2") {
        return Err(anyhow::anyhow!(
            "HTTP request failed: {status_line}: {}",
            String::from_utf8_lossy(body)
        ));
    }
    Ok(body.to_vec())
}

fn parse_http_url(url: &str) -> anyhow::Result<(String, u16, String)> {
    let rest = url
        .strip_prefix("http://")
        .ok_or_else(|| anyhow::anyhow!("only http:// URLs are supported: {url}"))?;
    let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
    let (host, port) = if let Some((host, port)) = authority.split_once(':') {
        (host.to_string(), port.parse::<u16>()?)
    } else {
        (authority.to_string(), 80)
    };
    Ok((
        host,
        port,
        format!("/{path}").trim_end_matches('/').to_string(),
    ))
}

pub fn load_dotenv_defaults(path: impl AsRef<Path>) -> anyhow::Result<()> {
    let Ok(contents) = fs::read_to_string(path) else {
        return Ok(());
    };
    for raw_line in contents.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line);
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() || std::env::var_os(key).is_some() {
            continue;
        }
        // SAFETY: This helper is called during process startup before server
        // threads are spawned in the demo binaries.
        unsafe {
            std::env::set_var(key, value.trim().trim_matches('"').trim_matches('\''));
        }
    }
    Ok(())
}

pub fn metric(
    name: &str,
    measured_value: f64,
    unit: &str,
    paper_target: &str,
    implementation: &str,
    verdict: &str,
) -> MetricResult {
    MetricResult {
        name: name.to_string(),
        category: "Performa".to_string(),
        measured_value,
        unit: unit.to_string(),
        paper_target: paper_target.to_string(),
        poc_implementation: implementation.to_string(),
        pass_fail_or_not_comparable: verdict.to_string(),
    }
}

pub fn render_metrics_markdown(metrics: &[MetricResult]) -> String {
    let rows: Vec<[String; 5]> = metrics
        .iter()
        .map(|metric| {
            [
                metric.name.clone(),
                format!("{:.3} {}", metric.measured_value, metric.unit),
                metric.paper_target.clone(),
                metric.poc_implementation.clone(),
                metric.pass_fail_or_not_comparable.clone(),
            ]
        })
        .collect();
    let headers = [
        "Metric".to_string(),
        "Measured".to_string(),
        "Paper target".to_string(),
        "PoC implementation".to_string(),
        "Verdict".to_string(),
    ];
    let mut widths = headers.each_ref().map(|header| header.len());
    for row in &rows {
        for (index, value) in row.iter().enumerate() {
            widths[index] = widths[index].max(value.len());
        }
    }

    let mut out = String::from(
        "### Verdict legend\n\n\
- `pass`: nilai PoC sudah memenuhi target paper yang comparable.\n\
- `fail`: nilai PoC sudah diukur, tetapi belum memenuhi target paper yang comparable.\n\
- `pass_warn`: nilai masih pass, tetapi melewati warn timeout demo.\n\
- `fail_timeout`: proof generation atau verification melewati hard timeout.\n\
- `not_comparable`: nilai tetap diukur, tetapi target paper memakai primitive berbeda, misalnya Bayer-Groth/SNARK penuh.\n\
- `measured`: nilai sudah diukur sebagai baseline baru, tetapi belum ada target pass/fail resmi dari PDF untuk metric tersebut.\n\
- `scale_report`: nilai dipakai untuk membaca scaling di luar baseline n=100, bukan verdict pass/fail utama.\n\n",
    );
    out.push_str(&format!(
        "| {:<metric_width$} | {:>measured_width$} | {:<target_width$} | {:<implementation_width$} | {:<verdict_width$} |\n",
        headers[0],
        headers[1],
        headers[2],
        headers[3],
        headers[4],
        metric_width = widths[0],
        measured_width = widths[1],
        target_width = widths[2],
        implementation_width = widths[3],
        verdict_width = widths[4]
    ));
    out.push_str(&format!(
        "| {:-<metric_width$} | {:-<measured_dash_width$}: | {:-<target_width$} | {:-<implementation_width$} | {:-<verdict_width$} |\n",
        "",
        "",
        "",
        "",
        "",
        metric_width = widths[0],
        measured_dash_width = widths[1].saturating_sub(1),
        target_width = widths[2],
        implementation_width = widths[3],
        verdict_width = widths[4]
    ));
    for metric in metrics {
        out.push_str(&format!(
            "| {:<metric_width$} | {:>measured_width$} | {:<target_width$} | {:<implementation_width$} | {:<verdict_width$} |\n",
            metric.name,
            format!("{:.3} {}", metric.measured_value, metric.unit),
            metric.paper_target,
            metric.poc_implementation,
            metric.pass_fail_or_not_comparable,
            metric_width = widths[0],
            measured_width = widths[1],
            target_width = widths[2],
            implementation_width = widths[3],
            verdict_width = widths[4]
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dto_serialization_roundtrip() {
        let request = WalletPaymentRequest {
            amount_cents: 200_000,
            scenario: "compliant".into(),
            tx_id_commitment: Some(CommitmentBytes::new("abc")),
        };
        let encoded = serde_json::to_string(&request).unwrap();
        let decoded: WalletPaymentRequest = serde_json::from_str(&encoded).unwrap();
        assert_eq!(request, decoded);
    }

    #[test]
    fn metrics_markdown_contains_all_rows() {
        let rows = vec![metric(
            "commitment_size",
            32.0,
            "bytes",
            "32-64 B",
            "Ristretto255",
            "pass",
        )];
        let markdown = render_metrics_markdown(&rows);
        assert!(markdown.contains("commitment_size"));
        assert!(markdown.contains("Ristretto255"));
        assert!(markdown.contains("### Verdict legend"));
        assert!(markdown.contains("`pass`: nilai PoC sudah memenuhi target"));
        assert!(markdown.contains("32.000 bytes"));
        assert!(markdown.contains("| Metric"));
    }
}
