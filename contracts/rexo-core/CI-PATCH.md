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

## Perbaikan C — kalau A dan B gagal

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

---

## Catatan urutan

Dua kegagalan di CI kamu **independen**. Log menampilkan keduanya karena
job berjalan paralel:

1. `rialo-build` gagal → error tipe di `rexo-core` (kode kita)
2. `cargo build` gagal → `zerocopy` (dependency)

Memperbaiki satu tidak memperbaiki yang lain. Kerjakan keduanya.
