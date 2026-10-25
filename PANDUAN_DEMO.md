# Panduan Demo — ZKP AML Cross-Border PoC

## Jalur Riset E1 Terpisah

Untuk laporan Phase 1 rev7, gunakan `smp-e1`, bukan `aml-metrics` atau UI payment.
E1 menguji opening satu koordinat pada EP uncompressed, EP compressed dan CF-H.
Mode hidden menyembunyikan nilai; indeks k tetap publik. Ini bukan benchmark
permutation/partition atau perbandingan end-to-end tiga metode membership lama.

1. Baca [panduan E1](docs/E1_IMPLEMENTATION.md) dan [catatan compressed hidden](docs/E1_COMPRESSED_HIDDEN.md).
2. Jalankan `cargo run --locked --release -p smp-e1 -- --profile smoke --output outputs/e1-smoke`.
3. Buka `summary.md` atau `e1_report.xlsx` pada direktori output; cocokkan dengan
   `attempts.csv`, `environment.json`, dan `build_provenance.json`.
4. Jelaskan bahwa n=64 adalah panjang vektor skalar, bukan jumlah pengulangan.
   Smoke berisi 2 warm-up + 5 recorded attempt untuk setiap kombinasi.
5. Tunjukkan pemisahan ukuran proof, statement, dan pesan yang baru dikirim:
   CF-H hidden memiliki proof 176 B dan C 48 B, sehingga pesan 224 B.
6. Tegaskan status: compressed hidden sudah dapat dijalankan tetapi konstruksinya
   perlu review matematis. Campaign final belum dijalankan dan perintah `full`
   sengaja mengembalikan status terblokir. Angka smoke bukan kesimpulan publikasi.

Q: Kenapa tidak langsung membandingkan angka dengan benchmark sebelumnya?
A: Grup, primitive, relasi, encoding, dan batas timer berbeda. Hasil lama adalah
demo AML/membership; E1 menguji fungsi opening sesuai rev7.

Q: Kalau semua test lulus, kenapa masih perlu review?
A: Test dapat menemukan bug dan menunjukkan correctness contoh, tetapi tidak
membuktikan zero knowledge atau soundness terhadap semua penyerang.

Q: Apakah ukuran compressed harus sama dengan tabel rev7?
A: Tidak dipaksakan. Rumus hidden di rev7 bersifat perkiraan bersyarat. Kandidat
ini menambah 80 B dibanding plain untuk mengikat response nilai bersama; detail
matematis dan keputusan yang dibutuhkan ada di dokumen review.

Bagian selanjutnya tetap merupakan panduan demo AML yang lama.

Dokumen ini adalah pegangan presentasi untuk menjelaskan PoC ke stakeholder
atau tim penguji. Fokus demo utama adalah **integrasi tiga server dummy**:

```text
User Wallet owner -> Alipay+ verifier -> PayPay prover
```

Demo all-in-one `aml-api` tetap ada, tetapi posisinya sekarang sebagai demo
audit SQLite dan regression path.

---

## Ringkasan Narasi

Kalimat pembuka yang aman:

> PoC ini mensimulasikan verifikasi AML lintas batas untuk Singapore. Sistem
> mengecek apakah total transaksi user dalam sliding window 365 hari melewati
> threshold publik SGD 20.000, tetapi identitas dan riwayat transaksi tidak
> dibuka secara langsung. Proof dan prosesnya dipisah ke tiga server dummy agar
> mendekati slide "Keseluruhan Sistem": Wallet owner, Alipay+ verifier, dan
> PayPay prover.

Pipeline kriptografi:

```text
Set Membership -> IBC -> Verifiable Shuffle -> Sliding Window -> ZK-MNNP Threshold
```

Status pendekatan:

| Bagian | Implementasi PoC | Alasan |
| --- | --- | --- |
| Set Membership | 3 mode: `explicit`, `implicit`, `hybrid` | Agar stakeholder bisa membandingkan pendekatan explicit/shuffle, vector-commitment-like, dan hybrid. |
| IBC / IdEq / VVer | Pedersen/Ristretto255 PoC | Cukup kuat untuk membuktikan binding identitas/value di PoC. |
| Verifiable Shuffle | Sigma/rerandomization proof | Bayer-Groth penuh terlalu berat untuk deadline PoC. |
| Sliding Window | Rust filter 365 hari | Tujuan PoC adalah membuktikan flow threshold AML. |
| Oblivious Update | Simplified Rust/sigma update | Belum full SNARK circuit. |
| Threshold | ZK-MNNP via `persephone-service` | Ini komponen riset utama yang tetap dipakai. |
| Audit | SQLite proof history + commitment ledger | Agar staging/demo bisa diaudit. |

