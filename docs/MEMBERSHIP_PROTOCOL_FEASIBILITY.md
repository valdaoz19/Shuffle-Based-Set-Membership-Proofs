# Kelayakan Protokol Set Membership: Setelah Kajian EP vs CF

Status: memo riset dan koreksi model keamanan, **bukan implementasi baru**.
Kode Rust `explicit`, `implicit`, dan `hybrid` tetap baseline PoC. Belum ada
benchmark Extended Pedersen (EP) maupun Catalano-Fiore (CF). Dokumen ini
melanjutkan [kajian EP vs CF](VC_EP_VS_CF_RESEARCH.md), bukan mengganti
[spesifikasi formal target](SET_MEMBERSHIP_FORMALIZATION.md) atau PDF-nya.

## 1. Keputusan sementara

**EP layak diprioritaskan untuk penelitian protokol global/permutasi; CF tetap
pembanding primitive VC.** Ini rekomendasi riset, bukan klaim bahwa EP sudah
memiliki `PermVC.ProvePerm`, `ZKVCMember`, atau bukti partisi yang siap dipakai.
Rumus komitmen vektor tidak otomatis menghasilkan protokol ZK untuk semua
relasi tersebut. Paper proyek memang menyebut hidden permutation proof sebagai
tantangan riset utama, sedangkan paper CF membahas opening posisi yang ringkas,
bukan proof permutasi atau hidden-index membership.
PDF proyek mengizinkan `S` dan `x` tetap rahasia; analisis protokol pembanding
di bawah sengaja memakai daftar *commitment* `S` yang terlihat verifier,
bukan membuka nilai ID-nya. Bila daftar commitment maupun ukuran set harus
tersembunyi, baseline tersebut tidak cukup dan pernyataan publik perlu
dirumuskan ulang.

Sebelum mengimplementasikan salah satu kandidat, ada empat prasyarat:

1. Verifier harus memiliki **anchor set/epoch independen dari proof PayPay**.
2. Relasi harus berupa **proof of knowledge** dengan asumsi generator yang
   jelas, bukan sekadar persamaan eksistensial.
3. Harus ditentukan siapa yang mengetahui witness rerandomisasi bila Wallet
   benar-benar membuat `c_tid` sendiri.
4. Membership harus dibedakan dari **otorisasi pemilik ID** dan dari bukti
   bahwa riwayat AML yang dihitung adalah riwayat ID yang sama.

## 2. Temuan matematis yang wajib diperbaiki

Ambil grup Ristretto berorde prima `q` dan generator non-identitas `h`.
Spesifikasi formal saat ini menulis:

```text
R_member(pp, S, c_tid) = 1 iff
  exists i in [0,n), rho in F_q: c_tid = S[i] + h*rho.
```

Karena setiap titik grup adalah kelipatan `h`, untuk **setiap** `c_tid` dan
`S[i]` terdapat suatu `rho` (mungkin tak dapat dihitung secara efisien).
Dengan demikian, relasi eksistensial itu benar untuk setiap target ketika
`S` tidak kosong. Klaim yang bermakna adalah *pengetahuan* `rho` untuk satu
anggota dari set yang sudah terikat:

```text
PoK { (i, rho): i in [0,n) AND D_i = c_tid - S[i] = h*rho }

Public context: pp, authenticated set_id/root/epoch, S or authenticated
descriptor, c_tid, request_id, protocol version.
```

Proof of knowledge tersebut **tidak membuktikan kepemilikan identitas**.
Siapa pun yang diberi commitment `S[i]` publik bisa membuat
`c_tid = S[i] + h*rho` sendiri dengan `rho` pilihannya. Untuk keputusan atas
Wallet tertentu, perlu bukti/credential terpisah bahwa Wallet berhak memakai
ID yang terikat pada `c_tid`, dan binding yang sama harus mencakup lookup
`gid` serta riwayat yang dipakai oleh threshold check. Parameter Pedersen
harus menghindari pihak yang mengetahui `log_h(g_id)`; jika tidak, pihak itu
dapat menghitung perubahan randomness ketika mengganti ID.

