use std::{fs, path::Path};

use aml_crypto_membership::{
    MembershipMetricThresholds, MembershipMode, security_profile_for_mode,
};
use rust_xlsxwriter::{Chart, ChartType, Format, Workbook, Worksheet};

use crate::{MembershipStabilityStats, MetricsResearchReport};

pub fn export_research_xlsx(
    report: &MetricsResearchReport,
    path: impl AsRef<Path>,
) -> anyhow::Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }

    let mut workbook = Workbook::new();
    let title = Format::new().set_bold().set_font_size(16);
    let section = Format::new().set_bold();
    let header = Format::new().set_bold();
    let number = Format::new().set_num_format("0.000");
    let integer = Format::new().set_num_format("#,##0");

    write_readme(&mut workbook, report, &title, &section)?;
    write_executive_summary(&mut workbook, report, &title, &section, &number)?;
    write_mode_comparison(&mut workbook, report, &header, &number, &integer)?;
    write_attempt_log(&mut workbook, report, &header, &number, &integer)?;
    write_stability_stats(&mut workbook, report, &header, &number, &integer)?;
    write_commitment_dataset(&mut workbook, report, &header, &integer)?;
    write_algorithm_analysis(&mut workbook, &title, &section)?;
    write_thresholds(&mut workbook, &header, &integer)?;
    write_charts(&mut workbook, report, &header, &number, &integer)?;

    workbook.save(path)?;
    Ok(())
}

fn write_readme(
    workbook: &mut Workbook,
    report: &MetricsResearchReport,
    title: &Format,
    section: &Format,
) -> anyhow::Result<()> {
    let worksheet = workbook.add_worksheet();
    worksheet.set_name("README")?;
    worksheet.set_column_width(0, 28)?;
    worksheet.set_column_width(1, 110)?;
    worksheet.write_with_format(0, 0, "AML ZKP Metrics XLSX Report", title)?;
    write_kv(worksheet, 2, "report_id", &report.report_id)?;
    write_kv(
        worksheet,
        3,
        "generated_at",
        &report.generated_at.to_rfc3339(),
    )?;
    write_kv(worksheet, 4, "runs", &report.runs.to_string())?;
    write_kv(worksheet, 5, "n", &report.n.to_string())?;
    write_kv(worksheet, 6, "seed", &report.seed.to_string())?;
    write_kv(
        worksheet,
        7,
        "modes",
        &report
            .modes
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", "),
    )?;

    worksheet.write_with_format(9, 0, "Interpretasi parameter", section)?;
    worksheet.write_string(10, 0, "n")?;
    worksheet.write_string(
        10,
        1,
        "Jumlah commitment/transaksi dummy di dataset membership proof. n=100 berarti ada 100 commitment, bukan 100 pengulangan.",
    )?;
    worksheet.write_string(11, 0, "runs")?;
    worksheet.write_string(
        11,
        1,
        "Jumlah attempt/pengulangan benchmark. runs=1000 berarti setiap mode diuji 1000 kali.",
    )?;
    worksheet.write_string(12, 0, "seed")?;
    worksheet.write_string(
        12,
        1,
        "Angka untuk mereproduksi dataset commitment dummy yang sama pada eksperimen berikutnya.",
    )?;

    worksheet.write_with_format(14, 0, "Catatan metodologi", section)?;
    worksheet.write_string(
        15,
        1,
        "Explicit adalah baseline shuffle-based sigma PoC dan tetap menjadi default payment flow karena paling kompatibel dengan arsitektur stakeholder saat ini.",
    )?;
    worksheet.write_string(
        16,
        1,
        "Implicit adalah vector-commitment-like/Merkle PoC, bukan KZG/vector commitment production.",
    )?;
    worksheet.write_string(
        17,
        1,
        "Hybrid adalah bucket/prefilter subset lalu explicit proof atas subset, sehingga performa diharapkan berada di tengah.",
    )?;
    worksheet.write_string(
        18,
        1,
        "Untuk n=100, verdict pass/fail memakai baseline awal. Untuk n selain 100, membership verdict memakai scale_report agar tidak overclaim.",
    )?;
    Ok(())
}