Kalimat penting:

> Ini bukan klaim production-ready. Ini adalah staging/demo PoC yang
> auditable, punya boundary tiga server, dan punya metrik pembanding.

Update formal protocol:

> Tiga kritik stakeholder benar: explicit saat ini belum formal shuffle proof,
> implicit belum vector commitment proof, dan hybrid belum global commitment
> proof. Karena itu UI/API sekarang menampilkan **Set Membership Security
> Profile** yang memisahkan `current_primitive`, `formal_target`, dan
> `formal_gap`. Pseudocode formal tersedia di
> `docs/SET_MEMBERSHIP_FORMALIZATION.md` dan PDF
> `output/pdf/set_membership_formalization.pdf`.

---

## Data Skenario Demo

Parameter publik:

| Parameter | Nilai | Arti |
| --- | ---: | --- |
| Threshold | SGD 20.000 / `2_000_000` cents | Batas total transaksi dalam window. |
| Window | 365 hari | Periode transaksi yang dihitung. |
| Max tx demo | SGD 50.000 | Guard input agar demo tidak menerima nominal tidak wajar. |

Skenario:

| Scenario | User key | History | Incoming | Projected total | Expected |
| --- | --- | ---: | ---: | ---: | --- |
| `compliant` | `scenario_compliant` | SGD 12.000 | SGD 2.000 | SGD 14.000 | `APPROVED` |
| `exceeded` | `scenario_exceeded` | SGD 19.000 | SGD 5.000 | SGD 24.000 | `REJECTED` |

Cerita sederhana:

- `compliant`: user masih di bawah limit SGD 20.000, maka transaksi disetujui.
- `exceeded`: transaksi baru membuat total melewati limit, maka transaksi ditolak.

---

## Menjalankan Demo Utama Tiga Server

Persiapan:

```bash
# Jalankan dari root checkout proyek.
cp .env.example .env
```

Jalankan tiga server:

```bash
bash scripts/run_three_servers.sh
```

Tunggu sampai muncul:

```text
Three-server demo is ready:
  Wallet   http://localhost:8891
  Alipay+  http://localhost:8892/api/v1/alipay/health
  PayPay   http://localhost:8893/api/v1/paypay/health
```

Buka:

```text
http://localhost:8891
```

Jika script menolak karena port dipakai, berarti masih ada proses lama. Stop
terminal lama dengan `Ctrl+C`, atau cek:

```bash
lsof -nP -iTCP:8891 -sTCP:LISTEN
lsof -nP -iTCP:8892 -sTCP:LISTEN
lsof -nP -iTCP:8893 -sTCP:LISTEN
```

Alasan memakai launcher ini:

- build binary sekali, tidak rebutan Cargo lock;
- start PayPay, Alipay+, Wallet secara berurutan;
- menunggu health check tiap server sebelum UI dipakai;
- mencegah error `Connection refused` karena service belum siap.

---

## Step-by-Step Demo UI Wallet

### Bagian 1 — Jelaskan Peran Tiga Server

Di browser `http://localhost:8891`, jelaskan:

| Server | Peran demo | Analogi |
| --- | --- | --- |
| Wallet | Membuat payment request dan menerima hasil | User wallet owner |
| Alipay+ | Verifier/client yang memutuskan approve/reject | Pihak penerima/verifier |
| PayPay | Prover/server yang menyimpan dummy state dan membuat proof | Sumber data/prover |

Kalimat presentasi:

> Wallet tidak memutuskan sendiri. Wallet hanya mengirim request. Alipay+
> meminta proof ke PayPay, memverifikasi proof, lalu mengeluarkan keputusan.

### Bagian 2 — Run Skenario APPROVED

