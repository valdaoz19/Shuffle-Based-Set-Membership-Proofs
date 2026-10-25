# Legacy AML Cross-Border Demonstrator

This guide covers the AML application and its legacy membership benchmarks.
For the Phase 1 vector-commitment experiments, use the [research README](README.md)
and [research status](RESEARCH_STATUS.md). The two benchmark families measure
different statements and must be interpreted separately.

Privacy-preserving Anti-Money Laundering (AML) verification for cross-border payments using Zero-Knowledge Proofs.

## Case Study

Verify whether a user exceeds a public transaction threshold (τ) within a **1-year sliding window** in Singapore — without revealing identity or exact transaction amounts.

## Cryptographic Pipeline

```
Set Membership → IBC (Commit, IdEq, VVer) → Verifiable Shuffle → Sliding Window → ZK-MNNP Threshold
```

| Component | Crate | Source |
|-----------|-------|--------|
| Set membership | `aml-crypto-membership` | Explicit shuffle, implicit VC-like, hybrid bucketed PoC |
| IBC + Π.IdEq + Π.VVer | `aml-crypto-ibc` | Custom (Pedersen Ristretto255) |
| Value verification / ZK-MNNP | `aml-persephone` | [persephone-service](https://github.com/ant-dtc/persephone-service) |
| Verifiable shuffle | `aml-crypto-shuffle` | Custom |
| Sliding window + to-do list | `aml-crypto-window` | Custom |
| Oblivious update | `aml-crypto-update` | Custom (simplified sigma) |
| Orchestrator | `aml-protocol` | Custom |
| HTTP API + audit trail | `aml-api` | Axum + SQLite (`sqlx`) |
| Three-server dummy integration | `aml-wallet`, `aml-alipay`, `aml-paypay` | Axum services |
| Metrics runner | `aml-metrics` | JSON + Markdown report |

This PoC intentionally uses a hybrid/pragmatic approach. The shuffle and
oblivious update layers are simplified sigma-protocol/Rust implementations,
while the threshold check uses the more mature ZK-MNNP module from
`persephone-service`. See [HYBRID_APPROACH.md](HYBRID_APPROACH.md).

## Requirements

- Rust 1.96.0 workspace baseline (edition 2024), installed through rustup
- Network access for first build (git dependency on persephone-service)
- SQLite is embedded through `sqlx`; no separate database server is required

## Compiler and CI

Local commands use the pinned Rust 1.96.0 toolchain. The Docker builder uses
the same version. `Cargo.lock` pins dependencies separately from the compiler.
CI runs format, strict Clippy, and workspace tests on Ubuntu 24.04 with both
Rust 1.96.0 and the latest stable toolchain; both jobs must pass.

See [CI toolchains and failure diagnosis](docs/CI_TOOLCHAINS.md) for the
compiler-version logs, local compatibility checks, and the Clippy parsing fix.

## Quick Start

```bash
# Run tests
cargo test --workspace

# Start legacy all-in-one API + web UI
cargo run -p aml-api
# Open http://localhost:8880

# Start the main three-server stakeholder demo
bash scripts/run_three_servers.sh
# Open http://localhost:8891

# Generate metrics report
cargo run -p aml-metrics -- --runs 100 --n 100
cargo run -p aml-metrics -- --runs 1000 --n 100 --modes explicit,implicit,hybrid

# Demo curl
bash assets/verify_demo.sh
```

Three-server Set Membership mode is selected through `.env` or shell env:

```bash
AML_MEMBERSHIP_MODE=explicit # default sigma shuffle
AML_MEMBERSHIP_MODE=implicit # vector-commitment-like Merkle PoC
AML_MEMBERSHIP_MODE=hybrid   # bucketed subset + explicit proof
```

The three-server payment flow uses one membership mode per request. This mirrors
a real prover/verifier agreement on one proof primitive. To compare all modes,
use the Wallet UI **Run Membership Comparison** button or:

```bash
cargo run -p aml-metrics -- --runs 1000 --n 100 --modes explicit,implicit,hybrid
```

Current PoC interpretation:

| Mode | Purpose |
| --- | --- |
| `explicit` | Default demo mode; closest to the approved shuffle-based baseline. |
| `implicit` | Fastest/smallest vector-commitment-like Merkle baseline. |
| `hybrid` | Bucketed subset compromise between explicit and implicit. |

By default the API stores audit records in `data/aml_audit.sqlite`. Override
with:

```bash
AML_AUDIT_DB_URL=sqlite:/tmp/aml_audit.sqlite cargo run -p aml-api
```

For simpler local/staging runs, copy `.env.example` to `.env` and edit values:

```bash
cp .env.example .env
cargo run -p aml-api
```

Existing shell environment variables override `.env` values. For one-off
staging hardening without editing `.env`:

```bash
AML_API_KEY=change-me \
AML_RATE_LIMIT_PER_MINUTE=60 \
AML_CORS_ORIGINS=http://localhost:8880 \
cargo run -p aml-api
```

Three-server flow notes are in [THREE_SERVER_FLOW.md](THREE_SERVER_FLOW.md).
Docker staging notes are in [STAGING_DEPLOYMENT.md](STAGING_DEPLOYMENT.md).
Server sizing and separated deployment notes are in
[SERVER_REQUIREMENTS.md](SERVER_REQUIREMENTS.md).

## API Endpoints

See [VERIFICATION_ENDPOINT.md](VERIFICATION_ENDPOINT.md) for full request/response schema, scenarios, and integration test guide.

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/health` | Health check |
| GET | `/api/v1/config` | Public τ and window config |
| GET | `/api/v1/demo/scenarios` | List APPROVED/REJECTED demo scenarios |
| POST | `/api/v1/demo/setup` | Initialize demo user + c_tid (`{"scenario":"compliant"}`) |
| POST | `/api/v1/verify` | Run full ZKP verification pipeline → `APPROVED` / `REJECTED` |
| GET | `/api/v1/proofs/{request_id}` | Fetch stored audit proof metadata |
| POST | `/api/v1/proofs/{request_id}/verify` | Recompute and verify audit payload hash |
| GET | `/api/v1/ledger/commitments` | List append-only commitment ledger entries |

## Workspace Layout

```
crates/
  aml-core/           Domain types
  aml-crypto-membership/ Pluggable set membership proof modes
  aml-crypto-ibc/     Identity Binding Commitment
  aml-persephone/     persephone-service adapter
  aml-crypto-shuffle/ Verifiable shuffle
  aml-crypto-window/  Sliding window
  aml-crypto-update/  Oblivious update
  aml-protocol/       Pipeline orchestrator
  aml-api/            Legacy all-in-one Axum server + SQLite audit
  aml-three/          Shared DTOs for three-server HTTP contracts
  aml-wallet/         User Wallet owner dummy server
  aml-alipay/         Alipay+ client/verifier dummy server
  aml-paypay/         PayPay server/prover dummy server
  aml-metrics/        Stakeholder metrics runner
  aml-bench/          Criterion benchmarks
web/                  Tailwind demo UI
assets/               Sample scripts
scripts/              Multi-service launchers
```

## Benchmarks

```bash
cargo bench -p aml-bench
cargo bench -p aml-crypto-shuffle
cargo run -p aml-metrics -- --runs 1000 --n 100
```

Targets (from IBC reference deck): IBC commit <10ms, verify <5ms; shuffle n=100 ~50ms prove / ~15ms verify.

## Reference Documents

- `Zero-Knowledge Multiplicative Non-Negative Proof (ZK-MNNP) _compressed.pdf`
- `Verifiable_Two_Way_Private_Information_Retrieval_compressed.pdf`
- `Identity Binding Commitment_compressed.pdf`
- `HYBRID_APPROACH.md`
- `THREE_SERVER_FLOW.md`
- `STAGING_DEPLOYMENT.md`
- `SERVER_REQUIREMENTS.md`
- `.env.example`
