// Copyright (c) 2026 Rexo
// SPDX-License-Identifier: Apache-2.0

//! State peluncuran.
//!
//! # Snapshot config, bukan referensi
//!
//! `Launch` menyimpan SALINAN parameter kurva dan fee dari `LaunchConfig`
//! saat peluncuran dibuat. Partner boleh memperbarui config-nya kapan saja,
//! dan peluncuran yang sudah jalan tidak boleh ikut berubah — kalau ikut,
//! harga historis tidak bisa direproduksi dan trader bisa dirugikan oleh
//! perubahan yang tidak mereka setujui.
//!
//! LaunchLab dan DBC melakukan hal yang sama: pool memegang parameternya
//! sendiri, config hanya cetakan.
//!
//! # Kenapa u64 di state tapi u128 di matematika
//!
//! ```text
//! k = virtual_quote * virtual_token
//!   = 30_000_000_000 * 1_073_000_000_000_000
//!   = 32_190_000_000_000_000_000_000_000
//! u64::MAX =    18_446_744_073_709_551_615
//! ```
//!
//! `k` 1.745.023x lebih besar dari kapasitas u64. Hitung kurva pakai u64
//! dan ia meluap di perkalian pertama. Setiap field yang DISIMPAN muat
//! u64; hanya hasil antara yang tidak, dan itu tidak pernah disimpan.

use crate::config::{FeeSplit, LaunchConfig, VestingSchedule};
use crate::curve::{CurveConfig, CurveState};
use crate::errors::RexoError;
use crate::fees::FeeLedger;

// ---------------------------------------------------------------------------
// Lifecycle
// ---------------------------------------------------------------------------

/// Tiga keadaan, mengikuti LaunchLab (`is_funding` / `is_migrate` /
/// `is_trading`). v1 punya enam dan dua di antaranya tidak pernah muncul
/// dari chain.
pub const STATE_FUNDING: u8 = 0;
/// Kurva habis, migrasi dijadwalkan lewat `AFTER`.
pub const STATE_MIGRATING: u8 = 1;
/// Migrasi selesai. Perdagangan pindah ke pool.
pub const STATE_MIGRATED: u8 = 2;

#[inline]
pub fn narrow(v: u128) -> Result<u64, RexoError> {
    u64::try_from(v).map_err(|_| RexoError::MathOverflow)
}

// ---------------------------------------------------------------------------
// Launch
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Launch {
    pub state: u8,

    // -- snapshot config, immutable setelah dibuat --
    pub cfg_virtual_quote: u64,
    pub cfg_virtual_token: u64,
    pub cfg_total_base_sell: u64,
    pub cfg_lp_reserve: u64,
    pub fees: FeeSplit,
    pub vesting: VestingSchedule,
    pub migrate_target: u8,
    pub migrate_delay_secs: u64,

    // -- reserve hidup --
    pub virtual_quote: u64,
    pub virtual_token: u64,
    /// Quote nyata di vault yang menjadi milik kurva (di luar fee).
    pub real_quote: u64,
    /// Token yang masih tersedia untuk dijual.
    pub real_token: u64,

    // -- fee --
    pub ledger: FeeLedger,

    // -- kreator --
    pub creator_allocation: u64,
    pub creator_claimed: u64,

    // -- waktu --
    pub created_at: u64,
    pub migrated_at: u64,
}

impl Launch {
    pub fn open(cfg: &LaunchConfig, now: u64) -> Result<Self, RexoError> {
        cfg.validate()?;
        Ok(Self {
            state: STATE_FUNDING,
            cfg_virtual_quote: cfg.virtual_quote,
            cfg_virtual_token: cfg.virtual_token,
            cfg_total_base_sell: cfg.total_base_sell,
            cfg_lp_reserve: cfg.lp_reserve,
            fees: cfg.fees,
            vesting: cfg.vesting,
            migrate_target: cfg.migrate_target,
            migrate_delay_secs: cfg.migrate_delay_secs,
            virtual_quote: cfg.virtual_quote,
            virtual_token: cfg.virtual_token,
            real_quote: 0,
            real_token: cfg.total_base_sell,
            ledger: FeeLedger::default(),
            creator_allocation: 0,
            creator_claimed: 0,
            created_at: now,
            migrated_at: 0,
        })
    }

    /// Konfigurasi kurva untuk modul matematika.
    ///
    /// `virtual_*` di sini nilai GENESIS, bukan reserve hidup. `curve.rs`
    /// hanya memakai keduanya di `new()` dan `validate()`; `buy`/`sell`
    /// memakai state. Memasukkan nilai hidup akan membuat `validate()`
    /// gagal begitu kurva terkuras.
    pub fn curve_config(&self) -> CurveConfig {
        CurveConfig {
            virtual_quote: self.cfg_virtual_quote as u128,
            virtual_token: self.cfg_virtual_token as u128,
            curve_supply: self.cfg_total_base_sell as u128,
            lp_reserve: self.cfg_lp_reserve as u128,
            fee_bps: self.fees.total_bps as u128,
        }
    }

    pub fn curve(&self) -> CurveState {
        CurveState {
            virtual_quote: self.virtual_quote as u128,
            virtual_token: self.virtual_token as u128,
            real_quote: self.real_quote as u128,
            real_token: self.real_token as u128,
            forfeited_quote: 0,
            complete: self.real_token == 0,
        }
    }