1. Pilih `Compliant - expected APPROVED`.
2. Biarkan `Amount cents = 200000`.
3. Kosongkan `Transaction ID commitment override`.
4. Klik **Run Payment Flow**.
5. Tunjukkan badge `APPROVED`.

Yang perlu dijelaskan:

- `amount_cents=200000` berarti SGD 2.000.
- History user SGD 12.000 + transaksi baru SGD 2.000 = SGD 14.000.
- Total masih di bawah SGD 20.000.
- Sistem tetap melewati proof flow, bukan hanya menghitung angka di UI.

### Bagian 3 — Baca Trace

Trace normal berisi langkah seperti:

| Aktor | Langkah | Makna |
| --- | --- | --- |
| Alipay+ | Forward lookup gid/tidc to PayPay | Alipay+ meminta PayPay mencari data berdasarkan commitment. |
| PayPay | Generate set membership proof | PayPay membuktikan `c_tid` anggota set commitment valid. |
| Alipay+ | Verify set membership proof | Alipay+ memverifikasi membership proof. |
| PayPay | Generate verifiable shuffle proof | PayPay menyamarkan urutan history. |
| Alipay+ | Verify shuffle proof | Alipay+ mengecek proof shuffle PoC. |
| PayPay | Generate sliding window + threshold proof | PayPay membuat bukti threshold. |
| Alipay+ | Verify F(S^T) and decide | Alipay+ menentukan approve/reject. |
| PayPay | Verified todo list log expire action | PayPay mencatat simulasi update/todo. |

Jika trace threshold sekitar 10-12 detik, jelaskan:

> Bottleneck terbesar ada di path threshold/ZK-MNNP Persephone pada demo HTTP.
> Ini catatan performa PoC, bukan kegagalan flow.

### Bagian 4 — Run Skenario REJECTED

1. Pilih `Exceeded - expected REJECTED`.
2. Biarkan `Amount cents = 500000`.
3. Klik **Run Payment Flow**.
4. Tunjukkan badge `REJECTED`.

Cerita:

> History user sudah SGD 19.000. Transaksi baru SGD 5.000 membuat total SGD
> 24.000, melewati threshold SGD 20.000, maka Alipay+ menolak.

### Bagian 5 — Baca Raw Payload

Nama teknis payload UI adalah `ThreeServerDecisionResponse`.

| Field | Cara menjelaskan |
| --- | --- |
| `request_id` | ID unik untuk request verifikasi. |
| `trace_id` | ID untuk melacak perjalanan request lintas server. |
| `status` | Hasil akhir: `APPROVED` atau `REJECTED`. |
| `threshold_compliant` | `true` jika masih di bawah limit, `false` jika melampaui. |
| `proof_audit_hash` | Handle/hash proof dari sisi demo PayPay. |
| `trace[]` | Urutan proses antar server dan waktunya. |
| `metrics[]` | Ringkasan performa request itu. |
| `membership_security_profile` | Ringkasan current primitive, formal target, gap formal, dan asumsi trust. |

Kalimat aman:

> Payload ini bukan data rahasia user. Ini metadata keputusan dan trace untuk
> audit/demo.

Jika UI menampilkan **Set Membership Security Profile**, jelaskan:

| Field | Arti |
| --- | --- |
| `claim_level` | Level klaim untuk code yang berjalan. Saat ini `poc_baseline`, bukan formal crypto production. |
| `current_primitive` | Primitive yang benar-benar dipakai saat payment flow. |
| `formal_target` | Target formal protocol yang diminta stakeholder. |
| `formal_gap` | Bagian kriptografi formal yang belum diimplementasikan. |
| `trust_anchor_current` | Apa yang saat ini diasumsikan/dipakai verifier untuk mengecek proof. |
| `trust_anchor_ideal` | Trust anchor yang seharusnya ada pada protokol formal. |

---

## Set Membership Mode

Ada tiga mode:

| Mode | Makna | Kapan dipakai |
| --- | --- | --- |
| `explicit` | Current: sigma rerandomization membership. Formal target: formal shuffle proof. | Default, paling dekat dengan pilihan awal stakeholder. |
| `implicit` | Current: Merkle membership proof. Formal target: vector commitment proof. | Paling cepat dan proof paling kecil di benchmark saat ini. |
| `hybrid` | Current: bucketed subset + explicit sigma proof. Formal target: global commitment proof + subset membership. | Kompromi: masih punya unsur subset proof, lebih ringan dari explicit. |

