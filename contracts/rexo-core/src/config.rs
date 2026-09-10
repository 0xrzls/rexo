// Copyright (c) 2026 Rexo
// SPDX-License-Identifier: Apache-2.0

//! `LaunchConfig` — seluruh ekonomi peluncuran sebagai data.
//!
//! # Kenapa modul ini menggantikan `constants.rs`
//!
//! Di v1 kurva, fee, dan ambang kelulusan dipaku di kode. Mengubah satu
//! angka berarti deploy ulang program.
//!
//! Raydium LaunchLab dan Meteora DBC menaruh semua itu di akun konfigurasi
//! yang dibuat partner. Akibatnya satu program melayani banyak launchpad:
//! letsbonk.fun bukan program tersendiri, ia satu `PlatformConfig` di atas
//! LaunchLab milik Raydium.
//!
//! Modul ini mengambil pola yang sama.
//!
//! # Yang TIDAK ada di sini
//!
//! Tidak ada field untuk fitur yang belum bisa dieksekusi. Verifikasi
//! sosial, lelang tersegel, dan denyut liveness membutuhkan primitif Rialo
//! yang bentuk sintaksisnya belum terkonfirmasi. Menaruh field-nya sekarang
//! berarti menjanjikan sesuatu yang tidak bisa dinyalakan — persis
//! kesalahan v1. Field-nya ditambahkan ketika primitifnya terbukti.

use crate::errors::RexoError;

pub const BPS_DENOM: u64 = 10_000;
pub const ONE_QUOTE: u64 = 1_000_000_000; // 9 desimal (kelvin)
pub const ONE_TOKEN: u64 = 1_000_000; // 6 desimal

// ---------------------------------------------------------------------------
// Seeds
// ---------------------------------------------------------------------------

/// LaunchConfig milik partner. seeds: [CONFIG_SEED, partner]
pub const CONFIG_SEED: &[u8] = b"rexo_config";
/// State peluncuran. seeds: [LAUNCH_SEED, config, base_mint]
pub const LAUNCH_SEED: &[u8] = b"rexo_launch";
/// Vault token. seeds: [BASE_VAULT_SEED, launch]
pub const BASE_VAULT_SEED: &[u8] = b"rexo_base_vault";
/// Vault quote (RLO). seeds: [QUOTE_VAULT_SEED, launch]
pub const QUOTE_VAULT_SEED: &[u8] = b"rexo_quote_vault";
/// Otoritas mint & penanda tangan vault. seeds: [AUTHORITY_SEED, launch]
pub const AUTHORITY_SEED: &[u8] = b"rexo_authority";
/// Catatan vesting. seeds: [VESTING_SEED, launch, beneficiary]
pub const VESTING_SEED: &[u8] = b"rexo_vesting";

// ---------------------------------------------------------------------------
// Tipe kurva
// ---------------------------------------------------------------------------

pub const CURVE_CONSTANT_PRODUCT: u8 = 0;

// ---------------------------------------------------------------------------
// Target migrasi
// ---------------------------------------------------------------------------

/// Likuiditas ditahan di vault program, siap ditarik integrator.
/// Ini satu-satunya mode yang bisa dieksekusi tanpa alamat program DEX.
pub const MIGRATE_HOLD: u8 = 0;
/// Setor ke pool eksternal. Butuh `migrate_target_program` terisi.
pub const MIGRATE_EXTERNAL: u8 = 1;

// ---------------------------------------------------------------------------
// Pembagian fee
// ---------------------------------------------------------------------------

/// Empat penerima. Jumlahnya harus persis `total_fee_bps`.
///
/// `referral_bps` dibayar langsung saat perdagangan ke alamat yang dikirim
/// pemanggil. Tiga sisanya diakumulasi di ledger dan ditarik terpisah —
/// pola tarik, bukan dorong. Lihat `fees.rs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeeSplit {
    pub total_bps: u64,
    pub protocol_bps: u64,
    pub partner_bps: u64,
    pub creator_bps: u64,
    pub referral_bps: u64,
}

