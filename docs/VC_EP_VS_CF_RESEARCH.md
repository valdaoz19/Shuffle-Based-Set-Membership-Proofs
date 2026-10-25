# Riset Teoretis: Extended Pedersen (EP) vs Catalano-Fiore (CF) Vector Commitment

Untuk pembahasan **fundamental primitive saja** yang diminta Mas Valda,
termasuk pseudocode EP, CF-CDH, dan CF-RSA, gunakan
[VC_EP_CF_FUNDAMENTALS.md](VC_EP_CF_FUNDAMENTALS.md). Dokumen ini lebih luas:
ia menilai kemungkinan integrasi membership dan masalah protokol lanjutan.

Status: riset pemilihan metode, **tanpa implementasi dan tanpa angka benchmark**.
Angka mode `implicit` (Merkle) yang ada di repo **bukan** hasil EP atau CF dan
tidak boleh dipakai sebagai proxy.
Rekomendasi EP di bawah adalah prioritas untuk desain protokol berikutnya,
**bukan** kesimpulan bahwa `PermVC`/membership anonim sudah terbangun atau
terbukti aman.

**Koreksi keamanan penting:** relasi `exists i,rho: c_tid = S[i] + h*rho`
sebagai persamaan eksistensial saja tidak mendefinisikan keanggotaan yang
berguna: di grup siklik berorde prima, untuk setiap pasangan titik selalu ada
`rho` matematis. Yang relevan adalah *proof of knowledge* atas `rho` terhadap
set yang telah di-commit dan dipercaya verifier, beserta otorisasi Wallet
yang terpisah. Analisis lengkap, protokol pembanding konkret, dan daftar
keputusan desain ada di [MEMBERSHIP_PROTOCOL_FEASIBILITY.md](MEMBERSHIP_PROTOCOL_FEASIBILITY.md).

## 0. Label bukti

Setiap klaim diberi label agar fakta dan perkiraan tidak tercampur.

| Label | Arti |
| --- | --- |
| **[P]** | Tertulis di PDF deskripsi proyek (`Set_Membership_Proofs___Project_Description_260429_144046.pdf`, 2 hlm.) |
| **[S]** | Tertulis di spesifikasi formal repo (`docs/SET_MEMBERSHIP_FORMALIZATION.md`); bukan bukti bahwa seluruh fungsi target telah diimplementasikan |
| **[CF §x]** | Diverifikasi dari teks lengkap paper Catalano-Fiore, ePrint 2011/495 (37 hlm.), bagian/teorema yang disebut |
| **[L]** | Latar literatur tambahan. Abstrak/bagian relevan Bulletproofs telah dicek dari paper penulis; klaim rinci Bayer-Groth masih perlu dicek dari paper aslinya. Bukan bukti bahwa adaptasi ke relasi proyek ini sudah benar. |
| **[A]** | Analisis/perkiraan saya dari rumus di atas. Belum ada rujukan yang memverifikasi, harus diperlakukan sebagai hipotesis |
| **[?]** | Masalah terbuka atau belum diketahui |

Catatan metode: ringkasan otomatis WebFetch atas paper CF pada percobaan pertama
ternyata tidak andal (jawabannya dari ingatan, bukan isi PDF). Semua klaim
**[CF]** di bawah berasal dari ekstraksi teks penuh PDF, bukan dari ringkasan itu.

## 1. Pernyataan proyek dan pilihan pemodelan

**Formulasi PDF [P].** Commit ke vektor terurut `v = (v_1..v_n)` dengan
`C = h^r * prod_i g_i^{v_i}`. Prover harus meyakinkan verifier bahwa
(a) `v` adalah permutasi dari himpunan `S`, dan (b) `v_1 = x`. PDF menyebut
tantangan utamanya: *hidden permutation proof*, dan riset hybrid (subset yang
masing-masing di-commit, lalu konsistensi global antar blok).

**Formulasi spesifikasi repo [S].** Protokol 2 meng-commit vektor `V` dengan
`V_k = ReRand(S[sigma(k)]; delta_k)`, lalu membuktikan `PermVC` dan
`ZKVCMember` dengan **indeks tersembunyi `i`**.

