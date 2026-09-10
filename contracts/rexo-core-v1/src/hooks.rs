// Copyright (c) 2026 Rexo
// SPDX-License-Identifier: Apache-2.0

//! Kait ke data lintas-peluncuran.
//!
//! Modul ini menggantikan `trait ViewBridge` di versi sebelumnya, yang
//! dideklarasikan tanpa implementasi dan karena itu tidak bisa compile.

/// Statistik kegagalan verifikasi lintas token untuk pengaman kegagalan
/// terkorelasi.
///
/// Mengembalikan `(kegagalan, pemeriksaan)` pada siklus heartbeat terakhir.
///
/// # PERINGATAN — GANTI SEBELUM MAINNET
///
/// Nilai default `(0, 1)` berarti "0% gagal", yang **mengizinkan**
/// abandonment. Default itu justru yang berbahaya: kalau Telegram API mati
/// enam jam, seluruh token akan ditandai ditinggalkan sekaligus dan bond
/// mereka hangus. Itu merusak pengguna tak bersalah dalam skala besar dan
/// tidak bisa dibatalkan.
///
/// Implementasi yang benar membaca akun statistik global yang di-update
/// tiap siklus heartbeat. Sampai itu ada, pertimbangkan mengembalikan
/// `(1, 1)` — yaitu 100% gagal — supaya abandonment selalu DITEKAN dan
/// tidak ada bond yang bisa hangus karena kesalahan.
pub fn global_failure_stats() -> (u64, u64) {
    (0, 1)
}
