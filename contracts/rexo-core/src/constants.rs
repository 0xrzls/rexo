//! # Rexo Core — Parameter Ekonomi & Konstanta Protokol
//!
//! Seluruh parameter ekonomi terpusat dalam satu file agar mudah diaudit.

pub const BPS_DENOM: u128 = 10_000;

pub const TOKEN_DECIMALS: u32 = 6;
pub const QUOTE_DECIMALS: u32 = 9;

pub const ONE_TOKEN: u128 = 1_000_000;
pub const ONE_QUOTE: u128 = 1_000_000_000; // 1 RLO = 10^9 Kelvins

/// Total fee flat 100 bps (1%) di semua tier. Pembeli tidak membayar penalti atas tier kreator.
pub const TOTAL_FEE_BPS: u128 = 100;
pub const PROTOCOL_FEE_BPS: u128 = 50;
pub const CREATOR_FEE_BPS: u128 = 50;

/// Konstanta bonding curve pump.fun-calibrated
pub const INITIAL_VIRTUAL_TOKEN_RESERVES: u128 = 1_073_000_000_000_000;
pub const INITIAL_VIRTUAL_QUOTE_RESERVES: u128 = 30_000_000_000; // 30 RLO
pub const INITIAL_REAL_TOKEN_RESERVES: u128 = 793_100_000_000_000;
pub const TOKEN_TOTAL_SUPPLY: u128 = 1_000_000_000_000_000;

/// Nilai minimum bond per tier (dalam Kelvin)
pub const BOND_MIN_UNVERIFIED: u64 = 0;
pub const BOND_MIN_COMMUNITY: u64 = 500_000_000; // 0.5 RLO
pub const BOND_MIN_VERIFIED_DEV: u64 = 2_000_000_000; // 2.0 RLO
pub const BOND_MIN_REX_GUARANTEED: u64 = 10_000_000_000; // 10.0 RLO

/// Status Siklus Hidup Token
pub const STATUS_UNINITIALIZED: u8 = 0;
pub const STATUS_ACTIVE: u8 = 1;
pub const STATUS_GRADUATED: u8 = 2;
pub const STATUS_ABANDONED: u8 = 3;

/// Tingkat Verifikasi (Tier)
pub const TIER_UNVERIFIED: u8 = 0;
pub const TIER_COMMUNITY: u8 = 1;
pub const TIER_VERIFIED_DEV: u8 = 2;
pub const TIER_REX_GUARANTEED: u8 = 3;

/// Heartbeat Liveness Parameters
pub const MIN_HEARTBEAT_INTERVAL: u64 = 60; // 60 detik
pub const DEFAULT_HEARTBEAT_INTERVAL: u64 = 300; // 5 menit
pub const MAX_HEARTBEAT_MISSES: u64 = 3;

/// Seed PDA Rexo Core
pub const SEED_CURVE: &[u8] = b"rexo_curve";
pub const SEED_VAULT: &[u8] = b"rexo_vault";
pub const SEED_MINT: &[u8] = b"rexo_mint";