Dua pilihan pemodelan yang mempengaruhi pilihan EP/CF **[A]**:

1. **Posisi target.** PDF menaruh `x` di posisi tetap (`v_1`) setelah permutasi
   rahasia. Ini *masih* menyembunyikan indeks asal `x` di `S`; jadi posisi tetap
   dan anonimitas indeks asal tidak bertentangan. Spesifikasi repo meminta hal
   tambahan: posisi `x` di vektor `V` juga tersembunyi. Untuk formulasi PDF,
   proof online dapat memakai relasi koordinat tetap, tetapi proof bahwa `V`
   adalah permutasi sah tetap wajib dan mungkin mendominasi biaya.
2. **Ruang pesan.** PDF: `v_i` adalah nilai (skalar). Spesifikasi repo: `V_k`
   adalah *commitment* (elemen grup, hasil rerandomisasi). Vector commitment
   Pedersen dan CF (CDH) sama-sama meng-commit **skalar** di `Z_p`. Meng-commit
   elemen grup tidak dapat diselesaikan dengan *hanya* meng-hash point ke
   skalar: hash tidak mempertahankan persamaan linear `ReRand`. Jalur desain
   yang perlu diteliti adalah meng-commit skalar ID lalu membuktikan link ke
   daftar commitment `S`, atau memakai representasi point di dalam proof
   tambahan. Ini belum diselesaikan oleh pilihan primitive VC saja.

Dokumen formal repo sudah memilih `S` sebagai daftar commitment. Penelitian
berikutnya perlu menurunkan relasi yang menghubungkan daftar itu dengan vektor
skalar pada formulasi PDF, bukan menganggap keduanya otomatis setara.

### 1.1 Kandidat relasi penghubung skalar dan commitment [A]

Gunakan notasi aditif sesuai Pedersen ID commitment pada spesifikasi repo.
Misalkan verifier mengetahui atau memiliki anchor tepercaya untuk
`S = (C_1,...,C_n)` dan target `c_tid`, dengan
`C_k = g_id * id_k + h * r_k`. Prover membentuk vektor skalar
`v_j = id_{sigma(j)}` dan EP vector commitment
`C_EP = H * r_EP + sum_j G_j * v_j`, dengan `sigma` permutasi rahasia.
Relasi yang **perlu** dibuktikan secara zero-knowledge dapat ditulis:

```text
exists sigma, v, r_EP, (r_1,...,r_n), rho such that:
  C_EP = H * r_EP + sum_j G_j * v_j
  for all j: C_{sigma(j)} = g_id * v_j + h * r_{sigma(j)}
  c_tid = C_{sigma(1)} + h * rho
  sigma is a permutation of {1,...,n}
```

Dengan relasi ini, posisi target pada `V` tetap `1`, sedangkan indeks asal
`sigma(1)` di `S` tetap rahasia *hanya jika proof tidak membuka mapping dan
metadata lain tidak membocorkannya*. Ini juga mengikat vektor skalar ke daftar
commitment tanpa meng-hash point ke skalar. **Ini baru statement/witness,
bukan protokol proof atau jaminan membership semantik.** Eksistensi `rho`
sendiri selalu benar secara aljabar; verifier membutuhkan proof of knowledge
yang sound, anchor `S` independen, dan bila keputusan pembayaran terkait
Wallet tertentu, bukti otorisasi/ownership terpisah. Masih perlu memilih
argument system, menjamin knowledge soundness dan zero-knowledge, serta
menghitung biaya pembuktian seluruh `n` link commitment. Jika `S` harus
disembunyikan seluruhnya dari verifier, pernyataan publik dan anchor-nya
harus dirancang lagi.

## 2. Tingkat 1: primitive VC (`Commit/Open/Verify`)

### 2.1 Definisi masing-masing

**EP [P]:** `C = h^r * prod_{i=1..n} g_i^{v_i}`. Sifat berikut umum diketahui
**[L]** (Pedersen hiding/binding memerlukan asumsi generator yang tepat;
paper Bulletproofs menjelaskan argumen inner-product dengan proof logaritmik,
tetapi bukan opening posisi gratis dari rumus EP):

