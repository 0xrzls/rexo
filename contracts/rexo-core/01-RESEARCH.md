# 01 — Riset

Semua yang di bawah punya sumber. Kalau tidak ada sumbernya, ditandai
sebagai belum terverifikasi.

---

## Bagian 1 — Status Rialo per Agustus 2026

### Fakta keras

| | |
|---|---|
| Entitas | Subzero Labs, didirikan Ade Adepoju (CEO) & Lu Zhang (CTO), keduanya eks-Mysten Labs (Sui) |
| Pendanaan | $20M seed, dipimpin Pantera Capital; Coinbase Ventures, Variant, Hashed, Susquehanna, Mysten Labs, Flowdesk, Mirana, Edge Capital |
| DevNet privat | Agustus 2025 |
| **Testnet publik** | **~9–10 Mei 2026** — sudah live, bisa klaim test token |
| Mainnet | **Target 2026, tanggal belum diumumkan** |
| Token RLO | **BELUM ADA.** Belum ada ticker, tokenomics, atau distribusi resmi |
| Partner | Nasdaq, CBOE, NYSE, Predicate, DoubleZero, M0, Keplr (wallet) |
| Roadmap | 10 dApp native di hari pertama mainnet, 30+ komitmen partnership akhir tahun |

### Apa artinya buat kamu

**Kamu tidak bisa launch di mainnet sekarang.** Yang bisa kamu lakukan
sekarang justru posisi terbaik: Rialo secara eksplisit menargetkan
"10 dApp native di hari pertama mainnet". Jumlah slot itu kecil. Sebuah
launchpad yang sudah jalan di testnet, dengan matematika teruji dan
desain yang benar-benar memakai primitif Rialo, adalah kandidat kuat.

Peringatan yang jujur: token RLO belum ada, jadi semua ekonomi di
dokumen ini pakai unit RLO hipotetis. Kalau desimal atau supply RLO
ternyata beda dari asumsi (9 desimal), parameter kurva harus dikalibrasi
ulang. Itu satu perubahan konstanta, bukan perubahan arsitektur.

> ⚠️ Semua angka di atas berasal dari sumber sekunder dan blog resmi
> Rialo. Cek `discord.gg/RialoProtocol` untuk status terbaru sebelum
> mengambil keputusan besar — status testnet/mainnet bisa berubah cepat.

---

## Bagian 2 — Primitif Rialo, dan mana yang berguna untuk launchpad

Diambil dari Dev Portal resmi (`rialo.io/for-devs`) dan whitepaper.

| Primitif | Klaim resmi | Nilai untuk launchpad |
|---|---|---|
| **Rialo Execution Engine** | Conditional Transactions, event-driven | ⭐ Auto-graduation, vesting reaktif, tanpa keeper |
| **Rialo Edge** | Web2 dua arah tanpa middleware, >100k webcall konkuren | ⭐ Verifikasi sosial on-chain per token |
| **REX** | Confidential computing (TEE), runtime privasi | ⭐ Sealed launch, API key tersegel |
| **Stake-for-Service** | Yield staking → service credit | ⭐ Token yang membiayai automasinya sendiri |
| **Rialo Cruise** | Transaksi gasless native | ⭐ Trading tanpa perlu punya RLO |
| **Rialo Workflow** | Automasi native, workflow kompleks | ⭐ Lifecycle token sebagai satu program |
| **Rialo Stream** | Data feed native, >40x lebih cepat dari oracle | Harga & chart tanpa indexer |
| **Rialo Interop + Omni Account** | Satu akun multi-network, >10x lebih cepat dari bridge | Beli dari Solana/ETH tanpa bridging |
| **Rialo IPC** | Identity, Privacy, Compliance kelas satu | Anti-sybil, gating yurisdiksi |
| **Rialo Consensus** | Multi-concurrent proposer, block time 50ms | Latensi trading |
| **Rialo VM** | RISC-V, kompatibel SVM | Migrasi program Solana |

### Reactive Transactions — mekanisme intinya

Dari whitepaper resmi (20 April 2026), ini cara kerjanya:

1. Developer mendeploy transaksi yang mendefinisikan **predikat** —
   ekspresi logis yang menentukan kapan sebuah transaksi layak dieksekusi.
2. Predikat disimpan on-chain dan dievaluasi terus-menerus selama
   eksekusi blok.
3. Di akhir eksekusi blok, Rialo mengevaluasi semua predikat yang
   dependensinya mungkin berubah. Evaluasinya deterministik — semua
   validator sampai pada kesimpulan yang sama.
4. Predikat yang menjadi true ditandai triggered; transaksi terkaitnya
   otomatis diantrekan.
5. Konsensus menjamin eksekusinya.

Predikat bisa merujuk: state on-chain, transisi state program lain di
blok yang sama, event dari transaksi sebelumnya, data oracle ter-attest
validator, kondisi berbasis waktu, dan hasil langkah sebelumnya dalam
sebuah workflow.

**Yang penting**: transaksi kondisional bisa membuat transaksi
kondisional lain. Artinya sebuah workflow bisa berjalan berhari-hari,
menunggu, bereaksi ke hasil antara, lalu lanjut — tanpa satu pun sistem
eksternal yang mendorongnya.

Itu bukan "cron on-chain". Itu perbedaan kategori.

### Stake-for-Service — kenapa ini yang paling penting

Dari whitepaper SfS (26 Desember 2025):

- Pengguna membuat posisi SfS dengan **routing fraction** — persentase
  yield staking masa depan yang diarahkan ke pembayaran layanan.
- Yield itu tidak lewat akun pengguna. Ia dirutekan ke **ServicePaymaster
  (SPM)**, yang mencetak service credit dengan backing 1:1 terhadap RLO.
- Credit dipakai untuk gas, storage, atau eksekusi terjadwal.
- Credit tidak bisa ditransfer di luar konteks layanan dan tidak bisa
  dispekulasikan.

Rialo secara eksplisit menyebut dua use case yang langsung relevan:

> **Self-maintaining protocols** — protokol bisa membiayai infrastruktur
> mereka sendiri selamanya dengan men-stake sebagian treasury dan
> merutekan yield-nya ke upkeep. Menghilangkan kebutuhan penggalangan
> dana berkala.

> **Bulk sponsorship** — sebuah exchange bisa men-stake RLO dan
> merutekan yield-nya untuk menutup gas sepuluh ribu trader harian.
> Biayanya bisa diprediksi dan dibatasi oleh routing fraction.

Ini bukan fitur pinggiran. Ini fondasi ekonomi untuk launchpad yang
tidak perlu mengekstraksi nilai dari penggunanya untuk tetap hidup.

### Privasi / REX — untuk sealed launch

Dari whitepaper privasi (29 Januari 2026):

> Lapisan komputasi privat memungkinkan pengguna mengenkripsi informasi
> order yang sensitif dan merutekan transaksi ke lingkungan eksekusi
> terlindungi untuk dieksekusi dan diselesaikan. Eksekusinya tersembunyi
> dari seluruh jaringan, hanya hasilnya yang tercermin on-chain.

Dan untuk verifikasi sosial:

> Aplikasi bisa menyimpan dan menggunakan kredensial autentikasi, seperti
> API key, untuk mengambil data dari sistem nyata tanpa pernah
> mengekspos kredensialnya atau membocorkan data yang diambil.

Rialo bahkan memberi contoh yang hampir identik dengan yang kita
butuhkan: sebuah perusahaan menjalankan tantangan Instagram, menyimpan
data partisipasi di database privat, dan menyediakan API key yang
dipakai node untuk mengambil data dan mengevaluasinya secara rahasia
di dalam REX. Payout publik; data mentah, kredensial, dan jejak
eksekusinya tetap privat.

---

## Bagian 3 — pump.fun, dibedah sampai konstanta

Sumber: `pump-fun/pump-public-docs` (IDL + README resmi), via DeepWiki.

### Tiga program

| Program | Alamat | Fungsi |
|---|---|---|
| Pump | `6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P` | Pembuatan token + trading kurva |
| PumpSwap | `pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA` | AMM constant-product pasca-lulus |
| Fee | `pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ` | Tier fee dinamis berbasis market cap |

