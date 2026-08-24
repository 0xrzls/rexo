# 04 — Audit

## Batasan yang harus kamu baca sebelum apa pun

Kamu minta ini "bener-bener teraudit full". Aku harus lurus soal ini,
karena salah paham di sini bisa membuatmu kehilangan uang orang lain.

**Aku tidak bisa memberimu audit keamanan.** Audit keamanan adalah
firma independen, berminggu-minggu, membaca kode yang benar-benar
di-deploy, dengan tanggung jawab profesional atas hasilnya. Yang aku
berikan adalah **laporan verifikasi**: matematika yang bisa dieksekusi
dan diuji, invariant yang ditegakkan, model ancaman, dan daftar jujur
apa yang belum diverifikasi.

Itu berbeda. Tapi itu nyata, dan itu prasyarat sebelum audit sungguhan —
auditor akan meminta persis dokumen seperti ini di hari pertama.

**Sebelum uang sungguhan masuk, kamu tetap butuh audit eksternal dari
firma yang punya pengalaman Rialo/Venus.**

---

## Bagian 1 — Apa yang benar-benar terverifikasi

### 1.1 Verifikasi eksekusi

`src/curve.rs` — **21 test, nol dependency.** Bisa kamu jalankan sendiri:

```bash
rustc --test src/curve.rs -o /tmp/curve_test && /tmp/curve_test
```

Aku tidak punya toolchain Rust di lingkungan ini, jadi aku memverifikasi
setiap nilai numerik lewat **model referensi independen di Python** yang
mengimplementasikan ulang matematika yang sama. Setiap konstanta yang
di-assert di test suite berasal dari eksekusi model itu, bukan dari
perkiraan.

Ini metodologi yang lemah di satu sisi (dua implementasi bisa salah
dengan cara yang sama) dan kuat di sisi lain (perbedaan pembulatan
antara Python integer division dan Rust u128 akan langsung terlihat).
**Jalankan `rustc --test` sendiri untuk menutup celah ini.** Kalau ada
test yang gagal, itu temuan nyata dan aku ingin tahu.

### 1.2 Paritas dengan pump.fun — terverifikasi persis

Test `constants_match_pumpfun_global_account` menegakkan:

| Konstanta | Nilai | Sumber |
|---|---|---|
| `virtual_token_reserves` | 1.073.000.000.000.000 | `Global` account pump.fun |
| `virtual_sol_reserves` | 30.000.000.000 | idem |
| `real_token_reserves` | 793.100.000.000.000 | idem |
| `token_total_supply` | 1.000.000.000.000.000 | idem |
| gap | 279.900.000.000.000 | turunan |

Rumus market cap juga identik:
`virtual_sol_reserves * token_total_supply / virtual_token_reserves`.

Hasil turunan yang terverifikasi:
- Kelulusan pada **85,005359057 RLO net** / 85,863999048 RLO gross
- Market cap awal ~27,96 RLO → akhir ~410 RLO (±15x)
- Round-trip beli-jual kehilangan persis 2% (dua kali fee 1%)

### 1.3 Perbedaan yang disengaja dari pump.fun

**Satu perbedaan yang harus kamu sadari:** pump.fun `buy` menerima
`token_amount` sebagai input dan menghitung `sol_cost` (exact-output).
`curve.rs` menerima `quote_in` dan menghitung `tokens_out`
(exact-input).

Keduanya sah dan menghasilkan kurva yang sama. Exact-input lebih ramah
untuk UI "beli senilai 5 RLO" dan lebih aman terhadap slippage karena
pengguna tahu persis berapa yang dibelanjakan. Tapi kalau kamu
membandingkan angka langsung dengan implementasi pump.fun, ingat
arahnya terbalik.

---

## Bagian 2 — Invariant yang ditegakkan

Ini kontrak yang dijaga test suite. Kalau kamu mengubah `curve.rs`,
jalankan ulang; kalau ada yang pecah, kamu baru saja memperkenalkan bug.

| # | Invariant | Test |
|---|---|---|
| I1 | `k` tidak pernah menyusut di operasi apa pun | `many_random_ops_never_break_invariants` |
| I2 | `real_token + token_dipegang == curve_supply` selalu | idem |
| I3 | Setiap RLO masuk terhitung: `gross == real_quote + fees` | `fees_never_touch_curve_reserves` |
| I4 | Fee terbagi persis, tidak ada unit hilang | `fee_split_is_exact_and_tier_dependent` |
| I5 | Pembeli membayar sama di semua tier | `buyer_pays_identical_price_across_all_tiers` |
| I6 | Harga monoton naik saat beli | `price_is_monotonic_and_curve_graduates` |
| I7 | State tidak berubah kalau operasi gagal | `slippage_guard_blocks_bad_fill_without_mutating_state` |
| I8 | Tidak bisa jual lebih dari yang beredar | `cannot_sell_more_than_circulating` |
| I9 | Tidak ada trading setelah lulus, selamanya | `no_trading_after_graduation` |
| I10 | Bond hangus tidak menggeser harga | `forfeited_bond_goes_to_lp_not_protocol` |
| I11 | Overbuy diisi persis, sisanya dikembalikan | `oversized_buy_fills_exactly_and_refunds` |

