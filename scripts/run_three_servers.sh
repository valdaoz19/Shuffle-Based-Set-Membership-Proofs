#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

if [[ -f .env ]]; then
  while IFS= read -r raw_line || [[ -n "$raw_line" ]]; do
    line="${raw_line#"${raw_line%%[![:space:]]*}"}"
    line="${line%"${line##*[![:space:]]}"}"
    [[ -z "$line" || "$line" == \#* ]] && continue
    line="${line#export }"
    [[ "$line" == *=* ]] || continue
    key="${line%%=*}"
    value="${line#*=}"
    key="${key%"${key##*[![:space:]]}"}"
    [[ "$key" =~ ^[A-Za-z_][A-Za-z0-9_]*$ ]] || continue
    value="${value#"${value%%[![:space:]]*}"}"
    value="${value%"${value##*[![:space:]]}"}"
    value="${value%\"}"
    value="${value#\"}"
    value="${value%\'}"
    value="${value#\'}"
    if [[ -n "$key" && -z "${!key:-}" ]]; then
      export "$key=$value"
    fi
  done < .env
fi

export WALLET_PORT="${WALLET_PORT:-8891}"
export ALIPAY_PORT="${ALIPAY_PORT:-8892}"
export PAYPAY_PORT="${PAYPAY_PORT:-8893}"
export ALIPAY_BASE_URL="${ALIPAY_BASE_URL:-http://localhost:${ALIPAY_PORT}}"
export PAYPAY_BASE_URL="${PAYPAY_BASE_URL:-http://localhost:${PAYPAY_PORT}}"

require_port_available() {
  local name="$1"
  local port="$2"

  if ! command -v lsof >/dev/null 2>&1; then
    return 0
  fi

  local listeners
  listeners="$(lsof -nP -iTCP:"$port" -sTCP:LISTEN | tail -n +2 || true)"
  if [[ -n "$listeners" ]]; then
    echo "Port ${port} for ${name} is already in use:"
    echo "$listeners"
    echo
    echo "Stop the existing process or choose different ports, for example:"
    echo "  WALLET_PORT=8991 ALIPAY_PORT=8992 PAYPAY_PORT=8993 \\"
    echo "  ALIPAY_BASE_URL=http://localhost:8992 PAYPAY_BASE_URL=http://localhost:8993 \\"
    echo "  bash scripts/run_three_servers.sh"
    return 1
  fi
}

PIDS=()
cleanup() {
  if (( ${#PIDS[@]} == 0 )); then
    return
  fi
  for pid in "${PIDS[@]}"; do
    kill "$pid" 2>/dev/null || true
  done
}
trap cleanup EXIT INT TERM

wait_for_url() {
  local name="$1"
  local url="$2"
  local pid="$3"
  local attempts="${4:-60}"

  printf "Waiting for %s health" "$name"
  for _ in $(seq 1 "$attempts"); do
    if ! kill -0 "$pid" 2>/dev/null; then
      printf " failed\n"
      echo "${name} process exited before health check passed."
      return 1
    fi
    if curl -fsS "$url" >/dev/null 2>&1; then
      printf " ok\n"
      return 0
    fi
    printf "."
    sleep 1
  done
  printf " timeout\n"
  echo "${name} did not become healthy at ${url}"
  return 1
}

require_port_available "Wallet" "$WALLET_PORT"
require_port_available "Alipay+" "$ALIPAY_PORT"
require_port_available "PayPay" "$PAYPAY_PORT"

echo "Building three-server binaries..."
cargo build -p aml-paypay -p aml-alipay -p aml-wallet

BIN_DIR="${CARGO_TARGET_DIR:-target}/debug"

echo "Starting PayPay prover on :${PAYPAY_PORT}"
"${BIN_DIR}/aml-paypay" &
PIDS+=("$!")
paypay_pid="$!"
wait_for_url "PayPay" "http://localhost:${PAYPAY_PORT}/api/v1/paypay/health" "$paypay_pid"

echo "Starting Alipay+ verifier on :${ALIPAY_PORT}"
"${BIN_DIR}/aml-alipay" &
PIDS+=("$!")
alipay_pid="$!"
wait_for_url "Alipay+" "http://localhost:${ALIPAY_PORT}/api/v1/alipay/health" "$alipay_pid"

echo "Starting Wallet owner on :${WALLET_PORT}"
"${BIN_DIR}/aml-wallet" &
PIDS+=("$!")
wallet_pid="$!"
wait_for_url "Wallet" "http://localhost:${WALLET_PORT}/api/v1/wallet/health" "$wallet_pid"

echo
echo "Three-server demo is ready:"
echo "  Wallet   http://localhost:${WALLET_PORT}"
echo "  Alipay+  http://localhost:${ALIPAY_PORT}/api/v1/alipay/health"
echo "  PayPay   http://localhost:${PAYPAY_PORT}/api/v1/paypay/health"
echo
echo "Press Ctrl+C to stop all services."
wait
