use std::{path::PathBuf, time::Instant};

use aml_core::{CommitmentBytes, Transaction};
use aml_crypto_ibc::{IbcCommitment, IbcScheme, IdEqProver, IdEqVerifier, PublicParams};
use aml_crypto_membership::{
    MembershipMetricThresholds, MembershipMode, MembershipSchemeImpl, MembershipStatement,
    finalize_membership_envelope, security_profile_for_mode,
};
use aml_crypto_shuffle::VerifiableShuffle;
use aml_crypto_update::ObliviousUpdate;
use aml_crypto_window::SlidingWindow;
use aml_three::{MembershipModeMetric, MetricsReport, metric, render_metrics_markdown};
use chrono::{DateTime, Utc};
use rand::{
    RngCore, SeedableRng,
    rngs::{OsRng, StdRng},
};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};

mod xlsx;

pub use xlsx::export_research_xlsx;

#[derive(Debug, Clone)]
pub struct MetricsResearchConfig {
    pub runs: usize,
    pub n: usize,
    pub modes: Vec<MembershipMode>,
    pub seed: Option<u64>,
    pub include_proof_json: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsResearchReport {
    pub report_id: String,
    pub generated_at: DateTime<Utc>,
    pub runs: usize,
    pub n: usize,
    pub seed: u64,
    pub modes: Vec<MembershipMode>,
    pub include_proof_json: bool,
    pub report: MetricsReport,
    pub attempts: Vec<MembershipAttemptLog>,
    pub stability_stats: Vec<MembershipStabilityStats>,
    pub commitment_dataset: Vec<CommitmentDatasetRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MembershipAttemptLog {
    pub report_id: String,
    pub mode: MembershipMode,
    pub attempt_no: usize,
    pub n: usize,
    pub target_index: usize,
    pub set_size: usize,
    pub subset_size: Option<usize>,
    pub merkle_path_len: Option<usize>,
    pub generation_ms: f64,
    pub verification_ms: f64,
    pub proof_size_bytes: usize,
    pub verify_ok: bool,
    pub verdict: String,
    pub proof_hash_short: String,
    pub proof_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MembershipStabilityStats {
    pub mode: MembershipMode,
    pub count: usize,
    pub min_generation_ms: f64,
    pub max_generation_ms: f64,
    pub avg_generation_ms: f64,
    pub p50_generation_ms: f64,
    pub p90_generation_ms: f64,
    pub p95_generation_ms: f64,
    pub p99_generation_ms: f64,
    pub stddev_generation_ms: f64,
    pub cv_generation_pct: f64,
    pub min_verification_ms: f64,
    pub max_verification_ms: f64,
    pub avg_verification_ms: f64,
    pub p50_verification_ms: f64,
    pub p90_verification_ms: f64,
    pub p95_verification_ms: f64,
    pub p99_verification_ms: f64,
    pub stddev_verification_ms: f64,
    pub cv_verification_pct: f64,
    pub min_proof_size_bytes: f64,
    pub max_proof_size_bytes: f64,
    pub avg_proof_size_bytes: f64,
    pub p95_proof_size_bytes: f64,
    pub generation_target_ms: u64,
    pub verification_target_ms: u64,
    pub size_target_bytes: usize,
    pub warn_timeout_ms: u64,
    pub hard_timeout_ms: u64,
    pub verdict: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitmentDatasetRow {
    pub index: usize,
    pub commitment_handle: String,
    pub is_target: bool,
    pub bucket_index: usize,
}

pub fn generate_report(runs: usize, n: usize) -> anyhow::Result<MetricsReport> {
    generate_report_for_modes(runs, n, MembershipMode::ALL.to_vec())
}

pub fn generate_research_report(
    config: MetricsResearchConfig,
) -> anyhow::Result<MetricsResearchReport> {
    let runs = config.runs.max(1);
    let n = config.n.max(1);
    let modes = normalized_modes(config.modes);
    let seed = config.seed.unwrap_or_else(|| OsRng.next_u64());
    let generated_at = Utc::now();
    let report_id = format!(
        "aml-metrics-{}_n{}_runs{}",
        generated_at.format("%Y%m%d_%H%M%S"),
        n,
        runs
    );

    let pp = PublicParams::setup();
    let mut rng = StdRng::seed_from_u64(seed);
    let mut commitments = Vec::with_capacity(n);
    for i in 0..n {
        let amount = (i as u64 + 1) * 10;
        let (commitment, _) = IbcScheme::commit(&pp, 1, amount, &mut rng);
        commitments.push(commitment);
    }

    let target_index = 0usize;
    let membership_statement = MembershipStatement {
        set_commitments: commitments.clone(),
        target: commitments[target_index],
        set_id: format!("metrics-research-n-{n}-seed-{seed}"),
    };

    let mut attempts = Vec::with_capacity(runs * modes.len());
    for mode in &modes {
        let scheme = MembershipSchemeImpl::for_mode(*mode);
        for attempt_no in 1..=runs {
            let generation_started = Instant::now();
            let mut envelope = scheme.prove(&pp, &membership_statement, &mut rng)?;
            let generation_ms = generation_started.elapsed().as_secs_f64() * 1000.0;
            envelope.generation_time_ms = generation_ms.round() as u64;

            let verification_started = Instant::now();
            let verify_ok = scheme.verify(&pp, &membership_statement, &envelope)?;
            let verification_ms = verification_started.elapsed().as_secs_f64() * 1000.0;
            finalize_membership_envelope(&mut envelope, verification_ms.round() as u64);
            if !verify_ok {
                envelope.verdict = "fail".into();
            } else if n != 100 {
                envelope.verdict = "scale_report".into();
            }

            let (subset_size, merkle_path_len) =
                proof_shape_metadata(*mode, &envelope.proof_json, n);
            attempts.push(MembershipAttemptLog {
                report_id: report_id.clone(),
                mode: *mode,
                attempt_no,
                n,
                target_index,
                set_size: n,
                subset_size,
                merkle_path_len,
                generation_ms,
                verification_ms,
                proof_size_bytes: envelope.proof_size_bytes,
                verify_ok,
                verdict: envelope.verdict,
                proof_hash_short: proof_hash_short(&envelope.proof_json),
                proof_json: config.include_proof_json.then_some(envelope.proof_json),
            });
        }
    }

    let stability_stats = modes
        .iter()
        .map(|mode| stability_stats_for_mode(*mode, &attempts, n))
        .collect::<Vec<_>>();
    let commitment_dataset = commitments
        .iter()
        .enumerate()
        .map(|(index, commitment)| {
            Ok(CommitmentDatasetRow {
                index,
                commitment_handle: IbcScheme::encode(commitment)?,
                is_target: index == target_index,
                bucket_index: commitment_bucket_index(commitment, 8),
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    let mut report = generate_report_for_modes(runs, n, modes.clone())?;
    report.generated_at = generated_at;

    Ok(MetricsResearchReport {
        report_id,
        generated_at,
        runs,
        n,
        seed,
        modes,
        include_proof_json: config.include_proof_json,
        report,
        attempts,
        stability_stats,
        commitment_dataset,
    })
}

pub fn default_xlsx_path(report: &MetricsResearchReport) -> PathBuf {
    PathBuf::from("outputs").join(format!(
        "aml_metrics_report_{}_n{}_runs{}.xlsx",
        report.generated_at.format("%Y%m%d_%H%M%S"),
        report.n,
        report.runs
    ))
}

pub fn export_research_xlsx_to_default(report: &MetricsResearchReport) -> anyhow::Result<PathBuf> {
    let path = default_xlsx_path(report);
    export_research_xlsx(report, &path)?;
    Ok(path)
}

pub fn generate_report_for_modes(
    runs: usize,
    n: usize,
    modes: Vec<MembershipMode>,
) -> anyhow::Result<MetricsReport> {
    let runs = runs.max(1);
    let n = n.max(1);
    let modes = normalized_modes(modes);
    let pp = PublicParams::setup();
    let mut rng = OsRng;

    let mut commitments = Vec::with_capacity(n);
    let mut openings = Vec::with_capacity(n);
    let mut transactions = Vec::with_capacity(n);
    for i in 0..n {
        let amount = (i as u64 + 1) * 10;
        let (commitment, opening) = IbcScheme::commit(&pp, 1, amount, &mut rng);
        commitments.push(commitment);
        openings.push(opening);
        transactions.push(Transaction {
            data_id: format!("tx-{i}"),
            group_id: "metrics_user".into(),
            amount_cents: amount,
            timestamp: Utc::now() - chrono::Duration::days((i % 360) as i64),
            commitment: CommitmentBytes::new(IbcScheme::encode(&commitment)?),
        });
    }

    let proof_gen_ms = avg_ms(runs, || {
        let (c0, o0) = IbcScheme::commit(&pp, 1, 10_000, &mut rng);
        let (c1, o1) = IbcScheme::commit(&pp, 1, 20_000, &mut rng);
        let _proof = IdEqProver::prove(&pp, &o0, &o1, &c0, &c1, &mut rng);
    });

    let (c0, o0) = IbcScheme::commit(&pp, 1, 10_000, &mut rng);
    let (c1, o1) = IbcScheme::commit(&pp, 1, 20_000, &mut rng);
    let proof = IdEqProver::prove(&pp, &o0, &o1, &c0, &c1, &mut rng);
    let verify_ms = avg_ms(runs, || {
        let _ok = IdEqVerifier::verify(&pp, &c0, &c1, &proof);
    });

    let commitment_size = c0.0.compress().as_bytes().len() as f64;
    let ibc_throughput = throughput(runs, || {
        let _ok = IdEqVerifier::verify(&pp, &c0, &c1, &proof);
    });

    let membership_target = commitments[0];
    let membership_statement = MembershipStatement {
        set_commitments: commitments.clone(),
        target: membership_target,
        set_id: format!("metrics-n-{n}"),
    };
    let mut membership_metrics = Vec::new();
    let mut membership_comparison = Vec::new();
    for mode in modes {
        let scheme = MembershipSchemeImpl::for_mode(mode);
        let membership_prove_ms = avg_ms(runs, || {
            let _proof = scheme.prove(&pp, &membership_statement, &mut rng).unwrap();
        });
        let mut envelope = scheme.prove(&pp, &membership_statement, &mut rng)?;
        let membership_verify_ms = avg_ms(runs, || {
            let _ok = scheme
                .verify(&pp, &membership_statement, &envelope)
                .unwrap();
        });
        finalize_membership_envelope(&mut envelope, membership_verify_ms.round() as u64);
        if n != 100 {
            envelope.verdict = "scale_report".into();
        }
        let thresholds = MembershipMetricThresholds::for_mode(mode);
        let implementation = envelope.implementation.clone();
        let profile = security_profile_for_mode(mode);
        membership_metrics.push(metric(
            &format!("Set membership proof generation time ({mode})"),
            membership_prove_ms,
            "ms",
            &format!("<= {} ms", thresholds.proof_generation_ms),
            &implementation,
            &envelope.verdict,
        ));
        membership_metrics.push(metric(
            &format!("Set membership verification time ({mode})"),
            membership_verify_ms,
            "ms",
            &format!("<= {} ms", thresholds.verification_ms),
            &implementation,
            &envelope.verdict,
        ));
        membership_metrics.push(metric(
            &format!("Set membership proof size ({mode})"),
            envelope.proof_size_bytes as f64,
            "bytes",
            &format!("<= {} bytes", thresholds.proof_size_bytes),
            &implementation,
            &envelope.verdict,
        ));
        membership_comparison.push(MembershipModeMetric {
            mode: mode.to_string(),
            proof_generation_ms: membership_prove_ms,
            verification_ms: membership_verify_ms,
            proof_size_bytes: envelope.proof_size_bytes,
            generation_target_ms: thresholds.proof_generation_ms,
            verification_target_ms: thresholds.verification_ms,
            size_target_bytes: thresholds.proof_size_bytes,
            warn_timeout_ms: thresholds.warn_timeout_ms,
            hard_timeout_ms: thresholds.hard_timeout_ms,
            verdict: envelope.verdict,
            poc_implementation: implementation,
            current_primitive: profile.current_primitive.into(),
            formal_target: profile.formal_target.into(),
            formal_gap: profile.formal_gap.into(),
            claim_level: profile.claim_level.into(),
        });
    }

    let shuffle_prove_ms = avg_ms(runs, || {
        let (_proof, _openings) =
            VerifiableShuffle::shuffle_and_prove(&pp, &commitments, &openings, &mut rng);
    });
    let (shuffle_proof, mut shuffled_openings) =
        VerifiableShuffle::shuffle_and_prove(&pp, &commitments, &openings, &mut rng);
    let shuffle_verify_ms = avg_ms(runs, || {
        let _ok = VerifiableShuffle::verify(&pp, &commitments, &shuffle_proof);
    });

    let window_start = Utc::now() - chrono::Duration::days(365);
    let window_ms = avg_ms(runs, || {
        let _filtered = SlidingWindow::filter(&transactions, window_start);
    });

    let mut shuffled_commitments = shuffle_proof.outputs.clone();
    let update = ObliviousUpdate::apply(
        &pp,
        &mut shuffled_commitments,
        &mut shuffled_openings,
        0,
        100,
        &mut rng,
    );
    let update_proof_size = serde_json::to_vec(&update)?.len() as f64;

    let end_to_end_ms = proof_gen_ms + shuffle_prove_ms + window_ms;
    let system_throughput = if end_to_end_ms > 0.0 {
        1000.0 / end_to_end_ms
    } else {
        0.0
    };

    let mut metrics = vec![
        metric(
            "IBC proof generation time",
            proof_gen_ms,
            "ms",
            "< 10 ms",
            "Pedersen/Ristretto255 IdEq proof",
            pass_if(proof_gen_ms, 10.0),
        ),
        metric(
            "IBC verification time",
            verify_ms,
            "ms",
            "< 5 ms",
            "Pedersen/Ristretto255 IdEq verify",
            pass_if(verify_ms, 5.0),
        ),
        metric(
            "Commitment size",
            commitment_size,
            "bytes",
            "32-64 B",
            "Compressed Ristretto commitment",
            if (32.0..=64.0).contains(&commitment_size) {
                "pass"
            } else {
                "fail"
            },
        ),
        metric(
            "IBC throughput",
            ibc_throughput,
            "TPS",
            "> 100 TPS",
            "Repeated IdEq verification loop",
            if ibc_throughput > 100.0 {
                "pass"
            } else {
                "fail"
            },
        ),
        metric(
            "Shuffle proof time",
            shuffle_prove_ms,
            "ms",
            "~50 ms n=100; O(n log n) n=10k",
            "PoC sigma rerandomization, not Bayer-Groth",
            "not_comparable",
        ),
        metric(
            "Shuffle verify time",
            shuffle_verify_ms,
            "ms",
            "~15 ms n=100; O(n)",
            "PoC sigma rerandomization verify by Alipay+",
            "not_comparable",
        ),
        metric(
            "Window computation time",
            window_ms,
            "ms",
            "< 5 ms n=100; O(n)",
            "Rust sliding-window filter",
            pass_if(window_ms, 5.0),
        ),
        metric(
            "Update proof size",
            update_proof_size,
            "bytes",
            "~200 B; O(1)",
            "Simplified sigma update, not SNARK",
            "not_comparable",
        ),
        metric(
            "End-to-end latency",
            end_to_end_ms,
            "ms",
            "< 200 ms n=100; < 1s n=10k",
            "Local PoC crypto segments only",
            pass_if(end_to_end_ms, 200.0),
        ),
        metric(
            "System throughput",
            system_throughput,
            "TPS",
            "> 50 TPS n=100; > 500 TPS n=10k",
            "Estimated from local end-to-end latency",
            if system_throughput > 50.0 {
                "pass"
            } else {
                "fail"
            },
        ),
    ];
    metrics.extend(membership_metrics);

    Ok(MetricsReport {
        generated_at: Utc::now(),
        runs,
        n,
        markdown: render_metrics_markdown(&metrics)
            + "\n"
            + &render_membership_comparison_markdown(&membership_comparison),
        metrics,
        membership_comparison,
    })
}

fn avg_ms(runs: usize, mut f: impl FnMut()) -> f64 {
    let started = Instant::now();
    for _ in 0..runs {
        f();
    }
    started.elapsed().as_secs_f64() * 1000.0 / runs as f64
}

fn throughput(runs: usize, mut f: impl FnMut()) -> f64 {
    let started = Instant::now();
    for _ in 0..runs {
        f();
    }
    runs as f64 / started.elapsed().as_secs_f64()
}

fn pass_if(value: f64, max: f64) -> &'static str {
    if value <= max { "pass" } else { "fail" }
}

fn normalized_modes(modes: Vec<MembershipMode>) -> Vec<MembershipMode> {
    if modes.is_empty() {
        MembershipMode::ALL.to_vec()
    } else {
        modes
    }
}

fn proof_shape_metadata(
    mode: MembershipMode,
    proof_json: &str,
    set_size: usize,
) -> (Option<usize>, Option<usize>) {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(proof_json) else {
        return (None, None);
    };
    match mode {
        MembershipMode::Explicit => (Some(set_size), None),
        MembershipMode::Implicit => {
            let path_len = value
                .get("path")
                .and_then(|path| path.as_array())
                .map(Vec::len);
            (None, path_len)
        }
        MembershipMode::Hybrid => {
            let subset_size = value
                .get("subset_commitments")
                .and_then(|subset| subset.as_array())
                .map(Vec::len);
            (subset_size, None)
        }
    }
}

fn stability_stats_for_mode(
    mode: MembershipMode,
    attempts: &[MembershipAttemptLog],
    n: usize,
) -> MembershipStabilityStats {
    let mode_attempts = attempts
        .iter()
        .filter(|attempt| attempt.mode == mode)
        .collect::<Vec<_>>();
    let generation = mode_attempts
        .iter()
        .map(|attempt| attempt.generation_ms)
        .collect::<Vec<_>>();
    let verification = mode_attempts
        .iter()
        .map(|attempt| attempt.verification_ms)
        .collect::<Vec<_>>();
    let proof_sizes = mode_attempts
        .iter()
        .map(|attempt| attempt.proof_size_bytes as f64)
        .collect::<Vec<_>>();
    let thresholds = MembershipMetricThresholds::for_mode(mode);
    let avg_generation_ms = average(&generation);
    let avg_verification_ms = average(&verification);
    let avg_proof_size_bytes = average(&proof_sizes);
    let mut verdict = thresholds.verdict(
        avg_generation_ms.round() as u64,
        avg_verification_ms.round() as u64,
        avg_proof_size_bytes.round() as usize,
    );
    if n != 100 {
        verdict = "scale_report".into();
    } else if mode_attempts.iter().any(|attempt| !attempt.verify_ok) {
        verdict = "fail".into();
    }

    MembershipStabilityStats {
        mode,
        count: mode_attempts.len(),
        min_generation_ms: min(&generation),
        max_generation_ms: max(&generation),
        avg_generation_ms,
        p50_generation_ms: percentile(&generation, 50.0),
        p90_generation_ms: percentile(&generation, 90.0),
        p95_generation_ms: percentile(&generation, 95.0),
        p99_generation_ms: percentile(&generation, 99.0),
        stddev_generation_ms: stddev(&generation),
        cv_generation_pct: cv_pct(&generation),
        min_verification_ms: min(&verification),
        max_verification_ms: max(&verification),
        avg_verification_ms,
        p50_verification_ms: percentile(&verification, 50.0),
        p90_verification_ms: percentile(&verification, 90.0),
        p95_verification_ms: percentile(&verification, 95.0),
        p99_verification_ms: percentile(&verification, 99.0),
        stddev_verification_ms: stddev(&verification),
        cv_verification_pct: cv_pct(&verification),
        min_proof_size_bytes: min(&proof_sizes),
        max_proof_size_bytes: max(&proof_sizes),
        avg_proof_size_bytes,
        p95_proof_size_bytes: percentile(&proof_sizes, 95.0),
        generation_target_ms: thresholds.proof_generation_ms,
        verification_target_ms: thresholds.verification_ms,
        size_target_bytes: thresholds.proof_size_bytes,
        warn_timeout_ms: thresholds.warn_timeout_ms,
        hard_timeout_ms: thresholds.hard_timeout_ms,
        verdict,
    }
}

fn average(values: &[f64]) -> f64 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f64>() / values.len() as f64
    }
}

fn min(values: &[f64]) -> f64 {
    values.iter().copied().reduce(f64::min).unwrap_or_default()
}

fn max(values: &[f64]) -> f64 {
    values.iter().copied().reduce(f64::max).unwrap_or_default()
}

fn percentile(values: &[f64], p: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let rank = ((p / 100.0) * (sorted.len().saturating_sub(1)) as f64).round() as usize;
    sorted[rank.min(sorted.len() - 1)]
}

fn stddev(values: &[f64]) -> f64 {
    if values.len() <= 1 {
        return 0.0;
    }
    let avg = average(values);
    let variance = values
        .iter()
        .map(|value| {
            let delta = value - avg;
            delta * delta
        })
        .sum::<f64>()
        / values.len() as f64;
    variance.sqrt()
}

fn cv_pct(values: &[f64]) -> f64 {
    let avg = average(values);
    if avg == 0.0 {
        0.0
    } else {
        stddev(values) / avg * 100.0
    }
}

fn proof_hash_short(proof_json: &str) -> String {
    let digest = Sha3_256::digest(proof_json.as_bytes());
    hex::encode(digest)[..16].to_string()
}

fn commitment_bucket_index(commitment: &IbcCommitment, bucket_count: usize) -> usize {
    let mut hasher = Sha3_256::new();
    hasher.update(commitment.0.compress().as_bytes());
    let digest = hasher.finalize();
    digest[0] as usize % bucket_count.max(1)
}

pub fn render_membership_comparison_markdown(rows: &[MembershipModeMetric]) -> String {
    let headers = [
        "Mode".to_string(),
        "Gen ms".to_string(),
        "Verify ms".to_string(),
        "Size bytes".to_string(),
        "Gen target".to_string(),
        "Verify target".to_string(),
        "Size target".to_string(),
        "Verdict".to_string(),
        "Implementation".to_string(),
        "Claim".to_string(),
        "Formal target".to_string(),
    ];
    let table_rows: Vec<[String; 11]> = rows
        .iter()
        .map(|row| {
            [
                row.mode.clone(),
                format!("{:.3}", row.proof_generation_ms),
                format!("{:.3}", row.verification_ms),
                row.proof_size_bytes.to_string(),
                format!("{} ms", row.generation_target_ms),
                format!("{} ms", row.verification_target_ms),
                format!("{} bytes", row.size_target_bytes),
                row.verdict.clone(),
                row.poc_implementation.clone(),
                row.claim_level.clone(),
                row.formal_target.clone(),
            ]
        })
        .collect();
    let mut widths = headers.each_ref().map(|header| header.len());
    for row in &table_rows {
        for (index, value) in row.iter().enumerate() {
            widths[index] = widths[index].max(value.len());
        }
    }

    let mut out = String::from("### Set Membership Mode Comparison\n\n");
    out.push_str(&format!(
        "| {:<mode_width$} | {:>gen_width$} | {:>verify_width$} | {:>size_width$} | {:>gen_target_width$} | {:>verify_target_width$} | {:>size_target_width$} | {:<verdict_width$} | {:<implementation_width$} | {:<claim_width$} | {:<formal_target_width$} |\n",
        headers[0],
        headers[1],
        headers[2],
        headers[3],
        headers[4],
        headers[5],
        headers[6],
        headers[7],
        headers[8],
        headers[9],
        headers[10],
        mode_width = widths[0],
        gen_width = widths[1],
        verify_width = widths[2],
        size_width = widths[3],
        gen_target_width = widths[4],
        verify_target_width = widths[5],
        size_target_width = widths[6],
        verdict_width = widths[7],
        implementation_width = widths[8],
        claim_width = widths[9],
        formal_target_width = widths[10],
    ));
    out.push_str(&format!(
        "| {:-<mode_width$} | {:-<gen_dash_width$}: | {:-<verify_dash_width$}: | {:-<size_dash_width$}: | {:-<gen_target_dash_width$}: | {:-<verify_target_dash_width$}: | {:-<size_target_dash_width$}: | {:-<verdict_width$} | {:-<implementation_width$} | {:-<claim_width$} | {:-<formal_target_width$} |\n",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        mode_width = widths[0],
        gen_dash_width = widths[1].saturating_sub(1),
        verify_dash_width = widths[2].saturating_sub(1),
        size_dash_width = widths[3].saturating_sub(1),
        gen_target_dash_width = widths[4].saturating_sub(1),
        verify_target_dash_width = widths[5].saturating_sub(1),
        size_target_dash_width = widths[6].saturating_sub(1),
        verdict_width = widths[7],
        implementation_width = widths[8],
        claim_width = widths[9],
        formal_target_width = widths[10],
    ));
    for row in table_rows {
        out.push_str(&format!(
            "| {:<mode_width$} | {:>gen_width$} | {:>verify_width$} | {:>size_width$} | {:>gen_target_width$} | {:>verify_target_width$} | {:>size_target_width$} | {:<verdict_width$} | {:<implementation_width$} | {:<claim_width$} | {:<formal_target_width$} |\n",
            row[0],
            row[1],
            row[2],
            row[3],
            row[4],
            row[5],
            row[6],
            row[7],
            row[8],
            row[9],
            row[10],
            mode_width = widths[0],
            gen_width = widths[1],
            verify_width = widths[2],
            size_width = widths[3],
            gen_target_width = widths[4],
            verify_target_width = widths[5],
            size_target_width = widths[6],
            verdict_width = widths[7],
            implementation_width = widths[8],
            claim_width = widths[9],
            formal_target_width = widths[10],
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_report_contains_required_metrics() {
        let report = generate_report(2, 3).unwrap();
        assert_eq!(report.membership_comparison.len(), 3);
        assert!(report.markdown.contains("Shuffle proof time"));
        assert!(report.markdown.contains("Set Membership Mode Comparison"));
    }

    #[test]
    fn research_report_attempt_log_count_matches_runs_times_modes() {
        let report = generate_research_report(MetricsResearchConfig {
            runs: 2,
            n: 4,
            modes: vec![MembershipMode::Explicit, MembershipMode::Implicit],
            seed: Some(7),
            include_proof_json: false,
        })
        .unwrap();
        assert_eq!(report.attempts.len(), 4);
        assert_eq!(report.stability_stats.len(), 2);
        assert_eq!(report.commitment_dataset.len(), 4);
        assert!(
            report
                .attempts
                .iter()
                .all(|attempt| attempt.proof_json.is_none())
        );
    }

    #[test]
    fn xlsx_export_creates_non_empty_file() {
        let report = generate_research_report(MetricsResearchConfig {
            runs: 1,
            n: 3,
            modes: vec![MembershipMode::Implicit],
            seed: Some(11),
            include_proof_json: false,
        })
        .unwrap();
        let path = std::env::temp_dir().join(format!("{}_test.xlsx", report.report_id));
        export_research_xlsx(&report, &path).unwrap();
        let metadata = std::fs::metadata(&path).unwrap();
        assert!(metadata.len() > 0);
        let _ = std::fs::remove_file(path);
    }
}
