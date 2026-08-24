# 02 — Arsitektur Coldstart

> **Coldstart** — nama dari masalah yang dipecahkan bonding curve.
> Dokumen resmi pump.fun sendiri menyebutnya: *"Pump is a token launch
> platform that solves the cold-start problem."* Dan cocok dengan
> Subzero/Rialo yang memakai metafora fisika suhu rendah.

---

## Tesis dalam satu paragraf

Dua sinyal secara empiris memprediksi apakah sebuah meme coin bertahan:
kehadiran sosial yang nyata (lift 17,4x) dan skin-in-the-game kreator
(HR 4,51). Di pump.fun keduanya cuma klaim tak terverifikasi, karena
Solana tidak bisa memanggil API dan tidak bisa menyimpan secret. Rialo
bisa keduanya, secara native. **Coldstart adalah launchpad di mana dua
hal yang benar-benar memprediksi kelangsungan hidup ditegakkan oleh
chain, bukan diklaim kreator.**

Itu bukan "pump.fun tapi lebih cepat". Itu kategori produk yang tidak
bisa ada di tempat lain.

---

## Yang sengaja TIDAK diubah

Ini sama pentingnya dengan yang diubah.

**Matematika kurva identik dengan pump.fun.** Konstanta yang sama,
rumus yang sama, ambang kelulusan yang sama. Alasannya:

1. Kurvanya bukan bagian yang rusak. Ia elegan dan sudah teruji miliaran
   dolar volume.
2. **Komparabilitas adalah fitur.** Progress bar 43% berarti hal yang
   sama persis di Coldstart dan di pump.fun. Trader tidak perlu belajar
   model baru, dan analis bisa membandingkan langsung.
3. Setiap perubahan pada kurva adalah permukaan serangan baru yang harus
   diaudit ulang. Simpan inovasimu untuk lapisan yang memang butuh.

**Total fee selalu 100 bps di semua tier.** Pembeli membayar harga yang
sama persis di mana pun. Tier adalah sinyal untuk kreator, bukan pajak
untuk pembeli. Ada test yang menegakkannya:
`buyer_pays_identical_price_across_all_tiers`.

---

## Tujuh mekanisme

Untuk tiap mekanisme: apa yang digantikan, dan kenapa mustahil di tempat lain.

### 1. Verified Social Floor

**Primitif:** Rialo Edge (webcall ter-attest) + REX (API key tersegel)

Saat launch, kreator menyerahkan handle Telegram dan X. Program melakukan
webcall ter-attest validator ke Telegram Bot API dan X API, memakai
kredensial yang hidup **di dalam enclave REX**. Yang keluar hanya angka:
jumlah anggota, umur akun, lolos/tidak. API key dan respons mentahnya
tidak pernah muncul on-chain.

Verdict itu menetapkan tier. **Kreator tidak pernah mengoper tier-nya
sendiri.** Kalau tier bisa diklaim, seluruh desain ini tidak ada artinya.

**Mustahil di Solana:** butuh oracle dengan API key untuk setiap token.
Dengan 20.000 peluncuran/hari, oracle itu jadi bisnis dengan insentifnya
sendiri — compound marginalization yang diserang tesis Rialo.

### 2. Liveness Heartbeat

**Primitif:** Conditional Transactions (`EVERY`)

Verifikasi sekali saat launch bisa dicurangi: sewa 100 anggota Telegram
selama satu jam. Jadi Coldstart mengecek ulang setiap 6 jam, selamanya,
lewat `EVERY`.

Empat kegagalan berturut-turut (~24 jam) → token ditandai **abandoned**
on-chain. Bond hangus. Token kreator yang belum vested di-burn.

**Ini membalik ekonomi rug pull.** Di pump.fun, meninggalkan token itu
gratis. Di Coldstart, meninggalkannya punya harga yang otomatis
ditagihkan.

**Mustahil di Solana:** butuh keeper yang memantau setiap token
selamanya. Itu bukan skrip, itu perusahaan.

### 3. Sealed Launch Window

**Primitif:** REX confidential execution

90 detik pertama adalah lelang batch tersegel. Order dienkripsi dengan
public key REX — tidak ada yang melihat ukuran, pengirim, atau urutan.
Termasuk validator.

