# Rexo v2 — Arsitektur

Ditulis ulang setelah mempelajari pump.fun, Raydium LaunchLab (yang
menjalankan letsbonk.fun), dan Meteora DBC.

---

# 1. Apa yang salah dengan yang kubangun

Aku mendesain dari nol dan mengulang kesalahan yang sudah dipecahkan orang
lain tiga generasi lalu. Empat kesalahan struktural:

**Konstanta, bukan konfigurasi.** Seluruh ekonomi Rexo dipaku di
`constants.rs` — kurva, fee, tier, bond. Untuk mengubah satu angka, kamu
harus deploy ulang program. LaunchLab dan DBC menyimpan semua itu di
**akun konfigurasi**. Satu program melayani banyak launchpad.

**Tidak ada lapisan partner.** letsbonk.fun bukan program. Ia cuma satu
`PlatformConfig` di atas LaunchLab milik Raydium. Arsitektur itu yang
membuat Raydium mendapat volume dari launchpad yang tidak mereka bangun.
Rexo tidak punya slot untuk itu sama sekali.

**Fee didorong, bukan ditarik.** Setiap `buy` di Rexo memindahkan fee ke
treasury dan creator vault — dua transfer tambahan per perdagangan. Semua
desain matang memakai `claim_creator_fee` / `claim_platform_fee`: fee
diakumulasi di pool, diklaim saat diminta. Lebih murah, dan menghilangkan
masalah akuntansi yang kutemukan sendiri (fee tersapu duluan sehingga
endowment SfS tidak punya sumber).

**Satu arah, satu mode.** Rexo cuma punya `buy(quote_in)` dan
`sell(tokens_in)`. LaunchLab punya empat: `buy_exact_in`, `buy_exact_out`,
`sell_exact_in`, `sell_exact_out`. Tanpa `exact_out`, kamu tidak bisa
bilang "aku mau tepat 1 juta token" — dan itu kebutuhan nyata.

---

# 2. Yang dilakukan desain yang sudah terbukti

| | pump.fun | LaunchLab | Meteora DBC | Rexo v1 |
|---|---|---|---|---|
| Kurva | konstanta global | `CurveParams` enum | **16 segmen** likuiditas | konstanta |
| Config | `Global` tunggal | `GlobalConfig` + `PlatformConfig` | `PoolConfig` per partner | tidak ada |
| Partner pihak ketiga | tidak | **ya** | **ya** | tidak |
| Instruksi dagang | 2 | **4** | 4 | 2 |
| Fee | push | **pull** | **pull** | push |
| Vault | 1 + ATA | base + quote terpisah | base + quote terpisah | 1 + ATA |
| Vesting | tidak | akun per penerima | `locked_vesting` | field di state |
| Target migrasi | PumpSwap | AMM atau CPSwap | DAMM v1 atau v2 | belum ada |
| Fee referral | tidak | `share_fee_rate` | ya, untuk swap host | tidak |
| Versi instruksi | — | `initialize_v2` | — | tidak |

### Pola yang paling penting: partner adalah data, bukan kode

Di DBC, seorang "partner" membuat satu `PoolConfig` berisi: quote mint,
fee claimer, penerima sisa, struktur fee (termasuk penjadwal fee dan rate
limiter), bentuk kurva, perilaku token, opsi migrasi, dan pengaturan pool
setelah lulus.

Lalu siapa pun bisa meluncurkan token memakai config key itu.

Artinya: **satu program, tak terbatas launchpad.** Masing-masing dengan
ekonomi sendiri, tanpa deploy apa pun. Itu yang kumaksud "upgradable"
sesungguhnya — bukan proxy pattern, tapi ruang parameter yang cukup lebar
sehingga kamu jarang perlu mengganti kode.

### Pemisahan peran yang tidak kumiliki

```
Protokol   pemilik program, ambil potongan dari semua
Partner    launchpad yang memakai program (letsbonk.fun)
Creator    orang yang meluncurkan token
Referrer   frontend/bot yang mengirim order
Trader     pembeli dan penjual
```

Rexo v1 cuma mengenal protokol dan creator. Dua peran yang hilang itu
persis yang menggerakkan distribusi: partner membawa audiens, referrer
membawa order flow.

---

# 3. Arsitektur Rexo v2

## 3.1 Model akun