- hiding sempurna jika `r` acak;
- binding: tidak ada relasi diskret-log nontrivial di antara `h, g_1..g_n`
  (generator dipilih *nothing-up-my-sleeve*, tanpa trapdoor);
- `Commit` = satu multi-eksponensiasi linear dalam `n`;
- Rumus dasar EP tidak menyediakan algoritme `Open(i)` ringkas seperti CF.
  Membuka seluruh vektor dan `r` membutuhkan `O(n)` skalar, tetapi proof
  koordinat yang lebih ringkas dapat dibangun dengan argumen tambahan
  (misalnya inner-product argument). Biaya dan sifat privasinya harus
  dilaporkan sebagai **EP + proof system**, bukan sifat rumus commitment saja.

**CF-CDH [CF §3.1, Thm 5]:**

- `KeyGen`: grup bilinear prime-order `G, G_T`, `h_i = g^{z_i}`,
  `h_{i,j} = g^{z_i z_j}` (`i != j`); `pp = (g, {h_i}, {h_{i,j}})`. Ruang pesan `Z_p`.
- `Com`: `C = h_1^{m_1} ... h_q^{m_q}`.
- `Open`: `Lambda_i = prod_{j != i} h_{i,j}^{m_j}` (= `(prod_{j!=i} h_j^{m_j})^{z_i}`).
- `Ver`: `e(C / h_i^{m_i}, h_i) = e(Lambda_i, g)`.
- Update: `C' = C * h_i^{m'-m}`; satu bukti posisi lain diperbarui
  `Lambda'_j = Lambda_j * h_{j,i}^{m'-m}`; bukti posisi `i` tidak berubah.
  Operasi `O(1)` berlaku untuk satu commitment atau satu opening; memperbarui
  seluruh `q` opening tetap membutuhkan kerja total `O(q)`.
- Keamanan: **position binding** di bawah Square-CDH, yang ekuivalen dengan
  CDH namun reduksinya **tidak tight**; kerugian faktor `q` (`eps/q`) pada
  tebakan indeks [CF Thm 5, §5.2].

**CF-RSA [CF §3.2, Thm 6]:**

- `KeyGen`: `N = p1*p2`, `q` bilangan prima `(l+1)`-bit `e_1..e_q`,
  `S_i = a^{prod_{j!=i} e_j}`; `pp = (N, a, S_1..S_q, e_1..e_q)`.
  Ruang pesan `{0,1}^l`.
- `Com`: `C = S_1^{m_1} ... S_q^{m_q} mod N`.
- `Open`: `Lambda_i = (prod_{j!=i} S_j^{m_j})^{1/e_i} mod N`, dapat dihitung
  dari `pp` **tanpa** faktorisasi `N`.
- `Ver`: `C = S_i^{m_i} * Lambda_i^{e_i} mod N`.
- Keamanan: position binding di bawah asumsi RSA (dengan `e` prima acak
  `(l+1)`-bit).
- Varian parameter konstan [CF §3.3, Thm 7]: `pp = (N, a, s, c)` lewat fungsi
  deterministik penghasil prima (teknik Hohenberger-Waters), butuh PRF.
  Paper menyebutnya "proof of concept" dan **secara komputasi kurang efisien**.

### 2.2 Tabel properti (primitive)

