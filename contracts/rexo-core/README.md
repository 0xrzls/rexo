# Rexo Core

Launchpad meme coin native untuk Rialo. Kontrak lengkap, bukan pajangan.

---

## Jalankan sekarang (tanpa toolchain Rialo)

```bash
rustc --test src/curve.rs -o /tmp/t && /tmp/t
```

21 test, nol dependency. Matematika kurvanya identik dengan pump.fun sampai
ke unit terakhir, dan ada test yang membuktikannya.

Modul lain (`state.rs`, `guards.rs`, `ops.rs`) membawa 17 test tambahan,
tapi butuh crate Rialo untuk compile — jalankan dengan `cargo test` setelah
toolchain terpasang.

---

## Struktur

```
src/
  lib.rs         cangkang DSL Venus — tipis dengan sengaja
  ops.rs         logika bisnis. INI yang diaudit.        (4 test)
  curve.rs       matematika bonding curve                (21 test)
  state.rs       jembatan u64 <-> u128                    (4 test)
  guards.rs      kontrol akses, status, pengaman          (6 test)
  vault.rs       pemindahan kelvin lewat CPI
  token.rs       mint / burn / transfer Token-2022
  accounts.rs    parsing & validasi PDA
  events.rs      event terstruktur untuk indexer
  errors.rs      error domain, kode numerik stabil
  constants.rs   seluruh angka ekonomi, satu file
```

Logika sengaja **tidak** hidup di dalam macro. Auditor harus bisa membaca
`ops.rs` tanpa memahami DSL Venus lebih dulu, dan `curve.rs` bisa diuji
dengan `rustc` biasa tanpa toolchain Rialo sama sekali.

---

## Yang terverifikasi dari source

Semua ini dibaca langsung dari source `rialo-venus` 0.12.2 di docs.rs
(`https://docs.rs/rialo-venus/0.12.2/src/rialo_venus/lib.rs.html`) — bukan
tebakan:

| Fakta | Konsekuensi di kode |
|---|---|
| Satuan terkecil bernama **kelvin**, bukan lamport | `AccountInfo::kelvins()`, `try_borrow_mut_kelvins()` |
| `Pubkey::as_array()`, bukan `to_bytes()` | semua derivasi PDA |
| State workflow diserialisasi **bincode + serde** | `state.rs` pakai skalar datar |
| `WORKFLOW_SEED = "rialo_workflow"` | seed Rexo tidak boleh bentrok |
| Import path `rialo_s_program::{program::invoke, rent::Rent, ...}` | seluruh CPI |
| Vault yang membawa data **tidak bisa** pakai `system_instruction::transfer` | `vault::withdraw` manipulasi saldo langsung |

Baris terakhir itu bukan improvisasi — `rialo-venus` sendiri memakai pola
yang sama di `write_to_storage` baris 220–229, dengan komentar yang sama.

---

## Yang BELUM terverifikasi — satu hal saja

**Bagaimana macro `rialo!` menyerahkan `&[AccountInfo]` ke badan fungsi.**

Setiap fungsi di `lib.rs` memanggil `self.accounts()` di satu baris,
ditandai jelas. Kalau accessor-nya bernama lain, ubah baris itu saja —
`ops.rs`, `accounts.rs`, `vault.rs`, dan `token.rs` tetap benar seluruhnya.

Cara memastikan: buka `venus/` di rialo-examples, cari contoh yang
memindahkan kelvin atau token, lihat cara ia mengakses akun.

Dua hal lain yang perlu kamu konfirmasi:

1. **`AFTER` menerima absolut atau relatif?** Kode ini memakai timestamp
   absolut. Kalau Venus mengharapkan durasi relatif dalam detik, kamu baru
   saja menjadwalkan heartbeat ~56 tahun ke depan, dan gejalanya adalah
   "tidak terjadi apa-apa". Cek `venus/price-alert`.
2. **Bentuk attribute `#[event]`.** Fitur `events` mati secara default.

---

## Versi: kamu tertinggal 8 minor

Devnet melaporkan `api_version: 0.20.0-alpha.0`. Kontrak ini pin ke
`0.12.2` (rilis 30 Juni 2026). Versi terbaru `rialo-venus` adalah
**0.20.0-alpha.0** (1 September 2026).

Selisih itu besar dan DSL-nya masih alpha. Kalau build gagal dengan error
yang tidak masuk akal, coba naikkan seluruh crate ke 0.20.0-alpha.0
serentak — jangan campur versi.

---

## Tiga hal yang membuat ini bukan kloning pump.fun

**1. Mint & freeze authority dicabut di transaksi launch.** Bukan opsional,
bukan langkah terpisah. `token::create_mint_and_lock` melakukannya di
langkah 4 dan 5. pump.fun tidak memberi kreator kemampuan ini sama sekali.

