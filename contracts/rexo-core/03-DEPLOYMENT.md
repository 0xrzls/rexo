# Rialo Deployment & Requirement Guide

Disusun dari analisis komparatif dua implementasi referensi: `triggerdesk-main` (alur verifikasi nyata) dan `Permitly-main` (analisis pseudo-DSL & mock).

---

## 0. Ringkasan Temuan

| Aspek | TriggerDesk (Kanonik) | Permitly |
|---|---|---|
| **Bahasa Kontrak** | Rust + Venus DSL (`rialo!` procedural macro) | File teks `.rialo` (pseudo-DSL) |
| **Compile Target** | PolkaVM RISC-V (`riscv64emac-solana-solana`) | Tidak ada proses compile |
| **Artifact di Repo** | File `.polkavm` (hash cocok registry) | Tidak ada artifact binary |
| **Cara Deploy** | `rialo client program deploy <binary>` | JSON-RPC `deployProgram` kirim source mentah |
| **Bukti On-Chain** | Tx signature, balance delta, hash payload 48-byte offset | Tidak ada |
| **Status** | **Jalur deploy nyata dan terverifikasi** | **Belum bisa di-deploy sungguhan** |

Semua langkah teknis di bawah ini mengadopsi standar kanonik TriggerDesk yang telah teruji secara on-chain di Rialo DevNet.

---

## 1. Requirement & Spesifikasi Sistem

### 1.1 Host Environment

| Item | Nilai Minimum / Rekomendasi |
|---|---|
| **OS** | Ubuntu 22.04 LTS (target terverifikasi), x86_64 atau aarch64 |
| **CPU / RAM / Disk** | 4 vCPU, 8 GB RAM, 80 GB SSD |
| **Akses** | Shell + sudo, outbound HTTPS (port 443) dan DNS |

*Catatan:* Ubuntu 24.04 didukung pada GitHub Actions / Codespace, namun script bootstrap akan mengeluarkan warning kompatibilitas. Pada Windows, gunakan WSL2 dengan Ubuntu 22.04.

### 1.2 Versi Pinned (Wajib Terkunci)

Komponen dan CDK terikat erat pada versi yang sama. Jangan mengganti versi secara acak:

| Komponen | Versi Teruji |
|---|---|
| **Rialo Release** | `stable@0.12.2` |
| **`rialoman`** | `0.3.0` (installer S3 melaporkan `0.3.0-alpha.0`) |
| **Rialo Rust Toolchain** | `0.0.3` (tercatat di `.rialo-toolchain`) |
| **Custom Target** | `riscv64emac-solana-solana` |
| **Source-Build Nightly** | `nightly-2025-05-10` |
| **Pinned Rust Commit** | `dcecb99176edf2eec51613730937d21cdd5c8f6e` |
| **Venus Crates (`rialo-s-*`, `rialo-venus`)** | `0.12.2` |
| **`@rialo/ts-cdk`** | `0.12.2` |
| **Node.js** | 20 / 22 / 24 LTS |

### 1.3 Paket Dependensi Sistem (apt)

```bash
sudo apt-get update && sudo apt-get install -y \
  build-essential ca-certificates clang cmake curl git libssl-dev \
  llvm ninja-build pkg-config protobuf-compiler python3 xz-utils
```

### 1.4 Jaringan & Endpoint RPC

| Keperluan | URL | Keterangan |
|---|---|---|
| **Installer rialoman (Aktif)** | `https://rialo-artifacts.s3.us-east-2.amazonaws.com/rialoman/stable/install.sh` | Endpoint resmi S3 aktif |
| **Installer Legacy (Mati)** | `https://rialoman.rialo.io/install.sh` | Domain tidak resolve lagi (hanya probe historis) |
| **RPC DevNet HTTPS** | `https://devnet.rialo.io:4101` | Endpoint TLS port 4101 |
| **RPC DevNet HTTP** | `http://devnet.rialo.io:4100` | Endpoint plain port 4100 |

### 1.5 Satuan & Alokasi Dana (Rent Buffer)

- **1 RLO** = 1.000.000.000 kelvin (10^9 kelvin).
- Deploy membutuhkan rent balance untuk buffer Loader V4.
- Kasus nyata di repo: transaksi gagal dengan `insufficient kelvins 999960000, need 1068062112`.
- **Aturan:** 1x airdrop (1 RLO) **tidak cukup**. Selalu minta airdrop 1 RLO sebanyak **2 kali** (total ~2 RLO) sebelum memulai deployment.

---

## 2. Bootstrap Toolchain

