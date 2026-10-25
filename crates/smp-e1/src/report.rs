use crate::{
    Config, Row,
    stats::{self, Stats},
};
use anyhow::Result;
use rust_xlsxwriter::{Chart, ChartType, Color, Format, Workbook};
use serde::Serialize;
use std::{collections::BTreeMap, fs};

#[derive(Serialize)]
struct Summary {
    n: usize,
    scheme: String,
    mode: String,
    operation: String,
    ns: Stats,
    proof_bytes: Option<usize>,
    statement_bytes: Option<usize>,
    total_message_bytes: Option<usize>,
    parameter_bytes: Option<usize>,
    peak_rss_bytes: Option<u64>,
    validation: String,
}
#[derive(Serialize)]
struct Amortization {
    n: usize,
    ep: String,
    scenario: String,
    delta_setup_ns: f64,
    delta_initial_opening_data_ns: f64,
    savings_per_opening_ns: f64,
    break_even_openings: Option<f64>,
    interpretation: String,
}
fn avg(rows: &[Row], n: usize, s: &str, m: &str, op: &str) -> Option<f64> {
    let v: Vec<_> = rows
        .iter()
        .filter(|r| {
            r.n == n && r.scheme == s && r.mode == m && r.operation == op && r.status == "ok"
        })
        .filter_map(|r| r.duration_ns.map(|n| n as f64))
        .collect();
    if v.is_empty() {
        None
    } else {
        Some(v.iter().sum::<f64>() / v.len() as f64)
    }
}
fn amortization(rows: &[Row], c: &Config) -> Vec<Amortization> {
    let mut out = Vec::new();
    for n in c.grid() {
        for ep in ["ep-uncompressed", "ep-compressed"] {
            let Some(delta_setup) = avg(rows, n, "cf-h", "both", "setup_crypto")
                .zip(avg(rows, n, ep, "both", "setup_crypto"))
                .map(|(a, b)| a - b)
            else {
                continue;
            };
            let Some(ep_time) = avg(rows, n, ep, "hidden", "prove")
                .zip(avg(rows, n, ep, "hidden", "verify"))
                .map(|(a, b)| a + b)
            else {
                continue;
            };
            let Some(cf_time) = avg(rows, n, "cf-h", "hidden", "prove")
                .zip(avg(rows, n, "cf-h", "hidden", "verify"))
                .map(|(a, b)| a + b)
            else {
                continue;
            };
            for scenario in ["fresh_opening_data", "reuse_opening_data"] {
                let mut savings = ep_time - cf_time;
                let mut initial_data = 0.;
                if scenario == "fresh_opening_data" {
                    let Some((a, b)) = avg(rows, n, ep, "hidden", "opening_one").zip(avg(
                        rows,
                        n,
                        "cf-h",
                        "hidden",
                        "opening_one",
                    )) else {
                        continue;
                    };
                    savings += a - b;
                } else {
                    let Some((a, b)) = avg(rows, n, ep, "hidden", "opening_one").zip(avg(
                        rows,
                        n,
                        "cf-h",
                        "hidden",
                        "opening_one",
                    )) else {
                        continue;
                    };
                    initial_data = b - a;
                }
                let threshold = if savings > 0. {
                    Some(((delta_setup + initial_data).max(0.) / savings).ceil())
                } else {
                    None
                };
                out.push(Amortization{n,ep:ep.into(),scenario:scenario.into(),delta_setup_ns:delta_setup,delta_initial_opening_data_ns:initial_data,savings_per_opening_ns:savings,break_even_openings:threshold,interpretation:if threshold.is_some(){"diagnostic estimate; excludes commit cost; actual compressed measurements; not a validated final campaign"}else{"tidak ada break-even pada kondisi terukur (penghematan per opening tidak positif)"}.into()});
            }
        }
    }
    out
}
pub fn export(c: &Config, rows: &[Row]) -> Result<()> {
    let mut groups: BTreeMap<(usize, String, String, String), Vec<&Row>> = BTreeMap::new();
    for r in rows
        .iter()
        .filter(|r| r.status == "ok" && r.duration_ns.is_some())
    {
        groups
            .entry((r.n, r.scheme.clone(), r.mode.clone(), r.operation.clone()))
            .or_default()
            .push(r);
    }
    let mut summary = Vec::new();
    for ((n, s, m, op), rs) in groups {
        let values: Vec<_> = rs.iter().map(|r| r.duration_ns.unwrap() as f64).collect();
        let seed = c.bootstrap_seed
            ^ u64::from_le_bytes(
                smp_primitives::digest(format!("{n}:{s}:{m}:{op}").as_bytes())[..8]
                    .try_into()
                    .unwrap(),
            );
        summary.push(Summary {
            n,
            scheme: s,
            mode: m,
            operation: op,
            ns: stats::summarize(&values, seed).unwrap(),
            proof_bytes: rs[0].proof_bytes,
            statement_bytes: rs[0].statement_bytes,
            total_message_bytes: rs[0].total_message_bytes,
            parameter_bytes: rs.iter().filter_map(|r| r.parameter_bytes).max(),
            peak_rss_bytes: rs.iter().filter_map(|r| r.peak_rss_bytes).max(),
            validation: rs[0].validation.clone(),
        });
    }
    let amort = amortization(rows, c);
    fs::write(
        c.output.join("summary.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"status":"diagnostic_not_final","timing_unit":"ns","statistics":summary,"amortization":amort,"crossover":crossovers(&summary)}),
        )?,
    )?;
    let mut a = String::from(
        "# Table A: coordinate opening (diagnostic only)\n\n| n | Scheme | Mode | Prove median ms | Verify median ms | Proof B | Statement B | Sent B |\n| ---: | --- | --- | ---: | ---: | ---: | ---: | ---: |\n",
    );
    for s in summary.iter().filter(|s| s.operation == "prove") {
        let verify = summary.iter().find(|v| {
            v.n == s.n && v.scheme == s.scheme && v.mode == s.mode && v.operation == "verify"
        });
        if let Some(v) = verify {
            a.push_str(&format!(
                "| {} | {} | {} | {:.6} | {:.6} | {} | {} | {} |\n",
                s.n,
                s.scheme,
                s.mode,
                s.ns.median / 1e6,
                v.ns.median / 1e6,
                s.proof_bytes.unwrap_or(0),
                s.statement_bytes.unwrap_or(0),
                s.total_message_bytes.unwrap_or(0)
            ));
        }
    }
    let mut b = String::from(
        "# Table B: preparation (diagnostic only)\n\n| n | Scheme | Mode | Operation | Mean ms | Parameter bytes | Peak RSS bytes |\n| ---: | --- | --- | --- | ---: | ---: | ---: |\n",
    );
    for s in summary
        .iter()
        .filter(|s| !["prove", "verify"].contains(&s.operation.as_str()))
    {
        let rss = rows
            .iter()
            .filter(|r| r.n == s.n && r.scheme == s.scheme && r.operation == s.operation)
            .filter_map(|r| r.peak_rss_bytes)
            .max();
        b.push_str(&format!(
            "| {} | {} | {} | {} | {:.6} | {} | {} |\n",
            s.n,
            s.scheme,
            s.mode,
            s.operation,
            s.ns.mean / 1e6,
            s.parameter_bytes
                .map(|x| x.to_string())
                .unwrap_or_else(|| "N/A".into()),
            rss.map(|x| x.to_string()).unwrap_or_else(|| "N/A".into())
        ));
    }
    for r in rows.iter().filter(|r| r.status != "ok") {
        b.push_str(&format!(
            "\n{} n={}: {}: {}\n",
            r.scheme, r.n, r.status, r.reason
        ));
    }
    let mut table_c = String::from(
        "# Table C: amortization (diagnostic estimates)\n\n| n | EP variant | Scenario | Delta setup ms | Delta initial data ms | Savings/opening ms | Break-even N |\n| ---: | --- | --- | ---: | ---: | ---: | ---: |\n",
    );
    for s in &amort {
        table_c.push_str(&format!(
            "| {} | {} | {} | {:.6} | {:.6} | {:.6} | {} |\n",
            s.n,
            s.ep,
            s.scenario,
            s.delta_setup_ns / 1e6,
            s.delta_initial_opening_data_ns / 1e6,
            s.savings_per_opening_ns / 1e6,
            s.break_even_openings
                .map(|x| format!("{x:.0}"))
                .unwrap_or_else(|| "tidak ada break-even pada kondisi terukur".into())
        ));
    }
    fs::write(c.output.join("table_a.md"), &a)?;
    fs::write(c.output.join("table_b.md"), &b)?;
    fs::write(c.output.join("table_c.md"), &table_c)?;
    fs::write(
        c.output.join("summary.md"),
        format!(
            "# E1 Diagnostic Report\n\nNot a final research campaign. EP compressed hidden requires construction review. Index k is public in both modes.\n\nProve includes proof encoding; verify includes canonical decoding, subgroup/range validation and re-encoding checks. Statement is already available to verifier. Sent bytes = proof + C (48 B, hidden only); statement bytes include R, index and x/C separately.\n\nMean CI: percentile bootstrap, 10,000 resamples. With 5 smoke samples this is diagnostic only. Standard deviation is sample standard deviation (undefined for count=1).\n\n{a}\n{b}\n{table_c}\n\nCrossover: {}\n\nRaw failed/infeasible cells retain a reason; missing observations are never zero timings.\n",
            serde_json::to_string_pretty(&crossovers(&summary))?
        ),
    )?;
    workbook(c, rows, &summary, &amort)?;
    for op in ["prove", "verify"] {
        plot(c, &summary, op)?;
    }
    Ok(())
}
fn crossovers(summary: &[Summary]) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    for ep in ["ep-uncompressed", "ep-compressed"] {
        for mode in ["plain", "hidden"] {
            let mut paired = Vec::new();
            for e in summary
                .iter()
                .filter(|s| s.scheme == ep && s.mode == mode && s.operation == "verify")
            {
                if let Some(cf) = summary.iter().find(|s| {
                    s.scheme == "cf-h" && s.mode == mode && s.operation == "verify" && s.n == e.n
                }) {
                    paired.push((e.n, e.ns.median - cf.ns.median));
                }
            }
            paired.sort_by_key(|p| p.0);
            let brackets: Vec<_> = paired
                .windows(2)
                .filter(|w| w[0].1 <= 0. && w[1].1 > 0.)
                .map(|w| [w[0].0, w[1].0])
                .collect();
            out.push(serde_json::json!({"ep":ep,"mode":mode,"observed_brackets":brackets,"interpretation":if paired.len()<2{"not observable from a single n"}else if brackets.is_empty(){"no crossing observed on measured grid"}else{"crossing bracket only; no interpolation claimed"}}));
        }
    }
    out
}
fn workbook(c: &Config, rows: &[Row], summaries: &[Summary], amort: &[Amortization]) -> Result<()> {
    let mut wb = Workbook::new();
    let header = Format::new()
        .set_bold()
        .set_background_color(Color::RGB(0x143B3A))
        .set_font_color(Color::White);
    let number = Format::new().set_num_format("0.000000");
    let readme = wb.add_worksheet();
    readme.set_name("README")?;
    readme.set_column_width(0, 30)?.set_column_width(1, 110)?;
    for(i,(k,v))in[("Status","Diagnostic smoke, NOT final research"),("Scope","Public-index coordinate opening; not full SMP or AML"),("Compressed hidden","Executable candidate, mathematical review required; see docs/E1_COMPRESSED_HIDDEN.md"),("n / runs","n = scalar vector length; runs = recorded repetitions per scheme/mode"),("Timer","Nanoseconds: prove includes encoding, verify includes canonical decode + validation"),("Sizes","Proof separate from statement. Sent = proof + 48-byte C in hidden mode; R,k already known"),("Statistics","Mean CI95 percentile bootstrap 10,000 resamples; sample SD; seeds in summary.json"),("Sources","rev7 Tables 7,9,19-23; Box 3; Attema-Cramer Protocol 5 https://ir.cwi.nl/pub/30160/30160.pdf"),("Reproduction","environment.json, schedule_n*.json, dataset_n*.bin and attempts.csv accompany this workbook")].iter().enumerate(){readme.write_string_with_format(i as u32,0,*k,&header)?;readme.write_string(i as u32,1,*v)?;}
    readme.set_freeze_panes(1, 0)?;
    let sheet = wb.add_worksheet();
    sheet.set_name("Table A")?;
    sheet
        .set_column_width(0, 10)?
        .set_column_width(1, 24)?
        .set_column_width(2, 15)?
        .set_column_range_width(3, 8, 20)?;
    sheet.set_freeze_panes(1, 3)?;
    for (j, h) in [
        "n",
        "Scheme",
        "Mode",
        "Prove median ms",
        "Verify median ms",
        "Proof bytes",
        "Statement bytes",
        "Sent bytes",
        "Review status",
        "Chart case",
    ]
    .iter()
    .enumerate()
    {
        sheet.write_string_with_format(0, j as u16, *h, &header)?;
    }
    let mut ar = 1u32;
    for s in summaries.iter().filter(|s| s.operation == "prove") {
        let v = summaries
            .iter()
            .find(|v| {
                v.n == s.n && v.scheme == s.scheme && v.mode == s.mode && v.operation == "verify"
            })
            .unwrap();
        sheet
            .write_number(ar, 0, s.n as f64)?
            .write_string(ar, 1, &s.scheme)?
            .write_string(ar, 2, &s.mode)?;
        sheet
            .write_number_with_format(ar, 3, s.ns.median / 1e6, &number)?
            .write_number_with_format(ar, 4, v.ns.median / 1e6, &number)?;
        for (col, value) in [
            (5, s.proof_bytes),
            (6, s.statement_bytes),
            (7, s.total_message_bytes),
        ] {
            if let Some(value) = value {
                sheet.write_number(ar, col, value as f64)?;
            }
        }
        sheet.write_string(ar, 8, &s.validation)?;
        sheet.write_string(ar, 9, format!("{} / {}", s.scheme, s.mode))?;
        ar += 1;
    }
    sheet.set_column_width(8, 44)?.set_column_width(9, 34)?;
    if ar > 1 {
        sheet.autofilter(0, 0, ar - 1, 9)?;
    }
    let flat_stats: Vec<_> = summaries
        .iter()
        .map(|s| {
            let mut row = serde_json::to_value(s).unwrap();
            let ns = row.as_object_mut().unwrap().remove("ns").unwrap();
            for (k, v) in ns.as_object().unwrap() {
                if k != "mean_ci95" {
                    row.as_object_mut().unwrap().insert(k.clone(), v.clone());
                }
            }
            row["mean_ci95_lower_ns"] = serde_json::json!(s.ns.mean_ci95[0]);
            row["mean_ci95_upper_ns"] = serde_json::json!(s.ns.mean_ci95[1]);
            row
        })
        .collect();
    for (name, data) in [
        ("Statistics", serde_json::to_value(flat_stats)?),
        ("Attempt Log", serde_json::to_value(rows)?),
        ("Table C", serde_json::to_value(amort)?),
    ] {
        let s = wb.add_worksheet();
        s.set_name(name)?;
        s.set_freeze_panes(1, 0)?;
        if let Some(first) = data
            .as_array()
            .and_then(|a| a.first())
            .and_then(|x| x.as_object())
        {
            let keys: Vec<_> = first.keys().collect();
            for (j, k) in keys.iter().enumerate() {
                s.write_string_with_format(0, j as u16, k.as_str(), &header)?;
                s.set_column_width(j as u16, 22)?;
            }
            for (i, row) in data.as_array().unwrap().iter().enumerate() {
                for (j, k) in keys.iter().enumerate() {
                    let v = &row[*k];
                    if k.contains("seed") && !v.is_null() {
                        s.write_string(i as u32 + 1, j as u16, v.to_string())?;
                    } else if let Some(n) = v.as_f64() {
                        s.write_number_with_format(i as u32 + 1, j as u16, n, &number)?;
                    } else if !v.is_null() {
                        s.write_string(
                            i as u32 + 1,
                            j as u16,
                            v.as_str()
                                .map(str::to_string)
                                .unwrap_or_else(|| v.to_string()),
                        )?;
                    }
                }
            }
            s.autofilter(
                0,
                0,
                data.as_array().unwrap().len() as u32,
                keys.len() as u16 - 1,
            )?;
        }
    }
    let s = wb.add_worksheet();
    s.set_name("Table B")?;
    s.set_column_width(0, 12)?
        .set_column_width(1, 23)?
        .set_column_width(2, 24)?
        .set_column_width(3, 22)?
        .set_column_width(4, 16)?
        .set_column_width(5, 22)?
        .set_column_width(6, 22)?;
    for (j, h) in [
        "n",
        "Scheme",
        "Operation",
        "Mean ms",
        "Mode",
        "Parameter bytes",
        "Peak RSS bytes",
    ]
    .iter()
    .enumerate()
    {
        s.write_string_with_format(0, j as u16, *h, &header)?;
    }
    for (i, r) in summaries
        .iter()
        .filter(|r| {
            [
                "setup_crypto",
                "parameter_encoding",
                "commit",
                "opening_one",
                "opening_all",
            ]
            .contains(&r.operation.as_str())
        })
        .enumerate()
    {
        s.write_number(i as u32 + 1, 0, r.n as f64)?
            .write_string(i as u32 + 1, 1, &r.scheme)?
            .write_string(i as u32 + 1, 2, &r.operation)?;
        s.write_number_with_format(i as u32 + 1, 3, r.ns.mean / 1e6, &number)?;
        s.write_string(i as u32 + 1, 4, &r.mode)?;
        if let Some(bytes) = r.parameter_bytes {
            s.write_number(i as u32 + 1, 5, bytes as f64)?;
        }
        if let Some(bytes) = r.peak_rss_bytes {
            s.write_number(i as u32 + 1, 6, bytes as f64)?;
        }
    }
    let s = wb.add_worksheet();
    s.set_name("Charts")?;
    s.write_string(
        0,
        0,
        "Diagnostic comparison; no scaling inference from one n",
    )?;
    for (i, col, title) in [
        (0, 3, "Prove median (ms)"),
        (1, 4, "Verify median (ms)"),
        (2, 5, "Proof bytes"),
    ] {
        if ar <= 1 {
            continue;
        }
        let mut chart = Chart::new(ChartType::Bar);
        chart.title().set_name(title);
        chart.legend().set_hidden();
        chart
            .add_series()
            .set_categories(("Table A", 1, 9, ar - 1, 9))
            .set_values(("Table A", 1, col, ar - 1, col));
        chart.set_width(800).set_height(380);
        s.insert_chart(2 + i * 21, 0, &chart)?;
    }
    wb.save(c.output.join("e1_report.xlsx"))?;
    Ok(())
}
fn plot(c: &Config, summary: &[Summary], op: &str) -> Result<()> {
    let data: Vec<_> = summary.iter().filter(|s| s.operation == op).collect();
    let max = data
        .iter()
        .map(|s| s.ns.median / 1e6)
        .fold(0., f64::max)
        .max(0.001);
    let ns = c.grid();
    let mut svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='1100' height='600' viewBox='0 0 1100 600'><rect width='1100' height='600' fill='white'/><g font-family='Arial' font-size='14' fill='#172620'><text x='60' y='32' font-size='22'>E1 {op}: diagnostic median, not final campaign</text><text x='60' y='56'>Single n cannot establish scaling or crossover. Units: ms.</text>"
    );
    for i in 0..=5 {
        let y = 480. - i as f64 * 76.;
        svg += &format!(
            "<path d='M90 {y}H800' stroke='#dfe5e2'/><text x='20' y='{}'>{:.3}</text>",
            y + 5.,
            max * i as f64 / 5.
        );
    }
    let colors = [
        "#006b5d", "#d45b35", "#366eb3", "#99631c", "#854b98", "#45565f",
    ];
    for (a, scheme) in ["ep-uncompressed", "ep-compressed", "cf-h"]
        .iter()
        .enumerate()
    {
        for (b, mode) in ["plain", "hidden"].iter().enumerate() {
            let color = colors[a * 2 + b];
            let mut points = Vec::new();
            for s in data
                .iter()
                .filter(|s| s.scheme == *scheme && s.mode == *mode)
            {
                let idx = ns.iter().position(|n| *n == s.n).unwrap();
                let x = if ns.len() == 1 {
                    440.
                } else {
                    90. + idx as f64 * 710. / (ns.len() - 1) as f64
                };
                let y = 480. - s.ns.median / 1e6 / max * 380.;
                points.push(format!("{x},{y}"));
                svg += &format!("<circle cx='{x}' cy='{y}' r='5' fill='{color}'/>");
            }
            svg += &format!(
                "<polyline points='{}' fill='none' stroke='{color}' stroke-width='2'/><text x='820' y='{}' fill='{color}'>{scheme} {mode}</text>",
                points.join(" "),
                110 + (a * 2 + b) * 32
            );
        }
    }
    for (i, n) in ns.iter().enumerate() {
        let x = if ns.len() == 1 {
            440.
        } else {
            90. + i as f64 * 710. / (ns.len() - 1) as f64
        };
        svg += &format!("<text x='{}' y='510'>n={n}</text>", x - 20.);
    }
    svg += "</g></svg>";
    fs::write(c.output.join(format!("{op}_vs_n.svg")), svg)?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_fake_crossover() {
        assert_eq!(
            crossovers(&[])[0]["observed_brackets"],
            serde_json::json!([])
        );
    }
}