| Aspek | EP | CF-CDH | CF-RSA |
| --- | --- | --- | --- |
| Rumus commit | `h^r prod g_i^{v_i}` [P] | `prod h_i^{m_i}` [CF §3.1] | `prod S_i^{m_i} mod N` [CF §3.2] |
| Ruang pesan | Skalar pada field berorde grup, mis. `Z_p` [L] | `Z_p` (arbitrer via CRH) [CF fn.8] | `{0,1}^l` (arbitrer via CRH) [CF fn.9] |
| Asumsi | DL / DL-relation [L] | Square-CDH ≡ CDH, non-tight [CF] | RSA (`e` prima acak) [CF] |
| Hiding | Perfect dengan `r` [L] | **Tidak** (deterministik). Paper tidak memformalkan hiding; solusi: commit dulu tiap elemen dengan commitment standar, lalu VC [CF §1.1, §3] | Sama dengan CDH |
| Binding | Computational, atas seluruh vektor [L] | Position binding [CF Def.4] | Position binding [CF Def.4] |
| Ukuran commitment | 1 elemen grup [P] | 1 elemen `G` [CF, concise] | 1 elemen `Z_N` [CF, concise] |
| Ukuran opening 1 posisi | Rumus dasar tidak menyediakan opening ringkas; argumen tambahan dapat lebih pendek [A/L] | 1 elemen `G` [CF, concise] | 1 elemen `Z_N` [CF, concise] |
| Verifikasi opening | Perlu verifikasi argumen tambahan untuk klaim koordinat [A] | Persamaan pairing [CF §3.1] | Pemeriksaan modular dengan eksponen `e_i` [CF §3.2] |
| Parameter publik | `n+1` generator, transparan [L] | Prover `O(q^2)`; verifier `O(q)`, atau konstan dengan tanda tangan atas `(i,h_i)` [CF §3.1, §5.2] | `O(q)`; verifier hanya perlu memeriksa `S_i` valid sekali [CF §3.2]; konstan di §3.3 |
| Setup | Transparan [L] | `z_i` adalah toxic waste: siapa pun yang tahu `z_i` bisa membuka ke nilai lain di posisi `i` [A, turunan dari rumus verifikasi] | Faktorisasi `N` harus tidak diketahui prover [A, karena reduksi RSA mengasumsikan `N` acak] |
| Update | `C * g_i^{delta}`; biaya memperbarui argumen tambahan bergantung protokolnya [A] | `O(1)` per commitment/opening; `O(q)` untuk semua opening [CF §3.1] | `O(1)` per commitment/opening; `O(q)` untuk semua opening [CF §3.2] |
| Kapasitas | Bertambah dengan menambah generator [A] | Terikat `q` saat `KeyGen` [CF Def.] | Terikat `q` (kecuali §3.3) |
| Kurva/kebutuhan lib | Grup prime-order biasa | **Pairing-friendly curve** | Big-int RSA modulus |

## 3. Tingkat 2: kelayakan untuk tujuan proyek

### 3.1 `PermVC`: bukti bahwa vektor ter-commit adalah permutasi dari `S`

**EP: kandidat alami.** Bayer-Groth (Eurocrypt 2012) membuktikan kebenaran
shuffle dengan meng-commit permutasi memakai *n-element Pedersen commitment*;
komunikasinya sub-linear [L, dikonfirmasi ringkasan web]. Arah ini berkaitan
dengan "hidden permutation proof" yang disebut PDF sebagai tantangan utama
[P], tetapi statement kriptografinya tidak boleh diasumsikan identik.
Kedekatan aljabarnya membuat EP kandidat yang layak diteliti, tetapi belum
menunjukkan bahwa konstruksi Bayer-Groth bisa langsung membuktikan relasi
`PermVC` pada proyek ini. Klaim rinci tentang konstruksi Bayer-Groth masih
perlu dicek di paper aslinya. Catatan kehati-hatian:

- Bayer-Groth membahas shuffle atas enkripsi homomorfik. Menerapkannya pada
  pernyataan "vektor ter-commit adalah permutasi/rerandomisasi dari daftar
  commitment `S`" adalah **adaptasi yang perlu didesain dan dibuktikan**
  ulang [A]. Bagaimana ruang pesan skalar-vs-elemen grup dipecahkan (bagian 1)
  menentukan apakah adaptasinya langsung atau tidak.
- Nilai/ukuran proof konkret: **[?]**, belum diukur.

**CF: tidak natural.** CF dirancang untuk opening satu posisi yang ringkas,
sedangkan `PermVC` adalah relasi **global** atas semua posisi [A]. Tiga hambatan:

1. `CF-CDH.Com` berbentuk `prod h_i^{m_i}` (mirip commitment Pedersen tanpa
   blinding), tetapi basis `h_i` dipublikasikan bersama `h_{i,j} = h_i^{z_j}`.
   Argumen permutasi/inner-product yang ada [L] dibuktikan aman di bawah asumsi
   relasi-DL atas generator independen. Apakah asumsi itu tetap berlaku ketika
   `h_{i,j}` ikut dipublikasi (dan ada pairing) **[?] tidak saya temukan
   rujukan**; jangan diasumsikan.