### 2.1 Titik konkret pada kode sekarang

| Temuan | Bukti kode | Dampak |
| --- | --- | --- |
| Target dibuat PayPay, lalu Wallet menyalin handle | [`demo_state.rs`](../crates/aml-protocol/src/demo_state.rs), [`aml-wallet`](../crates/aml-wallet/src/main.rs) | Bukan model Wallet membuat commitment secara independen. |
| Target adalah elemen yang **sama persis** dengan satu anggota `S` | [`demo_state.rs`](../crates/aml-protocol/src/demo_state.rs) memasukkan `tx_c`; [`membership`](../crates/aml-crypto-membership/src/lib.rs) mencari equality | Jika verifier melihat `S` dan `c_tid`, indeks asal dapat diketahui dari perbandingan byte. |
| Lookup mengharuskan equality byte target | [`aml-paypay`](../crates/aml-paypay/src/main.rs) membandingkan encoded target dengan handle tersimpan | Wallet target baru yang hanya rerandomization-equivalent ditolak **sebelum** membership proof. |
| `S` dikirim PayPay bersama proof | [`aml-paypay`](../crates/aml-paypay/src/main.rs), [`aml-alipay`](../crates/aml-alipay/src/main.rs) | Verifier belum mempunyai root/set tepercaya secara independen. PayPay dapat mengajukan set yang ia pilih sendiri. |
| `set_id` tidak diikat ke proof | [`MembershipStatement`](../crates/aml-crypto-membership/src/lib.rs) memuat field itu, tetapi proof tidak menggunakannya; PayPay mengisi `user_key`, Alipay+ mengisi literal lain | Bukti belum terikat ke policy/epoch/root sebagaimana spesifikasi. |
| Shuffle sigma memuat link per elemen | [`shuffle.rs`](../crates/aml-crypto-shuffle/src/shuffle.rs) mencoba tiap input untuk setiap link; [`proof.rs`](../crates/aml-crypto-shuffle/src/proof.rs) menyimpan digest permutasi | Ini bukan hidden-permutation proof. Mapping dapat direkonstruksi oleh verifier; digest bukan bukti dan pada `n` kecil dapat ditebak. |
| Implicit proof memuat `index`; hybrid memuat `bucket_index` | [`membership`](../crates/aml-crypto-membership/src/lib.rs) | Mode-mode itu belum menyembunyikan indeks/bucket. |
| Bucket dipilih dari hash bytes commitment | [`bucket_for`](../crates/aml-crypto-membership/src/lib.rs) memakai hash commitment terkompresi | Rerandomisasi mengubah bytes dan lazimnya bucket; aturan ini tidak mempertahankan membership rerandomization-equivalent. |

Ini bukan tuduhan bahwa demo saat ini tidak berfungsi: ia memverifikasi
**equality membership pada data dummy yang dibuat satu pihak**. Namun label
`APPROVED` tidak boleh dipresentasikan sebagai verifikasi set membership
anonim formal atau bukti otorisasi Wallet.
Set dummy pembayaran saat ini berisi delapan commitment, sedangkan `n=100`
pada `aml-metrics` adalah ukuran dataset benchmark yang berbeda.

### 2.2 Distribusi witness lintas tiga server

Jika Wallet benar-benar membuat `c_tid = g_id*tid + h*r_wallet`, sedangkan
PayPay menyimpan `S[i] = g_id*tid + h*r_paypay`, witness rerandomisasi ialah
`rho = r_wallet - r_paypay`. Wallet biasanya tidak tahu `r_paypay`; PayPay
tidak tahu `r_wallet`. Jadi **PayPay tidak dapat menjalankan PoK di atas hanya
dari dua commitment**, walaupun keduanya mengikat ID yang sama. Opsi desain:

- PayPay membuat/rerandomisasi target, lalu Wallet mengautorisasi penggunaannya.
  Ini paling dekat dengan demo sekarang, tetapi klaim "Wallet membuat
  commitment" harus diubah atau ditambah protokol terpisah.
