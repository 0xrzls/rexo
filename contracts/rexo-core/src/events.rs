//! # Rexo Core — Event Terstruktur untuk Indexer
//!
//! Event di-encode dan di-emit via log sistem agar dapat diindeks oleh RPC node & frontend.

use rialo_s_program::{msg, pubkey::Pubkey};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchEvent {
    pub creator: Pubkey,
    pub mint: Pubkey,
    pub tier: u8,
    pub bond_kelvins: u64,
    pub heartbeat_interval: u64,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuyEvent {
    pub buyer: Pubkey,
    pub mint: Pubkey,
    pub quote_in_kelvins: u64,
    pub tokens_out: u64,
    pub protocol_fee_kelvins: u64,
    pub creator_fee_kelvins: u64,
    pub real_quote_kelvins: u64,
    pub real_token_reserves: u64,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SellEvent {
    pub seller: Pubkey,
    pub mint: Pubkey,
    pub tokens_in: u64,
    pub quote_out_kelvins: u64,
    pub protocol_fee_kelvins: u64,
    pub creator_fee_kelvins: u64,
    pub real_quote_kelvins: u64,
    pub real_token_reserves: u64,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatEvent {
    pub mint: Pubkey,
    pub heartbeat_count: u64,
    pub next_expected: u64,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationEvent {
    pub mint: Pubkey,
    pub old_tier: u8,
    pub new_tier: u8,
    pub proof_hash: [u8; 32],
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraduateEvent {
    pub mint: Pubkey,
    pub final_quote_kelvins: u64,
    pub burned_lp: bool,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbandonEvent {
    pub mint: Pubkey,
    pub reason: u8,
    pub bond_forfeited_kelvins: u64,
    pub timestamp: u64,
}

pub fn emit_event<T: Serialize>(name: &str, event: &T) {
    if let Ok(serialized) = serde_json::to_string(event) {
        msg!("RexoEvent::{} {}", name, serialized);
    }
}