2. CF tidak hiding (di paper sendiri): meng-hiding-kan berarti menyusun
   commitment-di-atas-commitment (§3), yang mengubah objek yang dibuktikan dan
   menambah biaya [A].
3. Tidak ada argumen permutasi di paper CF; itu di luar cakupannya [CF].

Penilaian **[A]**: EP memberi jalur penelitian yang lebih langsung untuk
relasi global, tetapi protokol adaptasi, keamanan, dan biayanya belum
ditetapkan. Paper CF sendiri tidak menyediakan `PermVC`; ini tidak berarti
tidak mungkin dibangun di atas CF.

### 3.2 `ZKVCMember`: membership dengan indeks tersembunyi

Bergantung pada formulasi (bagian 1):

**Formulasi PDF (`v_1 = x`, posisi tetap) [A].** Dengan EP, hubungan koordinat
pertama dengan commitment target merupakan kandidat relasi linear untuk proof
ZK. Indeks asal di `S` tetap harus disembunyikan oleh proof permutasi. Karena
posisi pada `V` sudah diketahui, opening posisi ringkas CF tidak otomatis
memberi keuntungan pada proof online ini. Bentuk proof dan biaya total tetap
harus diturunkan, termasuk link dari vektor skalar ke commitment `S`.

**Formulasi spesifikasi (indeks `i` tersembunyi) [A].**

- *EP:* opening telanjang membuka `i`. Yang layak adalah bukti relasi:
  misalnya OR-proof `O(n)`, atau argumen product `prod (v_i - x) = 0` dari
  keluarga Bayer-Groth [L/A]. Ukuran/biaya: **[?]**.
- *CF-CDH:* verifier CF memakai parameter yang spesifik posisi (`h_i` pada
  `e(C/h_i^{m_i}, h_i) = e(Lambda_i, g)`) [CF §3.1]. Jika `i` tersembunyi,
  `h_i` juga tersembunyi, sehingga harus dibuktikan **dalam zero-knowledge**
  bahwa `(h_i, Lambda_i)` memenuhi persamaan pairing *dan* `h_i` adalah salah
  satu parameter sah. Optimisasi tanda-tangan di §3.1 (tanda tangan atas
  `(i, h_i)`) menyarankan jalur: bukti pengetahuan tanda tangan + bukti
  pairing (Groth-Sahai atau SNARK). Ini **inferensi saya**, bukan yang
  dibahas paper. Paper tidak membahas privasi indeks sama sekali.
- *CF-RSA:* sama, tetapi `S_i` dan `e_i` tersembunyi dan verifikasi terjadi di
  grup berorde tak diketahui. Bukti ZK di grup semacam itu jauh lebih berat
  dan tidak ada rujukan di paper [A].

Penilaian **[A]**: hidden-index dengan CF = **masalah terbuka/rekayasa besar**;
dengan EP = layak, tetapi biaya bergantung pada argumen yang dipilih.

### 3.3 Konsistensi global pada hybrid

**EP [A, aljabar langsung].** Bila bucket `j` memakai irisan generator yang
saling lepas dan indeks generator disepakati, `D_j = h^{r_j} prod_i
g_{j,i}^{v_{j,i}}`, maka `prod_j D_j = h^{sum r_j} prod_{j,i}
g_{j,i}^{v_{j,i}}`. Perkalian ini menggabungkan commitment bucket menjadi
commitment atas vektor gabungan. **Persamaan itu saja tidak membuktikan** bahwa
bucket adalah partisi/permutasi yang benar dari `S`, atau bahwa setiap elemen
muncul tepat sekali; `ProvePartition` tetap dibutuhkan. Meng-hash `D_j`
memberi anchor berbeda dan tidak otomatis mempertahankan persamaan aljabar
tersebut. Baik EP maupun CF juga memerlukan root/set yang disepakati verifier;
root yang baru dikirim PayPay bersama proof tidak menjadi anchor independen
[S].

