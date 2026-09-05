//! # Rexo Core — Parsing & Validasi Akun PDA
//!
//! Rialo menggunakan `Pubkey::as_array()` (bukan `to_bytes()`).
//! Seed Rexo (`rexo_curve`, `rexo_vault`) terpisah dari `WORKFLOW_SEED` rialo-venus.

use rialo_s_program::{account_info::AccountInfo, pubkey::Pubkey};

use crate::constants::*;
use crate::errors::RexoError;

/// Menemukan PDA State Kurva
pub fn find_curve_pda(mint: &Pubkey, program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[SEED_CURVE, mint.as_array()], program_id)
}

/// Menemukan PDA Vault Kelvin/Quote
pub fn find_vault_pda(mint: &Pubkey, program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[SEED_VAULT, mint.as_array()], program_id)
}

/// Validasi bahwa akun adalah PDA yang valid
pub fn assert_pda(
    account: &AccountInfo<'_>,
    expected: &Pubkey,
) -> Result<(), RexoError> {
    if account.key != expected {
        return Err(RexoError::InvalidPDA);
    }
    Ok(())
}

/// Daftar akun untuk operasi Launch
pub struct LaunchAccounts<'a> {
    pub creator: &'a AccountInfo<'_>,
    pub curve_state: &'a AccountInfo<'_>,
    pub vault: &'a AccountInfo<'_>,
    pub mint: &'a AccountInfo<'_>,
    pub vault_token_account: &'a AccountInfo<'_>,
    pub system_program: &'a AccountInfo<'_>,
    pub token_program: &'a AccountInfo<'_>,
}

impl<'a> LaunchAccounts<'a> {
    pub fn parse(accounts: &'a [AccountInfo<'_>]) -> Result<Self, RexoError> {
        if accounts.len() < 7 {
            return Err(RexoError::InvalidAccountData);
        }
        Ok(Self {
            creator: &accounts[0],
            curve_state: &accounts[1],
            vault: &accounts[2],
            mint: &accounts[3],
            vault_token_account: &accounts[4],
            system_program: &accounts[5],
            token_program: &accounts[6],
        })
    }
}

/// Daftar akun untuk operasi Buy / Sell
pub struct TradeAccounts<'a> {
    pub trader: &'a AccountInfo<'_>,
    pub curve_state: &'a AccountInfo<'_>,
    pub vault: &'a AccountInfo<'_>,
    pub treasury: &'a AccountInfo<'_>,
    pub creator: &'a AccountInfo<'_>,
    pub trader_token_account: &'a AccountInfo<'_>,
    pub vault_token_account: &'a AccountInfo<'_>,
    pub system_program: &'a AccountInfo<'_>,
    pub token_program: &'a AccountInfo<'_>,
}

impl<'a> TradeAccounts<'a> {
    pub fn parse(accounts: &'a [AccountInfo<'_>]) -> Result<Self, RexoError> {
        if accounts.len() < 9 {
            return Err(RexoError::InvalidAccountData);
        }
        Ok(Self {
            trader: &accounts[0],
            curve_state: &accounts[1],
            vault: &accounts[2],
            treasury: &accounts[3],
            creator: &accounts[4],
            trader_token_account: &accounts[5],
            vault_token_account: &accounts[6],
            system_program: &accounts[7],
            token_program: &accounts[8],
        })
    }
}