```bash
# 1. Unduh dan jalankan installer resmi dari bucket S3
curl -fsSL https://rialo-artifacts.s3.us-east-2.amazonaws.com/rialoman/stable/install.sh | bash -s -- --no-modify-path --default-toolchain none

# 2. Konfigurasi PATH ke shell environment
export PATH="$HOME/.cargo/bin:${XDG_DATA_HOME:-$HOME/.local/share}/rialo/bin:$PATH"

# 3. Pasang versi release stable@0.12.2
rialoman install stable@0.12.2 --default

# 4. Pasang toolchain rialo-rust 0.0.3
rialoman toolchain install rialo-rust --version 0.0.3

# 5. Validasi ketersediaan target RISC-V PolkaVM
rustc +rialo --print target-list | grep -Fx riscv64emac-solana-solana
```

*Peringatan:* Jika `grep -Fx riscv64emac-solana-solana` tidak menghasilkan output, hentikan proses. Jangan mengganti target ke target Solana generik (`bpf-solana-solana`) karena instruksi bytecode tidak akan dapat dieksekusi di runtime Rialo PolkaVM.

---

## 3. Kompilasi Program

### Cara Kanonik (PDK Build):
```bash
rialo-build \
  --program-path contracts/rexo-core \
  --output-dir contracts/rexo-core/artifacts
```

### Cara Alternatif (Crate Artifact / Cargo Direct):
```bash
cd contracts/rexo-core
cargo build --manifest-path artifact/Cargo.toml --release
```

Output kompilasi:
- `contracts/rexo-core/artifacts/rexo_core.polkavm` (Binary executable PolkaVM)
- `contracts/rexo-core/wit/rexo-core.wit` (WebAssembly Interface Type definition)
- `contracts/rexo-core/wit/rexo-core-manifest.json` (Instruction & account manifest)

### 3.1 Jebakan Reproducibility (Cargo Path Embedding)
Rialo 0.12.2 menanamkan path Cargo registry lokal ke dalam binary PolkaVM. Build dengan `CARGO_HOME` berbeda akan menghasilkan hash SHA-256 yang berbeda (contoh: selisih 16 byte akibat perbedaan username di `/home/<user>/.cargo`).

**Mitigasi:** Gunakan environment build kanonik (seperti GitHub Actions runner `/home/runner`) atau compiler path remapping (`--remap-path-prefix`) saat memverifikasi hash produksi.

### 3.2 Koreksi Client Manifest (Defect Venus 0.12.2)
Pada Venus 0.12.2, instruksi tertentu dapat menyisipkan `subscriber_interface` di posisi yang tidak sesuai dengan urutan akun yang di-expand oleh macro Rust (payer, workflow, system, subscription, vault).
Selalu jalankan generator manifest client untuk memverifikasi urutan akun secara deterministik sebelum integrasi frontend.

---

## 4. Persiapan Deployer Key & Faucet

```bash
# 1. Generate keypair disposable untuk DevNet
rialo keytool generate --output-file ~/.config/rialo/deployer.keypair

# 2. Cek saldo akun
rialo client account -n devnet --json <ADDRESS> | jq -r .kelvin

# 3. Lakukan 2x airdrop (minimal 1.1 RLO untuk memenuhi rent buffer Loader V4)
rialo client airdrop -n devnet --amount 1
sleep 2
rialo client airdrop -n devnet --amount 1
```

---

## 5. Deployment ke DevNet & Verifikasi On-Chain

### 5.1 Perintah Deploy
```bash
rialo client program deploy \
  contracts/rexo-core/artifacts/rexo_core.polkavm
```

#### Mengatasi Defect CLI 0.12.2:
1. **False Negative `InvalidArgument`:** CLI 0.12.2 terkadang melaporkan `InvalidArgument` saat melakukan polling status pasca-deploy padahal program sebenarnya sudah berhasil tersimpan di chain. Verifikasi kebenaran melalui query akun langsung (Bagian 5.2).
2. **Infinite Polling Hang:** Jika transaksi pembuatan buffer gagal (karena kekurangan saldo rent kelvin), CLI akan menggantung (polling tanpa batas). Pastikan saldo deployer telah diisi 2x airdrop sebelum deploy.

### 5.2 Verifikasi Deployment (48-Byte Loader Header Check)

Satu-satunya bukti valid deployment di Rialo adalah query struktur akun:

```bash
rialo client account -n devnet --json <PROGRAM_ID>
```

Kriteria validitas akun program:
1. **`owner`**: Wajib bernilai `RiscVLoader11111111111111111111111111111111`
2. **`executable`**: Bernilai `true`
3. **Loader Header**: 48 byte pertama pada data akun dialokasikan sebagai metadata loader.
4. **Payload Hash Matching**: Hash SHA-256 dari byte ke-49 dan seterusnya (`data[48:]`) harus **identik 100%** dengan hash SHA-256 file `.polkavm` lokal:

```bash
# Verifikasi hash payload on-chain vs artifact lokal:
rialo client account -n devnet --json <PROGRAM_ID> \
  | jq -r .data \
  | base64 -d | tail -c +49 | sha256sum

sha256sum contracts/rexo-core/artifacts/rexo_core.polkavm
```

---

## 6. Pendaftaran Program ID ke Frontend

Daftarkan program ID yang terverifikasi ke dalam registry `deployments/devnet.json`:

```json
{
  "rexoCore": {
    "programId": "<PROGRAM_ID_TERVERIFIKASI>",
    "schemaVersion": 1,
    "owner": "RiscVLoader11111111111111111111111111111111",
    "executable": true,
    "loaderAccountHeaderBytes": 48,
    "artifactSha256": "...",
    "manifestSha256": "...",
    "witSha256": "...",
    "deployedPayloadSha256": "...",
    "deploymentTransaction": null,
    "notes": "Verified via Loader V4 48-byte offset payload match"
  }
}
```

Frontend memanggil RPC via proxy server `/api/rpc` untuk menghindari masalah CORS browser.

---

## 7. Analisis Komparatif: Kasus Permitly

Studi kasus Permitly penting dipahami sebagai referensi apa yang **tidak boleh** dilakukan:

1. **File Pseudo-DSL (`.rialo`):** Permitly menggunakan file teks 963 baris dengan sintaks `workflow { state { ... } }` tanpa Rust crate, `Cargo.toml`, atau toolchain Venus.
2. **Script Deploy Non-Standar:** Menggunakan script yang mengirim string source mentah melalui method JSON-RPC `deployProgram` alih-alih meng-upload binary PolkaVM ke Loader V4.
3. **Silent Fallback to Mock:** Adapter RPC diam-diam beralih ke mode mock saat panggilan RPC gagal, sehingga transaksi tampak sukses padahal tidak pernah menyentuh chain.

**Prinsip Rexo:** Selalu gunakan alur kanonik Rust + Venus DSL, kompilasi ke target RISC-V PolkaVM, dan verifikasi hash payload 48-byte offset di on-chain Loader V4.

---

## 8. Limitasi Diketahui (Rialo 0.12.2)

| Isu / Konstruksi | Dampak & Solusi Workaround |
|---|---|
| **Konstruk `AFTER`** | Menghasilkan `active_commits = slot..=slot + 100` (jendela eksekusi ~100 blok). Workflow jeda panjang belum dapat dieksekusi reliabel di luar jendela tersebut. |
| **`unix_timestamp()`** | Mengembalikan nilai 0 di environment tertentu (terbaca 1970 pada UI), namun tidak memengaruhi logika slot-based. |
| **Konstruk `EVERY`** | Untuk recurring loop, implementasikan transisi berantai melalui beberapa handler event diskrit. |
| **Lineage Query** | `getWorkflowLineage` belum menyertakan child workflow lengkap; gunakan `getSignaturesForAddress` pada PDA workflow. |

---

## 9. Checklist Rilis Pra-Deploy

### Toolchain & Environment
- [ ] Host Ubuntu 22.04 LTS (4 vCPU / 8 GB RAM / 80 GB SSD).
- [ ] Toolchain `rialo-rust 0.0.3` terpasang dan target `riscv64emac-solana-solana` terdaftar di `rustc +rialo --print target-list`.
- [ ] `rustc --test contracts/rexo-core/src/curve.rs` lolos seluruh 21 tes invariasi matematika bonding curve.

### Build & Artifact
- [ ] Kompilasi via `rialo-build` atau `cargo build --manifest-path artifact/Cargo.toml --release` berhasil menghasilkan `.polkavm`.
- [ ] File WIT dan manifest akun sinkron dengan struktur interface Rust.
- [ ] SHA-256 binary artifact tercatat dan diverifikasi.

### Deploy & On-Chain Verification
- [ ] Menggunakan keypair disposable khusus DevNet.
- [ ] Saldo deployer memiliki minimal 2x airdrop (≥ ~1.1 RLO) untuk memenuhi rent buffer Loader V4.
- [ ] Program di-deploy menggunakan `rialo client program deploy`.
- [ ] Query akun membuktikan: `owner == RiscVLoader11111111111111111111111111111111`, `executable == true`, dan `sha256(data[48:]) == artifactSha256`.
- [ ] Program ID terdaftar di `deployments/devnet.json` dan diuji melalui frontend proxy.