```
LaunchConfig          dibuat partner. Kurva, fee, migrasi, aturan Rialo.
  └── Launch          PDA(base_mint, quote_mint, config) — satu token
        ├── base_vault     PDA(launch, base_mint)   token
        ├── quote_vault    PDA(launch, quote_mint)  RLO
        ├── FeeLedger      akumulasi fee, ditarik bukan didorong
        ├── VestingRecord  PDA(launch, beneficiary) — satu per penerima
        └── LivenessRecord PDA(launch) — riwayat denyut  ← khas Rialo
```

Perubahan dari v1 yang paling menentukan: **`Launch` diturunkan dari
config**, bukan berdiri sendiri. Mengganti config berarti mengganti
seluruh ekonomi peluncuran, tanpa menyentuh kode.

## 3.2 LaunchConfig — semua yang dulu jadi konstanta

```
quote_mint            aset quote (RLO, atau stablecoin)
fee_claimer           alamat partner
leftover_receiver     penerima sisa token

curve_type            0 konstan · 1 linear · 2 multi-segmen
curve_params          parameter sesuai tipe
total_base_sell       token yang dijual di kurva
migrate_threshold     ambang quote untuk lulus

fee_protocol_bps      potongan protokol
fee_partner_bps       potongan partner
fee_creator_bps       potongan kreator
fee_referral_bps      potongan pengirim order
collect_fee_mode      0 quote saja · 1 kedua sisi

migrate_target        0 pool internal · 1 DEX eksternal
migrate_fee_option    tier fee pool setelah lulus
lp_lock_bps           berapa LP dikunci, dan untuk siapa

vesting_schedule      cliff, durasi, porsi

── khas Rialo, tidak ada di desain mana pun ──
verify_mode           0 mati · 1 sosial · 2 sosial + identitas
heartbeat_interval    detik antar pemeriksaan
heartbeat_tolerance   kegagalan sebelum ditinggalkan
bond_schedule         bond minimum per tier
sealed_window_secs    0 mematikan lelang tersegel
sfs_routing_bps       porsi fee yang di-stake untuk membiayai automasi
```

Tiga puluh sekian parameter. Terdengar banyak, tapi itulah bedanya antara
launchpad yang bisa dipakai orang lain dan skrip yang cuma cocok untuk
satu kasus.

## 3.3 Instruksi

Dikelompokkan per peran, mengikuti DBC.

**Partner**
```
create_config              buat LaunchConfig
update_config              ubah yang boleh diubah
claim_partner_fee          tarik akumulasi
```

**Creator**
```
initialize                 luncurkan token dengan config key
initialize_v2              varian dengan opsi baru — jalur upgrade
claim_creator_fee
create_vesting_account
claim_vested_token
```

**Trading**
```
buy_exact_in     (quote_in,  min_base_out,  referrer)
buy_exact_out    (base_out,  max_quote_in,  referrer)
sell_exact_in    (base_in,   min_quote_out, referrer)
sell_exact_out   (quote_out, max_base_in,   referrer)
```

**Migrasi**
```
migrate                    ke target sesuai config
claim_locked_lp_fee        partner & creator klaim dari LP terkunci
```

**Khas Rialo**
```
heartbeat                  handler, dipicu AFTER
on_verification            handler, hasil webcall REX
settle_sealed_batch        handler, tutup lelang
try_abandon                hanguskan bond, buka kolam keluar
endow_service_stake        stake fee ke SfS
```

Perhatikan `initialize_v2`. Itu strategi upgrade LaunchLab: jangan ubah
instruksi lama, tambahkan versi baru. Klien lama tetap jalan, klien baru
dapat fitur baru. Jauh lebih aman daripada proxy yang bisa mengubah
perilaku di bawah kaki pengguna.

## 3.4 Fee: tarik, bukan dorong

```
setiap trade:  fee diakumulasi di FeeLedger   ← tidak ada transfer
kapan saja:    claim_partner_fee   → partner
               claim_creator_fee   → creator
               claim_protocol_fee  → treasury
               referral dibayar langsung saat trade
```

Ini menghapus dua transfer per perdagangan, dan menyelesaikan masalah yang
kutemukan di v1: fee tersapu duluan sehingga `fees_protocol` cuma
penghitung seumur hidup dan endowment SfS tidak punya sumber. Dengan
ledger, saldonya benar-benar ada saat dibutuhkan.

---

# 4. Lapisan Rialo — yang tidak bisa ditiru

Semua di atas adalah paritas. Ini bagian yang membuat Rexo bukan klon.