Default `bash scripts/run_three_servers.sh` adalah:

```bash
AML_MEMBERSHIP_MODE=explicit
```

Alasannya:

- menjaga kompatibilitas dengan flow yang sudah disetujui sebelumnya;
- tidak mengubah primitive utama demo secara diam-diam;
- explicit adalah baseline yang paling jelas untuk menjelaskan shuffle-based
  membership.

Ganti mode payment flow:

```bash
AML_MEMBERSHIP_MODE=implicit bash scripts/run_three_servers.sh
AML_MEMBERSHIP_MODE=hybrid bash scripts/run_three_servers.sh
```

Atau edit `.env`:

```env
AML_MEMBERSHIP_MODE=implicit
```

Lalu restart `bash scripts/run_three_servers.sh`.

Kenapa payment flow tidak menjalankan tiga mode sekaligus?

> Dalam protokol nyata, prover dan verifier harus sepakat pada satu proof
> primitive untuk satu keputusan transaksi. Menjalankan tiga proof sekaligus
> lebih cocok sebagai benchmark/debug mode, bukan flow bisnis utama. Kalau
> semua mode dipaksa berjalan pada satu payment, latency dan payload naik tanpa
> alasan compliance langsung.

Untuk membandingkan tiga mode, gunakan panel **Run Membership Comparison** di
UI Wallet atau command `aml-metrics`.

---

## Membership Comparison di UI

Di UI Wallet klik **Run Membership Comparison**.

Yang terjadi:

- Wallet memanggil endpoint metrics di Alipay+.
- Metrics runner membuat dataset dummy yang sama.
- Mode `explicit`, `implicit`, dan `hybrid` dijalankan berurutan.
- UI menampilkan generation time, verification time, proof size, target, dan
  verdict.
- UI juga menampilkan current primitive dan formal target agar comparison
  tidak dibaca sebagai klaim kriptografi formal penuh.

Cara menjelaskan:

> Payment flow memakai satu mode yang dikonfigurasi. Comparison panel bukan
> keputusan transaksi, melainkan alat evaluasi performa tiga pendekatan.

---

## Metrics via Terminal

Command cepat:

```bash
cargo run -p aml-metrics -- --runs 100 --n 100
```

Command laporan:

```bash
cargo run -p aml-metrics -- --runs 1000 --n 100 --modes explicit,implicit,hybrid
```

Command satu mode:

```bash
cargo run -p aml-metrics -- --runs 1000 --n 100 --mode explicit
cargo run -p aml-metrics -- --runs 1000 --n 100 --mode implicit
cargo run -p aml-metrics -- --runs 1000 --n 100 --mode hybrid
```

Arti parameter:

| Parameter | Arti |
| --- | --- |
| `--runs 100` | Setiap metric diulang 100 kali. Cepat untuk demo. |
| `--runs 1000` | Diulang 1000 kali. Lebih stabil untuk laporan. |
| `--n 100` | Jumlah commitment/transaksi dummy. Baseline pass/fail membership saat ini memakai `n=100`. |
| `--mode` | Jalankan satu mode membership. |
| `--modes` | Jalankan beberapa mode membership dengan dataset sama. |

Threshold membership `n=100`:

| Mode | Gen target | Verify target | Size target |
| --- | ---: | ---: | ---: |
| `explicit` | <= 120 ms | <= 500 ms | <= 65.536 bytes |
| `implicit` | <= 20 ms | <= 10 ms | <= 2.048 bytes |
| `hybrid` | <= 60 ms | <= 80 ms | <= 16.384 bytes |

Verdict yang mungkin muncul:

| Verdict | Makna |
| --- | --- |
| `pass` | Memenuhi target yang comparable. |
| `fail` | Sudah diukur, tetapi belum memenuhi target. |
| `pass_warn` | Masih pass, tetapi melewati warn timeout demo. |
| `fail_timeout` | Melewati hard timeout 30 detik. |
| `not_comparable` | Diukur, tetapi primitive PoC berbeda dari target paper. |
| `measured` | Diukur sebagai baseline tanpa target resmi. |
| `scale_report` | Ukuran `n` bukan baseline 100, jadi dibaca sebagai laporan scaling. |

