// Copyright (c) 2026 Rexo
// SPDX-License-Identifier: Apache-2.0

//! Pemeriksaan otorisasi dan status.
//!
//! Semua guard dikumpulkan di satu modul supaya auditor bisa membaca
//! seluruh permukaan kontrol akses tanpa menelusuri seluruh program.

use rialo_s_program::{account_info::AccountInfo, pubkey::Pubkey};

use crate::constants::*;
use crate::errors::RexoError;

type Guard = Result<(), RexoError>;

// ---------------------------------------------------------------------------
// Status
// ---------------------------------------------------------------------------

pub fn require_active(status: u8) -> Guard {
    match status {
        STATUS_ACTIVE => Ok(()),
        STATUS_SEALED => Err(RexoError::StillSealed),
        STATUS_GRADUATED | STATUS_FINALIZED => Err(RexoError::AlreadyGraduated),
        STATUS_ABANDONED => Err(RexoError::Abandoned),
        STATUS_UNINITIALIZED => Err(RexoError::NotInitialized),
        _ => Err(RexoError::WrongStatus),
    }
}

/// Guard untuk JALUR KELUAR (`sell`).
///
/// Sengaja lebih longgar dari `require_active`: menjual diizinkan juga saat
/// status ABANDONED.
///
/// Alasannya penting. Kalau `sell` memakai `require_active`, maka begitu
/// sebuah token ditandai ditinggalkan, pemegangnya TERKUNCI — tidak bisa
/// keluar sama sekali. Itu menghukum korban, bukan pelaku, dan persis
/// kebalikan dari maksud mekanisme abandonment.
///
/// Membeli tetap dilarang setelah abandonment. Keluar boleh, masuk tidak.
pub fn require_exitable(status: u8) -> Guard {
    match status {
        STATUS_ACTIVE | STATUS_ABANDONED => Ok(()),
        STATUS_SEALED => Err(RexoError::StillSealed),
        STATUS_GRADUATED | STATUS_FINALIZED => Err(RexoError::AlreadyGraduated),
        STATUS_UNINITIALIZED => Err(RexoError::NotInitialized),
        _ => Err(RexoError::WrongStatus),
    }
}

pub fn require_sealed(status: u8) -> Guard {
    if status == STATUS_SEALED {
        Ok(())
    } else {
        Err(RexoError::NotSealed)
    }
}

pub fn require_graduated(status: u8) -> Guard {
    if status == STATUS_GRADUATED {
        Ok(())
    } else {
        Err(RexoError::NotGraduated)
    }
}

pub fn require_uninitialized(status: u8) -> Guard {
    if status == STATUS_UNINITIALIZED {
        Ok(())
    } else {
        Err(RexoError::AlreadyInitialized)
    }
}

/// Jendela sealed sudah lewat menurut jam on-chain.
pub fn sealed_window_closed(now: u64, sealed_until: u64) -> bool {
    now >= sealed_until
}

// ---------------------------------------------------------------------------
// Otorisasi
// ---------------------------------------------------------------------------

pub fn require_signer(account: &AccountInfo<'_>) -> Guard {
    if account.is_signer {
        Ok(())
    } else {
        Err(RexoError::Unauthorized)
    }
}

pub fn require_creator(signer: &Pubkey, creator: &Pubkey) -> Guard {
    if signer == creator {
        Ok(())
    } else {
        Err(RexoError::Unauthorized)
    }
}

/// Tier hanya boleh berasal dari hasil verifikasi REX.
///
/// Kalau tier bisa dinaikkan lewat argumen, seluruh sistem verifikasi
/// runtuh: kreator tinggal mengirim `tier=2` dan mendapat bagi hasil fee
/// serta batas alokasi tertinggi tanpa membuktikan apa pun. Ini temuan A
/// di 04-AUDIT.md.
pub fn assert_tier_not_self_assigned(
    tier_from_caller: Option<u8>,
) -> Guard {
    match tier_from_caller {
        None => Ok(()),
        Some(TIER_UNVERIFIED) => Ok(()), // hanya tier terendah yang boleh
        Some(_) => Err(RexoError::TierSelfAssignment),
    }
}

// ---------------------------------------------------------------------------
// Bond & alokasi kreator
// ---------------------------------------------------------------------------

pub fn require_bond(tier: u8, bond: u64) -> Guard {
    let minimum = MIN_BOND[tier.min(2) as usize];
    if bond >= minimum {
        Ok(())
    } else {
        Err(RexoError::BondTooSmall)
    }
}

