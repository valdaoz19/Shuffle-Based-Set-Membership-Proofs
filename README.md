# Shuffle-Based Set Membership Proofs

Research workspace maintained by [valda](https://github.com/valdaoz19).
This repository contains Phase 1 vector-commitment coordinate-opening experiments
and a separate legacy AML application demonstrator.

Read [Research Status](RESEARCH_STATUS.md) before interpreting the code or results.
All six Phase 1 scheme/mode combinations are executable. EP compressed hidden
is an experimental construction requiring mathematical review, and the final
experiment campaign remains gated.

## Phase 1 / E1 Experiments

E1 compares these schemes over BLS12-381 using arkworks 0.4:

| Scheme | Plain opening | Hidden opening |
| --- | --- | --- |
| EP uncompressed | Coordinate value disclosed | Coordinate value linked to a Pedersen commitment |
| EP compressed | Attema-Cramer linear-form compression | Shared-response composition requiring review |
| CF-H | Pairing-based coordinate opening | Hiding-coordinate construction and linked Pedersen commitment |

The coordinate index `k` is public in both opening modes. Hidden mode hides the
coordinate value. These experiments do not implement anonymous-index membership,
permutation or partition proofs for Phase 2.

### Reproduction

Use Rust 1.96.0, selected by `rust-toolchain.toml`. `Cargo.lock` pins dependency
versions independently. The first build needs network access for crate packages
and the pinned Persephone Git dependency used by the legacy workspace.

Run from the repository root:

```bash
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo run --locked --release -p smp-e1 -- --profile smoke --output outputs/e1-smoke
```

Smoke uses `n=64`, two warm-ups and five recorded attempts for each of the six
scheme/mode combinations. `n` is the length of the scalar vector; it is not the
number of repetitions. An existing output directory is not overwritten.

The runner creates raw CSV/JSONL logs, provenance, summary statistics, tables,
an XLSX workbook and plots. Python, Node.js and Excel are not required to run
the Rust experiment. Preserve the raw data and provenance with any shared workbook.
Local experiment outputs are excluded from Git.

The `full` profile is deliberately blocked pending review of compressed hidden.
Smoke data are diagnostic observations and must not be described as final
campaign results. See [implementation and methodology](docs/E1_IMPLEMENTATION.md)
and [compressed hidden construction](docs/E1_COMPRESSED_HIDDEN.md).

## Workspace

All 18 crates are retained for reproducibility and the application context:

| Group | Crates | Role |
| --- | --- | --- |
| E1 primitives | `smp-primitives` | BLS12-381, canonical encoding, SHA-256 transcript, MSM |
| E1 protocols | `smp-vc` | EP/CF-H setup, commitment and plain/hidden openings |
| E1 runner | `smp-e1` | Experiment schedule, measurements, reports and provenance |
| Legacy cryptography/domain | `aml-core`, `aml-crypto-ibc`, `aml-persephone`, `aml-crypto-shuffle`, `aml-crypto-membership`, `aml-crypto-window`, `aml-crypto-update`, `aml-protocol` | Dummy AML pipeline and pragmatic proof components |
| Legacy services/contracts | `aml-api`, `aml-three`, `aml-wallet`, `aml-alipay`, `aml-paypay` | All-in-one and three-server HTTP demos |
| Legacy measurement | `aml-metrics`, `aml-bench` | Membership baseline reports and Criterion benchmarks |

## Legacy AML Demonstrator

The legacy membership modes are sigma rerandomization (`explicit`), Merkle
membership (`implicit`) and bucketed subset plus sigma (`hybrid`). Their metrics
are not EP/CF-H measurements. The threshold adapter uses
[persephone-service](https://github.com/ant-dtc/persephone-service).

For a local demo, create `.env` once from `.env.example`, then launch:

```bash
cp .env.example .env
bash scripts/run_three_servers.sh
```

Wallet: `http://localhost:8891`; Alipay+: port 8892; PayPay: port 8893.
The all-in-one `aml-api` uses port 8880. These are dummy application services.
See [AML overview](AML_OVERVIEW.md), [demo guide](PANDUAN_DEMO.md),
[three-server flow](THREE_SERVER_FLOW.md) and [deployment guide](STAGING_DEPLOYMENT.md).
The Docker image builds the AML applications with Rust 1.96.0; E1 reproduction
uses the Cargo commands and methodology above.

## Research Documents

- [E1 implementation and reproduction](docs/E1_IMPLEMENTATION.md).
- [Compressed hidden construction for review](docs/E1_COMPRESSED_HIDDEN.md).
- [Diagnostic validation record](docs/E1_VALIDATION.md).
- [Compiler baseline and CI](docs/CI_TOOLCHAINS.md).
- [EP/CF fundamentals](docs/VC_EP_CF_FUNDAMENTALS.md).
- [EP/CF research analysis](docs/VC_EP_VS_CF_RESEARCH.md).
- [Membership feasibility and open questions](docs/MEMBERSHIP_PROTOCOL_FEASIBILITY.md).
- [Formal membership specification](docs/SET_MEMBERSHIP_FORMALIZATION.md).

The formal membership specification describes research targets; abstract
functionality names are not evidence that the corresponding concrete protocols
have been implemented. Final PDFs are in `output/pdf/`. Their
[generators](scripts/pdf/README.md) document dependencies and source locations.
Paper references and upstream implementation attributions are retained in the
research documents.

## Continuous Integration

CI runs format, strict Clippy and workspace tests on Ubuntu 24.04 using Rust
1.96.0 and the latest stable toolchain. Both jobs must pass. Compatibility CI
does not establish cryptographic security or replace a controlled performance
campaign. Use the recorded compiler and machine configuration when comparing data.