fn write_executive_summary(
    workbook: &mut Workbook,
    report: &MetricsResearchReport,
    title: &Format,
    section: &Format,
    number: &Format,
) -> anyhow::Result<()> {
    let worksheet = workbook.add_worksheet();
    worksheet.set_name("Executive Summary")?;
    worksheet.set_column_width(0, 34)?;
    worksheet.set_column_width(1, 90)?;
    worksheet.write_with_format(0, 0, "Executive Summary", title)?;
    write_kv(
        worksheet,
        2,
        "Command shape",
        "cargo run -p aml-metrics -- --runs <runs> --n <n> --modes explicit,implicit,hybrid --xlsx",
    )?;
    write_kv(worksheet, 3, "Actual runs", &report.runs.to_string())?;
    write_kv(worksheet, 4, "Actual n", &report.n.to_string())?;
    write_kv(worksheet, 5, "Seed", &report.seed.to_string())?;
    write_kv(
        worksheet,
        6,
        "Mode terbaik gen time",
        &best_mode(report, |row| row.avg_generation_ms),
    )?;
    write_kv(
        worksheet,
        7,
        "Mode terbaik verify time",
        &best_mode(report, |row| row.avg_verification_ms),
    )?;
    write_kv(
        worksheet,
        8,
        "Mode terbaik proof size",
        &best_mode(report, |row| row.avg_proof_size_bytes),
    )?;

    worksheet.write_with_format(10, 0, "Kesimpulan naratif", section)?;
    worksheet.write_string(
        11,
        0,
        "Implicit biasanya paling kecil dan cepat karena cukup membuktikan opening/path terhadap root commitment, bukan membuktikan shuffle atas seluruh set.",
    )?;
    worksheet.write_string(
        12,
        0,
        "Explicit paling dekat dengan baseline flow awal, tetapi biaya proof dan verifikasi tumbuh mengikuti ukuran set.",
    )?;
    worksheet.write_string(
        13,
        0,
        "Hybrid adalah kompromi: subset diperkecil memakai bucket, lalu proof explicit hanya dilakukan pada subset tersebut.",
    )?;

    worksheet.write_with_format(15, 0, "Quick numbers", section)?;
    write_headers(
        worksheet,
        16,
        &[
            "Mode",
            "Avg gen ms",
            "Avg verify ms",
            "Avg size bytes",
            "Verdict",
        ],
        section,
    )?;
    for (idx, row) in report.stability_stats.iter().enumerate() {
        let xlsx_row = 17 + idx as u32;
        worksheet.write_string(xlsx_row, 0, row.mode.to_string())?;
        worksheet.write_with_format(xlsx_row, 1, row.avg_generation_ms, number)?;
        worksheet.write_with_format(xlsx_row, 2, row.avg_verification_ms, number)?;
        worksheet.write_with_format(xlsx_row, 3, row.avg_proof_size_bytes, number)?;
        worksheet.write_string(xlsx_row, 4, &row.verdict)?;
    }
    Ok(())
}