pub fn require_creator_allocation(tier: u8, tokens: u64) -> Guard {
    let cap = (INITIAL_REAL_TOKEN as u128 * MAX_CREATOR_BPS[tier.min(2) as usize] as u128
        / BPS_DENOM as u128) as u64;
    if tokens <= cap {
        Ok(())
    } else {
        Err(RexoError::CreatorCapExceeded)
    }
}

/// Apakah alokasi kreator masih terkunci pada progress kurva tertentu.
pub fn creator_locked(tranches_unlocked: u8, progress_bps: u64, graduated: bool) -> bool {
    match tranches_unlocked {
        0 => progress_bps < VEST_TRANCHE_1_PROGRESS_BPS,
        1 => !graduated,
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Exit pool — pembayaran bond yang hangus
// ---------------------------------------------------------------------------

/// Hitung bonus keluar untuk satu penjualan setelah abandonment.
///
/// Bond yang hangus TIDAK bisa menunggu kelulusan — token yang ditinggalkan
/// tidak akan pernah lulus. Jadi bond itu jadi kolam yang dibagikan
/// pro-rata ke pemegang token saat mereka keluar.
///
/// Mengembalikan `(bonus, sisa_pool, sisa_base)`. Pola "kurangi keduanya"
/// membuat pembulatan mengoreksi diri: penjual terakhir menerima sisanya,
/// dan kolam terkuras persis habis tanpa kebocoran.
pub fn exit_bonus(pool: u64, base: u64, tokens_in: u64) -> (u64, u64, u64) {
    if pool == 0 || base == 0 || tokens_in == 0 {
        return (0, pool, base);
    }
    let tokens = tokens_in.min(base);
    // pool * tokens / base, dibulatkan ke bawah
    let bonus = ((pool as u128 * tokens as u128) / base as u128) as u64;
    let bonus = bonus.min(pool);
    (bonus, pool - bonus, base - tokens)
}

// ---------------------------------------------------------------------------
// Pengaman kegagalan terkorelasi
// ---------------------------------------------------------------------------

/// Jangan pernah menghanguskan bond ketika kegagalan verifikasi bersifat
/// sistemik.
///
/// Kalau Telegram API mati enam jam, implementasi naif akan menandai ribuan
/// token sebagai ditinggalkan dan menghanguskan bond mereka semua. Itu
/// merusak pengguna tak bersalah dalam skala besar dan tidak bisa
/// dibatalkan — bug paling berbahaya di seluruh desain ini.
///
/// `global_failures` dan `global_checks` datang dari akumulator global
/// yang di-update tiap siklus heartbeat.
pub fn abandonment_permitted(global_failures: u64, global_checks: u64) -> Guard {
    if global_checks == 0 {
        return Err(RexoError::CorrelatedFailureGuard);
    }
    let rate_bps = global_failures.saturating_mul(BPS_DENOM) / global_checks;
    if rate_bps >= CORRELATED_FAILURE_THRESHOLD_BPS {
        Err(RexoError::CorrelatedFailureGuard)
    } else {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_transitions_are_exhaustive() {
        assert!(require_active(STATUS_ACTIVE).is_ok());
        assert_eq!(require_active(STATUS_SEALED), Err(RexoError::StillSealed));
        assert_eq!(require_active(STATUS_ABANDONED), Err(RexoError::Abandoned));
        assert_eq!(
            require_active(STATUS_GRADUATED),
            Err(RexoError::AlreadyGraduated)
        );
        assert_eq!(
            require_active(STATUS_UNINITIALIZED),
            Err(RexoError::NotInitialized)
        );
        // status tak dikenal ditolak, bukan diizinkan
        assert_eq!(require_active(200), Err(RexoError::WrongStatus));
    }

    #[test]
    fn tier_cannot_be_self_assigned_above_unverified() {
        assert!(assert_tier_not_self_assigned(None).is_ok());
        assert!(assert_tier_not_self_assigned(Some(TIER_UNVERIFIED)).is_ok());
        assert_eq!(
            assert_tier_not_self_assigned(Some(TIER_VERIFIED)),
            Err(RexoError::TierSelfAssignment)
        );
        assert_eq!(
            assert_tier_not_self_assigned(Some(TIER_COMMITTED)),
            Err(RexoError::TierSelfAssignment)
        );
    }

    #[test]
    fn bond_minimums_match_policy() {
        assert!(require_bond(TIER_UNVERIFIED, 0).is_ok());
        assert_eq!(
            require_bond(TIER_COMMITTED, 9 * ONE_RLO),
            Err(RexoError::BondTooSmall)
        );
        assert!(require_bond(TIER_COMMITTED, 10 * ONE_RLO).is_ok());
        // tier di luar rentang di-clamp ke yang paling ketat, tidak panic
        assert!(require_bond(200, 10 * ONE_RLO).is_ok());
    }

    #[test]
    fn creator_allocation_caps() {
        assert!(require_creator_allocation(TIER_UNVERIFIED, 7_931_000_000_000).is_ok());
        assert_eq!(
            require_creator_allocation(TIER_UNVERIFIED, 7_931_000_000_001),
            Err(RexoError::CreatorCapExceeded)
        );
        assert!(require_creator_allocation(TIER_COMMITTED, 39_655_000_000_000).is_ok());
        assert_eq!(
            require_creator_allocation(TIER_COMMITTED, 39_655_000_000_001),
            Err(RexoError::CreatorCapExceeded)
        );
    }

    #[test]
    fn vesting_lock_follows_progress_then_graduation() {
        assert!(creator_locked(0, 2_499, false));
        assert!(!creator_locked(0, 2_500, false));
        assert!(creator_locked(1, 10_000, false));
        assert!(!creator_locked(1, 0, true));
        assert!(!creator_locked(2, 0, false));
    }

    #[test]
    fn holders_can_always_exit_even_after_abandonment() {
        // Ini regression test untuk bug nyata: `sell` dulu memakai
        // require_active, yang mengunci pemegang token begitu ditinggalkan.
        assert!(require_exitable(STATUS_ACTIVE).is_ok());
        assert!(require_exitable(STATUS_ABANDONED).is_ok());
        // tapi MEMBELI token yang ditinggalkan tetap dilarang
        assert_eq!(require_active(STATUS_ABANDONED), Err(RexoError::Abandoned));
        // dan keluar tetap dilarang selama sealed / setelah lulus
        assert_eq!(require_exitable(STATUS_SEALED), Err(RexoError::StillSealed));
        assert_eq!(
            require_exitable(STATUS_GRADUATED),
            Err(RexoError::AlreadyGraduated)
        );
    }

    #[test]
    fn exit_pool_drains_exactly_with_no_leak() {
        // dua penjual, porsi sama
        let (b1, p1, base1) = exit_bonus(1_000, 100, 50);
        assert_eq!((b1, p1, base1), (500, 500, 50));
        let (b2, p2, base2) = exit_bonus(p1, base1, 50);
        assert_eq!((b2, p2, base2), (500, 0, 0));
        assert_eq!(b1 + b2, 1_000, "kolam harus terkuras persis");
    }

    #[test]
    fn exit_pool_handles_awkward_rounding() {
        // 1000 kelvin dibagi 3 token: 333/333/334, tanpa kebocoran
        let mut pool = 1_000u64;
        let mut base = 3u64;
        let mut total = 0u64;
        for _ in 0..3 {
            let (b, p, ba) = exit_bonus(pool, base, 1);
            total += b;
            pool = p;
            base = ba;
        }
        assert_eq!(total, 1_000);
        assert_eq!(pool, 0);
        assert_eq!(base, 0);
    }

    #[test]
    fn exit_pool_is_safe_when_nobody_holds_tokens() {
        // base 0 = tidak ada pemegang; jangan bagi dengan nol
        assert_eq!(exit_bonus(1_000, 0, 10), (0, 1_000, 0));
        assert_eq!(exit_bonus(0, 100, 10), (0, 0, 100));
        // penjualan melebihi base di-clamp, tidak overflow
        let (b, p, ba) = exit_bonus(1_000, 10, 999);
        assert_eq!((b, p, ba), (1_000, 0, 0));
    }

    #[test]
    fn correlated_failures_suppress_abandonment() {
        // 10% gagal: normal, boleh hanguskan
        assert!(abandonment_permitted(10, 100).is_ok());
        // 29.99% masih boleh
        assert!(abandonment_permitted(2_999, 10_000).is_ok());
        // 30% ke atas: ini masalah kita, bukan mereka
        assert_eq!(
            abandonment_permitted(30, 100),
            Err(RexoError::CorrelatedFailureGuard)
        );
        assert_eq!(
            abandonment_permitted(100, 100),
            Err(RexoError::CorrelatedFailureGuard)
        );
        // tanpa data sama sekali: tolak, jangan berasumsi aman
        assert_eq!(
            abandonment_permitted(0, 0),
            Err(RexoError::CorrelatedFailureGuard)
        );
    }
}
