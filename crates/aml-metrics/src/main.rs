fn main() -> anyhow::Result<()> {
    let mut runs = 100usize;
    let mut n = 100usize;
    let mut modes = None;
    let mut xlsx = false;
    let mut xlsx_path = None;
    let mut seed = None;
    let mut include_proof_json = false;
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let mut index = 0usize;
    while let Some(arg) = args.get(index) {
        match arg.as_str() {
            "--runs" => {
                index += 1;
                if let Some(value) = args.get(index) {
                    runs = value.parse()?;
                }
            }
            "--n" => {
                index += 1;
                if let Some(value) = args.get(index) {
                    n = value.parse()?;
                }
            }
            "--mode" | "--modes" => {
                index += 1;
                if let Some(value) = args.get(index) {
                    modes = Some(aml_crypto_membership::parse_membership_modes(value)?);
                }
            }
            "--xlsx" => {
                xlsx = true;
                if let Some(next) = args.get(index + 1).filter(|next| !next.starts_with("--")) {
                    xlsx_path = Some(std::path::PathBuf::from(next));
                    index += 1;
                }
            }
            "--xlsx-path" => {
                xlsx = true;
                index += 1;
                if let Some(value) = args.get(index) {
                    xlsx_path = Some(std::path::PathBuf::from(value));
                }
            }
            "--seed" => {
                index += 1;
                if let Some(value) = args.get(index) {
                    seed = Some(value.parse()?);
                }
            }
            "--include-proof-json" => {
                include_proof_json = true;
                if let Some(next) = args.get(index + 1).filter(|next| !next.starts_with("--")) {
                    include_proof_json = next.parse()?;
                    index += 1;
                }
            }
            _ => {}
        }
        index += 1;
    }

    if xlsx {
        let research_report =
            aml_metrics::generate_research_report(aml_metrics::MetricsResearchConfig {
                runs,
                n,
                modes: modes.unwrap_or_else(|| aml_crypto_membership::MembershipMode::ALL.to_vec()),
                seed,
                include_proof_json,
            })?;
        let output_path = if let Some(path) = xlsx_path {
            aml_metrics::export_research_xlsx(&research_report, &path)?;
            path
        } else {
            aml_metrics::export_research_xlsx_to_default(&research_report)?
        };
        println!("{}", serde_json::to_string_pretty(&research_report.report)?);
        println!(
            "\n--- Markdown report ---\n{}",
            research_report.report.markdown
        );
        println!("\nXLSX report written to {}", output_path.display());
    } else {
        let report = if let Some(modes) = modes {
            aml_metrics::generate_report_for_modes(runs, n, modes)?
        } else {
            aml_metrics::generate_report(runs, n)?
        };
        println!("{}", serde_json::to_string_pretty(&report)?);
        println!("\n--- Markdown report ---\n{}", report.markdown);
    }
    Ok(())
}