impl FeeSplit {
    pub fn validate(&self) -> Result<(), RexoError> {
        if self.total_bps == 0 || self.total_bps >= BPS_DENOM {
            return Err(RexoError::InvalidConfig);
        }
        let sum = self
            .protocol_bps
            .checked_add(self.partner_bps)
            .and_then(|v| v.checked_add(self.creator_bps))
            .and_then(|v| v.checked_add(self.referral_bps))
            .ok_or(RexoError::MathOverflow)?;
        if sum != self.total_bps {
            return Err(RexoError::FeeSplitMismatch);
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Vesting
// ---------------------------------------------------------------------------

/// Jadwal linear dengan cliff. Nol di semua field berarti tanpa vesting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VestingSchedule {
    /// Porsi alokasi kreator yang divesting, bps. Sisanya langsung cair.
    pub vested_bps: u64,
    /// Detik setelah kelulusan sebelum tranche pertama terbuka.
    pub cliff_secs: u64,
    /// Durasi total pelepasan linear setelah cliff.
    pub duration_secs: u64,
}

impl VestingSchedule {
    pub const NONE: Self = Self {
        vested_bps: 0,
        cliff_secs: 0,
        duration_secs: 0,
    };

    pub fn validate(&self) -> Result<(), RexoError> {
        if self.vested_bps > BPS_DENOM {
            return Err(RexoError::InvalidConfig);
        }
        // Jadwal dengan porsi tapi tanpa durasi akan melepas semuanya
        // sekaligus di detik cliff. Kalau itu yang dimau, pakai cliff saja
        // dan durasi 0 — tapi porsinya harus nol kalau tidak ada keduanya.
        if self.vested_bps > 0 && self.cliff_secs == 0 && self.duration_secs == 0 {
            return Err(RexoError::InvalidConfig);
        }
        Ok(())
    }

    /// Berapa yang sudah terbuka pada `elapsed` detik setelah kelulusan.
    pub fn unlocked(&self, total: u64, elapsed: u64) -> u64 {
        if self.vested_bps == 0 {
            return total;
        }
        let vested = (total as u128 * self.vested_bps as u128 / BPS_DENOM as u128) as u64;
        let immediate = total - vested;
        if elapsed < self.cliff_secs {
            return immediate;
        }
        if self.duration_secs == 0 {
            return total;
        }
        let since = elapsed - self.cliff_secs;
        if since >= self.duration_secs {
            return total;
        }
        immediate + (vested as u128 * since as u128 / self.duration_secs as u128) as u64
    }
}

// ---------------------------------------------------------------------------
// LaunchConfig
// ---------------------------------------------------------------------------

/// Dibuat partner sekali, lalu dipakai berapa pun peluncuran.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaunchConfig {
    // -- kurva --
    pub curve_type: u8,
    /// Reserve virtual awal, menentukan market cap pembukaan.
    pub virtual_quote: u64,
    pub virtual_token: u64,
    /// Token yang dijual lewat kurva.
    pub total_base_sell: u64,
    /// Token yang ditahan untuk likuiditas saat lulus.
    pub lp_reserve: u64,

    // -- fee --
    pub fees: FeeSplit,

    // -- batas kreator --
    /// Batas pembelian kreator saat launch, bps dari `total_base_sell`.
    pub max_creator_buy_bps: u64,

    // -- vesting --
    pub vesting: VestingSchedule,

    // -- migrasi --
    pub migrate_target: u8,
    /// Berapa lama setelah kurva habis sebelum migrasi dijalankan.
    /// Dieksekusi lewat `AFTER n seconds CALL [migrate]` — reaktif, tanpa
    /// keeper. Meteora butuh `dbc-keeper` untuk ini; LaunchLab butuh
    /// seseorang memanggil `migrate_to_amm`.
    pub migrate_delay_secs: u64,
}

impl LaunchConfig {
    /// Preset dengan ekonomi identik pump.fun. Ini nilai DEFAULT untuk
    /// config baru, bukan konstanta yang dipaku di kode.
    pub const fn pumpfun_like() -> Self {
        Self {
            curve_type: CURVE_CONSTANT_PRODUCT,
            virtual_quote: 30 * ONE_QUOTE,
            virtual_token: 1_073_000_000 * ONE_TOKEN,
            total_base_sell: 793_100_000 * ONE_TOKEN,
            lp_reserve: 206_900_000 * ONE_TOKEN,
            fees: FeeSplit {
                total_bps: 100,
                protocol_bps: 40,
                partner_bps: 30,
                creator_bps: 20,
                referral_bps: 10,
            },
            max_creator_buy_bps: 500,
            vesting: VestingSchedule::NONE,
            migrate_target: MIGRATE_HOLD,
            migrate_delay_secs: 1,
        }
    }

