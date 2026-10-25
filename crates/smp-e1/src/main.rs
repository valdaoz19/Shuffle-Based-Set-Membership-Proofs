mod report;
mod stats;
use anyhow::{Context, Result, ensure};
use ark_ec::{AffineRepr, pairing::Pairing};
use ark_ff::UniformRand;
use rand::{
    Rng, SeedableRng,
    rngs::{OsRng, StdRng},
    seq::SliceRandom,
};
use serde::{Deserialize, Serialize};
use smp_primitives::*;
use smp_vc::*;
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Config {
    profile: String,
    output: PathBuf,
    seed: u64,
    schedule_seed: u64,
    bootstrap_seed: u64,
    max_memory_mib: u64,
    worker_timeout_secs: u64,
}
impl Config {
    fn parse(args: impl Iterator<Item = String>) -> Result<Self> {
        let mut c = Self {
            profile: "smoke".into(),
            output: "outputs/e1-smoke".into(),
            seed: OsRng.r#gen(),
            schedule_seed: OsRng.r#gen(),
            bootstrap_seed: OsRng.r#gen(),
            max_memory_mib: 4096,
            worker_timeout_secs: 600,
        };
        let mut args = args.peekable();
        while let Some(key) = args.next() {
            let value = args.next().context(format!("missing value for {key}"))?;
            match key.as_str() {
                "--profile" => c.profile = value,
                "--output" => c.output = value.into(),
                "--seed" => c.seed = value.parse()?,
                "--schedule-seed" => c.schedule_seed = value.parse()?,
                "--bootstrap-seed" => c.bootstrap_seed = value.parse()?,
                "--max-memory-mib" => c.max_memory_mib = value.parse()?,
                "--worker-timeout-secs" => c.worker_timeout_secs = value.parse()?,
                _ => anyhow::bail!("unknown argument {key}"),
            }
        }
        ensure!(
            ["smoke", "full"].contains(&c.profile.as_str()),
            "profile must be smoke or full"
        );
        ensure!(
            c.max_memory_mib >= 64 && c.worker_timeout_secs > 0,
            "invalid resource limits"
        );
        Ok(c)
    }
    fn grid(&self) -> Vec<usize> {
        if self.profile == "full" {
            vec![64, 128, 256, 512, 1024, 2048, 4096]
        } else {
            vec![64]
        }
    }
    fn runs(&self) -> usize {
        if self.profile == "full" { 100 } else { 5 }
    }
    fn warmup(&self) -> usize {
        if self.profile == "full" { 10 } else { 2 }
    }
    fn setups(&self) -> usize {
        if self.profile == "full" { 5 } else { 1 }
    }
}
#[derive(Clone, Serialize, Deserialize)]
struct Row {
    n: usize,
    scheme: String,
    mode: String,
    attempt: usize,
    index: Option<usize>,
    operation: String,
    duration_ns: Option<u128>,
    serialization_ns: Option<u128>,
    deserialization_ns: Option<u128>,
    proof_bytes: Option<usize>,
    statement_bytes: Option<usize>,
    total_message_bytes: Option<usize>,
    peak_rss_bytes: Option<u64>,
    parameter_bytes: Option<usize>,
    status: String,
    reason: String,
    dataset_hash: String,
    validation: String,
}
impl Row {
    fn new(n: usize, scheme: Scheme, mode: &str, attempt: usize, op: &str, ns: u128) -> Self {
        Self {
            n,
            scheme: scheme.name().into(),
            mode: mode.into(),
            attempt,
            index: None,
            operation: op.into(),
            duration_ns: Some(ns),
            serialization_ns: None,
            deserialization_ns: None,
            proof_bytes: None,
            statement_bytes: None,
            total_message_bytes: None,
            peak_rss_bytes: None,
            parameter_bytes: None,
            status: "ok".into(),
            reason: String::new(),
            dataset_hash: String::new(),
            validation: if scheme == Scheme::EpCompressed && mode == "hidden" {
                "construction_review_required"
            } else {
                "implementation_tested_not_audited"
            }
            .into(),
        }
    }
}
struct Log {
    csv: File,
    jsonl: File,
    rows: Vec<Row>,
}
impl Log {
    fn new(path: &Path) -> Result<Self> {
        let mut csv = File::create(path.join("attempts.csv"))?;
        writeln!(
            csv,
            "n,scheme,mode,attempt,index,operation,duration_ns,serialization_ns,deserialization_ns,proof_bytes,statement_bytes,total_message_bytes,peak_rss_bytes,parameter_bytes,status,reason,dataset_hash,validation"
        )?;
        Ok(Self {
            csv,
            jsonl: File::create(path.join("attempts.jsonl"))?,
            rows: Vec::new(),
        })
    }
    fn add(&mut self, r: Row) -> Result<()> {
        writeln!(self.jsonl, "{}", serde_json::to_string(&r)?)?;
        self.jsonl.sync_data()?;
        let v = serde_json::to_value(&r)?;
        let cols = [
            "n",
            "scheme",
            "mode",
            "attempt",
            "index",
            "operation",
            "duration_ns",
            "serialization_ns",
            "deserialization_ns",
            "proof_bytes",
            "statement_bytes",
            "total_message_bytes",
            "peak_rss_bytes",
            "parameter_bytes",
            "status",
            "reason",
            "dataset_hash",
            "validation",
        ];
        let cells: Vec<_> = cols
            .iter()
            .map(|k| {
                let x = &v[*k];
                if x.is_null() {
                    String::new()
                } else {
                    format!(
                        "\"{}\"",
                        x.as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| x.to_string())
                            .replace('"', "\"\"")
                    )
                }
            })
            .collect();
        writeln!(self.csv, "{}", cells.join(","))?;
        self.csv.sync_data()?;
        self.rows.push(r);
        Ok(())
    }
}
#[derive(Serialize, Deserialize)]
struct SetupResult {
    setup_crypto_ns: u128,
    parameter_encoding_ns: u128,
    parameter_bytes: usize,
}
fn setup_worker(scheme: Scheme, n: usize) -> Result<()> {
    let now = Instant::now();
    let p = Parameters::setup(scheme, n, &mut OsRng)?;
    let total = now.elapsed().as_nanos();
    let encoding = match &p {
        Parameters::Ep(p) => p.encoding_ns,
        Parameters::CfH(p) => p.encoding_ns,
    };
    println!(
        "{}",
        serde_json::to_string(&SetupResult {
            setup_crypto_ns: total.saturating_sub(encoding),
            parameter_encoding_ns: encoding,
            parameter_bytes: p.bytes().len()
        })?
    );
    Ok(())
}
fn estimate_memory(n: usize, s: Scheme) -> u64 {
    128 * 1024 * 1024
        + if s == Scheme::CfH {
            n as u64 * n as u64 * 256
        } else {
            n as u64 * 1024
        }
}
fn isolated_setup(
    c: &Config,
    scheme: Scheme,
    n: usize,
    attempt: usize,
    log: &mut Log,
) -> Result<bool> {
    let mut row = Row::new(n, scheme, "both", attempt, "setup_crypto", 0);
    let budget = c.max_memory_mib * 1024 * 1024;
    if estimate_memory(n, scheme) > budget {
        row.duration_ns = None;
        row.status = "infeasible".into();
        row.reason = format!(
            "conservative estimate exceeds {} MiB worker budget",
            c.max_memory_mib
        );
        log.add(row)?;
        return Ok(false);
    }
    let stem = format!("setup_{}_n{n}_{attempt}", scheme.name());
    let stdout = c.output.join(format!("{stem}.json"));
    let stderr = c.output.join(format!("{stem}.time.txt"));
    let mut child = Command::new(std::env::current_exe()?)
        .args(["--setup-worker", scheme.name(), &n.to_string()])
        .stdout(Stdio::from(File::create(&stdout)?))
        .stderr(Stdio::from(File::create(&stderr)?))
        .spawn()?;
    let start = Instant::now();
    let mut peak = 0;
    let mut stopped = None;
    loop {
        if let Some(status) = child.try_wait()? {
            if !status.success() {
                stopped = Some(format!("worker exit {status}"));
            }
            break;
        }
        if let Ok(out) = Command::new("ps")
            .args(["-o", "rss=", "-p", &child.id().to_string()])
            .output()
            && let Ok(kib) = String::from_utf8_lossy(&out.stdout).trim().parse::<u64>()
        {
            peak = peak.max(kib * 1024);
        }
        if peak > budget || start.elapsed() > Duration::from_secs(c.worker_timeout_secs) {
            child.kill()?;
            child.wait()?;
            stopped = Some(if peak > budget {
                "RSS limit exceeded".into()
            } else {
                "setup wall-clock limit exceeded".into()
            });
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    row.peak_rss_bytes = if peak > 0 { Some(peak) } else { None };
    let usage = fs::read_to_string(&stderr)?;
    if let Some(value) = usage.lines().find_map(|s| {
        s.strip_prefix("peak_rss_bytes=")
            .and_then(|s| s.parse::<u64>().ok())
    }) {
        row.peak_rss_bytes = Some(value);
    }
    if let Some(reason) = stopped {
        row.duration_ns = None;
        row.status = if reason.starts_with("worker exit") {
            "error"
        } else {
            "infeasible"
        }
        .into();
        row.reason = reason;
        log.add(row)?;
        return Ok(false);
    }
    let result: SetupResult = serde_json::from_slice(&fs::read(&stdout)?)?;
    row.duration_ns = Some(result.setup_crypto_ns);
    row.parameter_bytes = Some(result.parameter_bytes);
    log.add(row)?;
    let mut encoding = Row::new(
        n,
        scheme,
        "both",
        attempt,
        "parameter_encoding",
        result.parameter_encoding_ns,
    );
    encoding.reason = format!(
        "{} canonical parameter bytes; encoding and SHA-256 digest, no I/O",
        result.parameter_bytes
    );
    encoding.parameter_bytes = Some(result.parameter_bytes);
    log.add(encoding)?;
    Ok(true)
}
fn command(cmd: &str, args: &[&str]) -> String {
    Command::new(cmd)
        .args(args)
        .output()
        .map(|x| String::from_utf8_lossy(&x.stdout).trim().into())
        .unwrap_or_else(|e| format!("unavailable: {e}"))
}
fn environment(c: &Config) -> Result<serde_json::Value> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let listing = Command::new("git")
        .current_dir(&root)
        .args([
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ])
        .output()?;
    let mut paths: Vec<_> = listing
        .stdout
        .split(|b| *b == 0)
        .filter_map(|b| std::str::from_utf8(b).ok())
        .filter(|p| p.ends_with(".rs") || p.ends_with(".toml") || p.ends_with("Cargo.lock"))
        .collect();
    paths.sort();
    paths.dedup();
    let mut content = Vec::new();
    for p in paths {
        if let Ok(bytes) = fs::read(root.join(p)) {
            content.extend((p.len() as u64).to_le_bytes());
            content.extend(p.as_bytes());
            content.extend((bytes.len() as u64).to_le_bytes());
            content.extend(bytes);
        }
    }
    Ok(
        serde_json::json!({"generated_at":chrono::Utc::now(),"config":c,"runs":c.runs(),"warmup":c.warmup(),"setup_runs":c.setups(),"grid":c.grid(),"rustc":command("rustc",&["-Vv"]),"cargo":command("cargo",&["-V"]),"os":command("sw_vers",&[]),"kernel":command("uname",&["-a"]),"hardware":command("sysctl",&["hw.model","machdep.cpu.brand_string","hw.memsize","hw.ncpu"]),"git_head":command("git",&["rev-parse","HEAD"]),"git_status":command("git",&["status","--porcelain"]),"source_hash_including_local_rust_toml_lock":hex::encode(digest(&content)),"binary_sha256":hex::encode(digest(&fs::read(std::env::current_exe()?)?)),"cargo_lock_sha256":hex::encode(digest(&fs::read(root.join("Cargo.lock"))?)),"debug_assertions":cfg!(debug_assertions),"rustflags":std::env::var("RUSTFLAGS").ok(),"cargo_encoded_rustflags":std::env::var("CARGO_ENCODED_RUSTFLAGS").ok(),"features":command("cargo",&["tree","-p","smp-e1","-e","features"]),"execution":"sequential; no worker parallelism; arkworks parallel feature disabled","cpu_controls":"No CPU pinning, fixed-frequency or thermal control claimed. Close competing applications. Record power mode externally.","build_flags_note":"Runtime flags may differ from build; binary hash identifies executable. See reproduction guide.","scope":"coordinate opening; public k in both modes; not anonymous membership","compressed_hidden_status":"construction_review_required; smoke is diagnostic, not final campaign"}),
    )
}
fn run(c: Config) -> Result<()> {
    fs::create_dir_all(&c.output)?;
    let lock = c.output.join("campaign.lock");
    let _lock = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock)
        .context("output in use or interrupted; use a new output directory")?;
    ensure!(
        !c.output.join("attempts.csv").exists(),
        "refusing to overwrite data"
    );
    fs::write(
        c.output.join("environment.json"),
        serde_json::to_vec_pretty(&environment(&c)?)?,
    )?;
    fs::write(
        c.output.join("build_provenance.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "compiler":env!("SMP_BUILD_RUSTC"),"target":env!("SMP_BUILD_TARGET"),"profile":env!("SMP_BUILD_PROFILE"),"opt_level":env!("SMP_BUILD_OPT_LEVEL"),"rustflags":env!("SMP_BUILD_CARGO_ENCODED_RUSTFLAGS"),"target_features":env!("SMP_BUILD_CARGO_CFG_TARGET_FEATURE"),"rayon_num_threads":std::env::var("RAYON_NUM_THREADS").ok()
        }))?,
    )?;
    if c.profile == "full" {
        fs::write(
            c.output.join("campaign_status.json"),
            serde_json::to_vec_pretty(
                &serde_json::json!({"status":"blocked_pending_research_review","reason":"Shared-response compressed hidden construction needs review: docs/E1_COMPRESSED_HIDDEN.md. No measurements fabricated."}),
            )?,
        )?;
        anyhow::bail!(
            "Full campaign gated pending compressed-hidden construction review; diagnostic smoke is available"
        );
    }
    let mut log = Log::new(&c.output)?;
    let mut datasets = StdRng::seed_from_u64(c.seed);
    let mut schedule_rng = StdRng::seed_from_u64(c.schedule_seed);
    for n in c.grid() {
        let values: Vec<Fr> = (0..n).map(|_| Fr::rand(&mut datasets)).collect();
        let dataset = values.iter().flat_map(encode).collect::<Vec<_>>();
        let data_hash = hex::encode(digest(&dataset));
        fs::write(c.output.join(format!("dataset_n{n}.bin")), dataset)?;
        let mut setups = Scheme::ALL.to_vec();
        setups.shuffle(&mut schedule_rng);
        let mut parameters = Vec::new();
        for scheme in setups {
            eprintln!("Setup {scheme:?}, n={n}");
            let mut feasible = true;
            for a in 0..c.setups() {
                if !isolated_setup(&c, scheme, n, a, &mut log)? {
                    feasible = false;
                    break;
                }
            }
            if feasible {
                parameters.push((scheme, Parameters::setup(scheme, n, &mut OsRng)?));
            }
        }
        let mut schedule = Vec::new();
        let mut gates = Vec::new();
        for (scheme, p) in &parameters {
            for mode in OpeningMode::ALL {
                correctness_gate(p, &values, mode)?;
                gates.push(serde_json::json!({"n":n,"scheme":scheme.name(),"mode":mode.name(),"status":"pass","checks":"honest/wrong value/wrong index/changed R/truncated/trailing/mutated proof"}));
            }
        }
        fs::write(
            c.output.join(format!("correctness_n{n}.json")),
            serde_json::to_vec_pretty(&gates)?,
        )?;
        for attempt in 0..c.warmup() + c.runs() {
            let k = schedule_rng.gen_range(0..n);
            let mut cells: Vec<_> = parameters
                .iter()
                .enumerate()
                .flat_map(|(i, _)| OpeningMode::ALL.map(|m| (i, m)))
                .collect();
            cells.shuffle(&mut schedule_rng);
            schedule.push(serde_json::json!({"attempt":attempt,"warmup":attempt<c.warmup(),"index":k,"cells":cells.iter().map(|(i,m)|(parameters[*i].0.name(),m.name())).collect::<Vec<_>>()}));
            for (i, mode) in cells {
                let (scheme, p) = &parameters[i];
                let blind = Fr::rand(&mut OsRng);
                let r = Fr::rand(&mut OsRng);
                let t = Instant::now();
                let commitment = p.commit(&values, blind)?;
                let commit_ns = t.elapsed().as_nanos();
                let t = Instant::now();
                let opening = p.opening_data(&values, blind, k)?;
                let opening_ns = t.elapsed().as_nanos();
                let s = Statement {
                    commitment,
                    index: k,
                    claim: match mode {
                        OpeningMode::Plain => Claim::Plain(values[k]),
                        OpeningMode::Hidden => Claim::Hidden(pedersen(values[k], r)),
                    },
                };
                let stmt_len = s.bytes().len();
                reset_io_times();
                let t = Instant::now();
                let proof = p.prove(&s, &opening, values[k], r, &mut OsRng)?;
                let prove_ns = t.elapsed().as_nanos();
                let serialization = io_times().0;
                reset_io_times();
                let t = Instant::now();
                let ok = p.verify(&s, &proof)?;
                let verify_ns = t.elapsed().as_nanos();
                let deserialization = io_times().1;
                ensure!(ok, "correctness gate failed {scheme:?}/{mode:?}");
                if attempt >= c.warmup() {
                    for (op, ns) in [
                        ("commit", commit_ns),
                        ("opening_one", opening_ns),
                        ("prove", prove_ns),
                        ("verify", verify_ns),
                    ] {
                        let mut row =
                            Row::new(n, *scheme, mode.name(), attempt - c.warmup(), op, ns);
                        row.index = Some(k);
                        row.dataset_hash = data_hash.clone();
                        row.proof_bytes = Some(proof.len());
                        row.statement_bytes = Some(stmt_len);
                        row.total_message_bytes =
                            Some(proof.len() + if mode == OpeningMode::Hidden { 48 } else { 0 });
                        if op == "prove" {
                            row.serialization_ns = Some(serialization);
                        }
                        if op == "verify" {
                            row.deserialization_ns = Some(deserialization);
                        }
                        log.add(row)?;
                    }
                }
            }
            fs::write(
                c.output.join(format!("schedule_n{n}.json")),
                serde_json::to_vec_pretty(&schedule)?,
            )?;
            eprintln!(
                "n={n}: attempt {}/{}{}",
                attempt + 1,
                c.warmup() + c.runs(),
                if attempt < c.warmup() {
                    " (warmup)"
                } else {
                    ""
                }
            );
        }
        for (scheme, p) in &parameters {
            let blind = Fr::rand(&mut OsRng);
            for attempt in 0..c.setups() {
                let t = Instant::now();
                for k in 0..n {
                    std::hint::black_box(p.opening_data(&values, blind, k)?);
                }
                log.add(Row::new(
                    n,
                    *scheme,
                    "both",
                    attempt,
                    "opening_all",
                    t.elapsed().as_nanos(),
                ))?;
            }
        }
        let bases: Vec<_> = (0..n + 2)
            .map(|i| generator(b"SMP-E1-MSM-MICROBENCH", i))
            .collect();
        let mut scalars = values.clone();
        scalars.extend([Fr::rand(&mut datasets), Fr::rand(&mut datasets)]);
        for a in 0..c.warmup() + c.runs() {
            for (size, name) in [
                (2, "msm_2"),
                (3, "msm_3"),
                (n, "msm_n"),
                (n + 1, "msm_n_plus_1"),
                (n + 2, "msm_n_plus_2"),
            ] {
                let input = std::hint::black_box((&bases[..size], &scalars[..size]));
                let t = Instant::now();
                let _ = std::hint::black_box(msm(input.0, input.1));
                let ns = t.elapsed().as_nanos();
                if a >= c.warmup() {
                    let mut row = Row::new(
                        n,
                        Scheme::EpUncompressed,
                        "primitive",
                        a - c.warmup(),
                        name,
                        ns,
                    );
                    let window = if size < 32 {
                        3
                    } else {
                        size.next_power_of_two().ilog2() * 69 / 100 + 2
                    };
                    row.reason =
                        format!("ark-ec 0.4.2 signed bucket MSM, {size} terms, window={window}");
                    log.add(row)?;
                }
            }
            let (g, h) = std::hint::black_box((G1Affine::generator(), G2Affine::generator()));
            let t = Instant::now();
            let e = Bls12_381::pairing(g, h);
            let ns = t.elapsed().as_nanos();
            if a >= c.warmup() {
                log.add(Row::new(
                    n,
                    Scheme::CfH,
                    "primitive",
                    a - c.warmup(),
                    "pairing",
                    ns,
                ))?;
            }
            let (e, scalar) = std::hint::black_box((e, values[a % n]));
            let t = Instant::now();
            let _ = std::hint::black_box(smp_vc::cfh::gt_exp(e, scalar));
            let ns = t.elapsed().as_nanos();
            if a >= c.warmup() {
                log.add(Row::new(
                    n,
                    Scheme::CfH,
                    "primitive",
                    a - c.warmup(),
                    "gt_exp",
                    ns,
                ))?;
            }
        }
        report::export(&c, &log.rows)?;
    }
    let complete = log
        .rows
        .iter()
        .filter(|r| r.operation == "prove" && r.status == "ok")
        .count()
        == c.grid().len() * 6 * c.runs();
    fs::write(
        c.output.join("campaign_status.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"status":if complete{"diagnostic_smoke_complete"}else{"diagnostic_partial"},"not_final_research":true,"rows":log.rows.len(),"pending":"compressed hidden mathematical review; full grid campaign"}),
        )?,
    )?;
    fs::remove_file(lock)?;
    println!("Diagnostic report: {}", c.output.display());
    Ok(())
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|s| s == "--setup-worker") {
        ensure!(args.len() == 3, "worker args");
        let scheme = Scheme::ALL
            .into_iter()
            .find(|s| s.name() == args[1])
            .context("scheme")?;
        let result = setup_worker(scheme, args[2].parse()?);
        peak_rss();
        return result;
    }
    if args.first().is_some_and(|s| s == "--help") {
        println!(
            "smp-e1 --profile smoke|full --output DIR [--seed N --schedule-seed N --bootstrap-seed N --max-memory-mib 4096 --worker-timeout-secs 600]\nFull gated pending compressed hidden review."
        );
        return Ok(());
    }
    run(Config::parse(args.into_iter())?)
}
fn correctness_gate(p: &Parameters, values: &[Fr], mode: OpeningMode) -> Result<()> {
    let k = values.len() / 2;
    let blind = Fr::rand(&mut OsRng);
    let r = Fr::rand(&mut OsRng);
    let s = Statement {
        commitment: p.commit(values, blind)?,
        index: k,
        claim: match mode {
            OpeningMode::Plain => Claim::Plain(values[k]),
            OpeningMode::Hidden => Claim::Hidden(pedersen(values[k], r)),
        },
    };
    let data = p.opening_data(values, blind, k)?;
    let proof = p.prove(&s, &data, values[k], r, &mut OsRng)?;
    ensure!(p.verify(&s, &proof)?, "honest proof failed");
    let mut wrong = s.clone();
    wrong.claim = match mode {
        OpeningMode::Plain => Claim::Plain(values[k] + Fr::from(1)),
        OpeningMode::Hidden => Claim::Hidden(pedersen(values[k] + Fr::from(1), r)),
    };
    ensure!(
        !p.verify(&wrong, &proof).unwrap_or(false),
        "wrong value accepted"
    );
    wrong = s.clone();
    wrong.index = (k + 1) % values.len();
    ensure!(
        !p.verify(&wrong, &proof).unwrap_or(false),
        "wrong index accepted"
    );
    wrong = s.clone();
    wrong.commitment = generator(b"SMP-E1-WRONG-COMMITMENT", 0);
    ensure!(
        !p.verify(&wrong, &proof).unwrap_or(false),
        "wrong R accepted"
    );
    let mut modified = proof.clone();
    modified[0] ^= 1;
    ensure!(
        !p.verify(&s, &modified).unwrap_or(false),
        "modified proof accepted"
    );
    let mut trailing = proof.clone();
    trailing.push(0);
    ensure!(p.verify(&s, &trailing).is_err(), "trailing bytes accepted");
    ensure!(
        p.verify(&s, &proof[..proof.len() - 1]).is_err(),
        "truncation accepted"
    );
    Ok(())
}
fn peak_rss() {
    // Process-local high-water RSS, including peaks between parent polls.
    #[cfg(unix)]
    unsafe {
        let mut usage = std::mem::zeroed::<libc::rusage>();
        if libc::getrusage(libc::RUSAGE_SELF, &mut usage) == 0 {
            let bytes = usage.ru_maxrss as u64 * if cfg!(target_os = "macos") { 1 } else { 1024 };
            eprintln!("peak_rss_bytes={bytes}");
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cli() {
        let c = Config::parse(
            ["--profile", "smoke", "--seed", "42", "--output", "x"]
                .map(str::to_string)
                .into_iter(),
        )
        .unwrap();
        assert_eq!(c.seed, 42);
        assert_eq!(c.runs(), 5);
        assert!(Config::parse(["--profile".into()].into_iter()).is_err());
        assert!(Config::parse(["--profile", "invalid"].map(str::to_string).into_iter()).is_err());
    }
    #[test]
    fn memory_guard() {
        assert!(estimate_memory(4096, Scheme::CfH) > 4 * 1024 * 1024 * 1024);
        assert!(estimate_memory(64, Scheme::CfH) < 256 * 1024 * 1024);
    }
}
