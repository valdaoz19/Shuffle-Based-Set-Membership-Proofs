# Toolchain CI dan Baseline Riset

Baseline per 2026-10-25: **Rust 1.96.0** untuk checkout lokal dan Docker builder.
CI menguji baseline dan stable terbaru secara terpisah pada `ubuntu-24.04`.
Kedua job wajib lulus; tidak ada pengecualian kegagalan untuk stable.

## Penyebab Kegagalan Sebelumnya

Pemeriksaan CI sebelumnya gagal saat Clippy mendeteksi `chunks_exact(32)` dalam
decoder proof EP. Lingkungan CI memakai Clippy 1.99 sedangkan pengujian lokal memakai
1.96. `-D warnings` mengubah lint gaya tersebut menjadi error dan langkah tests
pada run itu dilewati. Ini bukan hasil test Linux yang gagal atau temuan
keamanan kriptografi.

Decoder sekarang menggunakan `as_chunks::<32>()`, menolak remainder, dan tetap
memeriksa panjang proof persis sebelum parsing. Setiap scalar masih melalui
decoder kanonis yang sama. Tidak ada perubahan encoding, ukuran proof,
persamaan verifier, atau pengecualian lint. Regresi plain/hidden mencakup proof
valid, kosong, scalar parsial, satu byte terpotong, kekurangan/tambahan satu
scalar, dan scalar nonkanonis.

## Pemilihan Versi

- `rust-toolchain.toml` memilih `1.96.0` dengan rustfmt/clippy. Rustup dapat
  menginstalnya tanpa mengganti default global pengguna.
- Docker builder memakai `rust:1.96.0-bookworm`; compiler tidak diperlukan
  pada server yang hanya menjalankan runtime image AML.
- Matrix CI berisi `1.96.0` dan `stable`. Semua command menggunakan
  `cargo +<toolchain>` agar job stable tidak mengikuti pin repo secara tidak sengaja.
- Tiap job mencetak versi rustc, Cargo dan Clippy. Cache dipisahkan berdasarkan
  toolchain dan key compiler dari rust-cache. `fail-fast: false` memberi hasil
  kedua job walaupun salah satunya gagal; tidak mengubah status wajib lulus.
- `actions/checkout@v6` dan rust-cache v2 memakai runtime Node 24. Tidak perlu
  mengubah runtime aplikasi Rust atau menambahkan environment bypass Node 20.

`Cargo.lock` mengunci resolusi dependency, bukan compiler atau OS. Jangan
menghilangkan `--locked` untuk mengatasi lint. Pembaruan stable dapat memunculkan
lint baru; perbaiki kasus konkret, jalankan regresi, dan pertahankan baseline
riset sampai upgrade compiler direncanakan secara eksplisit.

## Validasi Lokal dan GitHub

Jalankan dari root repo. Rust 1.99.0 di bawah adalah versi yang digunakan run
bermasalah; untuk run mendatang cocokkan dengan versi bernomor pada log CI.

```bash
rustup toolchain install 1.96.0 1.99.0 --profile minimal --component rustfmt --component clippy
cargo +1.96.0 fmt --all -- --check
cargo +1.96.0 clippy --locked --workspace --all-targets -- -D warnings
cargo +1.96.0 test --locked --workspace
cargo +1.99.0 fmt --all -- --check
cargo +1.99.0 clippy --locked --workspace --all-targets -- -D warnings
cargo +1.99.0 test --locked --workspace
docker build -t aml-zkp-poc:ci-toolchain-check .
```

Hasil lokal macOS dan hasil GitHub Linux harus dilaporkan terpisah. Setelah
commit perbaikan di-push, periksa **run baru** pada commit tersebut. Re-run
workflow commit lama tetap membaca kode lama. Nyatakan Linux lulus hanya jika
Clippy dan tests benar-benar lulus pada kedua job di run baru.

## Hasil Validasi Perbaikan, 2026-10-25

| Lingkungan | Compiler | Format | Clippy `-D warnings` | Workspace tests |
| --- | --- | --- | --- | --- |
| macOS ARM64 | 1.96.0 | Lulus | Lulus | 58 lulus |
| macOS ARM64, target cache terpisah | 1.99.0 | Lulus | Lulus | 58 lulus |

Global default rustup tetap `stable`; checkout ini memilih baseline lewat file
toolchain. Pengujian 1.99.0 menggunakan selector versi bernomor, sehingga tidak
perlu meng-upgrade channel stable global milik pengguna. Resolusi dependency dan
hasil eksperimen historis tidak diubah.

Build image `aml-zkp-poc:ci-toolchain-check` berhasil pada Docker **Linux ARM64**
dengan builder 1.96.0. Pemeriksaan container tanpa network mengonfirmasi kelima
binary AML dan `web/index.html` tersedia. Ini pengujian build/image lokal,
bukan hasil workspace tests GitHub pada runner Ubuntu x64.

Struktur YAML, matrix wajib lulus, selector toolchain eksplisit, pin Docker/lokal,
dan tautan Markdown lokal juga telah diperiksa. Setelah initial commit dikirim,
periksa run GitHub pada repo ini untuk status Linux CI snapshot yang dipublikasikan.

Pin compiler dan perbaikan parsing tidak menyelesaikan review matematis
compressed hidden. Profil E1 `full` tetap memiliki pengaman review; data smoke
dan provenance terdahulu tetap merupakan hasil historis dengan konfigurasi aslinya.