Jika report menunjukkan contoh seperti:

| Mode | Gen | Verify | Size |
| --- | ---: | ---: | ---: |
| `explicit` | ~57 ms | ~292 ms | ~48 KB |
| `implicit` | ~5 ms | ~4 ms | ~758 B |
| `hybrid` | ~14 ms | ~24 ms | ~11 KB |

Kesimpulan performa:

- `implicit` paling cepat dan paling kecil.
- `hybrid` kompromi yang jauh lebih ringan dari explicit.
- `explicit` paling berat, tetapi paling dekat dengan baseline shuffle-based
  yang sudah dibahas awal.

Kalimat ke stakeholder:

> Jika targetnya performa demo, implicit paling unggul. Jika targetnya
> konsistensi dengan pendekatan awal shuffle-based, explicit adalah baseline.
> Hybrid adalah opsi tengah untuk dibandingkan lebih lanjut.

Catatan akademik:

> `explicit` saat ini belum formal shuffle proof, `implicit` masih Merkle proof,
> dan `hybrid` belum punya global commitment proof. Angka benchmark adalah
> baseline executable untuk membandingkan arah riset, bukan klaim bahwa formal
> protocol sudah selesai di code.

---

## Demo Audit SQLite All-in-One

Jalankan:

```bash
cargo run -p aml-api
```

Buka:

```text
http://localhost:8880
```

Fungsi demo all-in-one:

- menjalankan `/api/v1/verify`;
- menyimpan audit record ke SQLite;
- menyimpan commitment ledger append-only;
- menyediakan proof lookup dan hash verification.

Command audit:

```bash
curl -s http://localhost:8880/api/v1/proofs/<request_id> | jq .
curl -s -X POST http://localhost:8880/api/v1/proofs/<request_id>/verify | jq .
curl -s http://localhost:8880/api/v1/ledger/commitments | jq .
```

Kalimat presentasi:

> Endpoint audit tidak menjalankan ulang seluruh ZKP. Ia menghitung ulang hash
> payload audit tersimpan untuk membuktikan metadata proof tidak berubah.

Replay warning:

> Jika `tx_id_commitment` yang sama dipakai berulang, sistem memberi
> `replay_warning: true`, tetapi verifikasi tetap berjalan untuk kebutuhan demo.
> Di production, policy ini bisa dibuat strict reject.

---

## Q&A Stakeholder dan Tim Penguji

### Konsep dan Scope

**Q: Apakah ini sudah production-ready?**\
A: Belum. Ini PoC/staging demo yang auditable. Production masih perlu integrasi
real Alipay+/PayPay, auth antar service, database transaksi real, observability,
key management, hardening, dan review kriptografi.

**Q: Kenapa tetap disebut ZKP kalau ada bagian yang simplified?**\
A: Karena beberapa bagian memang memakai proof/commitment ZK-style, dan threshold
check memakai ZK-MNNP dari Persephone. Tetapi PoC jujur menandai bagian yang
belum full paper-grade, seperti Bayer-Groth shuffle dan SNARK oblivious update.

**Q: Kenapa tidak langsung Bayer-Groth/SNARK penuh?**\
A: Kompleksitas implementasi tinggi, dokumentasi minim, dan deadline PoC
terbatas. Stakeholder sudah menyetujui pendekatan hybrid agar fokus pada flow,
auditability, dan metrik pembanding terlebih dahulu.

**Q: Apa yang dibuktikan Set Membership?**\
A: Bahwa `c_tid`/transaction ID commitment yang dipakai dalam payment request
adalah anggota set commitment valid milik PayPay. Ini mencegah verifier menerima
commitment asing sebelum masuk pipeline AML.

**Q: Kenapa ada tiga mode Set Membership?**\
A: Karena stakeholder ingin membandingkan pendekatan explicit/shuffle, implicit
vector commitment, dan hybrid. PoC menyediakan ketiganya agar bisa diukur dengan
dataset sama.

**Q: Metode mana yang terbaik?**\
A: Dari metrik saat ini, `implicit` paling cepat dan proof paling kecil. Namun
`explicit` paling dekat dengan baseline shuffle-based awal, sedangkan `hybrid`
adalah kompromi. Pilihan akhir bergantung tujuan: performa, kesetiaan ke paper,
atau kompromi implementasi.

