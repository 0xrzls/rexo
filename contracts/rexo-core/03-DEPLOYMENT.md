# TriggerDesk — Compile & Deploy Program di Rialo DevNet

Panduan berurutan dari nol sampai program terverifikasi on-chain.
Semua isi diambil dari repo `triggerdesk` — tidak ada langkah karangan.

Tiga titik yang tidak terdokumentasi di repo ditandai dengan **⚠ CELAH** dan
disertai cara mengisinya sendiri.

---

## Langkah 0 — Siapkan host

| Item | Nilai |
|---|---|
| OS | Ubuntu 22.04 (target terverifikasi) |
| Arsitektur | x86_64 atau aarch64 |
| Spesifikasi | 4 vCPU, 8 GB RAM, 80 GB disk |
| Akses | shell + sudo, outbound HTTPS dan DNS |

Windows: pakai WSL Ubuntu 22.04. macOS native tidak didukung.
Ubuntu 24.04 pernah berhasil (GitHub Codespace `standardLinux32gb`), tapi
bootstrap akan mengeluarkan warning karena bukan target terverifikasi.

Host ini murni infrastruktur build/test. Ia tidak pernah jadi bagian dari
eksekusi pembayaran — tidak ada scheduler, keeper, atau key service di runtime.

---

## Langkah 1 — Clone dan jalankan bootstrap

```bash
git clone https://github.com/Ifem1/triggerdesk.git
cd triggerdesk
chmod +x scripts/bootstrap-rialo-dev.sh scripts/verify-rialo-v1.sh
./scripts/bootstrap-rialo-dev.sh
```

Script ini yang paling bisa dipercaya di seluruh repo — ia kode eksekusi,
bukan dokumen yang bisa basi. Urutan kerjanya:

1. Validasi Linux + Ubuntu, rekam OS/arch/DNS/HTTP sebagai evidence
2. `apt-get install` prerequisite build
3. Install Rust via rustup (`--profile minimal`, default stable)
4. Install `rialoman` 0.3.0
5. Install Rialo `stable@0.12.2`
6. Install Rialo Rust toolchain `0.0.3`
7. Validasi custom target tersedia

Log lengkap: `.rialo-bootstrap/bootstrap.log`

### Paket apt yang dipasang

```
build-essential ca-certificates clang cmake curl git libssl-dev
llvm ninja-build pkg-config protobuf-compiler python3 xz-utils
```

### Rantai fallback

Script tidak menyembunyikan kegagalan. Kalau satu jalur gagal, ia turun ke
jalur berikutnya dan tetap melaporkannya:

| Tahap | Utama | Fallback |
|---|---|---|
| `rialoman` | Installer S3 | `cargo install --locked --version 0.3.0 rialoman` |
| Rialo release | `rialoman install stable@0.12.2` | `rialoman install 0.12.2` (sintaks lama) |
| Rust toolchain | `rialoman toolchain install rialo-rust --version 0.0.3` | compile dari source via `scripts/rialo-toolchain-bootstrap` |

URL installer yang aktif:

```
https://rialo-artifacts.s3.us-east-2.amazonaws.com/rialoman/stable/install.sh
```

`rialoman.rialo.io` sudah mati dan tidak resolve. Ia masih ada di script hanya
sebagai probe diagnostik, dan `HANDOVER.md` masih salah menuliskannya sebagai
installer utama. Abaikan yang di HANDOVER.

---

## Langkah 2 — Verifikasi toolchain

```bash
export PATH="$HOME/.cargo/bin:${XDG_DATA_HOME:-$HOME/.local/share}/rialo/bin:$PATH"

rialoman --version
rialoman current
cargo +rialo --version
rustc +rialo --print target-list | grep -Fx riscv64emac-solana-solana
```

**Kalau grep terakhir tidak mengeluarkan apa pun, berhenti di sini.**
Jangan diganti target Solana generik — binary-nya tidak akan jalan di Rialo.
Ulangi Langkah 1 dan baca `.rialo-bootstrap/bootstrap.log` untuk cari
tahap mana yang gagal.

### Versi yang di-pin

| Komponen | Versi |
|---|---|
| Rialo release | `stable@0.12.2` |
| `rialoman` | `0.3.0` (installer S3 melapor `0.3.0-alpha.0`) |
| Rialo Rust toolchain | `0.0.3` |
| Custom target | `riscv64emac-solana-solana` |
| Source-build nightly | `nightly-2025-05-10` |
| Pinned Rust commit | `dcecb99176edf2eec51613730937d21cdd5c8f6e` |
| Venus crates | `0.12.2` |
| `@rialo/ts-cdk` | `0.12.2` |

Jangan campur versi. Program, manifest, dan CDK terikat ke `0.12.2` yang sama.

---

## Langkah 3 — Compile program

Perintah kanonik:

