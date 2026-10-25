# Three-Server Deployment Requirements

Dokumen ini adalah spesifikasi minimum untuk menjalankan demo **three-server
dummy integration**:

- `aml-wallet`: User Wallet owner
- `aml-alipay`: Alipay+ client/verifier
- `aml-paypay`: PayPay server/prover

Status dokumen: **staging/demo requirement**, bukan production final.

## Jawaban Singkat

Ya, project ini sudah dipisah menjadi tiga server aplikasi:

| Server | Binary | Default port | Fungsi |
| --- | --- | ---: | --- |
| Wallet owner | `aml-wallet` | `8891` | UI/demo entry point, membuat payment request, menampilkan hasil |
| Alipay+ verifier | `aml-alipay` | `8892` | Verifier/client, orkestrasi request ke PayPay, mengambil keputusan |
| PayPay prover | `aml-paypay` | `8893` | Prover/server, lookup dummy state, generate proof, update todo log |

Dalam laptop lokal, ketiganya berjalan sebagai tiga proses di tiga port. Dalam
staging, ketiganya bisa dijalankan di tiga VM/server/container berbeda.

## Topologi Deployment

```text
Browser/Demo User
    |
    v
Wallet Server (aml-wallet :8891)
    |
    | HTTP: POST /api/v1/payments
    v
Alipay+ Server (aml-alipay :8892)
    |
    | HTTP: lookup, shuffle proof, threshold proof, todo commit
    v
PayPay Server (aml-paypay :8893)
```

Catatan PoC: untuk memudahkan skenario demo, `aml-wallet` saat ini juga memanggil
endpoint setup dummy PayPay agar mendapat `tx_id_commitment` yang cocok dengan
dummy state PayPay. Pada integrasi production, setup commitment idealnya dibuat
di Wallet atau SDK Wallet, bukan mengambil data demo dari PayPay.

## Network Requirement

| Arah koneksi | Wajib? | Keterangan |
| --- | --- | --- |
| Browser/user -> Wallet | Ya | UI demo dibuka dari Wallet server. |
| Wallet -> Alipay+ | Ya | Wallet mengirim payment request ke verifier. |
| Wallet -> PayPay | Ya untuk PoC saat ini | Dipakai endpoint dummy setup commitment. |
| Alipay+ -> PayPay | Ya | Alipay+ meminta lookup/proof/threshold/todo commit ke PayPay. |
| PayPay -> Alipay+ | Tidak sebagai koneksi awal | PayPay hanya membalas request HTTP. |
| Alipay+ -> Wallet | Tidak sebagai koneksi awal | Alipay+ hanya membalas request HTTP dari Wallet. |

Port internal default:

| Service | Port | Exposure yang disarankan |
| --- | ---: | --- |
| Wallet | `8891` | Public/stakeholder access, idealnya via HTTPS reverse proxy |
| Alipay+ | `8892` | Internal/private network; public hanya jika perlu demo API |
| PayPay | `8893` | Internal/private network only |
| Legacy `aml-api` | `8880` | Opsional, untuk all-in-one audit demo lama |

Untuk staging yang dibuka stakeholder, gunakan reverse proxy HTTPS di depan
Wallet, misalnya:

```text
https://wallet-demo.example.com -> http://wallet-server:8891
```

## Environment Per Server

### Wallet Server

```env
WALLET_PORT=8891
ALIPAY_BASE_URL=http://<ALIPAY_HOST>:8892
PAYPAY_BASE_URL=http://<PAYPAY_HOST>:8893
```

`PAYPAY_BASE_URL` masih diperlukan karena PoC saat ini memakai endpoint dummy
setup PayPay untuk membuat commitment skenario demo.

### Alipay+ Server

```env
ALIPAY_PORT=8892
PAYPAY_BASE_URL=http://<PAYPAY_HOST>:8893
```

### PayPay Server

```env
PAYPAY_PORT=8893
AML_MEMBERSHIP_MODE=explicit
AML_MEMBERSHIP_WARN_TIMEOUT_MS=1000
AML_MEMBERSHIP_HARD_TIMEOUT_MS=30000
```

`AML_MEMBERSHIP_MODE` memilih proof primitive untuk payment flow utama:
`explicit` sebagai default shuffle-based baseline, `implicit` sebagai
vector-commitment-like/Merkle PoC, atau `hybrid` sebagai bucketed subset PoC.
Perbandingan tiga mode dilakukan lewat metrics runner/UI Wallet, bukan dengan
menjalankan tiga proof sekaligus pada satu payment request.