    /// Tulis balik hasil operasi kurva. Menolak, bukan memotong, kalau ada
    /// nilai yang tidak muat u64 — itu berarti invariant bocor dan
    /// transaksi HARUS dibatalkan.
    pub fn apply(&mut self, c: &CurveState) -> Result<(), RexoError> {
        self.virtual_quote = narrow(c.virtual_quote)?;
        self.virtual_token = narrow(c.virtual_token)?;
        self.real_quote = narrow(c.real_quote)?;
        self.real_token = narrow(c.real_token)?;
        Ok(())
    }

    pub fn is_funding(&self) -> bool {
        self.state == STATE_FUNDING
    }
    pub fn is_migrating(&self) -> bool {
        self.state == STATE_MIGRATING
    }
    pub fn is_migrated(&self) -> bool {
        self.state == STATE_MIGRATED
    }

    /// Saldo vault quote yang seharusnya, dipakai untuk memeriksa
    /// invariant: reserve kurva + fee yang belum ditarik.
    pub fn expected_quote_balance(&self) -> u64 {
        self.real_quote + self.ledger.outstanding()
    }

    /// Berapa alokasi kreator yang sudah bisa diklaim sekarang.
    pub fn creator_claimable(&self, now: u64) -> u64 {
        if self.creator_allocation == 0 {
            return 0;
        }
        // Vesting baru berjalan setelah migrasi. Sebelum itu, tidak ada
        // yang cair — kreator ikut menanggung risiko sampai kurva selesai.
        if !self.is_migrated() {
            return 0;
        }
        let elapsed = now.saturating_sub(self.migrated_at);
        let unlocked = self.vesting.unlocked(self.creator_allocation, elapsed);
        unlocked.saturating_sub(self.creator_claimed)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ONE_QUOTE, ONE_TOKEN};

    fn open() -> Launch {
        Launch::open(&LaunchConfig::pumpfun_like(), 1_000).unwrap()
    }

    #[test]
    fn narrow_rejects_overflow_instead_of_truncating() {
        assert_eq!(narrow(u64::MAX as u128).unwrap(), u64::MAX);
        assert_eq!(narrow(u64::MAX as u128 + 1), Err(RexoError::MathOverflow));
        let k = 30_000_000_000u128 * 1_073_000_000_000_000u128;
        assert_eq!(narrow(k), Err(RexoError::MathOverflow));
    }

    #[test]
    fn opens_in_funding_with_full_supply() {
        let l = open();
        assert!(l.is_funding());
        assert_eq!(l.real_token, 793_100_000 * ONE_TOKEN);
        assert_eq!(l.real_quote, 0);
        assert_eq!(l.ledger.outstanding(), 0);
    }

    #[test]
    fn full_drain_stays_within_u64() {
        let mut l = open();
        let cfg = l.curve_config();
        let mut c = l.curve();
        c.buy(&cfg, 200 * ONE_QUOTE as u128, 0).unwrap();
        l.apply(&c).expect("harus tetap muat u64");
        assert_eq!(l.real_token, 0);
        assert_eq!(l.real_quote, 85_005_359_057);
    }

    #[test]
    fn snapshot_is_independent_of_later_config_changes() {
        let mut cfg = LaunchConfig::pumpfun_like();
        let l = Launch::open(&cfg, 0).unwrap();
        let before = l.curve_config();
        // partner mengubah config setelahnya
        cfg.max_creator_buy_bps = 100;
        cfg.fees.creator_bps = 0;
        cfg.fees.protocol_bps = 60;
        // peluncuran yang sudah jalan tidak ikut berubah
        assert_eq!(l.curve_config(), before);
        assert_eq!(l.fees.creator_bps, 20);
    }

    #[test]
    fn vault_invariant_tracks_reserve_plus_unclaimed_fees() {
        let mut l = open();
        l.real_quote = 1_000;
        l.ledger.protocol = 7;
        l.ledger.partner = 5;
        l.ledger.creator = 3;
        assert_eq!(l.expected_quote_balance(), 1_015);
    }

    #[test]
    fn creator_cannot_claim_before_migration() {
        let mut l = open();
        l.creator_allocation = 1_000;
        assert_eq!(l.creator_claimable(999_999), 0, "belum migrasi");
        l.state = STATE_MIGRATED;
        l.migrated_at = 100;
        // tanpa vesting -> semuanya cair begitu migrasi
        assert_eq!(l.creator_claimable(100), 1_000);
    }

    #[test]
    fn creator_claim_is_not_double_counted() {
        let mut l = open();
        l.creator_allocation = 1_000;
        l.state = STATE_MIGRATED;
        l.migrated_at = 0;
        assert_eq!(l.creator_claimable(0), 1_000);
        l.creator_claimed = 400;
        assert_eq!(l.creator_claimable(0), 600);
        l.creator_claimed = 1_000;
        assert_eq!(l.creator_claimable(0), 0);
    }

    #[test]
    fn vesting_gates_creator_claims_over_time() {
        let mut cfg = LaunchConfig::pumpfun_like();
        cfg.vesting = VestingSchedule {
            vested_bps: 7_500,
            cliff_secs: 3_600,
            duration_secs: 86_400,
        };
        let mut l = Launch::open(&cfg, 0).unwrap();
        l.creator_allocation = 4_000;
        l.state = STATE_MIGRATED;
        l.migrated_at = 0;

        assert_eq!(l.creator_claimable(0), 1_000); // 25% langsung
        assert_eq!(l.creator_claimable(3_599), 1_000); // masih cliff
        assert_eq!(l.creator_claimable(3_600 + 43_200), 2_500); // separuh
        assert_eq!(l.creator_claimable(3_600 + 86_400), 4_000); // selesai
        assert_eq!(l.creator_claimable(999_999), 4_000); // tidak lebih
    }
}