Saat jendela tutup, REX mendekripsi seluruh batch, menghitung **satu
clearing price**, dan mengisi semua order pada harga itu secara pro-rata.

Menjadi orang pertama tidak memberi keuntungan apa pun, **karena di
dalam jendela tidak ada "pertama"**.

Jendela sealed yang sama dibuka lagi saat kelulusan — momen paling
berbahaya di pump.fun, di mana sniper front-run pembuatan LP lalu
menjual ke gelombang pembeli pertama.

**Mustahil di Solana:** mempool publik. Bahkan dengan Jito, isi order
terlihat oleh block builder.

### 4. Reactive Vesting Vault

**Primitif:** Predicates (transaksi kondisional)

Self-buy kreator adalah prediktor kuantitatif terkuat (HR 4,51). Tapi di
pump.fun dev bisa dump di blok berikutnya, jadi sinyalnya kosong.

Di Coldstart alokasi kreator terkunci dan terbuka lewat **predikat, bukan
timer**:

| Tranche | Predikat |
|---|---|
| 1 | progress kurva ≥ 25% |
| 2 | kurva lulus |
| 3 | 30 hari pasca-lulus **DAN** harga ≥ harga kelulusan |

Kalau heartbeat gagal, predikat lain menang: yang belum terbuka di-burn.

**Mustahil di Solana:** unlock berbasis kondisi butuh keeper per token.
Vesting berbasis waktu bisa (Solana punya clock), tapi "harga ≥ harga
kelulusan" tidak — itu butuh evaluasi state kontinu.

### 5. Self-Funding Token

**Primitif:** Stake-for-Service

Ini yang paling tidak bisa ditiru.

Saat kelulusan, separuh fee protokol tidak disapu ke treasury. Ia
di-stake ke posisi SfS, dan **yield-nya** dirutekan ke ServicePaymaster,
yang mencetak service credit untuk membiayai automasi token ini:
heartbeat, snapshot harga, manajemen LP.

Selamanya. Tanpa top-up. Tanpa bot yang kehabisan saldo.

Whitepaper SfS Rialo menyebut pola ini persis: *"self-maintaining
protocols — protokol bisa membiayai infrastruktur mereka sendiri
selamanya dengan men-stake sebagian treasury dan merutekan yield-nya ke
upkeep."*

**Mustahil di mana pun:** ini primitif tingkat protokol yang unik untuk
Rialo. Tidak ada chain lain yang mengubah yield staking jadi kredit
layanan.

### 6. Gasless Trading

**Primitif:** Stake-for-Service (bulk sponsorship) + Rialo Cruise

Platform men-stake RLO dan merutekan yield-nya untuk mensponsori gas
seluruh trader. Pengguna baru bisa trading tanpa pernah memegang RLO.

Rialo menyebut ini eksplisit sebagai use case: *"sebuah exchange bisa
men-stake RLO dan merutekan yield-nya untuk menutup gas sepuluh ribu
trader harian. Biayanya bisa diprediksi dan dibatasi oleh routing
fraction."*

Bandingkan: pump.fun butuh SOL untuk gas plus priority fee. Saat
kongesti, ini menghilangkan trader kecil sepenuhnya.

### 7. Cross-Chain Native Buy

**Primitif:** Rialo Omni Account + Interop

Pengguna Solana atau Ethereum membeli tanpa bridging. Satu akun,
interop native yang diklaim >10x lebih cepat dari bridge teratas.

Masalah distribusi sebenarnya sebuah launchpad baru adalah likuiditasnya
ada di tempat lain. Ini menjawabnya di lapisan protokol.

---

## Sistem tier

| | Unverified | Verified | Committed |
|---|---|---|---|
| Syarat | — | sosial live terbukti | sosial + bond + vesting |
| Bond minimum | 0 | 2 RLO | 10 RLO |
| Bagi fee kreator | **0 bps** | 25 bps | 50 bps |
| Bagi fee protokol | 100 bps | 75 bps | 50 bps |
| Batas alokasi kreator | 1% | 3% | 5% |
| Vesting reaktif | tidak | opsional | **wajib** |
| **Total fee pembeli** | **100 bps** | **100 bps** | **100 bps** |

