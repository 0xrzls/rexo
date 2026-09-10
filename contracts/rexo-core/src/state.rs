// Copyright (c) 2026 Rexo
// SPDX-License-Identifier: Apache-2.0

//! Jembatan antara state workflow Venus (skalar datar, u64) dan tipe kaya
//! di `curve.rs` (u128).
//!
//! # Kenapa dua representasi
//!
//! State workflow diserialisasi dengan bincode+serde (terkonfirmasi dari
//! source `rialo-venus::write_to_storage`). Field skalar adalah bentuk
//! paling aman dan paling stabil layout-nya.
//!
//! Tapi matematika kurva WAJIB u128:
//!
//! ```text
//! k = virtual_quote * virtual_token
//!   = 30_000_000_000 * 1_073_000_000_000_000
//!   = 32_190_000_000_000_000_000_000_000
//! u64::MAX =    18_446_744_073_709_551_615
//! ```
//!
//! `k` 1.745.023x lebih besar dari kapasitas u64. Hitung kurva pakai u64
//! dan ia overflow di perkalian pertama, sebelum satu trade pun terjadi.
//!
//! Menyimpan state tetap u64 aman karena tiap field terbatas:
//!
//! | field           | maksimum          | muat u64 |
//! |-----------------|-------------------|----------|
//! | virtual_quote   |     115.005.359.057 | ya     |
//! | virtual_token   | 1.073.000.000.000.000 | ya   |
//! | real_quote      |      85.005.359.057 | ya     |
//! | real_token      |   793.100.000.000.000 | ya   |
//!
//! Yang tidak muat hanya hasil antara `k`, dan itu tidak pernah disimpan.

use crate::constants::*;
use crate::curve::{CurveConfig, CurveState, LaunchTier};
use crate::errors::RexoError;

/// Konversi u128 -> u64 yang menolak diam-diam kehilangan data.
///
/// Jangan pakai `as u64`. Kalau invariant kita pernah bocor, `as` akan
/// membungkus nilainya tanpa suara dan merusak akuntansi tanpa jejak.
#[inline]
pub fn narrow(v: u128) -> Result<u64, RexoError> {
    u64::try_from(v).map_err(|_| RexoError::MathOverflow)
}

/// Pandangan bertipe atas state peluncuran.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaunchView {
    pub tier: u8,
    pub status: u8,
    pub virtual_quote: u64,
    pub virtual_token: u64,
    pub real_quote: u64,
    pub real_token: u64,
    pub fees_protocol: u64,
    pub fees_creator: u64,
    pub forfeited_quote: u64,
}

impl LaunchView {
    /// Konfigurasi kurva untuk tier ini.
    ///
    /// `virtual_quote` / `virtual_token` di sini adalah nilai GENESIS, bukan
    /// state hidup. `curve.rs` hanya memakai keduanya di `new()` dan
    /// `validate()` — `buy`/`sell` memakai state, bukan config. Memasukkan
    /// nilai hidup ke sini akan membuat `validate()` gagal begitu kurva
    /// terkuras (virtual_token turun di bawah curve_supply).
    pub fn config(&self) -> CurveConfig {
        CurveConfig {
            virtual_quote: INITIAL_VIRTUAL_QUOTE as u128,
            virtual_token: INITIAL_VIRTUAL_TOKEN as u128,
            curve_supply: INITIAL_REAL_TOKEN as u128,
            lp_reserve: LP_RESERVE as u128,
            tier: tier_from_u8(self.tier),
        }
    }

    pub fn curve(&self) -> CurveState {
        CurveState {
            virtual_quote: self.virtual_quote as u128,
            virtual_token: self.virtual_token as u128,
            real_quote: self.real_quote as u128,
            real_token: self.real_token as u128,
            fees_protocol: self.fees_protocol as u128,
            fees_creator: self.fees_creator as u128,
            forfeited_quote: self.forfeited_quote as u128,
            complete: self.status == STATUS_GRADUATED
                || self.status == STATUS_FINALIZED,
        }
    }

    /// Tulis balik hasil operasi kurva. Mengembalikan Err kalau ada nilai
    /// yang tidak muat u64 — itu berarti invariant bocor dan kita HARUS
    /// membatalkan transaksi, bukan memotong nilainya.
    pub fn apply(&mut self, st: &CurveState) -> Result<(), RexoError> {
        self.virtual_quote = narrow(st.virtual_quote)?;
        self.virtual_token = narrow(st.virtual_token)?;
        self.real_quote = narrow(st.real_quote)?;
        self.real_token = narrow(st.real_token)?;
        self.fees_protocol = narrow(st.fees_protocol)?;
        self.fees_creator = narrow(st.fees_creator)?;
        self.forfeited_quote = narrow(st.forfeited_quote)?;
        Ok(())
    }

