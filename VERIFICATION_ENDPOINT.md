# Verification Endpoint & Integration Guide

This document describes the official AML verification mechanism exposed by `aml-api` at `POST /api/v1/verify`.

## Overview

The endpoint executes the legacy/all-in-one cryptographic pipeline:

```
Set Membership → IBC (IdEq + VVer) → ZKNNP (historical txs) → Verifiable Shuffle → Sliding Window → Oblivious Update → ZK-MNNP Threshold
```

It returns a structured compliance verdict without revealing user identity or exact transaction amounts.

Each verification is also written to the SQLite audit store. The demo user
state is still in-memory, but proof metadata, audit hashes, and commitment
ledger entries persist in `data/aml_audit.sqlite` by default.

For the main stakeholder demo, use the three-server flow documented in
`THREE_SERVER_FLOW.md`. This file focuses on the all-in-one `/api/v1/verify`
endpoint and audit endpoints.

## Endpoint

| Property | Value |
|----------|-------|
| Method | `POST` |
| Path | `/api/v1/verify` |
| Content-Type | `application/json` |
| Default port | `8880` |

If `AML_API_KEY` is set, protected endpoints require:

```text
x-api-key: <AML_API_KEY>
```

Configuration can be supplied inline when running the server or through `.env`
copied from `.env.example`. Shell environment variables override `.env` values.

### Request body

```json
{
  "amount_cents": 200000,
  "tx_id_commitment": "<optional base64 c_tid from /api/v1/demo/setup>",
  "user_key": "<optional, defaults to demo user>",
  "scenario": "<optional: compliant or exceeded>"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `amount_cents` | `u64` | Yes | Simulated incoming payment in SGD cents |
| `tx_id_commitment` | `string` | No | IBC transaction ID commitment; uses demo user if omitted |
| `user_key` | `string` | No | Demo server user key; defaults to the active demo user |
| `scenario` | `string` | No | Demo scenario helper: `compliant` or `exceeded` |

### Response body

```json
{
  "status": "APPROVED",
  "execution_time_ms": 142,
  "crypto_proof_metadata": {
    "pipeline": "Set Membership → IBC → Verifiable Shuffle → Sliding Window → ZK-MNNP Threshold",
    "set_membership": "{...}",
    "ibc_identity_equality": "{...}",
    "ibc_value_verification": "{...}",
    "verifiable_shuffle": "{...}",
    "sliding_window": "{...}",
    "oblivious_update": "{...}",
    "zk_mnnp_threshold": "zk-mnnp:ok=true",
    "commitment_handles": ["...", "..."]
  },
  "request_id": "uuid",
  "message": "Payment approved: within AML threshold for sliding window",
  "threshold_compliant": true,
  "proof_audit_hash": "hex-sha3-256",
  "replay_warning": false,
  "audit_recorded": true
}
```

| Field | Description |
|-------|-------------|
| `status` | `APPROVED` (compliant) or `REJECTED` (threshold exceeded or proof failure) |
| `execution_time_ms` | Wall-clock time for the ZK-MNNP threshold verification phase |
| `crypto_proof_metadata` | Serialized IBC, shuffle, and threshold artifacts for academic audit |
| `request_id` | Unique verification request identifier |
| `message` | Human-readable outcome |
| `threshold_compliant` | Boolean result of threshold check |
| `proof_audit_hash` | SHA3-256 hash over the stored audit payload |
| `replay_warning` | `true` if the same `tx_id_commitment` was seen before; verification still runs for demo compatibility |
| `audit_recorded` | Whether SQLite audit persistence succeeded |

Validation:

- `amount_cents` must be greater than `0`
- `amount_cents` must be less than or equal to `max_tx_cents` from `/api/v1/config`
- If `AML_RATE_LIMIT_PER_MINUTE` is set, protected endpoints return `429` after the global per-minute limit is exceeded

## Audit Endpoints

### Fetch stored proof metadata

```bash
curl -s http://localhost:8880/api/v1/proofs/<request_id> | jq .
```

Returns the stored audit record, including `proof_metadata`,
`proof_audit_hash`, threshold config, scenario, status, and replay warning.

### Verify stored audit hash

```bash
curl -s -X POST http://localhost:8880/api/v1/proofs/<request_id>/verify | jq .
```

This recomputes the SHA3-256 audit hash from the stored payload and compares it
with `proof_audit_hash`. It does not rerun the full ZKP pipeline.

### List commitment ledger

```bash
curl -s http://localhost:8880/api/v1/ledger/commitments | jq .
```

Returns the latest append-only commitment handles generated from verification
proof bundles. There are no public update/delete endpoints.

## Scenarios

### Scenario A — APPROVED

**Condition:** Total spend in the 1-year sliding window (history + new amount) is strictly less than public threshold τ.

**Demo defaults:**

- τ = SGD 20,000 (`2_000_000` cents)
- Demo user history = SGD 12,000 (`1_200_000` cents)
- Example passing request: `amount_cents: 200_000` → total `1_400_000 < 2_000_000`

```bash
curl -s -X POST http://localhost:8880/api/v1/verify \
  -H 'Content-Type: application/json' \
  -d '{"amount_cents": 200000}' | jq .
