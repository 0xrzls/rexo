//! # Rexo Core — Kontrol Akses, Status, & Circuit Breaker Pengaman
//!
//! Menegakkan invariant on-chain: anti-self-assign tier, validasi bond, dan circuit breaker abandonment.

use rialo_s_program::account_info::AccountInfo;

use crate::constants::*;
use crate::errors::RexoError;

/// Memastikan akun telah menandatangani transaksi
pub fn assert_signer(account: &AccountInfo<'_>) -> Result<(), RexoError> {
    if !account.is_signer {
        return Err(RexoError::MissingSignature);
    }
    Ok(())
}

/// Menolak pembuatan token yang mengklaim tier di atas Unverified dari input kreator.
/// Tier Community, Verified Dev, atau REX Guaranteed HANYA bisa dicapai melalui attestation verifikasi on-chain.
pub fn assert_tier_not_self_assigned(requested_tier: u8) -> Result<(), RexoError> {
    if requested_tier != TIER_UNVERIFIED {
        return Err(RexoError::SelfAssignedTierForbidden);
    }
    Ok(())
}

/// Memastikan bond memenuhi batas minimum tier
pub fn assert_bond_meets_tier_requirement(tier: u8, bond_kelvins: u64) -> Result<(), RexoError> {
    let min_bond = match tier {
        TIER_UNVERIFIED => BOND_MIN_UNVERIFIED,
        TIER_COMMUNITY => BOND_MIN_COMMUNITY,
        TIER_VERIFIED_DEV => BOND_MIN_VERIFIED_DEV,
        TIER_REX_GUARANTEED => BOND_MIN_REX_GUARANTEED,
        _ => return Err(RexoError::InvalidAccountData),
    };

    if bond_kelvins < min_bond {
        return Err(RexoError::BondBelowMinimum);
    }
    Ok(())
}

/// Memastikan kurva berstatus aktif untuk dapat diperdagangkan
pub fn assert_active(status: u8) -> Result<(), RexoError> {
    if status != STATUS_ACTIVE {
        return Err(RexoError::InvalidStatus);
    }
    Ok(())
}

/// Memastikan kurva sudah berstatus graduated
pub fn assert_graduated(status: u8) -> Result<(), RexoError> {
    if status != STATUS_GRADUATED {
        return Err(RexoError::CurveNotComplete);
    }
    Ok(())
}

/// Circuit Breaker: Mengecek apakah abandonment diizinkan berdasarkan statistik kegagalan sistemik.
/// Jika kegagalan global di atas ambang batas (misal API Telegram/X sedang down massal),
/// protokol membekukan likuidasi penalti agar pengguna tidak teraniaya.
pub fn abandonment_permitted(
    failed_heartbeats: u64,
    global_failures: u64,
    global_total: u64,
) -> Result<bool, RexoError> {
    if failed_heartbeats < MAX_HEARTBEAT_MISSES {
        return Ok(false);
    }

    if global_total == 0 {
        return Ok(true);
    }

    // Jika rasio kegagalan jaringan melebihi 40%, ada kemungkinan downtime API eksternal
    let failure_rate_bps = (global_failures as u128 * 10_000) / (global_total as u128);
    if failure_rate_bps > 4_000 {
        return Err(RexoError::AbandonmentDisallowed);
    }

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_assert_tier_not_self_assigned() {
        assert!(assert_tier_not_self_assigned(TIER_UNVERIFIED).is_ok());
        assert_eq!(
            assert_tier_not_self_assigned(TIER_COMMUNITY),
            Err(RexoError::SelfAssignedTierForbidden)
        );
        assert_eq!(
            assert_tier_not_self_assigned(TIER_VERIFIED_DEV),
            Err(RexoError::SelfAssignedTierForbidden)
        );
    }

    #[test]
    fn test_assert_bond_requirements() {
        assert!(assert_bond_meets_tier_requirement(TIER_UNVERIFIED, 0).is_ok());
        assert!(assert_bond_meets_tier_requirement(TIER_COMMUNITY, 500_000_000).is_ok());
        assert_eq!(
            assert_bond_meets_tier_requirement(TIER_COMMUNITY, 499_999_999),
            Err(RexoError::BondBelowMinimum)
        );
        assert!(assert_bond_meets_tier_requirement(TIER_VERIFIED_DEV, 2_000_000_000).is_ok());
    }

    #[test]
    fn test_assert_active() {
        assert!(assert_active(STATUS_ACTIVE).is_ok());
        assert_eq!(assert_active(STATUS_UNINITIALIZED), Err(RexoError::InvalidStatus));
        assert_eq!(assert_active(STATUS_GRADUATED), Err(RexoError::InvalidStatus));
    }

    #[test]
    fn test_assert_graduated() {
        assert!(assert_graduated(STATUS_GRADUATED).is_ok());
        assert_eq!(assert_graduated(STATUS_ACTIVE), Err(RexoError::CurveNotComplete));
    }

    #[test]
    fn test_abandonment_circuit_breaker_allows_normal_case() {
        // 3 misses, 2% global failure rate -> Diizinkan
        let res = abandonment_permitted(3, 2, 100);
        assert_eq!(res, Ok(true));
    }

    #[test]
    fn test_abandonment_circuit_breaker_blocks_during_systemic_outage() {
        // 3 misses, tapi 50% kegagalan global -> Circuit breaker aktif menolak abandonment
        let res = abandonment_permitted(3, 50, 100);
        assert_eq!(res, Err(RexoError::AbandonmentDisallowed));
    }
}