| Primitif | Fungsi | Kenapa mustahil di Solana |
|---|---|---|
| Reactive transactions | denyut, vesting berpredikat, auto-migrasi | butuh keeper per token selamanya |
| Webcall + REX | verifikasi sosial dengan API key tersegel | butuh oracle per token |
| Confidential execution | lelang tersegel anti-sniper | mempool publik |
| Stake-for-Service | token membiayai automasinya sendiri | tidak ada primitif setara |
| Omni Account | beli lintas chain tanpa bridge | — |

Yang penting: semuanya **opsional lewat config**. `verify_mode = 0`
mematikan verifikasi. `sealed_window_secs = 0` mematikan lelang. Partner
yang cuma mau pump.fun biasa bisa mendapatkannya; partner yang mau
verifikasi penuh juga bisa. Satu program.

Itu juga jawaban jujur untuk masalah v1: verifikasi REX belum jalan karena
bentuk statement webcall belum ketemu. Dengan `verify_mode`, itu berhenti
menjadi janji yang tidak bisa ditepati dan menjadi flag yang mati sampai
siap.

---

# 5. Kurva sebagai data

v1 memaku konstanta pump.fun. v2 menyimpannya sebagai parameter:

```
curve_type 0  Konstan     k = vq · vt          ← preset pump.fun
curve_type 1  Linear      harga naik linear
curve_type 2  Multi-segmen  sampai 16 titik    ← model DBC
```

Preset `pumpfun_like` tetap ada sebagai **nilai default config**, bukan
sebagai konstanta di kode. Matematika di `curve.rs` yang sudah teruji 21
test tetap dipakai untuk `curve_type = 0` — tidak ada yang dibuang.

Multi-segmen penting karena memungkinkan bentuk yang tidak bisa dilakukan
kurva tunggal: curam di awal untuk penemuan harga, mendatar di akhir supaya
pembeli terlambat tidak dihukum. DBC menyebutnya distribusi likuiditas.

---

# 6. Jalur dari v1 ke v2

Jangan tulis ulang dari nol lagi. Yang bisa dipertahankan:

| Modul v1 | Nasib |
|---|---|
| `curve.rs` | **dipertahankan**, jadi implementasi `curve_type = 0` |
| `guards.rs` | dipertahankan, argumen dari config bukan konstanta |
| `state.rs` | dipertahankan, jembatan u64/u128 tetap perlu |
| `vault.rs` | dipecah jadi base_vault + quote_vault |
| `token.rs` | dipertahankan, sudah benar setelah perbaikan terakhir |
| `constants.rs` | **dibongkar** jadi `LaunchConfig` |
| `ops.rs` | ditulis ulang mengikuti pengelompokan peran |
| `errors.rs` | dipertahankan, tambah error config |

Urutan pengerjaan, masing-masing bisa dideploy:

1. `LaunchConfig` + `create_config` + `initialize` yang membacanya.
   Ekonominya persis sama dengan sekarang, tapi datang dari akun.
2. Ganti fee push jadi `FeeLedger` + tiga instruksi klaim.
3. Tambah `buy_exact_out` dan `sell_exact_out`.
4. Pecah vault jadi base + quote.
5. Tambah `fee_partner_bps` dan `fee_referral_bps`. Di titik ini orang
   lain bisa membangun launchpad di atas Rexo.
6. Vesting sebagai akun terpisah.
7. Baru lapisan Rialo: heartbeat, verifikasi, lelang tersegel, SfS.

Langkah 1 sampai 5 membuat Rexo setara LaunchLab. Langkah 7 membuatnya
sesuatu yang tidak ada di chain lain. Urutannya sengaja begitu: paritas
dulu, keunikan belakangan. Membalik urutannya persis kesalahan yang
kulakukan di v1 — aku membangun fitur eksotis di atas fondasi yang bahkan
tidak bisa mencetak token.

---

# 7. Yang tetap harus diverifikasi

Tiga hal yang menentukan apakah lapisan Rialo bisa dibangun sama sekali:

1. **Bisakah REX menyimpan API key dan memanggil API eksternal?**
   Whitepaper privasi Rialo mendeskripsikannya. Belum terkonfirmasi ada di
   testnet. Kalau tidak ada, `verify_mode` selamanya 0 dan tesis produknya
   perlu diganti.
2. **Bentuk statement webcall Venus.** `AFTER n seconds CALL [handler]`
   sudah terbukti. `SEND`/`EVERY`/`ON` belum.
3. **Apakah SfS tersedia untuk program, bukan hanya untuk pengguna?**

Cari jawaban ketiganya sebelum menulis kode lapisan Rialo. Langkah 1–6
tidak bergantung pada satu pun dari ini.