Peluncuran unverified **tetap diizinkan**. Ini penting: memblokirnya
akan mengubah Coldstart jadi gatekeeper dan mematikan kultur meme. Yang
terjadi adalah kreatornya tidak dapat apa pun dari fee, dan UI tidak
bisa menyembunyikan labelnya.

### Bond: pengganti perlombaan ke dasar

pump.fun mengenakan ~0,02 SOL untuk membuat token. Hasilnya 800.000+
peluncuran/bulan dan kelulusan 0,198%.

Coldstart mengenakan **bond yang bisa dikembalikan**:
- Lulus → bond kembali penuh ke kreator
- Ditinggalkan → bond hangus **ke LP**, bukan ke protokol

Yang terakhir itu penting secara moral dan praktis. Yang dirugikan
rug pull adalah pemegang token, jadi merekalah yang dikompensasi. Kalau
bond hangus ke protokol, kamu baru saja memberi platform insentif untuk
menginginkan token gagal.

---

## Mesin state

```
                    launch(bond, socials)
                             │
                             ▼
                    ┌────────────────┐
                    │  SEALED (90s)  │◄──── order terenkripsi diantre
                    └────────┬───────┘
                             │  AFTER 90s (reactive)
                             ▼
              REX: dekripsi batch → satu clearing price
                             │
                             ▼
                    ┌────────────────┐
        ┌──────────►│    TRADING     │◄──────────┐
        │           └────────┬───────┘           │
        │                    │                   │
   EVERY 6h              predikat:          predikat:
   heartbeat          real_token == 0     progress >= 25%
        │                    │                   │
        ▼                    ▼                   ▼
  4x gagal?            GRADUATING          tranche kreator
        │                    │
        ▼                    ├─► LP dibuat + di-burn
   ABANDONED                 ├─► sealed window dibuka lagi
   bond → LP                 ├─► bond kembali ke kreator
   token dev → burn          ├─► fee/2 → posisi SfS
        │                    ▼
        └──────────────►  FINALIZED
```

Perhatikan: **tidak ada satu pun panah di diagram ini yang dipicu bot.**
Semuanya predikat yang dievaluasi validator di akhir blok.

---

## Batasan Venus DSL yang membentuk desain ini

Statement async — `AFTER`, `EVERY`, `ON`, `SEND`, `START` — **dilarang**
di dalam `for`/`while`/`loop`, closure, dan async block.

Itu bukan gangguan kecil. Itu mengubah cara kamu menulis distribusi
batch. Pola yang diwajibkan: simpan indeks di workflow state, handler
menaikkan indeks lalu menerbitkan ulang operasinya.

Lihat `distribute_fills()` di `src/lib.rs` — itu bukan loop, itu
rekursi lewat state cursor. Kalau kamu menulisnya sebagai `for`, program
tidak akan compile.

Fungsi di blok `rex {}` tidak menerima workflow state. Setiap input
harus parameter eksplisit. Ini kenapa `verify_socials` mengambil handle
sebagai argumen alih-alih membacanya dari state.

---

## Yang paling mungkin gagal

Jujur soal ini lebih berguna daripada daftar fitur.

**1. Sealed batch mungkin belum ada API-nya.** Whitepaper privasi Rialo
mendeskripsikan lelang tersegel secara konseptual. Apakah REX sudah
mengekspos primitif batch decryption di testnet Agustus 2026 — belum
kuverifikasi. Kalau belum, mekanisme 3 mundur ke commit-reveal biasa,
yang lebih lemah tapi tetap jalan.

**2. Rate limit API sosial.** Telegram dan X membatasi request. Dengan
ribuan token yang heartbeat tiap 6 jam, kamu akan kena limit. Mitigasi:
heartbeat adaptif (token besar lebih sering), dan caching di dalam REX.

**3. Verifikasi sosial bisa dicurangi.** Anggota Telegram bisa dibeli.
Yang dilawan desain ini bukan penipu yang gigih — tapi 99% peluncuran
yang bahkan tidak repot memasang link. Studi SSRN menunjukkan sekadar
*punya* Telegram sungguhan memberi lift 8,94x. Menaikkan lantai dari
"tidak ada" ke "harus nyata dan tetap hidup" sudah menggeser distribusi.

**4. Ekonomi RLO belum ada.** Semua angka bond dan fee di sini pakai RLO
hipotetis. Kalibrasi ulang setelah tokenomics diumumkan.
