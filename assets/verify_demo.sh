#!/usr/bin/env bash
# Demo curl script — run while `cargo run -p aml-api` is listening on :8880

set -euo pipefail

BASE="${1:-http://localhost:8880}"

curl_auth() {
  if [[ -n "${AML_API_KEY:-}" ]]; then
    curl -s -H "x-api-key: $AML_API_KEY" "$@"
  else
    curl -s "$@"
  fi
}

echo "=== Health ==="
curl_auth "$BASE/api/v1/health" | jq .

echo
echo "=== Available demo scenarios ==="
curl_auth "$BASE/api/v1/demo/scenarios" | jq .

echo
echo "=== Scenario A — APPROVED (compliant) ==="
SETUP_A=$(curl_auth -X POST "$BASE/api/v1/demo/setup" \
  -H 'Content-Type: application/json' \
  -d '{"scenario":"compliant"}')
echo "$SETUP_A" | jq .
USER_A=$(echo "$SETUP_A" | jq -r .user_key)
TX_A=$(echo "$SETUP_A" | jq -c .tx_id_commitment)
RESP_A=$(curl_auth -X POST "$BASE/api/v1/verify" \
  -H 'Content-Type: application/json' \
  -d "{\"amount_cents\":200000,\"user_key\":\"$USER_A\",\"tx_id_commitment\":$TX_A,\"scenario\":\"compliant\"}")
echo "$RESP_A" | jq .
REQ_A=$(echo "$RESP_A" | jq -r .request_id)

echo
echo "=== Audit proof lookup ==="
curl_auth "$BASE/api/v1/proofs/$REQ_A" | jq .

echo
echo "=== Audit proof hash verify ==="
curl_auth -X POST "$BASE/api/v1/proofs/$REQ_A/verify" | jq .

echo
echo "=== Scenario B — REJECTED (exceeded) ==="
SETUP_B=$(curl_auth -X POST "$BASE/api/v1/demo/setup" \
  -H 'Content-Type: application/json' \
  -d '{"scenario":"exceeded"}')
echo "$SETUP_B" | jq .
USER_B=$(echo "$SETUP_B" | jq -r .user_key)
TX_B=$(echo "$SETUP_B" | jq -c .tx_id_commitment)
curl_auth -X POST "$BASE/api/v1/verify" \
  -H 'Content-Type: application/json' \
  -d "{\"amount_cents\":500000,\"user_key\":\"$USER_B\",\"tx_id_commitment\":$TX_B,\"scenario\":\"exceeded\"}" | jq .

echo
echo "=== Commitment ledger ==="
curl_auth "$BASE/api/v1/ledger/commitments" | jq .

echo
echo "=== Repeat verify (same user, should stay APPROVED) ==="
curl_auth -X POST "$BASE/api/v1/verify" \
  -H 'Content-Type: application/json' \
  -d "{\"amount_cents\":200000,\"user_key\":\"$USER_A\",\"tx_id_commitment\":$TX_A,\"scenario\":\"compliant\"}" | jq '.status, .message'