```

**Expected:** `"status": "APPROVED"`, `"threshold_compliant": true`

### Scenario B — REJECTED

**Condition:** Total spend exceeds τ; ZK-MNNP verification fails mathematically.

**Example failing request:** `amount_cents: 900_000` → total `2_100_000 > 2_000_000`

```bash
curl -s -X POST http://localhost:8880/api/v1/verify \
  -H 'Content-Type: application/json' \
  -d '{"amount_cents": 900000}' | jq .
```

**Expected:** `"status": "REJECTED"`, `"threshold_compliant": false`

## Integration Tests

### `aml-core`

Unit tests in `crates/aml-core/src/types.rs` verify `VerifyApiResponse::from_result` mapping for APPROVED and REJECTED.

```bash
cargo test -p aml-core
```

### `aml-protocol`

Pipeline integration tests in `crates/aml-protocol/tests/integration.rs`:

- `scenario_a_pipeline_approves_under_threshold`
- `scenario_b_pipeline_rejects_over_threshold`

```bash
cargo test -p aml-protocol --test integration
```

### `aml-api`

HTTP integration tests in `crates/aml-api/tests/verify_integration.rs`:

- `scenario_a_approves_when_under_threshold`
- `scenario_b_rejects_when_threshold_exceeded`
- proof audit lookup and hash verification
- amount validation
- replay warning
- commitment ledger persistence

```bash
cargo test -p aml-api --test verify_integration
```

Run the full suite:

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

## Legacy All-In-One Web UI

Static assets in `web/` are served by `aml-api`.

1. Start server: `cargo run -p aml-api`
2. Open: http://localhost:8880
3. Click **Run AML Verification Check**
4. Toggle **Technical Academic Proof (ZKP Metadata)** to inspect `crypto_proof_metadata`

This UI is kept for `/api/v1/verify` and SQLite audit demos. The main
stakeholder UI is the Wallet UI at `http://localhost:8891`.

UI badges:

| Status | Color |
|--------|-------|
| `APPROVED` | Green |
| `REJECTED` | Red |

The main three-server Wallet UI is served by `aml-wallet` at
`http://localhost:8891` and includes the Set Membership mode comparison panel.

## Crate Map

| Crate | Role |
|-------|------|
| `aml-core` | `VerifyApiResponse`, `CryptoProofMetadata`, `VerificationStatus` |
| `aml-crypto-membership` | Set Membership proof modes and thresholds |
| `aml-protocol` | Pipeline orchestrator + timing for ZK-MNNP phase |
| `aml-api` | HTTP handler, SQLite audit store, proof lookup, commitment ledger |
| `aml-wallet`, `aml-alipay`, `aml-paypay` | Three-server dummy integration |
| `aml-metrics` | Metrics report and membership comparison |

## Related Files

- [`crates/aml-core/src/types.rs`](crates/aml-core/src/types.rs) — response types
- [`crates/aml-api/src/routes.rs`](crates/aml-api/src/routes.rs) — `/api/v1/verify` handler
- [`crates/aml-api/src/audit.rs`](crates/aml-api/src/audit.rs) — SQLite audit store and hash verification
- [`crates/aml-protocol/src/orchestrator.rs`](crates/aml-protocol/src/orchestrator.rs) — pipeline + `zk_mnnp_execution_time_ms`
- [`web/index.html`](web/index.html) — demo UI
- [`assets/verify_demo.sh`](assets/verify_demo.sh) — curl samples