fn write_mode_comparison(
    workbook: &mut Workbook,
    report: &MetricsResearchReport,
    header: &Format,
    number: &Format,
    integer: &Format,
) -> anyhow::Result<()> {
    let worksheet = workbook.add_worksheet();
    worksheet.set_name("Mode Comparison")?;
    for col in 0..20 {
        worksheet.set_column_width(col, 18)?;
    }
    worksheet.set_column_width(16, 26)?;
    worksheet.set_column_width(17, 54)?;
    worksheet.set_column_width(18, 54)?;
    worksheet.set_column_width(19, 54)?;
    let headers = [
        "Mode",
        "Avg Gen ms",
        "Avg Verify ms",
        "Avg Proof Size bytes",
        "P95 Gen ms",
        "P99 Gen ms",
        "Stddev Gen ms",
        "CV Gen %",
        "P95 Verify ms",
        "P99 Verify ms",
        "Stddev Verify ms",
        "CV Verify %",
        "Gen Target ms",
        "Verify Target ms",
        "Size Target bytes",
        "Verdict",
        "Claim Level",
        "Current Primitive",
        "Formal Target",
        "Formal Gap",
    ];
    write_headers(worksheet, 0, &headers, header)?;
    for (idx, row) in report.stability_stats.iter().enumerate() {
        let xlsx_row = 1 + idx as u32;
        worksheet.write_string(xlsx_row, 0, row.mode.to_string())?;
        worksheet.write_with_format(xlsx_row, 1, row.avg_generation_ms, number)?;
        worksheet.write_with_format(xlsx_row, 2, row.avg_verification_ms, number)?;
        worksheet.write_with_format(xlsx_row, 3, row.avg_proof_size_bytes, number)?;
        worksheet.write_with_format(xlsx_row, 4, row.p95_generation_ms, number)?;
        worksheet.write_with_format(xlsx_row, 5, row.p99_generation_ms, number)?;
        worksheet.write_with_format(xlsx_row, 6, row.stddev_generation_ms, number)?;
        worksheet.write_with_format(xlsx_row, 7, row.cv_generation_pct, number)?;
        worksheet.write_with_format(xlsx_row, 8, row.p95_verification_ms, number)?;
        worksheet.write_with_format(xlsx_row, 9, row.p99_verification_ms, number)?;
        worksheet.write_with_format(xlsx_row, 10, row.stddev_verification_ms, number)?;
        worksheet.write_with_format(xlsx_row, 11, row.cv_verification_pct, number)?;
        worksheet.write_with_format(xlsx_row, 12, row.generation_target_ms as f64, integer)?;
        worksheet.write_with_format(xlsx_row, 13, row.verification_target_ms as f64, integer)?;
        worksheet.write_with_format(xlsx_row, 14, row.size_target_bytes as f64, integer)?;
        worksheet.write_string(xlsx_row, 15, &row.verdict)?;
        let profile = security_profile_for_mode(row.mode);
        worksheet.write_string(xlsx_row, 16, profile.claim_level)?;
        worksheet.write_string(xlsx_row, 17, profile.current_primitive)?;
        worksheet.write_string(xlsx_row, 18, profile.formal_target)?;
        worksheet.write_string(xlsx_row, 19, profile.formal_gap)?;
    }
    Ok(())
}

fn write_attempt_log(
    workbook: &mut Workbook,
    report: &MetricsResearchReport,
    header: &Format,
    number: &Format,
    integer: &Format,
) -> anyhow::Result<()> {
    let worksheet = workbook.add_worksheet();
    worksheet.set_name("Attempt Log")?;
    for col in 0..16 {
        worksheet.set_column_width(col, 20)?;
    }
    let mut headers = vec![
        "report_id",
        "mode",
        "attempt_no",
        "n",
        "target_index",
        "set_size",
        "subset_size",
        "merkle_path_len",
        "generation_ms",
        "verification_ms",
        "proof_size_bytes",
        "verify_ok",
        "verdict",
        "proof_hash_short",
    ];
    if report.include_proof_json {
        headers.push("proof_json");
    }
    write_headers(worksheet, 0, &headers, header)?;
    for (idx, row) in report.attempts.iter().enumerate() {
        let xlsx_row = 1 + idx as u32;
        worksheet.write_string(xlsx_row, 0, &row.report_id)?;
        worksheet.write_string(xlsx_row, 1, row.mode.to_string())?;
        worksheet.write_with_format(xlsx_row, 2, row.attempt_no as f64, integer)?;
        worksheet.write_with_format(xlsx_row, 3, row.n as f64, integer)?;
        worksheet.write_with_format(xlsx_row, 4, row.target_index as f64, integer)?;
        worksheet.write_with_format(xlsx_row, 5, row.set_size as f64, integer)?;
        write_optional_usize(worksheet, xlsx_row, 6, row.subset_size, integer)?;
        write_optional_usize(worksheet, xlsx_row, 7, row.merkle_path_len, integer)?;
        worksheet.write_with_format(xlsx_row, 8, row.generation_ms, number)?;
        worksheet.write_with_format(xlsx_row, 9, row.verification_ms, number)?;
        worksheet.write_with_format(xlsx_row, 10, row.proof_size_bytes as f64, integer)?;
        worksheet.write_boolean(xlsx_row, 11, row.verify_ok)?;
        worksheet.write_string(xlsx_row, 12, &row.verdict)?;
        worksheet.write_string(xlsx_row, 13, &row.proof_hash_short)?;
        if let Some(proof_json) = &row.proof_json {
            worksheet.write_string(xlsx_row, 14, proof_json)?;
        }
    }
    Ok(())
}