    pub fn total_supply(&self) -> u64 {
        self.total_base_sell + self.lp_reserve
    }

    /// Batas pembelian kreator dalam unit token mentah.
    pub fn max_creator_tokens(&self) -> u64 {
        (self.total_base_sell as u128 * self.max_creator_buy_bps as u128
            / BPS_DENOM as u128) as u64
    }

    pub fn validate(&self) -> Result<(), RexoError> {
        if self.curve_type != CURVE_CONSTANT_PRODUCT {
            return Err(RexoError::UnsupportedCurve);
        }
        if self.virtual_quote == 0
            || self.virtual_token == 0
            || self.total_base_sell == 0
            || self.max_creator_buy_bps > BPS_DENOM
        {
            return Err(RexoError::InvalidConfig);
        }
        // Reserve virtual harus melebihi supply kurva, kalau tidak
        // pembagi menjadi nol saat kurva hampir habis.
        if self.virtual_token <= self.total_base_sell {
            return Err(RexoError::InvalidConfig);
        }
        // Supply total harus muat u64 dan k harus muat u128.
        self.total_base_sell
            .checked_add(self.lp_reserve)
            .ok_or(RexoError::MathOverflow)?;
        (self.virtual_quote as u128)
            .checked_mul(self.virtual_token as u128)
            .ok_or(RexoError::MathOverflow)?;

        if self.migrate_target != MIGRATE_HOLD && self.migrate_target != MIGRATE_EXTERNAL {
            return Err(RexoError::InvalidConfig);
        }
        self.fees.validate()?;
        self.vesting.validate()?;
        Ok(())
    }

    /// Field mana yang boleh diubah setelah config dipakai.
    ///
    /// Kurva dan supply TIDAK boleh berubah: peluncuran yang sudah jalan
    /// memegang salinannya, dan mengubahnya di sini akan membuat data
    /// historis tidak bisa direproduksi. Yang boleh berubah hanya yang
    /// berlaku untuk peluncuran BERIKUTNYA.
    pub fn apply_update(&mut self, patch: &ConfigPatch) -> Result<(), RexoError> {
        if let Some(f) = patch.fees {
            f.validate()?;
            self.fees = f;
        }
        if let Some(v) = patch.vesting {
            v.validate()?;
            self.vesting = v;
        }
        if let Some(b) = patch.max_creator_buy_bps {
            if b > BPS_DENOM {
                return Err(RexoError::InvalidConfig);
            }
            self.max_creator_buy_bps = b;
        }
        if let Some(d) = patch.migrate_delay_secs {
            self.migrate_delay_secs = d;
        }
        self.validate()
    }
}

/// Perubahan yang diizinkan pada config yang sudah ada.
#[derive(Debug, Clone, Copy, Default)]
pub struct ConfigPatch {
    pub fees: Option<FeeSplit>,
    pub vesting: Option<VestingSchedule>,
    pub max_creator_buy_bps: Option<u64>,
    pub migrate_delay_secs: Option<u64>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_matches_pumpfun_constants() {
        let c = LaunchConfig::pumpfun_like();
        assert_eq!(c.virtual_token, 1_073_000_000_000_000);
        assert_eq!(c.virtual_quote, 30_000_000_000);
        assert_eq!(c.total_base_sell, 793_100_000_000_000);
        assert_eq!(c.total_supply(), 1_000_000_000_000_000);
        assert!(c.validate().is_ok());
    }

