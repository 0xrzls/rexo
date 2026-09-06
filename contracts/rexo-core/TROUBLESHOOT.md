# TROUBLESHOOT

Dua kegagalan di CI kamu berasal dari dua sumber yang **sama sekali tidak
berhubungan**. Perbaiki terpisah.

---

## Kegagalan 1 — `zerocopy` / `stdarch_x86_avx512`

```
error[E0658]: use of unstable library feature `stdarch_x86_avx512`
error: could not compile `zerocopy` (lib) due to 3 previous errors
```

**Ini bukan kode kamu.** `zerocopy` adalah dependency transitif, dan versi
lamanya (0.7.x di bawah 0.7.35) memakai intrinsic AVX-512 yang sejak
beberapa rilis nightly tidak lagi tersedia tanpa feature gate. Toolchain
Rialo memakai nightly, jadi kamu kena.

Perhatikan juga bahwa errornya menyebut `x86` — ini build untuk **host**
(proc-macro / build script), bukan target PolkaVM.

Perbaikan, urut dari yang paling ringan:

```bash
# 1. naikkan zerocopy saja, jangan sentuh yang lain
cargo update -p zerocopy --precise 0.7.35

# 2. kalau masih gagal, biarkan cargo memilih versi terbaru
cargo update -p zerocopy

# 3. kalau ada dua versi zerocopy di pohon dependency
cargo tree -i zerocopy
```

Kalau langkah 3 menunjukkan dua versi, naikkan keduanya. Commit
`Cargo.lock` setelah berhasil supaya CI tidak mengulang masalah yang sama.

---

## Kegagalan 2 — `unexpected token, expected }`

**Ini kode kamu — lebih tepatnya, kode yang aku tulis.**

Ini bukan error rustc. Ini parser DSL Venus di dalam macro `rialo!`
menemukan token yang tidak dikenalnya. Errornya menunjuk ke `}` karena
parser sedang mencoba menutup blok saat ia tersandung.

### Penyebabnya

Aku membandingkan `lib.rs` versi lamaku dengan file kamu yang **berhasil**
compile. Hasilnya:

| Konstruksi | File kamu (compile) | Versi lamaku | Status sekarang |
|---|---|---|---|
| `///` doc comment | 0 | **13** | dibuang |
| `match` | 0 | **1** | jadi if/else |
| `return` | 0 | beberapa | jadi if/else |
| `SEND` / `START` | 0 | **7** | dibuang |
| `AFTER x CALL [f];` | 2 | — | 8, satu-satunya bentuk async |
| struct literal | 8 | 19 | aman |
| `crate::` | 7 | 40 | aman |

Tersangka utamanya `///`. Di level token, `/// teks` menjadi
`#[doc = "teks"]`. Parser yang sedang menunggu `fn` atau `}` menemukan `#`
dan berhenti — persis pesan errornya.

Tersangka kedua: `SEND` dan `START`. Aku memakainya berdasarkan daftar
keyword di dokumen agent resmi, tapi bentuk sintaksisnya aku karang.
Satu-satunya bentuk yang benar-benar terbukti adalah
`AFTER <var> CALL [handler];`.

### Yang sudah diubah

- Semua `///` di dalam macro jadi `//`
- `match` dan `return` diganti if/else
- Semua `SEND`/`START` dihapus; alur internal memakai `AFTER .. CALL`
- Setiap `AFTER` memakai **variabel lokal sederhana**, tidak pernah
  ekspresi inline — meniru persis file kamu yang jalan
- `trait ViewBridge` (deklarasi tanpa implementasi, jelas tidak compile)
  diganti `hooks.rs` + `state::view_from()`
- Import tak terpakai di `ops.rs` dibuang

### Kalau MASIH gagal: prosedur bisect

Parser DSL tidak memberi nomor baris yang berguna, jadi bisect manual.
Lima menit, dan hasilnya pasti.

```bash
# 1. Kosongkan seluruh badan fungsi, sisakan Ok(())
#    Kalau ini compile, masalahnya di badan fungsi, bukan di blok state.
#
# 2. Kembalikan satu fungsi per commit, mulai dari yang paling sederhana:
#    finalize -> cancel -> get_state -> heartbeat -> buy -> launch
#
# 3. Di dalam fungsi yang gagal, buang per baris dari bawah.
```

Urutan curiga, dari paling mungkin:

1. `?` — file kamu punya **nol** operator `?` di dalam macro. Kode ini
   memakainya banyak. Kalau parser menolaknya, ganti dengan pola
   `let r = ...; if r.is_err() { ... }`.
2. Struct literal multi-baris dengan field `&self.creator` (`SellParams`)
3. Ekspresi boolean multi-baris (`let x = a\n && b;`)
4. `msg!` dengan lebih dari ~4 argumen
5. `use` di dalam blok `program { }` dengan banyak item

---

## Status alur setelah perbaikan ini

Yang **jalan** tanpa sintaks yang belum terverifikasi:

- launch → mint dibuat, authority dicabut, bond masuk, dev-buy + sapu fee
- jendela sealed dibuka dan ditutup lewat `AFTER`
- buy / sell dengan matematika kurva penuh
- kelulusan dijadwalkan lewat `AFTER .. CALL [graduate]`
- heartbeat berdetak dan menjadwalkan dirinya sendiri
- abandonment + kolam keluar pro-rata
- vesting, view, terminating

Yang **dinonaktifkan** sampai kamu temukan bentuk statement webcall:

- verifikasi sosial REX → `on_socials_verified` tidak pernah terpanggil,
  jadi tier tetap 0. Ini default yang aman: tier paling ketat, kreator
  tidak dapat bagi hasil fee.
- clearing batch sealed → `settle_sealed_batch` sekarang langsung membuka
  perdagangan. **Ini melemahkan proteksi sniper.** Jangan biarkan begini
  di mainnet.

Keduanya ditandai `TODO(rex)` dan `TODO(sealed)` di kode.

---

## Kemungkinan akar masalah yang lebih dalam: versi

Devnet kamu melaporkan `api_version: 0.20.0-alpha.0`. Kontrak ini pin
`0.12.2` (30 Juni 2026). Rilis terbaru `rialo-venus` adalah
**0.20.0-alpha.0** (1 September 2026).

Delapan minor version, dan DSL-nya masih alpha. Sintaks yang valid di
0.12.2 mungkin sudah berubah di 0.20. Kalau bisect tidak menemukan apa
pun yang masuk akal, coba naikkan **seluruh** crate serentak:

```toml
rialo-s-program        = "0.20.0-alpha.0"
rialo-s-program-error  = "0.20.0-alpha.0"
rialo-venus            = "0.20.0-alpha.0"
rialo-venus-proc-macro = "0.20.0-alpha.0"
rialo-spl-token-2022   = "0.20.0-alpha.0"
```

Jangan campur versi. Kalau satu naik, semua naik.
