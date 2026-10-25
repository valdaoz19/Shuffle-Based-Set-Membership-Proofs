# Phase 1 / E1: implementation and reproduction

Status (2026-10-24): **executable research implementation, not a completed E1 campaign**.
All three schemes and both modes have executable proofs, not simulated timings.
The compressed hidden construction is a candidate requiring mathematical review;
the final `full` campaign is gated. Smoke outputs are explicitly diagnostic.
Do not cite them as the final results of rev7.

Recorded checks and diagnostic artifacts: [E1_VALIDATION.md](E1_VALIDATION.md).

## Scope and source mapping

Primary specification: `report_phase1_shuffle_smp_rev7.pdf`, supplied by the stakeholder.
Compression source: [Attema-Cramer, Protocol 5, Section 5 and Appendix E](https://ir.cwi.nl/pub/30160/30160.pdf).
The [Dock compressed-linear-form implementation](https://github.com/docknetwork/crypto/blob/main/compressed_sigma/src/compressed_linear_form.rs)
was inspected as a reference, not imported as a dependency or independently audited oracle.

| Requirement / reference | Implementation | Important qualification |
| --- | --- | --- |
| Rev7 Table 7, CF-H setup | `smp-vc/src/cfh.rs`, `Parameters::setup` | Independent nonzero z and y; no diagonal cross terms; z/y buffers zeroized |
| CF-H commitment/opening/rerandomization | `commit`, `opening`, `rerandomize` | Hiding coordinate n, public position k < n |
| Rev7 Box 3 | CF-H `prove`, `verify` | Ek recomputed inside both operations; constant E cached at setup |
| Rev7 Table 9, uncompressed EP | `smp-vc/src/ep.rs` | Shared response for v[k] in hidden mode |
| Attema-Cramer Protocol 5 | EP compressed plain | Recursive and original-basis verifiers; explicit padding nullity binding |
| Compressed hidden opening | `docs/E1_COMPRESSED_HIDDEN.md` | New composition, not claimed verbatim Protocol 5; final review gate |
| Rev7 Tables 19-21 | `smp-e1/src/main.rs` | Single-thread sequential measurements, no Linux CPU-pinning claim |
| Rev7 M1-M5 / Table 22 | Long-format operation rows | Missing/infeasible != zero; operation units ns |
| Rev7 Tables A-C / Figures 2-3 | `smp-e1/src/report.rs` | Measured costs, no extrapolated crossover |

This measures **one-coordinate openings**, not three anonymous SMP formations.
`k` is public in BOTH modes. Hidden means the value is hidden behind `C`.
Neither permutation nor partition Phase 2 is implemented here. No HTTP/network,
Persephone, Merkle membership, or AML threshold result enters the E1 measurements.

## Crates and interfaces

- `smp-primitives`: BLS12-381 types, canonical encoding/decoding, SHA-256 transcript,
  independently hash-to-curve-derived EP/Pedersen generators, MSM, timer counters.
- `smp-vc`: `Scheme::{EpUncompressed, EpCompressed, CfH}`;
  `OpeningMode::{Plain, Hidden}`; `Parameters::setup`, `commit`, `opening_data`.
  Typed `PlainStatement`, `HiddenStatement`, `HiddenWitness`, `prove_plain`,
  `verify_plain`, `prove_hidden`, `verify_hidden`; verifier takes bytes.
- `smp-e1`: isolated setup workers, experiment schedule, data export and statistics.

EP opening data borrow the committed vector, not a recomputed proof. CF-H opening
data contain only Lambda_k. For the experimental setup, the same process creates
parameters and then drops/zeroizes setup scalars; it is NOT a distributed trusted
setup ceremony or a production key-management system. Rust heap zeroization does
not claim to erase every possible compiler/register/OS copy of a secret.

All arkworks crates resolve to the 0.4 family. Their `parallel` features are disabled.
`Cargo.lock` is intentionally no longer ignored. Build/run with `--locked`.
The research compiler is pinned to Rust 1.96.0 in `rust-toolchain.toml` and the
Docker builder. CI also tests the latest stable compiler on Ubuntu 24.04. Those
compatibility tests do not constitute a new benchmark campaign. See
[CI_TOOLCHAINS.md](CI_TOOLCHAINS.md) for version selection and validation.
The legacy workspace contains unrelated parallel libraries; inspect `cargo tree
-p smp-e1 -e features`, not just the entire workspace dependency graph.

## Run

From the repository root, first check the implementation:

```bash
cargo test --locked -p smp-primitives -p smp-vc -p smp-e1
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
```

Build once, then run the same release executable:

```bash
RUSTFLAGS="-C target-cpu=native" cargo build --locked --release -p smp-e1
RAYON_NUM_THREADS=1 target/release/smp-e1 --profile smoke --output outputs/e1-smoke
```

The requested shorthand also works (using the build flags in your environment):

```bash
cargo run --locked --release -p smp-e1 -- --profile smoke --output outputs/e1-smoke
```

Optional reproducible PUBLIC experiment seeds:

```bash
cargo run --locked --release -p smp-e1 -- --profile smoke \
  --seed 42 --schedule-seed 43 --bootstrap-seed 44 \
  --max-memory-mib 4096 --worker-timeout-secs 600 \
  --output outputs/e1-smoke-seeded
```

Do not confuse these seeds with cryptographic setup or proof randomness. Those use
`OsRng` and are not exported. Reusing public experiment seeds reproduces values and
schedule, NOT proof bytes or timings. Raw canonical scalar data are stored too,
since RNG algorithm changes across library upgrades can change seeded sequences.

Output directories are not overwritten. A `campaign.lock` also survives an
interruption to protect partial results. Use a new directory for another run.

```bash
cargo run --locked --release -p smp-e1 -- --profile full --output outputs/e1
```

Currently this writes a **blocked_pending_research_review** status and exits with
an error before benchmarking. It does not silently drop compressed hidden, use
simulation, or fabricate a successful full campaign. See the review document.
There is deliberately no CLI option that labels this candidate approved.

## Profiles and controls

| Profile | n | Warm-up / cell | Recorded / cell | Setup / scheme |
| --- | --- | ---: | ---: | ---: |
| smoke | 64 | 2 | 5 | 1 |
| full (gated) | 64,128,256,512,1024,2048,4096 | 10 | 100 | 5 |

For every n the scalar vector is shared by all cells. Each attempt draws one
uniform position shared by the six cells, then shuffles their execution order.
Setup scheme order is shuffled separately. There are no simultaneous benchmarking
threads. Parameter construction used by the main process is outside measured
opening loops and separate from the isolated setup samples.

Indices/attempt numbers are zero-based. Schedule attempt includes warm-ups;
CSV attempt starts at zero again for recorded samples. Setup samples and primitive
samples have their own counters. Do not join rows by attempt number alone: use
n, scheme, mode, operation and attempt. Each row's index is the actual opened k.

`getrusage(RUSAGE_SELF)` in each setup child records its actual process high-water
RSS (bytes on macOS, KiB converted on Linux). Parent `ps` polling enforces a memory
ceiling and wall-clock timeout; it is not inside prove/verify timers. Polling is
not a hard OS allocation sandbox and cannot prevent every transient overshoot.
An estimate of 128 MiB + 256*n^2 bytes for CF-H is checked before spawning.
At n=4096 that estimate exceeds the default 4096 MiB cap: report `infeasible`
under that cap, not zero performance. Raise the cap only after reviewing available
RAM. Published parameter bytes and in-memory RSS are different measurements.

No taskset, frequency lock, disabled turbo, thermal stability or performance-core
affinity is claimed on this Mac. Avoid competing work during a real campaign.
`build_provenance.json` captures compiler, target, optimization and flags at build
time; `environment.json` captures runtime machine, OS, dependency features, binary
hash and a hash of tracked AND untracked local Rust/TOML/lock sources.

## Encoding and timer boundaries

Canonical **arkworks 0.4** compressed G1/G2: 48/96 bytes; Fr: 32 bytes; GT: 576
bytes. This identifies a library encoding; it is not a promise of compatibility
with every BLS wire format. Decoder validates subgroup/range, requires exact proof
length and re-encodes to reject noncanonical infinity/flag representations.
Trailing bytes are rejected. Point-at-infinity itself is legal where algebraically
valid; malformed alternative infinity encodings are not.

Challenges use length-delimited domains/labels/bytes, binding scheme, mode,
parameter digest, R, public k, x/C, first messages and all prior rounds. Two
domain-separated SHA-256 blocks are reduced to Fr to avoid biased 256-bit reduction.
Native CF-H plain is a pairing equation, not a Fiat-Shamir protocol; there is no
imaginary challenge/domain-check claimed for that mode.

| Operation | Included | Excluded |
| --- | --- | --- |
| setup_crypto | Parameter generation, common bases, CF-H constant E cache | Parameter encoding/digest and file I/O |
| parameter_encoding | Canonical parameter encoding and SHA-256 digest | Filesystem I/O |
| commit | VC MSM + affine normalization | Dataset generation |
| opening_one | EP borrowed handle / CF-H Lambda MSM | Proof generation |
| opening_all | All n data computations, once per setup repetition | Prove/verify |
| prove | Nonces, algebra, transcripts, proof encoding | Opening-data preparation and C/statement construction |
| verify | Decode, canonical re-encode check, subgroup/range checks, algebra, transcript | Parameter/statement creation or parsing |
| serialization_ns | Actual proof serialization calls inside prove | Transcript/parameter serialization |
| deserialization_ns | Successful proof decoding, validation, canonical check | Algebra/transcript work |
| msm_2 / msm_3 / msm_n / msm_n_plus_1 / msm_n_plus_2 / pairing / gt_exp | Actual arkworks primitive operation, with warm-up | Input generation and logging |

Timer-counter overhead is included; it is common methodology, not subtracted
afterwards. `duration - serialization/deserialization` is a useful accounting
view, not an independent timing measurement. CF-H computes Ek per operation,
but caches constant E=e(G1,G2). No cached-Ek benchmark is claimed in this revision.
The statement's Pedersen C is prepared outside prove; report this boundary when
comparing with another implementation which includes C construction.

The primitive MSM rows record the ark-ec 0.4.2 signed-bucket window: 3 below
32 terms, otherwise floor(0.69*ceil(log2(terms)))+2. No custom AVX/ADX/blst path
or small-MSM Straus optimization is claimed. Build target features are recorded.
Bootstrap seeds are text in Excel to avoid loss of precision beyond 2^53.

CF-H parameter encoding includes the G1/G2 generators, U, Uhat, cross terms,
shared Pedersen G/H, and a version/n header. EP encoding also includes G/H; only
compressed EP adds independent padding bases and K. These implementation parameter
sizes are larger than Table 9's mathematical core; they are reported as actual bytes.
Verifiers currently hold a full parameter object. Do not claim this benchmark
demonstrates the report's 288-byte compact verifier storage.

## Reading results

| File | Meaning |
| --- | --- |
| `attempts.csv`, `attempts.jsonl` | One row per timed operation per attempt, flushed after each row |
| `dataset_n64.bin` | Canonical public synthetic scalar vector; SHA-256 in attempt rows |
| `schedule_n64.json` | Position, cell order, warm-up marker |
| `correctness_n64.json` | Pre-timing checks of all six cells at that n |
| `setup_*.json`, `setup_*.time.txt` | Isolated setup computation/encoding/parameter size and process peak RSS |
| `environment.json`, `build_provenance.json` | Runtime and build provenance |
| `summary.json`, `summary.md` | Statistics, measured amortization and limits |
| `table_a.md`, `table_b.md`, `table_c.md` | Rev7 Tables A-C layout with actual data |
| `e1_report.xlsx` | README, Table A, Statistics, Attempt Log, Table B, Table C, Charts |
| `prove_vs_n.svg`, `verify_vs_n.svg` | Median timing against measured n only |
| `campaign_status.json` | Diagnostic completion or final-campaign block |

Proof bytes exclude C. Statement bytes are 88 plain or 104 hidden (R48 + k8 +
x32/C48). `total_message_bytes` means NEW prover transmission given R,k already
known: proof alone in plain, proof+C48 in hidden. Thus CF-H hidden is 176/104/224
for proof/statement/sent respectively. It is not 176+the whole statement.

Statistics: arithmetic mean, sample standard deviation (undefined for one sample),
linear-interpolated quantiles, median, Q1/Q3, IQR, min/max. CI95 is the percentile
bootstrap interval of the mean, using 10,000 resamples and a recorded seed per
group. Five smoke samples are a sanity check, not an adequate publication dataset.

Table C uses measured hidden prove+verify, not Table 9's analytical upper-bound
claim. Fresh adds each scheme's measured opening_one cost. Reuse excludes it from
the per-opening cost; its one-time CF-H-minus-EP preparation difference is added
to the numerator for N*. Delta S is measured CF-H
setup minus measured EP setup. N*=ceil(max(Delta S + initial_data_delta,0)/savings) only for positive
savings. The main estimate excludes commit costs as in Section 2.1.8; Table B
reports them so differing measured costs are visible. In particular, no assumption
is made that uncompressed break-even bounds compressed EP.

Only measured adjacent grid sign changes produce a crossover bracket. A one-n
smoke cannot establish a crossover, nor does already-faster at the first n prove
where a crossing occurred. Failed/infeasible rows have absent timings and reasons.

## Remaining completion gates

1. Review the compressed hidden composition, padding checks and proof/security
   differences in `E1_COMPRESSED_HIDDEN.md`. Executable tests are not a security proof.
2. Close any resulting protocol changes and rerun adversarial/differential tests.
3. Enable the final profile only with a documented research decision, then run
   the entire grid under one release binary, resource budget and recorded controls.
4. Inspect final workbook/plots and distinguish infeasible cells from missing work.

Existing AML application code and membership primitives have not been replaced.
