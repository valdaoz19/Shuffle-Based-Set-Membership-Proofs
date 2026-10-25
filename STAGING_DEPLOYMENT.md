# Staging Deployment Guide

Panduan ini untuk menjalankan PoC sebagai staging/demo, bukan production final.

## Environment

Server startup automatically loads `.env` from the repo root if the file
exists. Copy the template first:

```bash
cp .env.example .env
```

Then edit `.env` and run:

```bash
cargo run -p aml-api
```

Existing shell environment variables override `.env` values.

| Env | Default | Fungsi |
| --- | --- | --- |
| `PORT` | `8880` | Port HTTP jika `AML_BIND_ADDR` tidak diset |
| `AML_BIND_ADDR` | `0.0.0.0:$PORT` | Bind address penuh, contoh `0.0.0.0:8880` |
| `AML_AUDIT_DB_URL` | `sqlite:data/aml_audit.sqlite` | Lokasi SQLite audit trail |
| `AML_API_KEY` | disabled | Jika diset, endpoint verify/audit wajib header `x-api-key` |
| `AML_RATE_LIMIT_PER_MINUTE` | disabled | Global request limit per menit untuk endpoint verify/audit |
| `AML_CORS_ORIGINS` | any origin | Comma-separated allowed origins untuk staging publik |
| `WALLET_PORT` | `8891` | Port Wallet owner dummy server |
| `ALIPAY_PORT` | `8892` | Port Alipay+ verifier dummy server |
| `PAYPAY_PORT` | `8893` | Port PayPay prover dummy server |
| `ALIPAY_BASE_URL` | `http://localhost:8892` | URL Alipay+ dari perspektif Wallet |
| `PAYPAY_BASE_URL` | `http://localhost:8893` | URL PayPay dari perspektif Wallet/Alipay+ |
| `AML_MEMBERSHIP_MODE` | `explicit` | Mode Set Membership untuk payment flow: `explicit`, `implicit`, atau `hybrid` |
| `AML_MEMBERSHIP_WARN_TIMEOUT_MS` | `1000` | Warning threshold performa membership, bukan target sukses |
| `AML_MEMBERSHIP_HARD_TIMEOUT_MS` | `30000` | Safety timeout agar demo tidak menggantung |

Endpoint yang diproteksi saat `AML_API_KEY` aktif:

- `POST /api/v1/verify`
- `GET /api/v1/proofs/{request_id}`
- `POST /api/v1/proofs/{request_id}/verify`
- `GET /api/v1/ledger/commitments`

Endpoint health/config/demo scenario tetap terbuka agar UI bisa load.

## Docker Build

Builder image memakai `rust:1.96.0-bookworm`, sesuai baseline yang dikunci di
`rust-toolchain.toml`. Runtime image tetap Debian Bookworm. Jika baseline compiler
di-upgrade, ubah pin lokal, Docker builder dan baseline CI bersama-sama.
CI stable terbaru adalah pemeriksaan kompatibilitas tambahan; lihat
[catatan CI](docs/CI_TOOLCHAINS.md).

```bash
docker build -t aml-zkp-poc:staging .
```

Image berisi binary:

- `aml-api`
- `aml-wallet`
- `aml-alipay`
- `aml-paypay`
- `aml-metrics`

Rekomendasi staging: build image sekali di laptop/CI/build server, lalu deploy
image yang sama ke setiap server dengan command berbeda. Dengan cara ini,
runtime server tidak perlu install Rust dan tidak perlu akses ke dependency Rust.

## Docker Run Lokal

Demo fresh tanpa API key:

```bash
docker run --rm -p 8880:8880 \
  -e AML_AUDIT_DB_URL=sqlite:/app/data/aml_audit.sqlite \
  aml-zkp-poc:staging
```

## Three-Server Local Demo

```bash
cp .env.example .env
bash scripts/run_three_servers.sh
```

Buka `http://localhost:8891`.

## Three-Server Deploy Terpisah

Untuk staging yang benar-benar dipisah, jalankan satu container/binary per
server. Best practice untuk PoC ini adalah **Docker per server**:

| Server | Binary | Env utama |
| --- | --- | --- |
| Wallet | `aml-wallet` | `WALLET_PORT`, `ALIPAY_BASE_URL`, `PAYPAY_BASE_URL` |
| Alipay+ | `aml-alipay` | `ALIPAY_PORT`, `PAYPAY_BASE_URL` |
| PayPay | `aml-paypay` | `PAYPAY_PORT`, `AML_MEMBERSHIP_MODE` |

Contoh mapping:

```env
# Wallet server
WALLET_PORT=8891
ALIPAY_BASE_URL=http://10.0.1.12:8892
PAYPAY_BASE_URL=http://10.0.1.13:8893

# Alipay+ server
ALIPAY_PORT=8892
PAYPAY_BASE_URL=http://10.0.1.13:8893

# PayPay server
PAYPAY_PORT=8893
AML_MEMBERSHIP_MODE=explicit
AML_MEMBERSHIP_WARN_TIMEOUT_MS=1000
AML_MEMBERSHIP_HARD_TIMEOUT_MS=30000
```

Spesifikasi minimum server, firewall, dan contoh `docker run`/`systemd` ada di
[SERVER_REQUIREMENTS.md](SERVER_REQUIREMENTS.md).

Ringkasnya:

- Build-time: butuh akses HTTPS ke `crates.io`, `index.crates.io`, dan
  `github.com` untuk mengambil dependency Rust.
- Runtime: cukup menjalankan Docker image/binary yang sudah jadi; tidak perlu
  Rust dan tidak perlu akses dependency registry.
- Recommended: Docker image yang sama, command berbeda:
  `aml-wallet`, `aml-alipay`, `aml-paypay`.

## Docker Compose Three-Server Demo

```bash
docker compose up --build wallet
```

Service yang naik:

- `wallet` di `http://localhost:8891`
- `alipay` di `http://localhost:8892`
- `paypay` di `http://localhost:8893`

All-in-one legacy API tetap tersedia sebagai profile:

```bash
docker compose --profile legacy up --build aml-api
```

Staging dengan API key, rate limit, dan CORS:

```bash
docker run --rm -p 8880:8880 \
  -e AML_API_KEY='change-me' \
  -e AML_RATE_LIMIT_PER_MINUTE=60 \
  -e AML_CORS_ORIGINS='https://stakeholder-demo.example.com,http://localhost:8880' \
  -e AML_AUDIT_DB_URL=sqlite:/app/data/aml_audit.sqlite \
  aml-zkp-poc:staging
```

Alternatif dengan file env:

```bash
docker run --rm -p 8880:8880 --env-file .env \
  aml-zkp-poc:staging
```

Jika `AML_API_KEY` aktif, isi field "Staging API key" di UI dengan nilai yang
sama, atau gunakan curl:

```bash
curl -s -X POST http://localhost:8880/api/v1/verify \
  -H 'Content-Type: application/json' \
  -H 'x-api-key: change-me' \
  -d '{"amount_cents":200000,"scenario":"compliant"}' | jq .
```

## Demo Checklist

Demo utama tiga server:

1. Buka `http://localhost:8891`.
2. Jalankan skenario `compliant` dan pastikan `APPROVED`.
3. Jalankan skenario `exceeded` dan pastikan `REJECTED`.
4. Klik **Run Membership Comparison** dan pastikan `explicit`, `implicit`, dan
   `hybrid` muncul dengan verdict membership.
5. Jika ingin mengganti mode payment flow, ubah `AML_MEMBERSHIP_MODE` lalu
   restart service PayPay/launcher.

Demo audit all-in-one:

1. Buka `http://localhost:8880`.
2. Jalankan skenario `compliant`.
3. Klik `View Audit Proof`.
4. Klik `Verify Audit Hash`, pastikan `valid: true`.
5. Klik `View Ledger`.
6. Klik verify kedua pada skenario yang sama untuk menunjukkan replay warning.

## Batas Staging

Versi ini sudah audit-ready untuk demo, tetapi belum production-ready penuh.
Integrasi Alipay+/PayPay sudah ada sebagai tiga server dummy via HTTP, tetapi
belum terhubung ke sistem Alipay+/PayPay real, database transaksi real, identity
provider, observability production, atau full Bayer-Groth/SNARK implementation.