- Wallet dan PayPay melakukan protokol bersama untuk membuktikan kesamaan ID
  tanpa saling membuka randomness. Ini menjaga pembuatan target di Wallet,
  tetapi lebih kompleks dan **belum didesain** di repo.
- Wallet diberi opening atau witness yang memadai saat registrasi, lalu ia
  dapat membuat target dan membuktikan relasi; peran prover proof membership
  perlu dipindah/dibagi, dan kebijakan pembagian secret harus ditentukan.

Tidak aman untuk menganggap `rho` otomatis tersedia hanya karena kedua
commitment mempunyai ID yang sama.

## 3. Protokol pembanding yang konkret: OR-Schnorr atas selisih commitment

Sebagai **baseline formal minimal** untuk set commitment publik-terikat,
protokol OR dari keluarga proof of partial knowledge dapat menunjukkan
pengetahuan `rho` untuk *satu dari n* titik `D_i`, tanpa membuka indeks.
Ini **bukan EP/CF**, bukan fitur runtime saat ini, dan ukuran proof-nya `O(n)`.
Nilainya untuk riset: memberi pembanding yang benar-benar terdefinisi,
sehingga EP/CF hanya dipilih bila memberi keuntungan pada protokol lengkap,
bukan hanya pada primitive `Commit`.

```text
Parameter: grup G berorde prima q; h in G non-identitas; H_FS hash-to-scalar.
Public: pp, S=(C_0,...,C_{n-1}), c_tid, trusted set_id/root, request_id.
Witness prover: i*, rho dengan c_tid - C_i* = h*rho.
Output: pi=(A_0,...,A_{n-1}, e_0,...,e_{n-1}, z_0,...,z_{n-1}).

Prover:
  D_i <- c_tid - C_i untuk semua i.
  Untuk setiap j != i*: pilih e_j,z_j <- F_q;
                     A_j <- h*z_j - e_j*D_j.  (simulated branch)
  Pilih alpha <- F_q; A_i* <- h*alpha.
  e <- H_FS(domain, pp, set_id, root, request_id, S, c_tid, A_0,...,A_{n-1}).
  e_i* <- e - sum_{j != i*} e_j mod q.
  z_i* <- alpha + e_i* * rho mod q.
  Kirim pi.

Verifier:
  Validasi group encodings, panjang n, set_id/root/epoch dan context request.
  Hitung ulang D_i serta e dari public context dan semua A_i.
  ACCEPT iff sum_i e_i = e (mod q) dan untuk semua i:
    h*z_i = A_i + e_i*D_i.
```

