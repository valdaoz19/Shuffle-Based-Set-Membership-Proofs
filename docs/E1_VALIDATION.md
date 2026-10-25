# E1: Catatan Validasi Implementasi

Dirangkum pada 2026-10-25. Status: **smoke diagnostik selesai, campaign final E1 belum selesai**.
Dokumen ini mencatat bukti pengujian implementasi, bukan sertifikasi keamanan kriptografi.

## Hasil Pemeriksaan

| Pemeriksaan | Hasil |
| --- | --- |
| `cargo test --locked --workspace` | Lulus; 57 test, tanpa kegagalan |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | Lulus |
| Smoke enam kombinasi scheme/mode | 30 proof terverifikasi, 5 recorded attempt per kombinasi |
| Data pengukuran smoke | 164 baris operasi, termasuk operasi pendukung |
| Correctness gate runner | Enam gate lulus sebelum pengukuran |
| Ukuran CF-H | Plain 48 byte, hidden 176 byte; hidden ditambah C menjadi 224 byte |
| XLSX | Tujuh sheet, tiga chart, tidak ditemukan error cell |
| Visual report | Tabel dan grafik terbaca; tidak ada klaim scaling dari satu n |
| Pengaman campaign full | Berhenti dengan `blocked_pending_research_review`, tanpa data pengukuran palsu |

Unit/integration tests meliputi proof jujur, perubahan nilai/posisi/commitment/
parameter, domain/mode yang tidak sesuai, proof terpotong, trailing bytes,
encoding nonkanonis, titik di luar subgroup, rerandomization CF-H, persamaan
extraction Sigma, dan perbandingan verifier referensi dengan verifier optimized.
Ini bukan audit independen dan tidak menggantikan pembuktian keamanan protokol.

## Reproduksi Smoke yang Diperiksa

```bash
RAYON_NUM_THREADS=1 cargo run --locked --release -p smp-e1 -- \
  --profile smoke --seed 42 --schedule-seed 43 --bootstrap-seed 44 \
  --output outputs/e1-smoke-verified
```

Direktori di atas sudah berisi hasil; gunakan nama direktori baru untuk pengulangan.
Build yang diperiksa memakai Rust 1.96.0, target `aarch64-apple-darwin`, profile
release, optimization level 3, dan RUSTFLAGS kosong. Perintah contoh dengan
`target-cpu=native` dalam panduan bukan konfigurasi build smoke ini.
Detail mesin, binary hash, source hash dan features ada di `environment.json`
dan `build_provenance.json` pada direktori hasil.

Parameter smoke: n=64, dua warm-up dan lima recorded attempt per kombinasi,
satu setup sample per scheme. Dataset dan posisi per attempt dipasangkan untuk
enam kombinasi; urutan eksekusi diacak. Randomness kriptografi tidak direkam.

## Ringkasan Diagnostik

Angka berikut adalah median smoke, bukan angka publikasi atau hasil grid penuh.

| Scheme | Mode | Prove ms | Verify ms | Proof bytes |
| --- | --- | ---: | ---: | ---: |
| EP uncompressed | plain | 2.347791 | 2.271208 | 2080 |
| EP uncompressed | hidden | 2.360750 | 2.326542 | 2144 |
| EP compressed | plain | 26.789417 | 4.928333 | 720 |
| EP compressed | hidden candidate | 27.214750 | 5.336500 | 800 |
| CF-H | plain | 0.000375 | 1.162333 | 48 |
| CF-H | hidden | 2.181000 | 3.646958 | 176 |

CF-H plain mengirim opening datum yang telah disiapkan di luar timer prove.
Karena itu prove-nya sangat kecil; jangan menyimpulkan seluruh biaya opening
hanya 0.000375 ms. Biaya `opening_one`, `opening_all`, setup dan commit dicatat
terpisah. Lihat [batas timer dan amortisasi](E1_IMPLEMENTATION.md).
Lima sample tidak memadai untuk kesimpulan performa final, meskipun statistik
dan confidence interval dihitung oleh runner.

## Artefak

Semua berada dalam `outputs/e1-smoke-verified/`:

- `e1_report.xlsx`: README, Table A, Statistics, Attempt Log, Table C, Table B, Charts.
- `attempts.csv` dan `attempts.jsonl`: pengukuran per operasi/per attempt.
- `summary.json`, `summary.md`, `table_a.md`, `table_b.md`, `table_c.md`.
- `prove_vs_n.svg` dan `verify_vs_n.svg`: satu titik n=64 per kombinasi.
- `environment.json`, `build_provenance.json`, dataset, jadwal, correctness dan setup logs.

Folder output diabaikan Git. Arsipkan hasil beserta provenance apabila akan
dibagikan atau dipakai sebagai bukti eksperimen; jangan hanya mengambil Excel.

## Syarat Sebelum Campaign Final

EP compressed hidden adalah komposisi shared-response yang terdokumentasi,
bukan klaim implementasi literal hidden protocol dari paper. Tambahan 80 byte
terhadap plain bukan satu-satunya hal yang perlu disetujui: argumentasi extraction
multi-round, padding/nullity dan privacy harus direview secara matematis.

Dokumen review: [E1_COMPRESSED_HIDDEN.md](E1_COMPRESSED_HIDDEN.md).
Jika konstruksi perlu diubah, sesuaikan implementasi dan ulangi correctness gate
sebelum mengizinkan campaign final. Persetujuan ukuran proof saja tidak cukup.
Tidak tersedia flag untuk melewati pengaman ini.

Uji pengaman telah dilakukan dengan:

```bash
RAYON_NUM_THREADS=1 target/release/smp-e1 --profile full \
  --output outputs/e1-full-gate-check
```

Exit code 1 dan status `blocked_pending_research_review` adalah hasil yang
diharapkan saat ini. Tidak ada hasil full n=64..4096 yang diklaim telah diukur.
Demo AML dan tiga server tetap merupakan jalur berbeda; logika Rust AML tidak
diubah oleh implementasi E1 ini.