    pub fn genesis(tier: u8) -> Self {
        Self {
            tier,
            status: STATUS_SEALED,
            virtual_quote: INITIAL_VIRTUAL_QUOTE,
            virtual_token: INITIAL_VIRTUAL_TOKEN,
            real_quote: 0,
            real_token: INITIAL_REAL_TOKEN,
            fees_protocol: 0,
            fees_creator: 0,
            forfeited_quote: 0,
        }
    }
}

pub fn tier_from_u8(t: u8) -> LaunchTier {
    match t {
        TIER_COMMITTED => LaunchTier::Committed,
        TIER_VERIFIED => LaunchTier::Verified,
        _ => LaunchTier::Unverified,
    }
}

pub fn tier_to_u8(t: LaunchTier) -> u8 {
    match t {
        LaunchTier::Committed => TIER_COMMITTED,
        LaunchTier::Verified => TIER_VERIFIED,
        LaunchTier::Unverified => TIER_UNVERIFIED,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn narrow_rejects_overflow_instead_of_truncating() {
        assert_eq!(narrow(u64::MAX as u128).unwrap(), u64::MAX);
        assert_eq!(narrow(u64::MAX as u128 + 1), Err(RexoError::MathOverflow));
        // k tidak akan pernah muat — inilah alasan modul ini ada
        let k = INITIAL_VIRTUAL_QUOTE as u128 * INITIAL_VIRTUAL_TOKEN as u128;
        assert_eq!(narrow(k), Err(RexoError::MathOverflow));
    }

    #[test]
    fn genesis_round_trips_through_curve() {
        let v = LaunchView::genesis(TIER_COMMITTED);
        let cfg = v.config();
        let st = v.curve();
        assert_eq!(st.real_token, INITIAL_REAL_TOKEN as u128);
        assert_eq!(st.progress_bps(&cfg), 0);
        assert!(!st.complete);
    }

    #[test]
    fn full_buy_cycle_stays_within_u64() {
        let mut v = LaunchView::genesis(TIER_COMMITTED);
        let cfg = v.config();
        let mut st = v.curve();
        // beli sampai lulus; setiap langkah harus tetap muat u64
        st.buy(&cfg, 200 * ONE_RLO as u128, 0).unwrap();
        v.apply(&st).expect("nilai harus tetap muat u64");
        assert_eq!(v.real_token, 0);
        assert_eq!(v.real_quote, 85_005_359_057);
    }

    #[test]
    fn tier_mapping_is_total_and_defaults_safe() {
        assert_eq!(tier_from_u8(0), LaunchTier::Unverified);
        assert_eq!(tier_from_u8(1), LaunchTier::Verified);
        assert_eq!(tier_from_u8(2), LaunchTier::Committed);
        // nilai tak dikenal jatuh ke tier PALING KETAT, bukan paling longgar
        assert_eq!(tier_from_u8(99), LaunchTier::Unverified);
        assert_eq!(tier_from_u8(255), LaunchTier::Unverified);
    }
}

/// Bangun `LaunchView` dari field skalar.
///
/// Sengaja fungsi bebas dengan parameter posisional, bukan method di atas
/// tipe state hasil macro. Alasannya: badan fungsi di dalam `rialo! { }`
/// hanya boleh memakai konstruksi yang terbukti diterima parser DSL —
/// `let`, penugasan field, `if`, pemanggilan fungsi, dan `msg!`. Menaruh
/// `impl` atau method biasa di dalam blok `program { }` belum terverifikasi.
#[allow(clippy::too_many_arguments)]
pub fn view_from(
    tier: u8,
    status: u8,
    virtual_quote: u64,
    virtual_token: u64,
    real_quote: u64,
    real_token: u64,
    fees_protocol: u64,
    fees_creator: u64,
    forfeited_quote: u64,
) -> LaunchView {
    LaunchView {
        tier,
        status,
        virtual_quote,
        virtual_token,
        real_quote,
        real_token,
        fees_protocol,
        fees_creator,
        forfeited_quote,
    }
}
