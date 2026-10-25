# Hybrid Approach: Audit-Ready ZKP AML PoC

## Ringkasan

Project ini memakai pendekatan hybrid:

```text
set membership -> IBC + IdEq/VVer -> sigma shuffle -> sliding window -> simplified oblivious update -> ZK-MNNP
```

Keputusan: Bayer-Groth shuffle dan SNARK oblivious update penuh
tidak wajib untuk versi ini. Alternatif yang lebih ringan boleh dipakai selama
riset ZK-MNNP dari `persephone-service` tetap menjadi bagian inti threshold check.

## Kenapa Hybrid

Bayer-Groth shuffle dan SNARK circuit untuk oblivious update adalah pekerjaan
besar, dokumentasinya terbatas, dan rawan menghabiskan waktu riset untuk
implementasi kriptografi low-level. Untuk itu, fokusnya adalah membuktikan
alur bisnis AML privacy-preserving secara end-to-end:

- user memiliki transaction ID commitment (`c_tid`)
- server membuktikan `c_tid` adalah anggota set commitment valid PayPay lewat
  mode Set Membership yang dapat diganti (`explicit`, `implicit`, `hybrid`)
- server membuktikan identity equality dan value verification
- history diacak/disamarkan dengan proof rerandomization sederhana
- sliding window 365 hari dihitung
- threshold AML dibuktikan lewat ZK-MNNP dari Persephone
- hasilnya diaudit lewat SQLite proof history dan commitment ledger

## Yang Production-Grade vs PoC

| Bagian | Status Saat Ini |
| --- | --- |
| Set membership proof | Mode pluggable dengan security profile: explicit sigma rerandomization, implicit Merkle, hybrid bucketed subset |
| IBC, IdEq, VVer | PoC kuat, mengikuti model Pedersen/Ristretto255 |
| ZK-MNNP threshold | Menggunakan `persephone-service` |
| Verifiable shuffle | Sigma-protocol sederhana, bukan Bayer-Groth |
| Oblivious update | Simplified Rust logic, bukan full SNARK circuit |
| Audit trail | SQLite staging/demo audit dengan hash payload |
| Integrasi Alipay+/PayPay | Dummy tiga server via HTTP: `aml-wallet`, `aml-alipay`, `aml-paypay` |
| Auth/rate limit/hardening | Belum production-ready |

## Formal Protocol Specification

Update terbaru memisahkan **current executable baseline** dari **formal target
protocol**. Ini dilakukan karena tiga catatan stakeholder benar:

| Mode | Current code | Formal protocol target | Gap yang ditampilkan |
| --- | --- | --- | --- |
| `explicit` | Sigma rerandomization membership over shuffled set | Formal verifiable shuffle proof, misalnya Bayer-Groth-style shuffle argument | Belum ada formal shuffle proof |
| `implicit` | Merkle membership proof over `H_leaf(c_tid)` | Vector commitment opening proof, misalnya KZG/vector commitment | Belum ada VC opening proof |
| `hybrid` | Bucketed subset + explicit sigma proof | Global commitment proof + subset membership proof | Belum ada `pi_global` terhadap trusted `R_S` |

Karena itu API/UI sekarang membawa metadata `security_profile`:

- `current_primitive`: primitive yang benar-benar berjalan.
- `formal_target`: primitive formal yang menjadi target protocol.
- `formal_gap`: gap kriptografi yang belum diimplementasikan.
- `trust_anchor_current`: asumsi/verifikasi saat ini.
- `trust_anchor_ideal`: trust anchor yang dibutuhkan protokol formal.
- `claim_level`: saat ini `poc_baseline`.

Dokumen formal protocol ada di `docs/SET_MEMBERSHIP_FORMALIZATION.md` dan
`output/pdf/set_membership_formalization.pdf`.

## Integrasi Tiga Server

Versi terbaru menambahkan tiga binary terpisah untuk mengikuti slide
"Keseluruhan Sistem":

- `aml-wallet`: simulasi User Wallet owner
- `aml-alipay`: simulasi Alipay+ sebagai client/verifier
- `aml-paypay`: simulasi PayPay sebagai server/prover

Boundary prover/verifier kini dipisah di level HTTP:

- PayPay melakukan lookup, generate set membership proof, generate shuffle
  proof, sliding-window computation, threshold proof response, dan todo/log
  expire action.
- Alipay+ menerima payment request, memverifikasi set membership proof dan
  respons proof lain secara pragmatis, lalu memutuskan `APPROVED` atau
  `REJECTED`.
- Wallet memulai payment request dan menerima hasil akhir.

Detail endpoint dan command demo ada di `THREE_SERVER_FLOW.md`.

## Metrics

Metrik dari slide IBC dan keseluruhan sistem diukur lewat:

```bash
cargo run -p aml-metrics -- --runs 100 --n 100
```

Untuk setiap metric, report menyimpan `paper_target`, `poc_implementation`,
`measured_value`, dan `pass_fail_or_not_comparable`. Metric yang target paper-nya
Bayer-Groth/SNARK penuh diberi verdict `not_comparable`, bukan dipaksakan pass.
Set membership proof sekarang memiliki tiga mode benchmark:

- `explicit`: current sigma rerandomization atas seluruh set; formal target
  adalah formal shuffle proof.
- `implicit`: current Merkle membership proof; formal target adalah vector
  commitment opening proof.
- `hybrid`: current bucketed subset + explicit sigma proof; formal target adalah
  global commitment proof + subset membership proof.

Payment flow tiga server memakai satu mode per request melalui
`AML_MEMBERSHIP_MODE`. Ini disengaja: dalam sistem nyata prover dan verifier
harus sepakat pada satu primitive proof untuk satu keputusan transaksi. Tiga
mode sekaligus dipakai untuk benchmark/comparison, bukan flow bisnis utama.

Default `explicit` dipilih agar demo tetap kompatibel dengan keputusan awal
stakeholder yang menurunkan Bayer-Groth menjadi shuffle/sigma PoC. Hasil
benchmark saat ini biasanya menunjukkan `implicit` paling cepat dan paling
ringkas, tetapi labelnya tetap jujur sebagai vector-commitment-like/Merkle PoC,
bukan KZG/vector commitment production-grade.

Baseline `n=100` untuk verdict pass/fail:

| Mode | Generation | Verification | Proof size |
| --- | ---: | ---: | ---: |
| `explicit` | <= 120 ms | <= 500 ms | <= 64 KB |
| `implicit` | <= 20 ms | <= 10 ms | <= 2 KB |
| `hybrid` | <= 60 ms | <= 80 ms | <= 16 KB |

`AML_MEMBERSHIP_WARN_TIMEOUT_MS=1000` memberi sinyal demo mulai lambat,
sedangkan `AML_MEMBERSHIP_HARD_TIMEOUT_MS=30000` hanya safety timeout, bukan
parameter sukses.

## Audit Trail

Setiap `/api/v1/verify` menghasilkan:

- `request_id`
- status `APPROVED` atau `REJECTED`
- proof metadata
- replay warning flag
- threshold config snapshot
- `proof_audit_hash`
- commitment ledger entries

Endpoint audit:

```bash
GET  /api/v1/proofs/{request_id}
POST /api/v1/proofs/{request_id}/verify
GET  /api/v1/ledger/commitments
```

Hash verification bersifat pragmatis: server menghitung ulang hash dari payload
audit yang tersimpan dan membandingkannya dengan `proof_audit_hash`. Ini bukan
rerun full ZKP, tetapi cukup untuk demo audit integrity.