**CF [CF §3.1-3.2; A].** Commitment atas posisi yang saling lepas juga bisa
digabung dengan perkalian (`C = prod h_i^{m_i}`), dan `Update`/`ProofUpdate`
publik `O(1)` berguna bila set bergantung epoch dan sering berubah (set_id
mengikat epoch/versi [S]). Namun, konsistensi partisi (setiap elemen muncul
tepat sekali, bucket adalah rerandomisasi dari `S`) tetap butuh argumen
permutasi yang tidak disediakan CF (lihat 3.1).

### 3.4 Ringkasan kelayakan

| Tujuan proyek | EP | CF-CDH | CF-RSA |
| --- | --- | --- | --- |
| `VC.Commit/Open/Verify` (primitive) | Commit ya; opening ringkas tidak | Ya, concise [CF] | Ya, concise [CF] |
| Hiding | Ya (native) | Tidak (harus dilapis) | Tidak (harus dilapis) |
| `PermVC` | Kandidat yang lebih dekat dengan teknik shuffle Pedersen [L/A]; adaptasi belum dibuktikan | Tidak disediakan paper CF [CF]; integrasi terbuka | Sama |
| `ZKVCMember` (formulasi PDF) | Posisi `V` tetap, tetapi link dan permutasi rahasia tetap perlu proof [A] | Opening posisi ringkas bukan solusi lengkap [A] | Sama |
| `ZKVCMember` (indeks tersembunyi) | Layak, biaya ? [?] | Terbuka, butuh bukti pairing + set-membership atas `h_i` [A] | Terbuka, sangat berat [A] |
| Konsistensi global (hybrid) | Homomorfisme memberi komposisi commitment [A]; partisi tetap perlu proof | Komposisi mungkin [A]; partisi tak ditangani paper CF | Sama |
| Setup | Transparan | Trusted setup / toxic waste `z_i` [A] | Modulus RSA tak diketahui faktornya [A] |
| Kecocokan stack repo | Sama keluarga dengan IBC/Pedersen Ristretto255 | Butuh pustaka pairing baru | Butuh pustaka big-int/RSA baru |

Kecocokan stack: workspace saat ini hanya memakai `curve25519-dalek` (+ `merlin`)
untuk IBC, shuffle, update, dan adapter Persephone (dicek dari `Cargo.toml`);
tidak ada pustaka pairing. Kurva yang dipakai internal `cklr_service` belum
saya periksa.

## 4. Pro dan kontra

**EP**

- Pro: hiding sempurna pada rumus commitment dengan randomness acak; asumsi
  discrete-log relation; setup generator dapat dibuat transparan; satu
  keluarga dengan IBC/Pedersen di repo; kandidat basis untuk argumen
  permutasi/shuffle dan inner-product [L]; homomorfik untuk komposisi bucket
  jika generator/index disepakati [A].
- Kontra: rumus dasarnya tidak menyediakan opening per-posisi ringkas
  (pembukaan penuh `O(n)`); proof yang lebih ringkas memerlukan argumen lain;
  semua "opening" praktis harus berupa argumen ZK sehingga kerumitan pindah ke
  protokol pembukti; tidak ada mekanisme update bukti posisi bawaan [A].

**CF (umum)**

- Pro: opening dan commitment ukuran konstan, verifikasi publik; update
  commitment atau satu bukti posisi `O(1)` [CF]; dua instansiasi dengan asumsi klasik
  (CDH/RSA) [CF]; paper menunjukkan aplikasi ke accumulator, verifiable DB,
  ZK-EDB [CF §4-6].
- Kontra: **tidak hiding** (harus dilapis) [CF]; position binding saja [CF
  Def.4]; kapasitas `q` terikat saat setup [CF]; reduksi CDH tidak tight
  [CF §5.2]; tidak ada dukungan privasi indeks dan tidak ada argumen permutasi
  di paper [CF, A].
  - CDH khusus: parameter prover `O(q^2)`; butuh pairing; toxic waste `z_i`
    [A].
  - RSA khusus: elemen sebesar modulus RSA; pesan `l`-bit; modulus tak boleh
    diketahui faktornya; ZK di atas grup berorde tak diketahui berat [A]; varian
    parameter konstan lebih lambat [CF §3.3].

## 5. Kapan EP lebih cocok, kapan CF lebih cocok