    #[test]
    fn fee_split_must_sum_to_total() {
        let mut c = LaunchConfig::pumpfun_like();
        assert!(c.fees.validate().is_ok());
        c.fees.creator_bps += 1; // 101 != 100
        assert_eq!(c.fees.validate(), Err(RexoError::FeeSplitMismatch));
    }

    #[test]
    fn rejects_curve_that_would_divide_by_zero() {
        let mut c = LaunchConfig::pumpfun_like();
        c.virtual_token = c.total_base_sell;
        assert_eq!(c.validate(), Err(RexoError::InvalidConfig));
    }

    #[test]
    fn rejects_unsupported_curve_type() {
        let mut c = LaunchConfig::pumpfun_like();
        c.curve_type = 2;
        assert_eq!(c.validate(), Err(RexoError::UnsupportedCurve));
    }

    #[test]
    fn creator_cap_matches_bps() {
        let c = LaunchConfig::pumpfun_like();
        assert_eq!(c.max_creator_tokens(), 39_655_000_000_000); // 5%
    }

    #[test]
    fn update_cannot_touch_curve() {
        let mut c = LaunchConfig::pumpfun_like();
        let before = (c.virtual_quote, c.virtual_token, c.total_base_sell);
        c.apply_update(&ConfigPatch {
            max_creator_buy_bps: Some(100),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(c.max_creator_buy_bps, 100);
        assert_eq!((c.virtual_quote, c.virtual_token, c.total_base_sell), before);
    }

    #[test]
    fn update_rejects_invalid_fee_split() {
        let mut c = LaunchConfig::pumpfun_like();
        let bad = FeeSplit {
            total_bps: 100,
            protocol_bps: 50,
            partner_bps: 50,
            creator_bps: 50,
            referral_bps: 0,
        };
        assert_eq!(
            c.apply_update(&ConfigPatch {
                fees: Some(bad),
                ..Default::default()
            }),
            Err(RexoError::FeeSplitMismatch)
        );
    }

    // -- vesting --

    #[test]
    fn no_vesting_releases_everything_immediately() {
        let v = VestingSchedule::NONE;
        assert_eq!(v.unlocked(1_000, 0), 1_000);
    }

    #[test]
    fn vesting_respects_cliff_then_releases_linearly() {
        // 60% divesting, cliff 100 detik, durasi 400 detik
        let v = VestingSchedule {
            vested_bps: 6_000,
            cliff_secs: 100,
            duration_secs: 400,
        };
        assert_eq!(v.unlocked(1_000, 0), 400); // hanya porsi langsung
        assert_eq!(v.unlocked(1_000, 99), 400); // masih di cliff
        assert_eq!(v.unlocked(1_000, 100), 400); // cliff lewat, linear mulai
        assert_eq!(v.unlocked(1_000, 300), 700); // separuh durasi
        assert_eq!(v.unlocked(1_000, 500), 1_000); // selesai
        assert_eq!(v.unlocked(1_000, 9_999), 1_000); // tidak melebihi total
    }

    #[test]
    fn vesting_never_exceeds_total_on_odd_splits() {
        let v = VestingSchedule {
            vested_bps: 3_333,
            cliff_secs: 7,
            duration_secs: 13,
        };
        for t in 0..40u64 {
            let u = v.unlocked(1_000_003, t);
            assert!(u <= 1_000_003, "t={} u={}", t, u);
        }
        assert_eq!(v.unlocked(1_000_003, 20), 1_000_003);
    }

    #[test]
    fn rejects_vesting_portion_without_any_schedule() {
        let v = VestingSchedule {
            vested_bps: 5_000,
            cliff_secs: 0,
            duration_secs: 0,
        };
        assert_eq!(v.validate(), Err(RexoError::InvalidConfig));
    }
}