### Sistem reserve ganda

Ini inovasi intinya. Reserve **virtual** menentukan harga; reserve
**nyata** melacak kepemilikan aktual. `real_token_reserves` mulai lebih
kecil dari `virtual_token_reserves`, menciptakan "gap" yang membuat
kurva tetap likuid bahkan setelah token nyatanya habis.

**Konstanta dari `Global` account:**

```
initial_virtual_token_reserves = 1_073_000_000_000_000
initial_virtual_sol_reserves   =        30_000_000_000
initial_real_token_reserves    =   793_100_000_000_000
token_total_supply             = 1_000_000_000_000_000
gap                            =   279_900_000_000_000
```

**Rumus:**
```
virtual_token_reserves * virtual_sol_reserves = k

beli:  new_vt = vt - token_amount
       sol_cost = k / new_vt - vs

jual:  new_vt = vt + token_amount
       sol_out = vs - k / new_vt

mcap = virtual_sol_reserves * token_total_supply / virtual_token_reserves
```

**Selesai** ketika `real_token_reserves == 0`. Flag `complete` tidak
pernah kembali false — menjual tidak bisa "membatalkan" kelulusan.
Migrasi bersifat permissionless dan idempoten.

`src/curve.rs` di repo ini mereplikasi seluruh angka itu persis, dan ada
test `constants_match_pumpfun_global_account` yang menegakkannya.

### Struktur biaya (per 2026)

- Pembuatan token: ~0.02 SOL
- Fase kurva: 1% setiap beli dan jual
- Pasca-lulus di PumpSwap: 0.25%
- Migrasi: **gratis** sejak PumpSwap (dulu ~6 SOL ke Raydium)
- Fee dinamis: tier berdasarkan market cap, di-query lewat CPI ke Fee Program
- Creator fee: masuk ke `creator_vault` PDA, bisa dibagi ke maksimal 10 shareholder
- Januari 2026: model fee dinamis di mana trader bisa memengaruhi creator fee

### Yang TIDAK bisa dikontrol kreator di pump.fun

Ini penting karena jadi celah diferensiasi:

- Tidak bisa burn atau lock LP token
- Tidak bisa mencabut mint/freeze authority sendiri
- Tidak bisa mengubah supply, desimal, atau fitur
- Kontrol kreator: minimal

---

## Bagian 4 — Data kegagalan. Ini bagian terpenting.

### Angka utamanya

**Studi SSRN, Juli 2026** — analisis survival Kaplan-Meier dan Cox
proportional-hazards atas **832.941 peluncuran pump.fun**, diamati
kontinu 8 Mei – 10 Juni 2026:

| Temuan | Angka |
|---|---|
| Tingkat kelulusan gabungan | **0,198%** (CI 95% [0,189%, 0,208%]) |
| Steady-state | 0,207% |
| Penurunan dari Sep–Okt 2025 (0,63%) | **3,18x** |
| Punya kanal Telegram | **1,485%** vs 0,166% tanpa → **lift 8,94x** |
| Cox hazard ratio Telegram | **5,40** (CI [4,73, 6,17], p = 6.6e-138) |
| Punya ketiga kanal sosial | **1,919%** vs 0,110% → **lift 17,4x** |
| Initial mcap > 30 SOL (proksi self-buy kreator) | HR **4,51** |
| Kuartil teratas (mcap > 31 SOL) | lulus **0,634%** |

Dan kesimpulan penulisnya, yang menentukan seluruh desain Coldstart:

> Penurunan lintas-rezim sebagian besar disebabkan oleh **pergeseran
> komposisi peluncuran ke arah token tanpa self-buy**, bukan oleh
> perubahan lain.

**DEXTools, 21 Juni 2026** memberi konfirmasi independen: tingkat
kelulusan ~0,26% pertengahan Juni, turun 80% dalam tiga bulan. Fee
jaringan Solana turun dari ~33.000 SOL/hari di Januari ke ~5.300 SOL di
Juni — turun ~84%.

