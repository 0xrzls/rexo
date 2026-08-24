# Coldstart

Launchpad meme coin untuk **Rialo** — bukan port pump.fun, tapi desain
yang hanya bisa ada di Rialo.

---

## Mulai dari sini (5 menit, tanpa toolchain Rialo)

```bash
rustc --test src/curve.rs -o /tmp/curve_test && /tmp/curve_test
```

21 test. Nol dependency. Matematika kurvanya identik dengan pump.fun
sampai ke unit terakhir — dan ada test yang membuktikannya.

---

## Tesisnya

Studi SSRN atas **832.941 peluncuran pump.fun** (Mei–Jun 2026) menemukan
tingkat kelulusan **0,198%**. Tapi dua faktor mengubahnya drastis:

| Faktor | Efek |
|---|---|
| Punya kanal Telegram | lulus **1,485%** vs 0,166% → **8,94x** |
| Punya ketiga kanal sosial | lulus **1,919%** vs 0,110% → **17,4x** |
| Self-buy kreator di atas default | hazard ratio **4,51** |

Dua sinyal terkuat itu, di pump.fun, **cuma klaim yang tidak
diverifikasi.** Siapa pun bisa menempelkan link Telegram mati.

Bukan karena pump.fun malas. **Karena Solana tidak bisa memanggil API dan
tidak bisa menyimpan secret.**

Rialo bisa keduanya, secara native.

> **Coldstart adalah launchpad di mana dua hal yang benar-benar
> memprediksi kelangsungan hidup ditegakkan oleh chain, bukan diklaim
> kreator.**

---

## Isi repo

| File | Isi | Kepastian |
|---|---|---|
| `01-RESEARCH.md` | Status Rialo Agt 2026, primitif, bedah pump.fun, data kegagalan | Bersumber |
| `02-ARCHITECTURE.md` | Tujuh mekanisme, sistem tier, mesin state | Desain |
| `03-DEPLOYMENT.md` | Setup, deploy, operasi, checklist pra-mainnet | CLI terverifikasi |
| `04-AUDIT.md` | Laporan verifikasi, invariant, model ancaman | **Baca sebelum apa pun** |
| `src/curve.rs` | Mesin bonding curve | ✅ Teruji, jalan hari ini |
| `src/lib.rs` | Program Venus | ⚠️ Kerangka desain, tidak compile |

---

## Tujuh mekanisme

| # | Mekanisme | Primitif Rialo | Menggantikan |
|---|---|---|---|
| 1 | Verified Social Floor | Edge + REX | Link sosial yang tak dicek siapa pun |
| 2 | Liveness Heartbeat | `EVERY` | Rug pull yang gratis |
| 3 | Sealed Launch Window | REX confidential | Sniper first-block |
| 4 | Reactive Vesting | Predicates | Dev dump instan |
| 5 | Self-Funding Token | Stake-for-Service | Bot keeper yang kehabisan saldo |
| 6 | Gasless Trading | SfS bulk sponsorship | Wajib punya SOL untuk gas |
| 7 | Cross-Chain Buy | Omni Account + Interop | Bridging |

Detail dan alasan tiap pilihan ada di `02-ARCHITECTURE.md`.

---

## Yang sengaja tidak diubah

Matematika kurvanya identik dengan pump.fun. Kurvanya bukan bagian yang
rusak — **komposisi peluncurannya yang rusak**, dan studi SSRN
mengatakan itu secara eksplisit. Menjaga kurva tetap sama berarti
progress bar 43% berarti hal yang sama di kedua platform, dan setiap
inovasi diletakkan di lapisan yang memang butuh.

Total fee juga selalu 100 bps di semua tier. **Pembeli membayar sama di
mana pun.** Tier adalah sinyal untuk kreator, bukan pajak untuk pembeli.

---

## Tiga hal jujur

**1. Rialo belum mainnet.** Testnet publik live sejak ~9 Mei 2026;
mainnet ditarget 2026 tanpa tanggal. Token RLO belum ada. Kamu tidak bisa
launch produksi sekarang — tapi Rialo menargetkan "10 dApp native di hari
pertama mainnet", dan slot itu sedikit.

**2. `src/lib.rs` tidak compile.** `docs.rialo.io` memblokir akses
otomatis, jadi sintaks Venus yang sebenarnya tidak bisa kuverifikasi.
Ambil `https://docs.rialo.io/user/latest/llms-full.txt` ke editor kamu —
itu seluruh dokumentasi dalam satu file.

**3. Ini bukan audit keamanan.** Ini laporan verifikasi: matematika yang
teruji, invariant yang ditegakkan, dan model ancaman dengan **7 celah
terbuka yang kutulis eksplisit** di `04-AUDIT.md`. Kamu tetap butuh audit
eksternal sebelum uang sungguhan masuk.

---

## Langkah berikutnya, sesuai urutan risiko

1. Jalankan test kurvanya. Pahami ekonominya.
2. **Konfirmasi di Discord Rialo: bisakah REX menyimpan API key dan
   memanggil API eksternal dengannya?** Kalau tidak, tesisnya mati dan
   lebih baik tahu sekarang daripada setelah tiga bulan.
3. Deploy `venus/http-fetch` apa adanya sampai sukses.
4. Ganti isinya dengan buy/sell kurva. Itu sudah launchpad yang jalan.
5. Bangun mekanisme 1 (verifikasi sosial). Itu pembeda intinya.

Jangan bangun tujuh mekanisme sebelum satu pun jalan di testnet.
