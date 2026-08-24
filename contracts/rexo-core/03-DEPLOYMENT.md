# 03 — Deployment & Operasi

Perintah di bagian DevNet/Testnet dikutip dari `AGENTS.md`/`CLAUDE.md`
resmi di repo `SubzeroLabs/rialo-examples`. Perintah itu terverifikasi.
Yang di luar itu ditandai.

---

## Fase 0 — Jalankan matematikanya sekarang (5 menit, tanpa toolchain Rialo)

Ini bukan langkah basa-basi. Ini satu-satunya bagian yang bisa kamu
verifikasi sendiri hari ini.

```bash
cd coldstart
rustc --test src/curve.rs -o /tmp/curve_test && /tmp/curve_test
```

Yang harus kamu lihat: 21 test lolos. Termasuk
`constants_match_pumpfun_global_account`, yang membuktikan kurvanya
identik dengan pump.fun asli sampai ke unit terakhir.

Main-mainkan dulu. Ubah `creator_fee_bps` di `LaunchTier::policy()`,
jalankan lagi, lihat test mana yang pecah. Itu cara tercepat memahami
ekonominya sebelum menyentuh chain.

---

## Fase 1 — Toolchain

```bash
# 1. Rust standar (kalau belum ada)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 2. Toolchain Rialo lewat rialoman
#    Panduan resmi:
#    docs.rialo.io/user/latest/1-introduction/installation-using-rialoman.html

# 3. Verifikasi
rialo --version
```

Kalau `rialoman` gagal, jangan diakali. Tanya di
`discord.gg/RialoProtocol` — toolchain-nya masih muda dan channel
support-nya aktif.

---

## Fase 2 — Pelajari sintaks yang sebenarnya

**Ini langkah yang tidak boleh dilewati.** `src/lib.rs` di repo ini
adalah kerangka desain, bukan kode yang compile. Sintaks aslinya ada di
sini:

```bash
git clone https://github.com/SubzeroLabs/rialo-examples
cd rialo-examples

# Type-check semuanya. Cuma butuh Rust standar.
cargo check --workspace

# INI sumber kebenaran sintaks. Contoh terkecil, mulai dari sini.
cat venus/http-fetch/src/lib.rs
```

Lalu ambil seluruh konteks dalam satu file — ini yang paling menghemat
waktu kalau kamu pakai coding agent:

```
https://docs.rialo.io/user/latest/llms-full.txt
```

Isinya seluruh buku user + referensi DSL Venus (`usage-guide.md`) +
`src/lib.rs` contoh-contoh anchor, digabung jadi satu markdown.

Contoh yang paling relevan untuk Coldstart:

| Contoh | Untuk mekanisme |
|---|---|
| `venus/http-fetch` | scaffold terkecil + pola `artifact/` crate |
| `venus/price-alert` | pola `EVERY` → heartbeat, snapshot |
| `venus/websocket-feed` | streaming → live chart |
| `venus/rex-wasm-pipeline` | blok `rex {}` → verifikasi sosial |
| `venus/rex-wasm-crypto` | enkripsi untuk TEE → sealed batch |
| direktori swap & prediction market | pola AMM & vault |
| direktori compliant stablecoin | pola mint authority |

Catatan penting: contoh dengan blok `rex {}` membawa
`wit/rex-component.wit` yang di-vendor. Macro Venus membaca interface
REX dari path itu saat build di luar monorepo, dan crate yang
dipublikasikan **tidak** menyertakannya. Jangan hapus file itu saat
menyalin scaffold.

---

## Fase 3 — Scaffold

```bash
# Salin contoh terkecil sebagai basis
cp -r rialo-examples/venus/http-fetch ~/coldstart-live
cd ~/coldstart-live

# Ambil dependency yang sudah di-pin dari Cargo.toml root rialo-examples
# ke Cargo.toml proyekmu. Jangan tebak nama crate-nya.

# Bawa masuk matematika yang sudah teruji
cp ~/coldstart/src/curve.rs src/curve.rs

cargo check
```

Kalau `cargo check` lolos di titik ini, environment-mu beres dan
`curve.rs` kompatibel. Itu milestone nyata.

Tidak semua contoh Venus punya crate `artifact/` (`http-fetch` tidak;
`rex-wasm-pipeline` punya). Untuk deploy contoh yang tidak punya,
tambahkan crate `artifact/` tiga-file persis seperti yang dijelaskan
tutorial "Fetch web data from a program" — `deploy-venus` mengharapkan
layout itu.

---

## Fase 4 — DevNet

Perintah terverifikasi:

```bash
rialo keytool generate
rialo config network switch devnet        # RPC: http://devnet.rialo.io:4100
rialo client airdrop --amount 1           # cap 1 RLO/request; ulangi untuk menumpuk

cargo build --manifest-path artifact/Cargo.toml
rialo client program deploy-venus .

rialo client program invoke <PROGRAM_ID> --program-dir . \
    --function launch --arg workflow_pda_slug=random \
    --arg name=Doge --arg symbol=DOGE

# Ikuti SELURUH rantai transaksi yang dipicu framework.
# Untuk workflow reaktif, ini alat debugging utamamu.
rialo client get-workflow-lineage <TX_SIGNATURE> --full-id true
```

Nama argumen untuk `--arg` datang dari `wit/<name>-manifest.json`.