```bash
rialo-build \
  --program-path programs/scheduled-transfer-v2 \
  --output-dir programs/scheduled-transfer-v2/artifacts
```

Output:

```
programs/scheduled-transfer-v2/artifacts/scheduled-transfer-v2-riscv/scheduled_transfer_v2.polkavm
programs/scheduled-transfer-v2/wit/scheduled-transfer-v2.wit
programs/scheduled-transfer-v2/wit/scheduled-transfer-v2-manifest.json
```

`rialo-build` sekaligus meregenerasi WIT dan manifest. Keduanya harus
byte-identik dengan yang sudah ada di repo:

```bash
git diff --exit-code -- programs/scheduled-transfer-v2/wit
```

### Alternatif low-level

Tercatat di `HANDOVER.md`, tapi tidak meregenerasi manifest:

```bash
cd programs/scheduled-transfer-v2
cargo build --release --target riscv64emac-solana-solana
```

Pakai `rialo-build` saja kecuali kamu tahu persis kenapa butuh yang ini.

### Jebakan: hash bergantung pada username

Rialo 0.12.2 **menanamkan path Cargo registry ke dalam binary PolkaVM**.
Build dengan `CARGO_HOME` berbeda menghasilkan hash berbeda — kasus nyata di
repo: selisih 16 byte, karena binary V1 mengandung `/home/achinnys/.cargo`.

Untuk mereproduksi artifact lama:

```bash
CARGO_HOME=/home/achinnys/.cargo rialo-build \
  --program-path programs/scheduled-transfer \
  --output-dir /tmp/verify
```

Untuk program baru: pakai path build kanonik atau compiler path remapping,
supaya reproducibility tidak bergantung pada nama user siapa pun.

### Manifest client (khusus Scheduled Transfer V2)

Venus 0.12.2 punya defect. Pada instruksi `cancel`, Rust hasil expand menyusun
akun sebagai payer/workflow/system/subscription/vault, tapi manifest yang
di-generate menyisipkan `subscriber_interface` sebelum dua akun user.

Koreksinya deterministik:

```bash
node scripts/generate-scheduled-v2-client-manifest.mjs
```

Script ini *fail closed* — menolak jalan kalau urutan dari upstream bukan
bentuk 0.12.2 yang sudah diaudit. Ini koreksi urutan interface, bukan
tebakan discriminant atau substitusi akun.

---

## Langkah 4 — Verifikasi hasil build

```bash
./scripts/verify-rialo-v1.sh
```

Script ini build ulang, lalu `sha256sum` + `cmp` hasil build melawan artifact
yang di-commit. Evidence ditulis ke `.rialo-evidence/v1/`.

Hash artifact yang ada di repo (sudah dicek cocok dengan `deployments/devnet.json`):

| Program | SHA-256 |
|---|---|
| `scheduled-transfer` (V1) | `f0ede60dcd4471c0f1961cca76ad7d04e822fd727a0fc72b4425b563a89d256a` |
| `recurring-allowance` (V1) | `c8bcdc706730aeb5dd1cb912124bc951c2bd5cc7443266e290ad43148c5c15bb` |
| `scheduled-transfer-v2` | `c010cc54305fdf2a71592639377cd95e781da798a45aa6d8fdc4c10570c840d1` |
| `triggerdesk-phase0` | `a8308579e75e7c37d441d691e70bcfc4c7cd3351fe10018e416a34064ade694f` |
| `funds-path-probe` | `f3edd8503087a987ea6b2cadee8d6352256f71d2e908a5a0b2d20c6c698d01ba` |
| `recurring-allowance-v2` | `6f3ce27cea28518e9c3cb2f3a441592cf1e6de8afe147ae23c7247747bedc3c9` |

Dua yang terakhir belum terdaftar di `deployments/devnet.json` — sudah
di-compile, belum di-deploy.

**Kalau tujuanmu cuma deploy ulang program yang sudah ada, Langkah 3 dan 4
bisa dilewati.** File `.polkavm` sudah ada di repo dan hash-nya terverifikasi.
Langsung ke Langkah 5.

---

## Langkah 5 — Siapkan deployer key

**⚠ CELAH 1 — perintah keypair tidak ada di repo.**

`PHASE0-REPORT.md` hanya menyebut hasilnya:

- alias: `phase0`
- pubkey: `BJEbqxj2r8LyNHAwdkGEN9jLA4E9NUFu25x9uju9oZ8g`
- file: `/home/achinnys/.config/rialo/phase0.keypair`

Perintah pembuatnya tidak pernah ditulis. Cari sendiri di mesin build:

```bash
rialo client --help
rialo client keypair --help
```

Aturan yang dipakai repo:

- Keypair **disposable**, satu per deployment, DevNet-only
- Jangan pernah di-commit
- `.rialo-bootstrap/` dan `.rialo-evidence/` dilarang berisi signing key