Persamaan cabang nyata benar karena
`h*(alpha+e_i*rho)=h*alpha+e_i*(h*rho)`. Cabang lain disimulasikan
tanpa mengetahui witness. Pengikat Fiat-Shamir harus memakai encoding
kanonik, domain separation, seluruh statement, root terpercaya, dan nonce
atau `request_id`; pembuktian keamanan NIZK memerlukan model/analisis
tersendiri. Literatur primer menjelaskan disjunctive sigma proof dan versi
one-out-of-many yang lebih ringkas: [Cramer-Damgard-Schoenmakers,
1995](https://ir.cwi.nl/pub/1368) dan [Groth-Kohlweiss,
2015](https://discovery.ucl.ac.uk/1502142/1/Groth_764.pdf).
Groth-Kohlweiss membuktikan satu dari banyak commitment membuka ke nol;
`D_i` di sini tepat memiliki bentuk itu bila witness `rho` diketahui.
**Jangan** mengklaim konstruksi di atas telah melalui audit keamanan atau
bahwa biaya Groth-Kohlweiss otomatis berlaku pada integrasi kita.

Privasi indeks tetap gagal bila metadata membocorkan indeks: misalnya
`c_tid` sama byte-for-byte dengan `C_i`, set hanya satu elemen, atau
`bucket_index`/permutation links dikirim terbuka. Bukti ZK tidak dapat
memperbaiki kebocoran dari statement publik itu sendiri.

## 4. Jalur EP: relasi lengkap yang harus dibuktikan

Untuk formulasi PDF, letakkan ID target di koordinat tetap setelah
permutasi rahasia. Ini menyembunyikan indeks *asal* di `S`, bukan mewajibkan
koordinat di vektor tersembunyi. Bila `S` adalah daftar commitment seperti
spesifikasi repo, tetapi EP meng-commit skalar, perlu satu proof gabungan:

```text
Public: pp, authenticated S=(C_0,...,C_{n-1}), c_tid, set_id, C_EP.
Witness: permutation sigma, ids v_0..v_{n-1}, randomness r_EP,
         openings r_0..r_{n-1}, target rerandomization rho.

C_EP = H*r_EP + sum_{j=0}^{n-1} G_j*v_j
for all j: C_{sigma(j)} = g_id*v_j + h*r_{sigma(j)}
c_tid = C_{sigma(0)} + h*rho
sigma is a bijection on [0,n).
```

Proof harus mengikat **witness yang sama** untuk ketiga baris. `C_EP`
berukuran satu titik, tetapi itu tidak menentukan ukuran/biaya argument
`PermVC`; semua `n` link commitment dan kendala permutasi tetap harus
dibuktikan. Persamaan di atas adalah *spesifikasi relasi*, bukan implementasi
`PermVC.ProvePerm`. Bukti [Bayer-Groth untuk shuffle ciphertext
ElGamal](https://www0.cs.ucl.ac.uk/staff/J.Groth/MinimalShuffle.pdf)
adalah literatur relevan, tetapi **bukan** drop-in proof untuk statement
EP-plus-Pedersen-ID ini. Penelitian berikutnya harus menemukan atau
menurunkan argument system yang cocok, dengan analisis soundness/ZK dan
biaya nyata. Jika ternyata lebih mahal daripada one-out-of-many langsung,
EP tidak boleh dipilih hanya karena commitment-nya kecil.

## 5. Jalur CF dan hybrid

CF memberi `Commit/Open/Verify` yang ringkas dan dapat diperbarui per posisi,
tetapi paper aslinya tidak memberi `PermVC`, hidden-index member, atau
`ProvePartition`. CF-CDH tidak hiding sendiri dan membutuhkan grup bilinear;
CF-RSA memakai modulus RSA. Keduanya tetap memerlukan argumen ZK tambahan
untuk target proyek. Bandingkan **biaya protokol lengkap**, bukan ukuran
opening CF melawan ukuran seluruh proof EP. Sumber primer:
[Catalano-Fiore, Vector Commitments and their Applications](https://www.iacr.org/archive/pkc2013/77780054/77780054.pdf).

Untuk hybrid, kondisi yang cukup bukan hanya dua proof terpisah dengan
kalimat "ada bucket valid" dan "ada member valid". Keduanya harus
merujuk ke **bucket commitment `D_j` yang sama**, meski `j` tersembunyi:

```text
PoK { j, D_j, B_j, tau, rho, path_j, partition_witness:
    R_global authenticates D_j at j
    AND D_j binds B_j
    AND c_tid = B_j[tau] + h*rho
    AND all buckets form a one-to-one rerandomized partition of trusted S
}
```

`VerifyGlobal` dan `BucketMember.Verify` yang tidak berbagi witness/commitment
terikat bisa benar untuk bucket berbeda dan **tidak menyimpulkan** membership
global. Hash bucket dari bytes commitment juga tidak stabil saat
rerandomisasi; bucket harus ditetapkan dari atribut yang stabil dan dapat
dibuktikan, atau penentuan bucket dimasukkan ke proof ZK bersama. `R_global`
yang dibawa PayPay bersama proof belum cukup sebagai trust anchor; Alipay+
harus sudah memperoleh root/epoch itu lewat kanal autentik atau registry.

## 6. Gate keputusan dan pertanyaan untuk Mas Valda

Asumsi kerja paling hemat perubahan: `S` adalah daftar commitment pada epoch
tertentu; Alipay+ menyimpan root tepercaya; PDF-style `v_0=x` boleh digunakan
asalkan indeks asal di `S` tetap tersembunyi; implementasi tetap ditunda
sampai model witness dan otorisasi jelas. **Tiga konfirmasi yang menentukan**:

1. **Siapa menerbitkan dan menandatangani root/set `S` untuk Alipay+?**
   Apakah hanya PayPay sebagai otoritas yang dipercaya untuk keanggotaan,
   atau ada registri independen? Jika PayPay dipercaya menentukan isi `S`,
   proof hanya mencegah inkonsistensi terhadap *set pilihannya*, bukan
   menjamin kelayakan bisnis/AML.
2. **Siapa sebenarnya membuat `c_tid` dan siapa memegang randomness-nya?**
   Jika Wallet membuatnya sendiri, bagaimana PayPay memperoleh witness
   kesamaan tanpa membuka ID/randomness? Jika PayPay membuatnya seperti demo,
   apa bentuk otorisasi Wallet agar PayPay tidak dapat memakai ID orang lain?
3. **Target privasi mana yang wajib?** Cukup sembunyikan indeks asal anggota
   di `S` (PDF: target boleh di koordinat pertama vektor rahasia), atau juga
   sembunyikan posisi di `V` serta bucket di hybrid? Pilihan ini mengubah
   relasi dan perkiraan biaya proof.

Teks singkat untuk dikirim:

> Mas, agar saya bisa mengunci desain proof EP/CF, boleh konfirmasi tiga hal:
> (1) root/set commitment `S` yang dipakai Alipay+ ditetapkan siapa dan apakah
> Alipay+ menyimpannya sebelum request? (2) `c_tid` dibuat Wallet atau PayPay,
> dan siapa yang mengetahui randomness untuk proof kesamaan ID? (3) privasi
> wajibnya menyembunyikan indeks asal di `S` saja, atau posisi vektor/bucket
> juga? Saya tetap mulai dari asumsi PDF: target di posisi pertama setelah
> permutasi rahasia, tetapi asalnya tersembunyi.

## 7. Urutan kerja yang disarankan

1. **Sekarang:** review tiga jawaban di atas; tetapkan threat model,
   ownership, anchor, dan relasi gabungan membership + otorisasi + history.
2. **Desain:** tulis protokol lengkap yang benar-benar mengisi `PermVC` dan
   link ke Pedersen-ID, beserta completeness, knowledge soundness, ZK,
   dan estimasi kompleksitas. Pakai OR-Schnorr atau Groth-Kohlweiss sebagai
   baseline pembanding, bukan sebagai hasil EP/CF.
3. **Baru implementasi:** prototipe proof terpilih dengan negative tests:
   set palsu/root salah, target asing, witness salah, metadata yang membuka
   indeks, rerandomisasi lintas bucket, proof beda epoch, dan linkage history
   salah. Lalu benchmark `n=100/1000` pada hardware, profil build, dan data
   yang sama. Jangan memakai hasil Merkle PoC sebagai angka EP/CF.

## 8. Rujukan dan batas klaim

- [PDF deskripsi proyek](../Set_Membership_Proofs___Project_Description_260429_144046.pdf): masalah, formulasi vektor Pedersen, target posisi pertama, dan hybrid; tidak memberikan konstruksi lengkap hidden permutation.
- [Spesifikasi formal repo](SET_MEMBERSHIP_FORMALIZATION.md): fungsi dan relasi target; beberapa syarat keamanan penting pada memo ini belum tercermin di PDF formal lama.
- [Cramer-Damgard-Schoenmakers](https://ir.cwi.nl/pub/1368): proof of partial knowledge sebagai dasar pembanding OR.
- [Groth-Kohlweiss](https://discovery.ucl.ac.uk/1502142/1/Groth_764.pdf): one-out-of-many proof dan aplikasi membership atas commitment.
- [Bayer-Groth](https://www0.cs.ucl.ac.uk/staff/J.Groth/MinimalShuffle.pdf): proof shuffle ciphertext homomorfik; adaptasi ke relasi proyek belum tersedia.
- [Catalano-Fiore](https://www.iacr.org/archive/pkc2013/77780054/77780054.pdf): primitive VC/CDH/RSA, bukan protokol membership tersembunyi proyek.