**Aturan keamanan yang tidak bisa ditawar:** pakai keypair DevNet baru
yang didanai faucet saja. Jangan pernah endpoint mainnet atau secret
produksi. Dan **argumen program adalah data transaksi publik** — jangan
pernah mengoper secret lewat `--arg`. API key Telegram/X harus hidup di
dalam REX, bukan di argumen.

### Testnet publik

Testnet publik live sejak ~9–10 Mei 2026. Perintah switch network-nya
belum bisa kuverifikasi — cek `rialo config network --help` atau docs.
Polanya kemungkinan besar `rialo config network switch testnet`.

---

## Fase 5 — Urutan membangun

Jangan bangun tujuh mekanisme sekaligus. Urutan ini disusun supaya kamu
punya sesuatu yang jalan di setiap langkah.

| # | Langkah | Kenapa di sini |
|---|---|---|
| 1 | Deploy `http-fetch` apa adanya | Membuktikan toolchain beres |
| 2 | Ganti isinya dengan buy/sell kurva, tanpa async | Ini sudah launchpad yang bisa dipakai |
| 3 | Tambah `EVERY` untuk snapshot harga | Fitur pertama yang butuh keeper di Solana |
| 4 | Tambah `SEND` verifikasi sosial + blok `rex` | **Ini pembeda intinya.** Bangun ini lebih awal |
| 5 | Tambah `START graduate()` | Migrasi LP otomatis |
| 6 | Tambah heartbeat + bond forfeit | Membalik ekonomi rug pull |
| 7 | Tambah sealed window | Paling sulit; butuh API REX yang belum terverifikasi |
| 8 | Tambah SfS endowment | Butuh ekonomi RLO yang belum ada |
| 9 | UI | Terakhir |

**Langkah 4 adalah yang paling penting.** Kalau cuma satu mekanisme yang
sempat kamu bangun sebelum mainnet, bangun itu. Verifikasi sosial adalah
satu-satunya yang punya bukti empiris langsung (lift 8,94x) dan
satu-satunya yang mustahil ditiru pump.fun.

---

## Operasi

### Yang perlu dipantau

| Metrik | Kenapa | Alarm |
|---|---|---|
| Rasio kelulusan per tier | Tesis produkmu berdiri atau jatuh di sini | Committed tidak > 5x Unverified |
| Tingkat kegagalan heartbeat | Rate limit API vs abandonment sungguhan | Lonjakan mendadak = masalah API, bukan rug |
| Latensi settlement sealed batch | Kalau lambat, pengalaman launch rusak | > 5 detik pasca-jendela |
| Saldo credit SfS | Automasi mati kalau habis | < 30 hari runway |
| Bond hangus / bond kembali | Rasio kesehatan platform | Hangus > 50% = tier terlalu longgar |

Rasio kelulusan per tier adalah metrik utamamu. Kalau token Committed
tidak lulus jauh lebih sering daripada Unverified, tesisnya salah dan
kamu harus tahu itu cepat, bukan setelah setahun.

### Rate limit API — masalah operasional pertamamu

Telegram Bot API dan X API punya limit. Dengan 1.000 token aktif yang
heartbeat tiap 6 jam, itu 4.000 request/hari — masih aman. Dengan
50.000 token, tidak.

Mitigasi, sesuai urutan yang harus dicoba:
1. Heartbeat adaptif — token dengan volume lebih tinggi dicek lebih sering
2. Cache di dalam REX dengan TTL
3. Batch beberapa handle per webcall
4. Backoff pada token yang sudah lama tidak aktif

### Kalau verifikasi sosial mati total

Failure mode-nya harus **fail-safe, bukan fail-closed**. Kalau API
Telegram down 6 jam, kamu tidak boleh menandai ribuan token sebagai
abandoned.

Karena itu ambangnya 4 kegagalan berturut-turut (~24 jam), bukan 1. Dan
sebelum menandai abandonment massal, program harus mengecek apakah
kegagalannya terkorelasi lintas token — kalau semua token gagal
bersamaan, itu masalahmu, bukan masalah mereka. **Ini belum ada di
`lib.rs` dan harus ditambahkan.**

### Kunci & secret

- Kredensial API sosial: hidup di dalam REX, tidak pernah on-chain, tidak pernah di `--arg`
- Upgrade authority program: multisig, tidak pernah satu key
- Treasury: terpisah dari upgrade authority
- Mint authority tiap token: dicabut di transaksi launch, permanen

---

## Checklist pra-mainnet

Jangan sentuh uang sungguhan sebelum semua ini centang.

- [ ] `rustc --test src/curve.rs` — 21 test lolos
- [ ] Mint authority dan freeze authority dicabut di transaksi launch
- [ ] LP di-burn atau di-lock saat kelulusan, terverifikasi on-chain
- [ ] Tier ditulis oleh REX, tidak pernah diterima sebagai argumen kreator
- [ ] Order sealed tidak pernah tersimpan plaintext di state
- [ ] Deteksi kegagalan heartbeat terkorelasi terpasang
- [ ] Bond hangus ke LP, bukan ke protokol
- [ ] Akuntansi vault diaudit terpisah dari matematika kurva
- [ ] Audit eksternal oleh firma yang punya pengalaman Rialo/Venus
- [ ] Program bug bounty live sebelum peluncuran publik
- [ ] Desimal RLO dikonfirmasi, parameter kurva dikalibrasi ulang
- [ ] Runbook incident response tertulis dan sudah dilatih