---

## Langkah 6 — Danai deployer

**⚠ CELAH 2 — perintah airdrop CLI tidak ada di repo.**

Yang terdokumentasi hanya `requestAirdropAndConfirm` dari CDK di browser.
Untuk CLI, cek:

```bash
rialo client --help
```

**Yang penting dan sudah terbukti: satu airdrop 1 RLO TIDAK CUKUP.**

Deploy butuh rent untuk buffer Loader V4. Kegagalan nyata yang tercatat:

```
Transfer: insufficient kelvins 999960000, need 1068062112
```

Jadi butuh ~1,07 RLO, sementara satu airdrop cuma memberi 1 RLO. Kejadian ini
terulang dua kali di repo (funds-path-probe dan deploy P0 surplus), dan
dua-duanya baru berhasil setelah airdrop kedua. **Minta airdrop dua kali.**

Cek saldo:

```bash
rialo client account -n devnet --json <ADDRESS> | jq -r .kelvin
```

Satuan: 1 RLO = 1.000.000.000 kelvin.

---

## Langkah 7 — Deploy

**⚠ CELAH 3 — flag network tidak ditulis di contoh deploy.**

`HANDOVER.md` dan `PHASE0-REPORT.md` menulisnya tanpa network:

```bash
rialo client program deploy \
  programs/scheduled-transfer-v2/artifacts/scheduled-transfer-v2-riscv/scheduled_transfer_v2.polkavm
```

Padahal semua perintah lain di repo pakai `-n devnet`
(`rialo client account -n devnet`, `rialo client get-block-height -n devnet`).
Kemungkinan ada default config, tapi tidak dikonfirmasi. Cek:

```bash
rialo client program deploy --help
```

### Dua defect CLI 0.12.2 yang harus diantisipasi

**1. Deploy berhasil tapi CLI melaporkan `InvalidArgument`.**
Ini terjadi di retry/poll pasca-deploy. Deployment `FkqUGXxy…` sukses meski
CLI mengeluarkan error ini. **Jangan percaya exit code** — verifikasi lewat
query akun di Langkah 8.

**2. CLI menggantung selamanya.**
Kalau transaksi pembuatan buffer gagal (biasanya rent kurang), CLI polling
terus ke buffer yang tidak pernah ada, dan error transaksinya tidak pernah
disurfacing. Kelihatan seperti hang. Kalau ini terjadi, cek transaksi di chain
— hampir pasti masalah dana, bukan masalah CLI.

CLI 0.12.2 juga tidak menyimpan satu deployment transaction ID. Itu sebabnya
semua entry di `deployments/devnet.json` punya `"deploymentTransaction": null`.

---

## Langkah 8 — Verifikasi deployment (WAJIB)

Ini satu-satunya bukti yang dipercaya repo — bukan output CLI.

```bash
rialo client account -n devnet --json <PROGRAM_ID>
```

Cek empat hal:

| Cek | Nilai yang benar |
|---|---|
| `owner` | `RiscVLoader11111111111111111111111111111111` |
| `executable` | `true` |
| Loader header | 48 byte pertama account data |
| Hash payload | `sha256(data[48:])` == `artifactSha256` |

Hitung hash payload:

```bash
rialo client account -n devnet --json <PROGRAM_ID> \
  | jq -r .data \
  | base64 -d | tail -c +49 | sha256sum
```

Contoh nyata yang sudah terbukti (`3BA494eLRy15oHN4ST2Fq8Bx231xdPDfJy1tpP7hyoD6`):

- account: 141.026 byte
- header: 48 byte
- payload: 140.978 byte
- hash: `c010cc54305fdf2a71592639377cd95e781da798a45aa6d8fdc4c10570c840d1`

Angka itu persis sama dengan `.polkavm` di repo. Kalau hash-mu cocok,
deployment-nya sah — apa pun yang dikatakan CLI.

---

## Langkah 9 — Daftarkan program ID

Registry tunggal: `deployments/devnet.json`. Dibaca `lib/rialo/network.ts`,
diekspor lewat `lib/rialo/constants.ts`. Network selain `devnet` fail closed.

Program ID **tidak pernah** ditulis hardcode di komponen UI.

Tambahkan entry:

```json
"scheduledTransferV3": {
  "programId": "<PROGRAM_ID_BARU>",
  "schemaVersion": 3,
  "owner": "RiscVLoader11111111111111111111111111111111",
  "executable": true,
  "loaderAccountHeaderBytes": 48,
  "artifactSha256": "…",
  "manifestSha256": "…",
  "clientManifestSha256": "…",
  "witSha256": "…",
  "deployedPayloadSha256": "…",
  "deploymentTransaction": null,
  "notes": "…"
}
```

Lalu arahkan `DEVNET.scheduledTransfer` di `lib/rialo/network.ts` ke entry baru.

