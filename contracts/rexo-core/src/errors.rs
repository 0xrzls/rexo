//! # Rexo Core — Error Domain & Kode Numerik Stabil
//!
//! Kode numerik stabil (6000+) memudahkan integrasi client SDK, indexer, dan audit invariant.

use rialo_s_program_error::ProgramError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum RexoError {
    InvalidStatus = 6000,
    CurveComplete = 6001,
    CurveNotComplete = 6002,
    InsufficientLiquidity = 6003,
    SlippageExceeded = 6004,
    ZeroAmount = 6005,
    Overflow = 6006,
    Unauthorized = 6007,
    SelfAssignedTierForbidden = 6008,
    BondBelowMinimum = 6009,
    AbandonmentDisallowed = 6010,
    InvalidPDA = 6011,
    InvalidAccountData = 6012,
    MissingSignature = 6013,
    HeartbeatMissedLimitExceeded = 6014,
}

impl From<RexoError> for ProgramError {
    fn from(e: RexoError) -> Self {
        ProgramError::Custom(e as u32)
    }
}

impl core::fmt::Display for RexoError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            RexoError::InvalidStatus => "RexoError[6000]: Kurva tidak dalam status yang valid untuk operasi ini",
            RexoError::CurveComplete => "RexoError[6001]: Kurva bonding telah mencapai batas kelulusan (graduated)",
            RexoError::CurveNotComplete => "RexoError[6002]: Kurva bonding belum mencapai target kelulusan",
            RexoError::InsufficientLiquidity => "RexoError[6003]: Likuiditas token tidak mencukupi",
            RexoError::SlippageExceeded => "RexoError[6004]: Batas slippage terlampaui",
            RexoError::ZeroAmount => "RexoError[6005]: Jumlah transaksi tidak boleh nol",
            RexoError::Overflow => "RexoError[6006]: Overflow aritmatika saat perhitungan kurva",
            RexoError::Unauthorized => "RexoError[6007]: Akses ditolak: penandatangan tidak berwenang",
            RexoError::SelfAssignedTierForbidden => "RexoError[6008]: Tier di atas Unverified hanya dapat ditetapkan via verifikasi REX",
            RexoError::BondBelowMinimum => "RexoError[6009]: Bond kreator di bawah syarat minimum tier",
            RexoError::AbandonmentDisallowed => "RexoError[6010]: Pembatalan token ditolak oleh circuit breaker global",
            RexoError::InvalidPDA => "RexoError[6011]: Alamat PDA atau bump seed tidak cocok",
            RexoError::InvalidAccountData => "RexoError[6012]: Struktur data akun tidak valid",
            RexoError::MissingSignature => "RexoError[6013]: Penandatangan yang dibutuhkan tidak ada",
            RexoError::HeartbeatMissedLimitExceeded => "RexoError[6014]: Batas toleransi keterlambatan heartbeat terlampaui",
        })
    }
}