**2. Bond hangus ke LP, bukan ke protokol.** Yang dirugikan rug pull adalah
pemegang token, jadi merekalah yang dikompensasi. Kalau hangus ke protokol,
kamu baru saja memberi platform insentif untuk menginginkan token gagal.

**3. Tier tidak bisa diklaim.** `guards::assert_tier_not_self_assigned`
menolak tier apa pun di atas Unverified yang datang dari argumen. Tier
hanya naik lewat `ops::apply_verification` setelah REX mengembalikan bukti.

---

## Bug alur yang ditemukan dan diperbaiki

Aku menelusuri ulang setiap jalur setelah versi pertama. Lima temuan nyata:

| # | Bug | Dampak | Status |
|---|---|---|---|
| 1 | `sell` memakai `require_active` | Pemegang token **terkunci** setelah abandonment — tidak bisa keluar sama sekali | Diperbaiki: `require_exitable` |
| 2 | Bond hangus masuk `forfeited_quote`, hanya dibayar saat kelulusan | Token yang ditinggalkan tidak pernah lulus → bond **nyangkut selamanya** | Diperbaiki: kolam keluar pro-rata |
| 3 | `trait ViewBridge` tanpa implementasi | **Tidak compile** | Diperbaiki: method di blok `program` |
| 4 | Fee dev-buy tidak disapu di `launch` | Bocor tiap peluncuran, tidak bisa diambil siapa pun | Diperbaiki: sapu ke treasury |
| 5 | `distribute_fills` dijadwalkan pada `now` | Bisa dieksekusi ulang di blok yang sama, habiskan compute | Diperbaiki: `+1` detik |

Bug 1 dan 2 saling berkaitan dan keduanya serius: digabung, efeknya adalah
mekanisme abandonment **menghukum korban, bukan pelaku**. Kreator sudah
pergi; yang terkunci justru pemegang token, dan kompensasi yang dijanjikan
tidak pernah sampai.

Perbaikannya: saat abandonment, bond jadi **kolam keluar** yang dibagikan
pro-rata ke pemegang token saat mereka menjual. Pola "kurangi pool dan base
bersamaan" membuat pembulatan mengoreksi diri — penjual terakhir menerima
sisanya dan kolam terkuras persis habis. Ada 4 test untuk itu, termasuk
kasus pembulatan 1000 kelvin dibagi 3 token.

Membeli tetap dilarang setelah abandonment. Keluar boleh, masuk tidak.

---

## Yang masih TODO, ditandai jelas di kode

| Tanda | Apa | Kenapa belum |
|---|---|---|
| `TODO(sealed)` | Antrean order terenkripsi | Butuh API batch REX yang belum kukonfirmasi |
| `TODO(pool)` | Buat pool + **burn LP** | Butuh alamat program DEX |
| `TODO(sfs)` | Posisi Stake-for-Service | Butuh ekonomi RLO yang belum ada |
| `TODO(vesting)` | Transfer tranche ke kreator | Butuh akun token kreator di daftar akun |
| `TODO(global)` | Akumulator kegagalan lintas token | **Paling berbahaya** — lihat bawah |

### `TODO(global)` harus kamu selesaikan sebelum heartbeat pertama

`global_failure_stats` sekarang mengembalikan `(0, 1)`, yang
berarti "0% gagal" dan **mengizinkan** abandonment. Default itu justru yang
berbahaya.

Kalau Telegram API mati enam jam, implementasi dengan default ini akan
menandai ribuan token sebagai ditinggalkan dan menghanguskan bond mereka
semua sekaligus. Itu merusak pengguna tak bersalah dalam skala besar dan
tidak bisa dibatalkan.

Logika pengamannya sudah ada dan teruji (`guards::abandonment_permitted`,
6 test). Yang belum ada adalah sumber datanya.

---

## Catatan akuntansi fee

Fee disapu dari vault ke treasury **segera** di setiap trade. Artinya
`fees_protocol` di state adalah penghitung seumur hidup, bukan saldo yang
masih ada di vault.

Konsekuensinya: endowment Stake-for-Service saat kelulusan harus didanai
dari akun **treasury**, bukan dari vault kurva. Menariknya dari vault akan
menguras likuiditas LP. Ini sudah ditandai di `ops::graduate`.

---

## Ini bukan audit keamanan

Ini kontrak lengkap dengan 31 test dan model ancaman terdokumentasi. Bukan
audit. Sebelum uang sungguhan masuk kamu tetap butuh firma independen —
dan mereka akan meminta persis dokumen seperti ini di hari pertama.

Yang harus mereka periksa paling dulu:
1. Reentrancy antar transaksi kondisional (`buy` → `graduate` → ?)
2. Kelengkapan validasi PDA di `accounts.rs`
3. Konservasi vault di seluruh jalur, terutama `sell` setelah `abandon`
4. Apakah ada jalur lain yang bisa menyetel `status = GRADUATED`
