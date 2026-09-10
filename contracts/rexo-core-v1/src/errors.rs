// Copyright (c) 2026 Rexo
// SPDX-License-Identifier: Apache-2.0

//! Error domain Rexo.
//!
//! Setiap error punya kode numerik stabil. Jangan pernah menyisipkan varian
//! di tengah enum setelah deploy — kode akan bergeser dan klien yang sudah
//! memetakan kode lama akan salah membaca. Tambahkan di akhir saja.

use rialo_s_program::program_error::ProgramError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum RexoError {
    // -- lifecycle --
    AlreadyInitialized = 100,
    NotInitialized = 101,
    WrongStatus = 102,
    StillSealed = 103,
    NotSealed = 104,
    AlreadyGraduated = 105,
    NotGraduated = 106,
    Abandoned = 107,

    // -- otorisasi --
    Unauthorized = 200,
    CreatorLocked = 201,
    /// Tier dicoba ditetapkan dari luar hasil verifikasi REX.
    TierSelfAssignment = 202,

    // -- kurva --
    ZeroAmount = 300,
    SlippageExceeded = 301,
    ExceedsCirculating = 302,
    CurveComplete = 303,
    MathOverflow = 304,
    InvalidCurveConfig = 305,

    // -- tier & bond --
    CreatorCapExceeded = 400,
    BondTooSmall = 401,
    BondAlreadySettled = 402,

    // -- akun --
    InvalidVault = 500,
    InvalidMintAuthority = 501,
    InvalidMint = 502,
    InsufficientVaultBalance = 503,
    AccountNotRentExempt = 504,

    // -- verifikasi --
    SocialVerificationFailed = 600,
    /// Kegagalan heartbeat terkorelasi lintas token: ini masalah kita,
    /// bukan token yang ditinggalkan. Jangan hanguskan bond.
    CorrelatedFailureGuard = 601,
}

impl From<RexoError> for ProgramError {
    fn from(e: RexoError) -> Self {
        ProgramError::Custom(e as u32)
    }
}

impl core::fmt::Display for RexoError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            RexoError::AlreadyInitialized => "launch already initialized",
            RexoError::NotInitialized => "launch not initialized",
            RexoError::WrongStatus => "operation not allowed in current status",
            RexoError::StillSealed => "sealed window has not closed",
            RexoError::NotSealed => "not in sealed window",
            RexoError::AlreadyGraduated => "curve already graduated",
            RexoError::NotGraduated => "curve has not graduated",
            RexoError::Abandoned => "launch abandoned",
            RexoError::Unauthorized => "signer not authorized",
            RexoError::CreatorLocked => "creator allocation still locked",
            RexoError::TierSelfAssignment => "tier may only be set by REX verification",
            RexoError::ZeroAmount => "amount resolves to zero",
            RexoError::SlippageExceeded => "slippage limit exceeded",
            RexoError::ExceedsCirculating => "exceeds circulating supply",
            RexoError::CurveComplete => "curve complete",
            RexoError::MathOverflow => "arithmetic overflow",
            RexoError::InvalidCurveConfig => "invalid curve config",
            RexoError::CreatorCapExceeded => "creator allocation cap exceeded",
            RexoError::BondTooSmall => "launch bond below tier minimum",
            RexoError::BondAlreadySettled => "bond already returned or forfeited",
            RexoError::InvalidVault => "vault PDA mismatch",
            RexoError::InvalidMintAuthority => "mint authority PDA mismatch",
            RexoError::InvalidMint => "mint account mismatch",
            RexoError::InsufficientVaultBalance => "vault balance too low",
            RexoError::AccountNotRentExempt => "account not rent exempt",
            RexoError::SocialVerificationFailed => "social verification failed",
            RexoError::CorrelatedFailureGuard => "correlated verification failure; abandonment suppressed",
        })
    }
}

/// Jembatan dari error kurva murni ke error program.
impl From<crate::curve::CurveError> for RexoError {
    fn from(e: crate::curve::CurveError) -> Self {
        use crate::curve::CurveError as C;
        match e {
            C::ZeroAmount => RexoError::ZeroAmount,
            C::CurveComplete => RexoError::CurveComplete,
            C::CurveNotComplete => RexoError::NotGraduated,
            C::Overflow => RexoError::MathOverflow,
            C::SlippageExceeded => RexoError::SlippageExceeded,
            C::ExceedsCirculating => RexoError::ExceedsCirculating,
            C::InvalidConfig => RexoError::InvalidCurveConfig,
            C::CreatorCapExceeded => RexoError::CreatorCapExceeded,
            C::BondTooSmall => RexoError::BondTooSmall,
        }
    }
}

impl From<crate::curve::CurveError> for ProgramError {
    fn from(e: crate::curve::CurveError) -> Self {
        ProgramError::from(RexoError::from(e))
    }
}