fn write_stability_stats(
    workbook: &mut Workbook,
    report: &MetricsResearchReport,
    header: &Format,
    number: &Format,
    integer: &Format,
) -> anyhow::Result<()> {
    let worksheet = workbook.add_worksheet();
    worksheet.set_name("Stability Stats")?;
    for col in 0..14 {
        worksheet.set_column_width(col, 19)?;
    }
    let headers = [
        "Mode", "Metric", "Count", "Min", "Max", "Average", "P50", "P90", "P95", "P99", "Stddev",
        "CV %", "Target", "Verdict",
    ];
    write_headers(worksheet, 0, &headers, header)?;
    let mut xlsx_row = 1u32;
    for row in &report.stability_stats {
        write_stability_metric_row(
            worksheet,
            xlsx_row,
            row,
            "generation_ms",
            row.min_generation_ms,
            row.max_generation_ms,
            row.avg_generation_ms,
            row.p50_generation_ms,
            row.p90_generation_ms,
            row.p95_generation_ms,
            row.p99_generation_ms,
            row.stddev_generation_ms,
            row.cv_generation_pct,
            row.generation_target_ms as f64,
            number,
            integer,
        )?;
        xlsx_row += 1;
        write_stability_metric_row(
            worksheet,
            xlsx_row,
            row,
            "verification_ms",
            row.min_verification_ms,
            row.max_verification_ms,
            row.avg_verification_ms,
            row.p50_verification_ms,
            row.p90_verification_ms,
            row.p95_verification_ms,
            row.p99_verification_ms,
            row.stddev_verification_ms,
            row.cv_verification_pct,
            row.verification_target_ms as f64,
            number,
            integer,
        )?;
        xlsx_row += 1;
        write_stability_metric_row(
            worksheet,
            xlsx_row,
            row,
            "proof_size_bytes",
            row.min_proof_size_bytes,
            row.max_proof_size_bytes,
            row.avg_proof_size_bytes,
            0.0,
            0.0,
            row.p95_proof_size_bytes,
            0.0,
            0.0,
            0.0,
            row.size_target_bytes as f64,
            number,
            integer,
        )?;
        xlsx_row += 1;
    }
    Ok(())
}

fn write_commitment_dataset(
    workbook: &mut Workbook,
    report: &MetricsResearchReport,
    header: &Format,
    integer: &Format,
) -> anyhow::Result<()> {
    let worksheet = workbook.add_worksheet();
    worksheet.set_name("Commitment Dataset")?;
    worksheet.set_column_width(0, 12)?;
    worksheet.set_column_width(1, 58)?;
    worksheet.set_column_width(2, 12)?;
    worksheet.set_column_width(3, 14)?;
    write_headers(
        worksheet,
        0,
        &["index", "commitment_handle", "is_target", "bucket_index"],
        header,
    )?;
    for (idx, row) in report.commitment_dataset.iter().enumerate() {
        let xlsx_row = 1 + idx as u32;
        worksheet.write_with_format(xlsx_row, 0, row.index as f64, integer)?;
        worksheet.write_string(xlsx_row, 1, &row.commitment_handle)?;
        worksheet.write_boolean(xlsx_row, 2, row.is_target)?;
        worksheet.write_with_format(xlsx_row, 3, row.bucket_index as f64, integer)?;
    }
    Ok(())
}