### Legacy All-In-One API

Opsional jika ingin tetap menjalankan demo audit SQLite lama:

```env
PORT=8880
AML_AUDIT_DB_URL=sqlite:data/aml_audit.sqlite
AML_API_KEY=change-me
AML_RATE_LIMIT_PER_MINUTE=60
```

## Minimum Server Specification

Spesifikasi ini cukup untuk staging/demo kecil dengan 1-5 user demo bersamaan.

| Server | CPU minimum | RAM minimum | Disk minimum | Catatan |
| --- | ---: | ---: | ---: | --- |
| Wallet | 1 vCPU | 512 MB | 5 GB | UI ringan dan forwarding request. |
| Alipay+ | 1 vCPU | 1 GB | 5 GB | Verifikasi response, trace, metrics endpoint. |
| PayPay | 2 vCPU | 2 GB | 10 GB | Paling berat karena generate proof dan menjalankan pipeline ZK-MNNP demo. |

Rekomendasi nyaman untuk demo stakeholder:

| Server | CPU recommended | RAM recommended | Disk recommended |
| --- | ---: | ---: | ---: |
| Wallet | 1-2 vCPU | 1 GB | 10 GB |
| Alipay+ | 2 vCPU | 2 GB | 10 GB |
| PayPay | 4 vCPU | 4-8 GB | 20 GB |

## Build Server vs Runtime Server

Ada dua jenis kebutuhan yang sering tertukar:

| Jenis server | Tugas | Perlu Rust? | Perlu akses dependency internet? |
| --- | --- | --- | --- |
| Build server / CI | Compile source code menjadi Docker image atau binary release | Ya | Ya |
| Runtime server | Menjalankan Docker image atau binary yang sudah jadi | Tidak | Tidak wajib |

**Akses dependency internet hanya diperlukan saat build**, bukan saat runtime.
Yang dimaksud dependency internet adalah akses ke:

- `https://crates.io` dan `https://index.crates.io` untuk package Rust umum
- `https://github.com` karena project memakai dependency Git
  `ant-dtc/persephone-service`

Jika build dilakukan langsung di server, sediakan minimal:

- 2-4 vCPU
- 4 GB RAM
- 20 GB disk kosong
- akses HTTPS keluar ke `crates.io`, `index.crates.io`, dan `github.com`

Untuk runtime server, lebih ringan karena cukup menjalankan binary/container
yang sudah dibuild di CI/laptop/build server. Runtime server tidak perlu install
Rust dan tidak perlu download dependency Rust.

## Deployment Recommendation

Rekomendasi best practice untuk staging/demo ini adalah **Docker image per
service**:

- source code tetap satu repo
- build dilakukan sekali menjadi satu image
- image yang sama bisa menjalankan `aml-wallet`, `aml-alipay`, atau `aml-paypay`
  dengan command berbeda
- runtime server tidak perlu Rust
- konfigurasi cukup lewat environment variable
- lebih mudah dipindah ke VM, VPS, cloud, Docker Compose, atau Kubernetes

Native binary + systemd tetap didokumentasikan sebagai alternatif jika
stakeholder tidak ingin memakai Docker.

## Software Requirement

Pilihan A, Docker/container (**recommended**):

- Linux x86_64 atau arm64
- Docker Engine 24+
- Docker Compose v2 jika ingin orchestration sederhana
- Akses network antar server/container

Pilihan B, native binary + systemd:

- Linux x86_64 atau arm64, direkomendasikan Ubuntu 22.04/24.04 LTS
- `systemd`
- `ca-certificates`
- `libsqlite3-0` hanya diperlukan untuk legacy `aml-api`
- Binary hasil build: `aml-wallet`, `aml-alipay`, `aml-paypay`

Build native membutuhkan ini hanya di mesin build, bukan di runtime server:

- Rust sesuai `rust-toolchain.toml`
- `git`
- `pkg-config`
- `libsqlite3-dev`

## Deployment Option A: Docker Per Server (Recommended)

Build image di laptop/CI/build server:

```bash
docker build -t aml-zkp-poc:staging .
```

Wallet server:

```bash
docker run -d --name aml-wallet \
  -p 8891:8891 \
  -e WALLET_PORT=8891 \
  -e ALIPAY_BASE_URL=http://<ALIPAY_HOST>:8892 \
  -e PAYPAY_BASE_URL=http://<PAYPAY_HOST>:8893 \
  aml-zkp-poc:staging aml-wallet
```

Alipay+ server:

```bash
docker run -d --name aml-alipay \
  -p 8892:8892 \
  -e ALIPAY_PORT=8892 \
  -e PAYPAY_BASE_URL=http://<PAYPAY_HOST>:8893 \
  aml-zkp-poc:staging aml-alipay
```

PayPay server:

```bash
docker run -d --name aml-paypay \
  -p 8893:8893 \
  -e PAYPAY_PORT=8893 \
  aml-zkp-poc:staging aml-paypay
```

Health checks:

```bash
curl http://<WALLET_HOST>:8891/api/v1/wallet/health
curl http://<ALIPAY_HOST>:8892/api/v1/alipay/health
curl http://<PAYPAY_HOST>:8893/api/v1/paypay/health
```

## Deployment Option B: Binary + Systemd (Alternative)

Build release:

```bash
cargo build --release -p aml-wallet -p aml-alipay -p aml-paypay
```

Copy binary ke masing-masing server:

```text
/usr/local/bin/aml-wallet
/usr/local/bin/aml-alipay
/usr/local/bin/aml-paypay
```

Contoh systemd Wallet:

```ini
[Unit]
Description=AML ZKP Wallet Demo
After=network-online.target

[Service]
ExecStart=/usr/local/bin/aml-wallet
Environment=WALLET_PORT=8891
Environment=ALIPAY_BASE_URL=http://<ALIPAY_HOST>:8892
Environment=PAYPAY_BASE_URL=http://<PAYPAY_HOST>:8893
Restart=always
RestartSec=3

[Install]
WantedBy=multi-user.target
```

Contoh systemd Alipay+:

```ini
[Unit]
Description=AML ZKP Alipay Verifier Demo
After=network-online.target

[Service]
ExecStart=/usr/local/bin/aml-alipay
Environment=ALIPAY_PORT=8892
Environment=PAYPAY_BASE_URL=http://<PAYPAY_HOST>:8893
Restart=always
RestartSec=3

[Install]
WantedBy=multi-user.target
```

Contoh systemd PayPay:

```ini
[Unit]
Description=AML ZKP PayPay Prover Demo
After=network-online.target

[Service]
ExecStart=/usr/local/bin/aml-paypay
Environment=PAYPAY_PORT=8893
Restart=always
RestartSec=3

[Install]
WantedBy=multi-user.target
```

## Security Minimum Untuk Staging

Minimum yang disarankan sebelum dibuka ke stakeholder:

- Wallet diakses via HTTPS.
- Alipay+ dan PayPay hanya bisa diakses dari private network/VPN/security group.
- Jangan expose PayPay langsung ke internet.
- Set firewall/security group:
  - allow inbound Wallet: `443` atau `8891` dari stakeholder
  - allow inbound Alipay+: `8892` hanya dari Wallet
  - allow inbound PayPay: `8893` hanya dari Wallet dan Alipay+
- Log server disimpan minimal stdout/container logs.
- Gunakan host/domain yang jelas agar URL tidak berubah saat demo.

Belum tersedia di PoC ini dan perlu ditambahkan jika menuju production:

- mTLS/API key antar service
- persistent database PayPay
- audit trail terpisah untuk three-server flow
- observability production: metrics exporter, tracing collector, dashboard
- full Bayer-Groth shuffle atau SNARK update jika scope riset dinaikkan

## Demo Acceptance Checklist

Sebelum demo ke stakeholder, pastikan:

- `GET /api/v1/wallet/health` mengembalikan `status: ok`
- `GET /api/v1/alipay/health` mengembalikan `status: ok`
- `GET /api/v1/paypay/health` mengembalikan `status: ok`
- Wallet UI bisa dibuka
- Scenario `compliant` menghasilkan `APPROVED`
- Scenario `exceeded` menghasilkan `REJECTED`
- Trace menampilkan langkah Alipay+ dan PayPay
- Wallet UI tombol **Run Membership Comparison** menampilkan mode `explicit`,
  `implicit`, dan `hybrid`
- Metrics report bisa dijalankan:

```bash
cargo run -p aml-metrics -- --runs 1000 --n 100 --modes explicit,implicit,hybrid
```
