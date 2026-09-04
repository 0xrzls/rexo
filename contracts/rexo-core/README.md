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

Semua ini dibaca langsung dari source `rialo-venus` 0.12.2 di docs.rs:

| Fakta | Konsekuensi di kode |
|---|---|
| Satuan terkecil bernama **kelvin**, bukan lamport | `AccountInfo::kelvins()`, `try_borrow_mut_kelvins()` |
| `Pubkey::as_array()`, bukan `to_bytes()` | semua derivasi PDA |
| State workflow diserialisasi **bincode + serde** | `state.rs` pakai skalar datar |
| `WORKFLOW_SEED = "rialo_workflow"` | seed Rexo tidak boleh bentrok |
| Import path `rialo_s_program::{program::invoke, rent::Rent, ...}` | seluruh CPI |
| Vault yang membawa data **tidak bisa** pakai `system_instruction::transfer` | `vault::withdraw` manipulasi saldo langsung |

---

## Tiga hal yang membuat ini bukan kloning pump.fun

1. **Mint & freeze authority dicabut di transaksi launch.** Bukan opsional, bukan langkah terpisah. `token::create_mint_and_lock` melakukannya di langkah 4 dan 5.
2. **Bond hangus ke LP, bukan ke protokol.** Yang dirugikan rug pull adalah pemegang token, jadi merekalah yang dikompensasi.
3. **Tier tidak bisa diklaim.** `guards::assert_tier_not_self_assigned` menolak tier apa pun di atas Unverified yang datang dari argumen. Tier hanya naik lewat `ops::apply_verification`.