**I1 adalah yang paling penting.** `k` yang menyusut berarti pembulatan
membocorkan nilai dari pool — kelas bug yang menguras AMM di dunia
nyata. Test acak menjalankan 2.000 operasi campuran dan memeriksa `k`
setelah setiap satu.

### Kebijakan pembulatan

Setiap pembulatan menguntungkan pool, tidak pernah trader. Ini dicapai
dengan `ceil_div` pada **pembagi**, bukan hasilnya:

```rust
tokens_out = virtual_token - ceil_div(k, new_vq)   // out dibulatkan KE BAWAH
gross_out  = virtual_quote - ceil_div(k, new_vt)   // out dibulatkan KE BAWAH
```

Jangan ubah arahnya tanpa menjalankan test acak.

---

## Bagian 3 — Yang BELUM terverifikasi

Bagian ini lebih penting daripada bagian 1. Baca sampai habis.

### 3.1 `src/lib.rs` tidak compile dan tidak diklaim compile

Ini kerangka desain. Struktur workflow-nya mengikuti aturan terverifikasi
dari `AGENTS.md` resmi repo rialo-examples (peran fungsi, statement
async, larangan loop, semantik blok `rex`). Tapi hal-hal berikut **aku
karang berdasarkan pola yang masuk akal, bukan dokumentasi**:

- sintaks deklarasi struct state
- signature `SEND`, `ctx.sender()`, `ctx.now()`
- API pembuatan mint, transfer token, vault
- API sealed batch / enkripsi REX
- API posisi Stake-for-Service

Kenapa: `docs.rialo.io` memblokir akses otomatis, dan direktori repo
rialo-examples juga. Aku bisa membaca `README.md` dan `CLAUDE.md`-nya
(itu file, bukan direktori) tapi tidak `venus/*/src/lib.rs`.

**Konsekuensinya:** jangan copy-paste `lib.rs` lalu berharap `cargo
check` lolos. Samakan dulu dengan `venus/http-fetch/src/lib.rs`.

### 3.2 Asumsi yang bisa membatalkan kalibrasi

| Asumsi | Risiko kalau salah | Perbaikan |
|---|---|---|
| RLO punya 9 desimal | Semua angka bond & kurva meleset 10^n | Ubah satu konstanta |
| RLO ada dan bisa ditransfer | Ekonomi belum bisa dijalankan | Tunggu tokenomics |
| REX mengekspos batch decryption | Sealed window mundur ke commit-reveal | Desain ulang mekanisme 3 |
| SfS tersedia untuk program, bukan cuma user | Token self-funding tidak jalan | Fallback ke treasury biasa |
| Webcall bisa memakai secret tersegel | Verifikasi sosial mustahil | **Ini membatalkan seluruh tesis** |

Baris terakhir itu yang harus kamu konfirmasi paling awal. Kalau REX
tidak bisa menyimpan API key dan memanggil API eksternal dengannya,
mekanisme utama Coldstart tidak bisa dibangun dan kamu perlu tesis lain.

