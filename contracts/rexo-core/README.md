# Rexo v2

Launchpad token untuk Rialo. Ditulis ulang setelah mempelajari pump.fun,
Raydium LaunchLab (yang menjalankan letsbonk.fun), dan Meteora DBC.

**49 test, semuanya nyata.**

```
config.rs   11    ekonomi sebagai data
curve.rs    20    matematika AMM
state.rs     8    snapshot, lifecycle, vesting
fees.rs      7    pembagian & ledger
ops.rs       3    invariant vault lintas perdagangan
```

`rustc --test src/curve.rs -o /tmp/t && /tmp/t` jalan tanpa toolchain Rialo.
Sisanya lewat `cargo test`.

---

## Perubahan struktural dari v1

| | v1 | v2 |
|---|---|---|
| Ekonomi | dipaku di `constants.rs` | `LaunchConfig`, akun |
| Partner pihak ketiga | tidak ada | fee share + config sendiri |
| Fee | didorong tiap trade | **ditarik** lewat `claim_*` |
| Instruksi dagang | 2 | **4** (`exact_in` + `exact_out`) |
| Vault | 1 + ATA | `base_vault` + `quote_vault` |
| Referral | tidak ada | `referral_bps`, dibayar seketika |
| Lifecycle | 6 status, 2 tak pernah muncul | 3 (funding/migrating/migrated) |

Yang paling menentukan: **`Launch` menyimpan salinan parameter config**.
Partner boleh memperbarui config kapan saja; peluncuran yang sudah jalan
tidak ikut berubah. Tanpa ini harga historis tidak bisa direproduksi.

---

## Satu keunggulan Rialo yang dipakai hari ini

Saat kurva habis, `AFTER 1 seconds CALL [migrate]` menjalankan migrasi
sebagai transaksi terpisah. **Tanpa keeper.**

Meteora butuh `dbc-keeper` untuk langkah ini. LaunchLab butuh seseorang
memanggil `migrate_to_amm`. Di sini chain yang melakukannya, dan sintaksnya
sudah terbukti compile.

---

## Yang SENGAJA tidak ada

Verifikasi sosial REX, lelang tersegel anti-sniper, dan denyut liveness
membutuhkan primitif Rialo yang bentuk sintaksisnya belum terkonfirmasi
(`SEND` webcall, eksekusi konfidensial).

**Tidak ada stub. Tidak ada timer kosong. Tidak ada field config yang
menjanjikannya.** Itu kesalahan v1: menjual tier yang tidak bisa
diverifikasi, dan menjalankan heartbeat yang tidak memeriksa apa pun.

Ditambahkan ketika primitifnya terbukti, bukan sebelumnya.

---

## Satu TODO yang jujur

`accounts::read_config` mengembalikan preset, belum mem-parse akun config.
Layout serialisasi untuk akun non-workflow di Venus belum terkonfirmasi.
Memakai preset lebih jujur daripada mem-parse byte dengan asumsi yang
belum diverifikasi — tapi sampai itu selesai, satu program masih melayani
satu ekonomi.

Arsitekturnya sudah siap; yang kurang tinggal pembacaan akunnya.

---

## Aliran dana

**initialize**
```
payer → quote_vault    pembelian kreator (kalau ada)
mint  → base_vault     seluruh supply, lalu authority DICABUT
```

**buy**
```
pembeli    → quote_vault   quote_spent (net + fee)
quote_vault → referrer     bagian referral, kalau ada
base_vault  → pembeli      token
                           fee MENUMPUK, tidak dipindah
```

**sell**
```
penjual     → base_vault   token   (masuk dulu)
quote_vault → penjual      hasil
quote_vault → referrer     bagian referral
```

**claim_\***
```
quote_vault → penerima     saldo ledger, dikosongkan
```

**migrate** (dipicu chain)
```
base_vault  → lp_base_dest    lp_reserve
quote_vault → lp_quote_dest   real_quote saja
                              fee yang belum ditarik TETAP di vault
```

Baris terakhir itu penting: menarik fee ke pool akan mencuri hak partner
dan kreator. Ada test untuk itu.