fn write_algorithm_analysis(
    workbook: &mut Workbook,
    title: &Format,
    section: &Format,
) -> anyhow::Result<()> {
    let worksheet = workbook.add_worksheet();
    worksheet.set_name("Algorithm Analysis")?;
    worksheet.set_column_width(0, 18)?;
    worksheet.set_column_width(1, 36)?;
    worksheet.set_column_width(2, 36)?;
    worksheet.set_column_width(3, 44)?;
    worksheet.set_column_width(4, 54)?;
    worksheet.set_column_width(5, 54)?;
    worksheet.write_with_format(0, 0, "Analisa 3 Pendekatan Set Membership", title)?;
    write_headers(
        worksheet,
        2,
        &[
            "Mode",
            "Alur logika",
            "Kenapa performanya berbeda",
            "Trade-off",
            "Formal target Opsi C",
            "Gap code saat ini",
        ],
        section,
    )?;
    let rows = [
        (
            "explicit",
            "Prover membuktikan target ada dalam full set lewat shuffle/rerandomization sigma proof.",
            "Biaya mengikuti seluruh set. Semakin besar n, proof dan verifikasi ikut membesar.",
            "Paling dekat dengan flow awal stakeholder, tetapi paling berat untuk skala besar.",
            "Formal shuffle proof, misalnya Bayer-Groth-style shuffle argument.",
            "Current code masih sigma rerandomization membership, belum formal shuffle proof.",
        ),
        (
            "implicit",
            "Prover membuktikan opening/path terhadap root commitment seperti Merkle/vector-commitment-like PoC.",
            "Verifier hanya cek path/root, sehingga proof kecil dan verifikasi cepat.",
            "Paling baik untuk performa, tetapi PoC ini belum KZG/vector commitment production-grade.",
            "Vector commitment opening proof seperti KZG/vector commitment.",
            "Current code masih Merkle path, belum vector commitment proof production.",
        ),
        (
            "hybrid",
            "Commitment dipetakan ke bucket/subset dulu, lalu explicit proof hanya atas subset target.",
            "Biaya bergantung ukuran subset, bukan full n. Biasanya lebih ringan dari explicit.",
            "Kompromi riset yang masih butuh desain bucket/prefilter lebih matang untuk production.",
            "Global commitment proof untuk bucket/subset consistency plus subset membership.",
            "Current code belum punya pi_global terhadap trusted global root R_S.",
        ),
    ];
    for (idx, row) in rows.iter().enumerate() {
        let xlsx_row = 3 + idx as u32;
        worksheet.write_string(xlsx_row, 0, row.0)?;
        worksheet.write_string(xlsx_row, 1, row.1)?;
        worksheet.write_string(xlsx_row, 2, row.2)?;
        worksheet.write_string(xlsx_row, 3, row.3)?;
        worksheet.write_string(xlsx_row, 4, row.4)?;
        worksheet.write_string(xlsx_row, 5, row.5)?;
    }
    Ok(())
}

fn write_thresholds(
    workbook: &mut Workbook,
    header: &Format,
    integer: &Format,
) -> anyhow::Result<()> {
    let worksheet = workbook.add_worksheet();
    worksheet.set_name("Thresholds")?;
    for col in 0..7 {
        worksheet.set_column_width(col, 20)?;
    }
    write_headers(
        worksheet,
        0,
        &[
            "Mode",
            "Gen Pass ms",
            "Verify Pass ms",
            "Proof Size Pass bytes",
            "Warn Timeout ms",
            "Hard Timeout ms",
            "Scope",
        ],
        header,
    )?;
    for (idx, mode) in MembershipMode::ALL.iter().enumerate() {
        let thresholds = MembershipMetricThresholds::for_mode(*mode);
        let xlsx_row = 1 + idx as u32;
        worksheet.write_string(xlsx_row, 0, mode.to_string())?;
        worksheet.write_with_format(xlsx_row, 1, thresholds.proof_generation_ms as f64, integer)?;
        worksheet.write_with_format(xlsx_row, 2, thresholds.verification_ms as f64, integer)?;
        worksheet.write_with_format(xlsx_row, 3, thresholds.proof_size_bytes as f64, integer)?;
        worksheet.write_with_format(xlsx_row, 4, thresholds.warn_timeout_ms as f64, integer)?;
        worksheet.write_with_format(xlsx_row, 5, thresholds.hard_timeout_ms as f64, integer)?;
        worksheet.write_string(
            xlsx_row,
            6,
            "Pass/fail baseline utama untuk n=100; n lain dipakai sebagai scale_report.",
        )?;
    }
    Ok(())
}

