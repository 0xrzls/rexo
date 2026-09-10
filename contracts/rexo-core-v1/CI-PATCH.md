# Patch CI — perbaikan `zerocopy`

`zerocopy` masih gagal di log terakhir dengan error yang sama persis:

```
error[E0658]: use of unstable library feature `stdarch_x86_avx512`
error: could not compile `zerocopy` (lib) due to 3 previous errors
```

Ini **bukan kode kamu**. `zerocopy` 0.7.x di bawah 0.7.35 memakai intrinsic
AVX-512 yang tidak lagi tersedia tanpa feature gate di nightly baru.
Perhatikan errornya menyebut `x86` — ini build untuk host, bukan PolkaVM.

Menjalankan `cargo update` di laptop tidak cukup: kalau `Cargo.lock` tidak
ikut di-commit, CI akan resolve ulang ke versi lama dan gagal lagi.

---

## Perbaikan A — di mesin kamu, lalu commit lock file

Ini yang paling benar dan permanen.

```bash
cd contracts/rexo-core
cargo update -p zerocopy --precise 0.7.35

# cek tidak ada versi lain yang tertinggal
cargo tree -i zerocopy

git add Cargo.lock
git commit -m "fix: pin zerocopy 0.7.35 (stdarch_x86_avx512 on new nightly)"
```

Kalau `cargo tree -i zerocopy` menampilkan **dua** versi, naikkan keduanya:

```bash
cargo update -p zerocopy@0.7.32 --precise 0.7.35
cargo update -p zerocopy@0.6.6 --precise 0.7.35   # sesuaikan versinya
```

---

## Perbaikan B — di workflow, kalau lock file tidak di-commit

Sisipkan step ini di `.github/workflows/deploy.yml` **sebelum** step build
(sebelum `cargo build --manifest-path artifact/Cargo.toml` dan sebelum
`rialo-build`):

```yaml
      - name: Patch zerocopy (stdarch_x86_avx512 pada nightly baru)
        working-directory: contracts/rexo-core
        run: |
          echo "Menaikkan zerocopy ke versi yang compile di nightly saat ini..."
          cargo update -p zerocopy --precise 0.7.35 \
            || cargo update -p zerocopy \
            || echo "::warning::zerocopy tidak bisa dinaikkan; build mungkin gagal"
          cargo tree -i zerocopy || true
```

---

## KALAU KAMU SUDAH MENCOBA A ATAU B DAN DAPAT INI

```
error: failed to select a version for the requirement `zerocopy = "^0.8.23"`
```

**Batalkan dulu.** `cargo update -p zerocopy` tanpa `--precise` melompat ke
jalur 0.8.x, yang tidak kompatibel dengan dependency lain di pohonmu. Itu
saranku yang salah — maaf.

```bash
# 1. buang perubahan resolusi yang gagal
git checkout Cargo.lock 2>/dev/null || rm -f Cargo.lock

# 2. pastikan TIDAK ADA entri zerocopy di Cargo.toml
grep -n zerocopy Cargo.toml   # harus kosong; kalau ada, hapus barisnya

# 3. tetap di jalur 0.7.x
cargo update -p zerocopy --precise 0.7.35
```

Kalau `0.7.35` juga ditolak, jangan lanjut mengutak-atik dependency.
Akar masalahnya bukan di situ — lompat ke Perbaikan C.

---

## Perbaikan C — pin nightly (INI YANG SEBENARNYA BENAR)

Berarti nightly yang dipakai toolchain Rialo terlalu baru untuk seluruh
pohon dependency. Pin nightly-nya:

```toml
# rust-toolchain.toml di root repo
[toolchain]
channel = "nightly-2026-06-30"
components = ["rustfmt", "clippy"]
```

Pilih tanggal yang dekat dengan rilis `rialo-venus` 0.12.2 (30 Juni 2026).
Toolchain Rialo `0.0.3` kemungkinan besar dibangun terhadap nightly sekitar
tanggal itu.

Kenapa ini yang benar: `stdarch_x86_avx512` menjadi unstable pada rilis
nightly TERTENTU. Semua crate lama yang memakainya akan gagal di nightly
yang lebih baru dari itu, bukan cuma `zerocopy`. Menaikkan satu crate hanya
memindahkan masalah ke crate berikutnya. Menurunkan nightly menyelesaikan
seluruh kelasnya sekaligus.

Kalau CI memakai `rialoman`, cek apakah ia memasang nightly-nya sendiri:

```bash
rustc --version          # di dalam step CI, sebelum build
rustup toolchain list
```

Kalau `rialoman` memaksa nightly terbaru, `rust-toolchain.toml` mungkin
diabaikan. Dalam kasus itu, pasang toolchain eksplisit di workflow:

```yaml
      - name: Pin nightly
        run: |
          rustup toolchain install nightly-2026-06-30 --profile minimal
          rustup default nightly-2026-06-30
          rustc --version
```

---

## Catatan urutan

Dua kegagalan di CI kamu **independen**. Log menampilkan keduanya karena
job berjalan paralel:

1. `rialo-build` gagal → error tipe di `rexo-core` (kode kita)
2. `cargo build` gagal → `zerocopy` (dependency)

Memperbaiki satu tidak memperbaiki yang lain. Kerjakan keduanya.