**EP lebih cocok bila:**
1. Tujuannya membuktikan relasi global tersembunyi (permutasi/rerandomisasi)
   seperti pada PDF, bukan membuka satu posisi publik;
2. Privasi/hiding adalah persyaratan inti (identitas dan riwayat tidak boleh
   bocor);
3. Sistem harus tetap di `curve25519-dalek` tanpa pairing dan tanpa trusted setup;
4. Ukuran `n` mungkin besar dan tidak ingin terikat kapasitas `q` saat setup;
5. Formulasi memakai posisi tetap `v_1 = x`, sementara indeks asal di `S`
   disembunyikan melalui proof permutasi.

**CF lebih cocok bila:**
1. Yang dibutuhkan adalah **opening satu posisi yang ringkas, diverifikasi
   publik** atas registry/ledger besar (mis. bukti "nilai di posisi `i`" tanpa
   pengungkapan seluruh vektor), dan indeks tidak perlu disembunyikan;
2. Set sering berubah dan pemegang bukti perlu memperbarui bukti secara efisien
   (`ProofUpdate`);
3. Pairing dan trusted setup (CDH) atau modulus tepercaya (RSA) dapat
   diterima, dan hiding dapat dilapis di luar VC;
4. Kapasitas `q` diketahui di muka.

**Bila CF dipilih:** CF-CDH perlu dibandingkan dengan CF-RSA pada pustaka dan
parameter konkret. Dugaan awal bahwa CF-CDH lebih mudah dipadukan dengan proof
ZK atas grup prime-order adalah **hipotesis**, bukan hasil benchmark atau
jaminan integrasi dengan Ristretto255 [A].

## 6. Rekomendasi bersyarat

1. **Jika formulasi tetap seperti PDF** (vektor ter-commit adalah permutasi
   dari `S`, `x` di posisi tetap, hidden permutation): **prioritaskan EP untuk
   desain protokol berikutnya.** Struktur aljabarnya tampak dekat dengan
   kebutuhan proof global [P, L/A]. CF memberi opening posisi ringkas, tetapi
   paper CF tidak menyediakan proof permutasi. Ini belum pemilihan final:
   `PermVC`, link ke `S`, dan keamanan komposisinya harus diturunkan dahulu.
2. **Jika formulasi tetap seperti spesifikasi repo** (indeks tersembunyi `i`
   pada commitment atas *elemen grup*): EP tetap kandidat utama, tetapi
   keputusan ruang pesan (skalar vs elemen grup) harus diputuskan dulu, karena
   itu menentukan apakah `PermVC` adaptasi langsung atau tidak [A].
3. **Pertimbangkan CF terutama jika** kebutuhan bergeser ke registri dengan bukti
   posisi ringkas yang bisa diperbarui dan tidak perlu menyembunyikan indeks.
   Pilihan CF-CDH versus CF-RSA masih memerlukan evaluasi parameter, setup,
   pustaka, dan biaya konkret.
4. **Yang tidak boleh diklaim:** bahwa salah satu dari keduanya sudah
   memberi `ZKVCMember` atau konsistensi global hybrid secara utuh. Belum ada
   yang terbukti untuk CF; untuk EP masih butuh desain/pembuktian adaptasi.

Rekomendasi ini bersifat kualitatif; belum divalidasi oleh ukuran nyata.
Angka mode Merkle (`implicit`) tidak boleh dipindahkan sebagai hasil EP/CF.

## 7. Daftar fakta, perkiraan, dan masalah terbuka

**Didukung rujukan (dibaca dari sumber):**
- Rumus EP [P]; tujuan permutasi + `v_1 = x` dan tantangan hybrid [P].
- Rumus/keamanan CF-CDH dan CF-RSA, parameter, update, non-tight,
  tidak hiding, dan varian parameter konstan [CF §3.1-3.3, Thm 5-7, §5.2].
- Relasi dan protokol formal target di repo [S].

**Latar literatur, perlu diverifikasi ke paper asli:**
- Konstruksi detail Bayer-Groth dan adaptasinya ke relasi pada proyek ini.
  Paper Bulletproofs sudah dicek pada bagian awal yang relevan untuk klaim
  argumen logaritmik; sifat tepatnya untuk proof koordinat proyek ini belum
  diturunkan.