fn write_charts(
    workbook: &mut Workbook,
    report: &MetricsResearchReport,
    header: &Format,
    number: &Format,
    integer: &Format,
) -> anyhow::Result<()> {
    let worksheet = workbook.add_worksheet();
    worksheet.set_name("Charts")?;
    worksheet.set_column_width(0, 18)?;
    worksheet.set_column_width(1, 18)?;
    worksheet.set_column_width(2, 18)?;
    worksheet.set_column_width(3, 22)?;
    write_headers(
        worksheet,
        0,
        &["Mode", "Avg Gen ms", "Avg Verify ms", "Avg Size bytes"],
        header,
    )?;
    for (idx, row) in report.stability_stats.iter().enumerate() {
        let xlsx_row = 1 + idx as u32;
        worksheet.write_string(xlsx_row, 0, row.mode.to_string())?;
        worksheet.write_with_format(xlsx_row, 1, row.avg_generation_ms, number)?;
        worksheet.write_with_format(xlsx_row, 2, row.avg_verification_ms, number)?;
        worksheet.write_with_format(xlsx_row, 3, row.avg_proof_size_bytes, integer)?;
    }

    let last_row = report.stability_stats.len() as u32;
    if last_row > 0 {
        let mut generation_chart = Chart::new(ChartType::Column);
        generation_chart
            .add_series()
            .set_categories(("Charts", 1, 0, last_row, 0))
            .set_values(("Charts", 1, 1, last_row, 1));
        generation_chart
            .title()
            .set_name("Avg Proof Generation Time");
        worksheet.insert_chart(6, 0, &generation_chart)?;

        let mut verification_chart = Chart::new(ChartType::Column);
        verification_chart
            .add_series()
            .set_categories(("Charts", 1, 0, last_row, 0))
            .set_values(("Charts", 1, 2, last_row, 2));
        verification_chart.title().set_name("Avg Verification Time");
        worksheet.insert_chart(6, 8, &verification_chart)?;

        let mut size_chart = Chart::new(ChartType::Column);
        size_chart
            .add_series()
            .set_categories(("Charts", 1, 0, last_row, 0))
            .set_values(("Charts", 1, 3, last_row, 3));
        size_chart.title().set_name("Avg Proof Size");
        worksheet.insert_chart(22, 0, &size_chart)?;
    }
    Ok(())
}

fn write_headers(
    worksheet: &mut Worksheet,
    row: u32,
    headers: &[&str],
    header: &Format,
) -> anyhow::Result<()> {
    for (col, value) in headers.iter().enumerate() {
        worksheet.write_with_format(row, col as u16, *value, header)?;
    }
    Ok(())
}

fn write_kv(worksheet: &mut Worksheet, row: u32, key: &str, value: &str) -> anyhow::Result<()> {
    worksheet.write_string(row, 0, key)?;
    worksheet.write_string(row, 1, value)?;
    Ok(())
}

fn write_optional_usize(
    worksheet: &mut Worksheet,
    row: u32,
    col: u16,
    value: Option<usize>,
    integer: &Format,
) -> anyhow::Result<()> {
    if let Some(value) = value {
        worksheet.write_with_format(row, col, value as f64, integer)?;
    } else {
        worksheet.write_string(row, col, "")?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn write_stability_metric_row(
    worksheet: &mut Worksheet,
    xlsx_row: u32,
    row: &MembershipStabilityStats,
    metric: &str,
    min: f64,
    max: f64,
    average: f64,
    p50: f64,
    p90: f64,
    p95: f64,
    p99: f64,
    stddev: f64,
    cv: f64,
    target: f64,
    number: &Format,
    integer: &Format,
) -> anyhow::Result<()> {
    worksheet.write_string(xlsx_row, 0, row.mode.to_string())?;
    worksheet.write_string(xlsx_row, 1, metric)?;
    worksheet.write_with_format(xlsx_row, 2, row.count as f64, integer)?;
    worksheet.write_with_format(xlsx_row, 3, min, number)?;
    worksheet.write_with_format(xlsx_row, 4, max, number)?;
    worksheet.write_with_format(xlsx_row, 5, average, number)?;
    worksheet.write_with_format(xlsx_row, 6, p50, number)?;
    worksheet.write_with_format(xlsx_row, 7, p90, number)?;
    worksheet.write_with_format(xlsx_row, 8, p95, number)?;
    worksheet.write_with_format(xlsx_row, 9, p99, number)?;
    worksheet.write_with_format(xlsx_row, 10, stddev, number)?;
    worksheet.write_with_format(xlsx_row, 11, cv, number)?;
    worksheet.write_with_format(xlsx_row, 12, target, integer)?;
    worksheet.write_string(xlsx_row, 13, &row.verdict)?;
    Ok(())
}

fn best_mode(
    report: &MetricsResearchReport,
    score: impl Fn(&MembershipStabilityStats) -> f64,
) -> String {
    report
        .stability_stats
        .iter()
        .min_by(|a, b| score(a).total_cmp(&score(b)))
        .map(|row| row.mode.to_string())
        .unwrap_or_else(|| "-".into())
}
