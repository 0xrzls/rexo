// Copyright (c) 2026 Rexo
// SPDX-License-Identifier: Apache-2.0

//! Event terstruktur.
//!
//! Rialo punya `rialo-sol-attribute-event` dengan `#[event]` dan `emit!`,
//! yang memakai syscall `rlo_log_data`. Itu jalur yang benar untuk data
//! yang akan dibaca indexer — jauh lebih baik daripada mem-parse string
//! `msg!` dengan regex.
//!
//! `msg!` tetap dipakai untuk diagnostik manusia. Dua tujuan berbeda:
//! `emit!` untuk mesin, `msg!` untuk orang yang membaca log transaksi.
//!
//! CATATAN VERIFIKASI: nama crate dan makro terkonfirmasi dari docs.rs
//! (`rialo_sol_attribute_event::{event, emit}`). Bentuk persis attribute-nya
//! belum kuuji compile — kalau `#[event]` menolak, bandingkan dengan contoh
//! di rialo-examples yang memancarkan event.

use rialo_s_program::pubkey::Pubkey;

#[cfg(feature = "events")]
use rialo_sol_attribute_event::{emit, event};

/// Token diluncurkan. Diterbitkan sekali per mint.
#[cfg_attr(feature = "events", event)]
#[derive(Debug, Clone)]
pub struct LaunchCreated {
    pub mint: Pubkey,
    pub creator: Pubkey,
    pub tier: u8,
    pub bond: u64,
    pub sealed_until: u64,
    pub timestamp: u64,
}

/// Hasil verifikasi sosial dari REX. Hanya angka, tidak pernah data mentah.
#[cfg_attr(feature = "events", event)]
#[derive(Debug, Clone)]
pub struct SocialVerified {
    pub mint: Pubkey,
    pub tier: u8,
    pub telegram_members: u64,
    pub x_account_age_days: u64,
    pub passed: bool,
    pub timestamp: u64,
}

/// Satu perdagangan. `is_buy` membedakan arah.
#[cfg_attr(feature = "events", event)]
#[derive(Debug, Clone)]
pub struct Trade {
    pub mint: Pubkey,
    pub trader: Pubkey,
    pub is_buy: bool,
    pub quote_amount: u64,
    pub token_amount: u64,
    pub fee_protocol: u64,
    pub fee_creator: u64,
    pub virtual_quote: u64,
    pub virtual_token: u64,
    pub real_quote: u64,
    pub real_token: u64,
    pub progress_bps: u64,
    pub timestamp: u64,
}

/// Batch sealed selesai diselesaikan pada satu clearing price.
#[cfg_attr(feature = "events", event)]
#[derive(Debug, Clone)]
pub struct SealedBatchCleared {
    pub mint: Pubkey,
    pub order_count: u32,
    pub total_quote: u64,
    pub total_tokens: u64,
    pub clearing_price: u64,
    pub timestamp: u64,
}

/// Heartbeat verifikasi sosial.
#[cfg_attr(feature = "events", event)]
#[derive(Debug, Clone)]
pub struct Heartbeat {
    pub mint: Pubkey,
    pub sequence: u64,
    pub passed: bool,
    pub consecutive_failures: u32,
    pub timestamp: u64,
}

/// Token ditinggalkan; bond hangus ke LP.
#[cfg_attr(feature = "events", event)]
#[derive(Debug, Clone)]
pub struct Abandoned {
    pub mint: Pubkey,
    pub forfeited_bond: u64,
    pub burned_creator_tokens: u64,
    pub timestamp: u64,
}

/// Tranche vesting kreator terbuka.
#[cfg_attr(feature = "events", event)]
#[derive(Debug, Clone)]
pub struct VestingUnlocked {
    pub mint: Pubkey,
    pub tranche: u8,
    pub amount: u64,
    pub timestamp: u64,
}

/// Kurva lulus; likuiditas pindah ke pool.
#[cfg_attr(feature = "events", event)]
#[derive(Debug, Clone)]
pub struct Graduated {
    pub mint: Pubkey,
    pub lp_tokens: u64,
    pub lp_quote: u64,
    pub fees_protocol: u64,
    pub fees_creator: u64,
    pub bond_returned: u64,
    pub sfs_endowment: u64,
    pub timestamp: u64,
}

/// Helper agar call site tetap bersih ketika fitur `events` mati.
#[macro_export]
macro_rules! rexo_emit {
    ($e:expr) => {{
        #[cfg(feature = "events")]
        {
            $crate::events::reexport::emit!($e);
        }
        #[cfg(not(feature = "events"))]
        {
            let _ = &$e;
        }
    }};
}

#[cfg(feature = "events")]
pub mod reexport {
    pub use rialo_sol_attribute_event::emit;
}