**Perkiraan saya [A]:**
- Rumus EP tidak menyediakan opening posisi ringkas sendiri; hidden-index CF
  memerlukan proof tambahan atas persamaan pairing atau RSA yang tidak
  dibahas paper; model setup/toxic-waste; konsistensi hybrid pada EP mungkin
  dapat memakai komposisi bucket dan proof permutasi atas vektor gabungan;
  asumsi DL-relation atas basis CF belum terjamin untuk argumen lain.

**Masalah terbuka [?]:**
- Definisi ruang pesan yang mempertahankan `ReRand` (skalar vs elemen grup).
- Adaptasi argumen permutasi Bayer-Groth ke pernyataan spesifikasi repo dan
  biaya konkretnya.
- Apakah argumen permutasi/inner-product tetap aman atas basis `h_i` CF yang
  dipublikasikan bersama `h_{i,j}`.
- Anchor kepercayaan `R_S` untuk hybrid (tata kelola, bukan primitive).
- Semua angka waktu/ukuran: belum ada, menunggu tahap implementasi dan benchmark.

## 8. Keputusan riset berikutnya

Untuk tahap kajian ini, tidak perlu mengirim empat pertanyaan sekaligus ke
stakeholder. Asumsi kerja yang sudah didukung dokumen proyek dan repo:

1. Gunakan posisi tetap `v_1 = x` **setelah permutasi rahasia** sebagai
   formulasi awal dari PDF. Indeks asal anggota di `S` tetap tersembunyi.
   Bandingkan juga varian indeks tersembunyi di `V` sebagai target yang lebih
   kuat pada spesifikasi repo, dengan biaya tambahan yang belum diketahui.
2. Pertahankan `S` sebagai daftar commitment sesuai spesifikasi formal repo.
   Turunkan secara eksplisit relasi yang menghubungkan commitment `S_k`,
   skalar ID `v_j`, dan `c_tid`; jangan menyamakan hash point dengan proof
   rerandomization. Alternatif group-valued vector membutuhkan proof system
   berbeda dan dapat dicatat sebagai pekerjaan lanjutan.
3. Tinjau CF sebagai kandidat maupun pembanding teoretis. Nilai CF-CDH dan
   CF-RSA pada dua level: primitive VC dan protokol membership lengkap.
4. Tulis model trust/setup yang diperlukan setiap kandidat, termasuk siapa
   yang menetapkan root `R_S` di pihak verifier.

Pertanyaan untuk Mas Valda baru diperlukan bila ia bermaksud **mengubah**
asumsi kerja tersebut, misalnya mewajibkan indeks di dalam `V` tetap rahasia
atau mengganti `S` dari daftar commitment menjadi daftar ID terbuka.

## 9. Rencana benchmark (tahap berikutnya, belum dijalankan)

Metrik yang perlu diukur nanti: waktu `Commit`, `Open`, `Verify` (dan waktu
bukti ZK di atasnya), ukuran commitment/opening/proof, ukuran dan waktu
pembuatan parameter, biaya `Update`/`ProofUpdate`, pada beberapa nilai `n`
(mis. 100, 1000, 10000). Semua diukur pada primitive yang benar-benar
diimplementasikan (EP, CF-CDH, opsional CF-RSA), bukan dari modul Merkle
yang ada.

## 10. Referensi

- Set Membership Proofs, Project Description (`Set_Membership_Proofs___Project_Description_260429_144046.pdf`).
- `docs/SET_MEMBERSHIP_FORMALIZATION.md`.
- D. Catalano, D. Fiore, *Vector Commitments and their Applications*,
  ePrint 2011/495 (https://eprint.iacr.org/2011/495.pdf).
- S. Bayer, J. Groth, *Efficient Zero-Knowledge Argument for Correctness of a
  Shuffle*, Eurocrypt 2012 (http://www0.cs.ucl.ac.uk/staff/J.Groth/MinimalShuffle.pdf).
- B. Bünz et al., *Bulletproofs: Short Proofs for Confidential Transactions
  and More* (https://web.stanford.edu/~buenz/pubs/bulletproofs.pdf).
