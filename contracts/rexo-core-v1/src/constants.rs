// Copyright (c) 2026 Rexo
// SPDX-License-Identifier: Apache-2.0

//! Konstanta protokol Rexo.
//!
//! Semua angka yang menentukan ekonomi ada di satu file supaya audit bisa
//! membaca satu tempat saja. Jangan sebar magic number ke modul lain.

// ---------------------------------------------------------------------------
// Unit
// ---------------------------------------------------------------------------

/// Desimal RLO. Satuan terkecilnya bernama "kelvin" — ini nama resmi,
/// terkonfirmasi dari source `rialo-venus` 0.12.2 (`AccountInfo::kelvins()`,
/// `close_account` "transfer its kelvin to the payer account").
pub const QUOTE_DECIMALS: u8 = 9;
pub const ONE_RLO: u64 = 1_000_000_000;

/// Desimal token meme yang dibuat launchpad.
pub const TOKEN_DECIMALS: u8 = 6;
pub const ONE_TOKEN: u64 = 1_000_000;

// ---------------------------------------------------------------------------
// Parameter bonding curve
//
// Disalin persis dari `Global` account pump.fun. Lihat 01-RESEARCH.md.
// Bentuk kurva sengaja identik supaya progress bar dan market cap bisa
// dibandingkan langsung, dan supaya inovasi kita ada di lapisan lain.
// ---------------------------------------------------------------------------

pub const INITIAL_VIRTUAL_QUOTE: u64 = 30_000_000_000; //          30 RLO
pub const INITIAL_VIRTUAL_TOKEN: u64 = 1_073_000_000_000_000; // 1.073B token
pub const INITIAL_REAL_TOKEN: u64 = 793_100_000_000_000; //     793.1M token
pub const LP_RESERVE: u64 = 206_900_000_000_000; //             206.9M token
pub const TOTAL_SUPPLY: u64 = 1_000_000_000_000_000; //            1B token

/// Total fee perdagangan, basis points. SAMA di semua tier — pembeli tidak
/// pernah membayar lebih karena kreator memilih tier rendah.
pub const TOTAL_FEE_BPS: u64 = 100;

pub const BPS_DENOM: u64 = 10_000;

// ---------------------------------------------------------------------------
// PDA seeds
//
// `rialo_workflow` dipakai internal oleh Venus untuk workflow PDA
// (rialo_venus::WORKFLOW_SEED). Seed di bawah milik Rexo sendiri dan tidak
// boleh bentrok dengan itu.
// ---------------------------------------------------------------------------

/// Vault yang memegang RLO hasil kurva. Owner = program ini.
/// seeds: [VAULT_SEED, mint]
pub const VAULT_SEED: &[u8] = b"rexo_vault";

/// Otoritas mint & holder token kurva.
/// seeds: [MINT_AUTHORITY_SEED, mint]
pub const MINT_AUTHORITY_SEED: &[u8] = b"rexo_mint_auth";

/// Vault fee kreator.
/// seeds: [CREATOR_VAULT_SEED, creator]
pub const CREATOR_VAULT_SEED: &[u8] = b"rexo_creator_vault";

/// Escrow bond peluncuran.
/// seeds: [BOND_SEED, mint]
pub const BOND_SEED: &[u8] = b"rexo_bond";

// ---------------------------------------------------------------------------
// Kebijakan tier
//
// Tier ditetapkan oleh hasil verifikasi REX, TIDAK PERNAH oleh argumen
// kreator. Lihat guards::assert_tier_not_self_assigned.
// ---------------------------------------------------------------------------

pub const TIER_UNVERIFIED: u8 = 0;
pub const TIER_VERIFIED: u8 = 1;
pub const TIER_COMMITTED: u8 = 2;

/// Bagian fee kreator per tier, bps dari TOTAL_FEE_BPS.
pub const CREATOR_FEE_BPS: [u64; 3] = [0, 25, 50];

/// Batas alokasi kreator per tier, bps dari INITIAL_REAL_TOKEN.
pub const MAX_CREATOR_BPS: [u64; 3] = [100, 300, 500]; // 1% / 3% / 5%

/// Bond minimum per tier, dalam kelvin.
pub const MIN_BOND: [u64; 3] = [0, 2 * ONE_RLO, 10 * ONE_RLO];

// ---------------------------------------------------------------------------
// Status peluncuran
// ---------------------------------------------------------------------------

pub const STATUS_UNINITIALIZED: u8 = 0;
pub const STATUS_SEALED: u8 = 1;
pub const STATUS_ACTIVE: u8 = 2;
pub const STATUS_GRADUATED: u8 = 3;
pub const STATUS_ABANDONED: u8 = 4;
pub const STATUS_FINALIZED: u8 = 5;

// ---------------------------------------------------------------------------
// Parameter waktu (detik)
// ---------------------------------------------------------------------------

/// Jendela sealed di awal peluncuran. Order tidak diisi berurutan;
/// semuanya diisi pada satu clearing price.
pub const SEALED_WINDOW_SECS: u64 = 90;

/// Interval heartbeat verifikasi sosial.
pub const HEARTBEAT_INTERVAL_SECS: u64 = 21_600; // 6 jam

/// Kegagalan heartbeat berturut-turut sebelum token dianggap ditinggalkan.
pub const HEARTBEAT_FAILURES_BEFORE_ABANDON: u32 = 4; // ~24 jam

/// Ambang kegagalan global. Kalau lebih banyak dari ini gagal dalam satu
/// siklus, itu masalah API kita — bukan token yang ditinggalkan. Jangan
/// hanguskan bond siapa pun. Lihat 04-AUDIT.md temuan C.
pub const CORRELATED_FAILURE_THRESHOLD_BPS: u64 = 3_000; // 30%

// ---------------------------------------------------------------------------
// Ambang verifikasi sosial
// ---------------------------------------------------------------------------

pub const MIN_TELEGRAM_MEMBERS: u64 = 50;
pub const MIN_X_ACCOUNT_AGE_DAYS: u64 = 30;

// ---------------------------------------------------------------------------
// Vesting kreator (bps progress kurva)
// ---------------------------------------------------------------------------

pub const VEST_TRANCHE_1_PROGRESS_BPS: u64 = 2_500; // 25% kurva terisi
pub const VEST_TRANCHE_3_DELAY_SECS: u64 = 2_592_000; // 30 hari pasca-lulus

/// Berapa bagian fee protokol yang di-stake ke posisi Stake-for-Service
/// saat kelulusan, bps. Yield-nya membiayai automasi token ini selamanya.
pub const SFS_ENDOWMENT_BPS: u64 = 5_000; // 50%