### Cara membacanya

Dari 1.000 token yang diluncurkan, sekitar **dua** yang lulus.

Tapi lihat lebih dekat. Dua prediktor terkuat kelangsungan hidup adalah:

1. **Kehadiran sosial yang nyata** (lift sampai 17,4x)
2. **Skin-in-the-game kreator** (HR 4,51)

Dan di pump.fun, **keduanya cuma klaim yang tidak diverifikasi.**

Siapa pun bisa menempelkan link Telegram mati. Tidak ada yang mengecek
kanalnya ada, apalagi punya anggota. Self-buy memang terlihat on-chain,
tapi tidak terikat apa pun — dev bisa dump di blok berikutnya.

Kenapa pump.fun tidak memverifikasinya? Bukan karena tidak mau. **Karena
Solana tidak bisa memanggil API dan tidak bisa menyimpan secret.**
Memverifikasi satu kanal Telegram butuh oracle dengan API key. Untuk
20.000 peluncuran per hari, oracle itu jadi bisnis tersendiri dengan
insentifnya sendiri — persis "compound marginalization" yang diserang
seluruh tesis Rialo.

### Masalah struktural kedua: sniper di momen kelulusan

Dari analisis J.Tools (Mei 2026): kelulusan adalah jendela paling
berbahaya untuk membeli buta. Sniper front-run pembuatan LP, menjual ke
gelombang pembeli pasar pertama, dan meninggalkan pool baru dalam posisi
merugi.

Solana tidak bisa memperbaiki ini karena mempool-nya publik. Bahkan
dengan Jito, isi order terlihat oleh block builder.

### Satu angka yang menyesatkan

CoinGecko melaporkan 73,3% trader pump.fun untung di April 2026. Angka
itu mengukur **wallet, bukan dolar**. Trader yang untung $4 di satu
token lalu rugi $400 di token berikutnya tetap terhitung "untung" kalau
P&L bersihnya melewati nol di akhir bulan. Rata-ratanya berisik;
mediannya jauh lebih sunyi.

Jangan bangun tesis produk di atas angka itu.

---

## Bagian 5 — Kesimpulan riset

pump.fun tidak sedang rusak di bagian matematikanya. Kurvanya elegan dan
sudah terbukti. Yang rusak adalah **komposisi peluncurannya** — dan
studi SSRN mengatakan itu secara eksplisit.

Biaya pembuatan 0,02 SOL menghasilkan 800.000+ peluncuran per bulan dan
tingkat kelulusan 0,198%. Menurunkan biaya lebih jauh akan memperburuk.
Menaikkannya secara buta akan mengusir kreator sungguhan.

Jawaban yang benar bukan harga, tapi **bukti**. Buat dua sinyal yang
secara empiris memprediksi kelangsungan hidup menjadi sesuatu yang
**ditegakkan chain**, bukan diklaim kreator.

Itu tidak bisa dibangun di Solana. Itu bisa dibangun di Rialo hari ini.

→ Lanjut ke `02-ARCHITECTURE.md`.

---

## Sumber

- Rialo Dev Portal — https://rialo.io/for-devs
- Reactive Transactions (20 Apr 2026) — https://www.rialo.io/posts/reactive-transactions-a-model-for-native-automation-on-rialo
- Stake for Service (26 Des 2025) — https://www.rialo.io/posts/stake-for-service
- Native Privacy (29 Jan 2026) — https://www.rialo.io/posts/building-native-privacy-for-real-world-blockchain-adoption
- pump.fun program docs — https://deepwiki.com/pump-fun/pump-public-docs
- Bonding curve mechanism — https://deepwiki.com/pump-fun/pump-public-docs/3.1-bonding-curve-mechanism
- Studi survival SSRN (Jul 2026) — https://papers.ssrn.com/sol3/papers.cfm?abstract_id=6915560
- DEXTools graduation collapse (21 Jun 2026) — https://www.dextools.io/news/pump-fun-graduation-collapse-solana-fees-2026
- Contoh resmi Rialo — https://github.com/SubzeroLabs/rialo-examples