**Q: Kenapa payment flow tidak menjalankan tiga membership sekaligus?**\
A: Satu transaksi seharusnya memakai satu proof primitive yang disepakati
prover/verifier. Tiga mode sekaligus adalah benchmark, bukan flow bisnis normal.

**Q: Stakeholder bilang explicit masih sigma rerandomization, benar?**\
A: Benar. Current code `explicit` adalah sigma rerandomization membership
baseline. Formal target adalah formal shuffle proof, misalnya
Bayer-Groth-style shuffle argument. Karena itu UI menampilkan `formal_gap`,
bukan mengklaim explicit sudah formal shuffle.

**Q: Stakeholder bilang implicit masih Merkle, belum vector commitment, benar?**\
A: Benar. Current code `implicit` memakai witness `index + Merkle path`.
Formal target adalah `VC.Commit`, `VC.Open`, dan `VC.Verify` dengan
trusted root `R_S`. Mode saat ini dipakai sebagai baseline performa implicit,
bukan klaim KZG/vector commitment production.

**Q: Stakeholder bilang hybrid belum ada global commitment proof, benar?**\
A: Benar. Current code hybrid mengecek bucket/subset dari `set_commitments`
yang dikirim PayPay. Formal target adalah menambah trusted global root
`R_S` dan proof `VerifyGlobal(R_S,j,D_j,pi_global)=1`, baru kemudian subset
membership dibuktikan.

**Q: Jadi kenapa code belum langsung diganti ke formal crypto penuh?**\
A: Karena formal crypto penuh membutuhkan pilihan konstruksi kriptografi yang besar:
formal shuffle construction, vector commitment construction, dan desain global
root/registry. Langkah sekarang adalah **Formal Protocol Specification**: code jujur
menandai gap, PDF memberi pseudocode formal, lalu implementasi formal bisa
dipecah bertahap setelah stakeholder menyetujui definisi matematisnya.

### Flow Tiga Server

**Q: Apakah ini benar-benar tiga server?**\
A: Ya. Ada tiga binary HTTP berbeda: `aml-wallet`, `aml-alipay`, `aml-paypay`.
Mereka berjalan di port berbeda dan berkomunikasi lewat HTTP. Dalam deploy
terpisah, masing-masing bisa dijalankan di mesin/container berbeda.

**Q: Kenapa satu repo, bukan tiga repo?**\
A: Untuk PoC, satu workspace menjaga DTO, tests, dan dependency crypto tetap
konsisten. Binary tetap terpisah, jadi deployment bisa dipisah tanpa memecah git
repo.

**Q: PayPay dan Alipay+ ini real integration?**\
A: Belum. Ini dummy simulation yang meniru boundary dan peran server. Tujuannya
menunjukkan flow dan proof exchange, bukan koneksi ke production Alipay+/PayPay.

**Q: Apa peran Wallet?**\
A: Wallet membuat payment request, menerima status final, dan menyediakan UI demo.
Wallet tidak menghitung keputusan compliance.

### Metrik

**Q: Kenapa `End-to-end latency` UI bisa fail sekitar 10-12 detik?**\
A: Karena path demo HTTP menjalankan threshold/ZK-MNNP Persephone yang berat.
Itu catatan performa PoC, bukan proof bahwa flow salah. Microbenchmark lokal di
`aml-metrics` mengukur bagian crypto secara lebih terisolasi.

**Q: Kenapa `System throughput` bisa fail?**\
A: TPS dihitung dari estimasi latency lokal. Dev build, path PoC, dan crypto yang
belum dioptimasi bisa membuat TPS di bawah target slide. Ini bahan optimasi,
bukan blocker fungsi demo.

**Q: Apa arti `not_comparable`?**\
A: Metric tetap diukur, tetapi target paper memakai primitive berbeda. Contoh:
target Bayer-Groth/SNARK penuh, sedangkan PoC memakai sigma/simplified update.

**Q: Apakah `measured` berarti gagal?**\
A: Tidak. `measured` berarti belum ada target pass/fail resmi untuk metric itu.
Pada membership baseline `n=100`, sekarang verdict sudah `pass/fail`; `measured`
mungkin masih muncul untuk metric lain yang memang hanya baseline.