Yang sudah terdaftar:

| Key | Program ID | Status |
|---|---|---|
| `scheduledTransferV2` | `FkqUGXxy8y4PHGdRmtJLvKMp1h48EeeWaz8KCFi9ZAwS` | V2 schema-stable |
| `scheduledTransferV2SurplusP0Test` | `3BA494eLRy15oHN4ST2Fq8Bx231xdPDfJy1tpP7hyoD6` | **aktif dipakai UI** |
| `scheduledTransferV1` | `7BcfcJEJPxatpejoHjbWfPNnEnEsnk3fh1toN4pYCuxh` | historis |
| `recurringAllowanceV1` | `6TpMo9xFFLYktHhmXzaTkBp2rPTzAuLrk699W7NAW7RZ` | historis |

---

## Langkah 10 — Deploy frontend

Gate lokal dulu:

```bash
npm ci
npm run lint
npm run typecheck
npm test -- --runInBand
npm run build
npm audit --audit-level=high
```

Vercel:

| Setting | Nilai |
|---|---|
| Framework | Next.js |
| Build command | `npm run build` |
| Output | `.next` |
| Install | `npm ci` |
| Env vars | Tidak ada yang wajib; opsional `NEXT_PUBLIC_RIALO_NETWORK=devnet` |

Browser tidak memanggil DevNet langsung — lewat proxy `/api/rpc` dengan
allowlist method, untuk menghindari CORS. Monitoring lewat `/api/health`.

Kalau build gagal karena `@rialo/ts-cdk` tidak ketemu, konfigurasi `.npmrc`
ke registry Rialo.

RPC DevNet:

- HTTPS: `https://devnet.rialo.io:4101` (dipakai `deployments/devnet.json`)
- HTTP: `http://devnet.rialo.io:4100`

**Urutan wajib:** deploy frontend hanya setelah registry berisi program ID
yang sudah diverifikasi.

---

## Limitasi Rialo 0.12.2

| Isu | Dampak |
|---|---|
| `AFTER` → `active_commits = slot..=slot + 100` | Jendela eksekusi hanya ~100 blok. Workflow 5 menit terlewat dan tidak jalan. Ini alasan tombol create di UI dimatikan |
| `unix_timestamp()` mengembalikan 0 | Created-at tampil 1970. Tidak memengaruhi eksekusi |
| Tidak ada konstruk `EVERY` | Recurring allowance diakali 3x `AFTER` terpisah, jumlah tetap |
| Kunci ephemeral | Keypair per tab di sessionStorage, hilang saat tab ditutup |
| DevNet-only | Tidak ada jalur mainnet |
| `getWorkflowLineage` | `workflowChildren` dan `subscriptions` kosong; pakai `getSignaturesForAddress` pada workflow PDA |

---

## Aturan yang tidak boleh dilanggar

- Jangan auto-deploy program finansial dari merge ke `main`
- Jangan percaya exit code CLI deploy — verifikasi lewat query akun
- Jangan ganti target ke Solana generik kalau toolchain Rialo gagal dipasang
- Jangan bayar dari backend kalau callback gagal; pakai instruksi recovery on-chain
- Jangan commit signing key ke direktori evidence mana pun
- Jangan klaim scheduling jangka panjang selama limitasi 100 blok belum hilang

---

## Checklist

**Toolchain**
- [ ] Ubuntu 22.04, 4 vCPU / 8 GB / 80 GB
- [ ] `bootstrap-rialo-dev.sh` selesai tanpa error
- [ ] `rustc +rialo --print target-list` memuat `riscv64emac-solana-solana`

**Build** *(lewati kalau deploy ulang artifact yang sudah ada)*
- [ ] `rialo-build` menghasilkan `.polkavm` + WIT + manifest
- [ ] `git diff --exit-code` pada `wit/` bersih
- [ ] Hash build == hash artifact di repo
- [ ] Client manifest V2 di-generate ulang dan lolos fail-closed check

**Deploy**
- [ ] Keypair disposable DevNet dibuat
- [ ] Saldo ≥ ~1,1 RLO (dua kali airdrop)
- [ ] `rialo client program deploy` dijalankan
- [ ] Owner = Loader, executable = true, header 48 byte, hash payload cocok
- [ ] Program ID + semua hash dicatat di `deployments/devnet.json`

**Frontend**
- [ ] npm ci / lint / typecheck / test / build / audit lolos
- [ ] `network.ts` menunjuk entry registry yang benar
- [ ] Deploy Vercel, `/api/health` hijau
- [ ] Smoke test: connect → airdrop → create → callback → **balance penerima berubah**

Poin terakhir yang menentukan. Perubahan state saja tidak cukup — delta saldo
penerima adalah satu-satunya bukti dana benar-benar bergerak.
