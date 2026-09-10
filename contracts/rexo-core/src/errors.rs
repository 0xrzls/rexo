// Copyright (c) 2026 Rexo
// SPDX-License-Identifier: Apache-2.0

//! Error domain. Kode numerik stabil — jangan sisipkan varian di tengah
//! setelah deploy, tambahkan di akhir saja.

use rialo_s_program::program_error::ProgramError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum RexoError {
    // -- config --
    InvalidConfig = 100,
    UnsupportedCurve = 101,
    FeeSplitMismatch = 102,
    ConfigImmutableField = 103,

    // -- lifecycle --
    AlreadyInitialized = 200,
    NotInitialized = 201,
    NotFunding = 202,
    NotMigrating = 203,
    AlreadyMigrated = 204,

    // -- otorisasi --
    Unauthorized = 300,
    NotPartner = 301,
    NotCreator = 302,

    // -- perdagangan --
    ZeroAmount = 400,
    SlippageExceeded = 401,
    ExceedsCirculating = 402,
    CurveComplete = 403,
    MathOverflow = 404,
    ExceedsMaxIn = 405,

    // -- batas --
    CreatorCapExceeded = 500,

    // -- akun --
    InvalidVault = 600,
    InvalidAuthority = 601,
    InvalidMint = 602,
    InsufficientVaultBalance = 603,
    AccountMismatch = 604,

    // -- vesting --
    NothingToClaim = 700,
    VestingNotStarted = 701,

    // -- fee --
    NoFeesToClaim = 800,
}

impl From<RexoError> for ProgramError {
    fn from(e: RexoError) -> Self {
        ProgramError::Custom(e as u32)
    }
}

impl core::fmt::Display for RexoError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            RexoError::InvalidConfig => "invalid launch config",
            RexoError::UnsupportedCurve => "curve type not supported",
            RexoError::FeeSplitMismatch => "fee shares do not sum to total",
            RexoError::ConfigImmutableField => "field cannot change after config is in use",
            RexoError::AlreadyInitialized => "already initialized",
            RexoError::NotInitialized => "not initialized",
            RexoError::NotFunding => "launch is not accepting trades",
            RexoError::NotMigrating => "launch is not ready to migrate",
            RexoError::AlreadyMigrated => "already migrated",
            RexoError::Unauthorized => "signer not authorized",
            RexoError::NotPartner => "signer is not the config partner",
            RexoError::NotCreator => "signer is not the launch creator",
            RexoError::ZeroAmount => "amount resolves to zero",
            RexoError::SlippageExceeded => "output below minimum",
            RexoError::ExceedsCirculating => "exceeds circulating supply",
            RexoError::CurveComplete => "curve complete",
            RexoError::MathOverflow => "arithmetic overflow",
            RexoError::ExceedsMaxIn => "input above maximum",
            RexoError::CreatorCapExceeded => "creator allocation cap exceeded",
            RexoError::InvalidVault => "vault PDA mismatch",
            RexoError::InvalidAuthority => "authority PDA mismatch",
            RexoError::InvalidMint => "mint mismatch",
            RexoError::InsufficientVaultBalance => "vault balance too low",
            RexoError::AccountMismatch => "account does not match expected PDA",
            RexoError::NothingToClaim => "nothing to claim",
            RexoError::VestingNotStarted => "vesting has not started",
            RexoError::NoFeesToClaim => "no fees accrued",
        })
    }
}

impl From<crate::curve::CurveError> for RexoError {
    fn from(e: crate::curve::CurveError) -> Self {
        use crate::curve::CurveError as C;
        match e {
            C::ZeroAmount => RexoError::ZeroAmount,
            C::CurveComplete => RexoError::CurveComplete,
            C::CurveNotComplete => RexoError::NotMigrating,
            C::Overflow => RexoError::MathOverflow,
            C::SlippageExceeded => RexoError::SlippageExceeded,
            C::ExceedsCirculating => RexoError::ExceedsCirculating,
            C::InvalidConfig => RexoError::InvalidConfig,
        }
    }
}

impl From<crate::curve::CurveError> for ProgramError {
    fn from(e: crate::curve::CurveError) -> Self {
        ProgramError::from(RexoError::from(e))
    }
}