**Q: Kenapa hard timeout 30 detik tidak jadi target sukses?**\
A: 30 detik terlalu longgar untuk metric ms-level. Itu hanya safety agar demo
tidak menggantung. Target sukses tetap memakai threshold per mode.

### Audit dan Data

**Q: Apakah data user disimpan permanen?**\
A: Dummy user/history masih in-memory dan reset saat server restart. Audit
metadata pada all-in-one `aml-api` disimpan di SQLite.

**Q: Apa fungsi `proof_audit_hash`?**\
A: Untuk membuktikan metadata audit yang tersimpan tidak berubah. Server bisa
menghitung ulang hash dan membandingkannya.

**Q: Apakah endpoint audit memverifikasi ulang proof ZKP penuh?**\
A: Tidak. Endpoint audit hash hanya integrity check payload audit. Full proof
verification terjadi saat payment flow.

**Q: Kenapa repeated verify tetap pass tapi ada replay warning?**\
A: Untuk demo, replay dibuat warn-only agar request bisa diulang. Production bisa
mengubah policy menjadi strict reject.

### Security dan Production

**Q: Apa risiko terbesar sebelum production?**\
A: Integrasi real data source, key management, service authentication, stricter
replay protection, rate limiting, observability, persistent state PayPay, dan
review kriptografi.

**Q: Apakah proof JSON aman dibuka?**\
A: Untuk demo, proof metadata dibuka agar bisa diaudit akademik. Production harus
menentukan redaction, retention, dan access control.

**Q: Apakah implicit mode sudah vector commitment akademik penuh?**\
A: Belum. Labelnya sengaja `vector_commitment_like_merkle_poc`. Ini baseline
performansi untuk arah implicit, bukan klaim KZG/vector commitment production.

**Q: Apa next step paling masuk akal?**\
A: Jika fokus demo stakeholder, tambahkan dropdown mode payment di UI. Jika fokus
riset, perkuat implicit ke vector commitment/KZG yang lebih akademik. Jika fokus
staging, deploy tiga service dengan auth dan observability.

---

## Checklist Hari-H Demo

- [ ] Stop proses lama di port `8891`, `8892`, `8893`, `8880`.
- [ ] Jalankan `bash scripts/run_three_servers.sh`.
- [ ] Buka `http://localhost:8891`.
- [ ] Run `compliant` dan pastikan `APPROVED`.
- [ ] Run `exceeded` dan pastikan `REJECTED`.
- [ ] Klik **Run Membership Comparison** dan tunjukkan explicit/implicit/hybrid.
- [ ] Jalankan terminal report `cargo run -p aml-metrics -- --runs 1000 --n 100 --modes explicit,implicit,hybrid`.
- [ ] Jika perlu audit SQLite, jalankan `cargo run -p aml-api` dan cek proof lookup/hash.
- [ ] Siapkan jawaban bahwa PoC belum production-ready tetapi sudah auditable dan punya tiga-server boundary.

---

## Glosarium Singkat

| Istilah | Arti sederhana |
| --- | --- |
| Commitment | Nilai terkunci/tersamarkan yang bisa dibuktikan tanpa membuka isi. |
| `c_tid` | Transaction ID commitment. |
| IBC | Identity Binding Commitment, mengikat identitas transaksi. |
| IdEq | Proof bahwa dua commitment punya identitas sama. |
| VVer | Proof bahwa value yang diklaim sesuai commitment. |
| Set Membership | Proof bahwa satu commitment adalah anggota set valid. |
| Verifiable Shuffle | Proof bahwa data diacak/rerandomized tanpa mengubah validitas. |
| Sliding Window | Perhitungan transaksi dalam periode 365 hari. |
| ZK-MNNP | Proof threshold/non-negative dari Persephone. |
| Prover | Pihak yang membuat proof, di demo ini PayPay. |
| Verifier | Pihak yang mengecek proof, di demo ini Alipay+. |
| `not_comparable` | Metric diukur tetapi tidak adil dibandingkan langsung dengan target paper. |
| `scale_report` | Metric untuk ukuran data selain baseline pass/fail utama. |