Whitepaper privasi Rialo mendeskripsikan kemampuan ini secara eksplisit
("menyimpan dan menggunakan kredensial autentikasi seperti API key...
tanpa pernah mengekspos kredensialnya"). Tapi whitepaper adalah
pernyataan niat, bukan bukti API-nya sudah ada di testnet Agustus 2026.
**Verifikasi ini di Discord sebelum menulis kode apa pun.**

---

## Bagian 4 — Model ancaman

### 4.1 Yang sudah dimitigasi dalam desain

| Ancaman | Mitigasi | Status |
|---|---|---|
| Sniper first-block | Sealed window, satu clearing price | Desain; butuh API REX |
| Sniper saat kelulusan | Sealed window dibuka ulang pasca-migrasi | Desain |
| Rug pull lewat mint tambahan | Cabut mint+freeze authority di tx launch | **TODO di lib.rs** |
| Rug pull lewat tarik LP | Burn/lock LP saat kelulusan | **TODO di lib.rs** |
| Dev dump instan | Vesting reaktif berbasis predikat | Desain |
| Token ditinggalkan | Heartbeat 6 jam, bond hangus ke LP | Desain |
| Rounding drain | `ceil_div` pada pembagi, test acak | **Terverifikasi** |
| Slippage / MEV pada trade biasa | `min_out` di buy dan sell | **Terverifikasi** |
| Overflow aritmetika | u128 + `checked_*` di seluruh jalur | **Terverifikasi** |
| Fee mencuri dari kurva | Akuntansi terpisah, I3 | **Terverifikasi** |

### 4.2 Ancaman yang BELUM dimitigasi

Ini yang harus kamu selesaikan sendiri. Aku menyebutnya karena
menyembunyikannya jauh lebih berbahaya daripada daftar fitur yang
terlihat lengkap.

**A. Escalation tier lewat argumen.** Kalau `tier` bisa dikirim sebagai
argumen program, seluruh sistem verifikasi runtuh. Di `lib.rs` tier
hanya ditulis oleh handler `on_socials_verified`. Pastikan tidak ada
jalur lain — termasuk fungsi admin — yang bisa menaikkan tier.

**B. Kebocoran order sealed lewat state.** Kalau order antre disimpan
plaintext di workflow state, sniper cukup membaca state dan seluruh
mekanisme bocor. Di `lib.rs` ini ditandai TODO. Ini bukan detail
implementasi; ini keamanan.

**C. Kegagalan heartbeat terkorelasi.** Kalau API Telegram down 6 jam,
implementasi naif akan menandai ribuan token sebagai abandoned dan
menghanguskan bond mereka. **Ini bug paling berbahaya di seluruh
desain**, karena merusak pengguna yang tidak bersalah dalam skala besar
dan tidak bisa dibatalkan.

Mitigasi wajib: sebelum menandai abandonment, cek apakah kegagalannya
terkorelasi lintas token. Kalau tingkat kegagalan global melewati ambang,
bekukan seluruh penilaian abandonment sampai pulih. **Belum ada di
`lib.rs`. Tambahkan sebelum apa pun.**

**D. Sybil pada verifikasi sosial.** Anggota Telegram bisa dibeli murah.
Desain ini tidak melawan penyerang yang gigih — ia menaikkan lantai dari
"tidak ada usaha" ke "harus nyata dan tetap hidup". Data SSRN menunjukkan
itu saja sudah menggeser distribusi (lift 8,94x). Tapi jangan
memasarkannya sebagai anti-scam. Itu klaim yang tidak bisa kamu tepati.

**E. Akuntansi vault.** `curve.rs` sudah teruji, tapi ia **tidak tahu
apa-apa soal siapa yang benar-benar memegang token**. Transfer, saldo
vault, dan otorisasi hidup di lapisan program — dan di situlah bug
biasanya hidup. Audit lapisan itu **terpisah** dari matematika kurva.

**F. Reentrancy lewat transaksi kondisional.** Model reaktif Rialo
memungkinkan transaksi kondisional membuat transaksi kondisional lain.
Semantik reentrancy dari pola itu belum kupelajari. Kalau `buy()` memicu
`graduate()` yang memicu sesuatu yang memanggil `buy()` lagi, apa yang
terjadi? **Aku tidak tahu jawabannya dan kamu harus mencari tahu.**

**G. Front-running predikat.** Predikat dievaluasi di akhir blok dan
terlihat on-chain. Penyerang bisa membaca predikat kelulusanmu dan
memposisikan diri di blok sebelum ia menyala. Sealed window pasca-lulus
memitigasi sebagian, tapi tidak seluruhnya.

---

## Bagian 5 — Rekomendasi

Berdasarkan urutan risiko, bukan urutan kemudahan.

1. **Konfirmasi kemampuan REX (webcall dengan secret) sebelum menulis
   kode apa pun.** Kalau ini tidak ada, tesisnya mati dan lebih baik tahu
   sekarang.
2. **Tambahkan deteksi kegagalan terkorelasi** sebelum heartbeat pertama
   pernah berjalan di jaringan publik.
3. **Cabut authority dan burn LP** — dua TODO di `lib.rs` yang tanpa itu
   semua matematika di `curve.rs` tidak ada artinya.
4. Jalankan `rustc --test src/curve.rs` sendiri dan konfirmasi 21 test lolos.
5. Pelajari semantik reentrancy transaksi kondisional.
6. Audit lapisan vault terpisah dari lapisan kurva.
7. Audit eksternal + bug bounty sebelum mainnet.

---

## Ringkasan status

| Komponen | Status |
|---|---|
| Matematika kurva | ✅ Terverifikasi, 21 test, paritas persis dengan pump.fun |
| Sistem tier & fee | ✅ Terverifikasi |
| Bond & forfeit | ✅ Terverifikasi (lapisan kurva) |
| Model ancaman | ✅ Terdokumentasi, 7 celah terbuka teridentifikasi |
| Arsitektur workflow | ⚠️ Desain, mengikuti aturan Venus terverifikasi |
| Sintaks program Venus | ❌ Belum terverifikasi — samakan dengan contoh resmi |
| Sealed batch | ❌ Ketersediaan API belum dikonfirmasi |
| Integrasi SfS | ❌ Butuh ekonomi RLO yang belum ada |
| Audit keamanan | ❌ **Belum ada. Wajib sebelum mainnet.** |
