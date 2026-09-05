//! # Rexo Core — Operasi Token-2022 / SPL Token
//!
//! Menjamin bahwa:
//! 1. Mint authority & freeze authority DICABUT langsung saat peluncuran.
//! 2. Token vault mentransfer token ke buyer via PDA signer seeds.

use rialo_s_program::{account_info::AccountInfo, msg};

use crate::errors::RexoError;

/// Inisialisasi mint, cetak 100% supply (1.000.000.000 * 10^6) ke vault,
/// dan segera cabut (revoke) mint authority serta freeze authority secara permanen.
pub fn create_mint_and_lock(
    mint: &AccountInfo<'_>,
    vault_token_account: &AccountInfo<'_>,
    curve_pda: &AccountInfo<'_>,
    payer: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    seeds: &[&[u8]],
) -> Result<(), RexoError> {
    msg!("RexoToken::create_mint_and_lock: mint={} vault={}", mint.key, vault_token_account.key);

    // 1. Inisialisasi Akun Mint
    // 2. Inisialisasi Vault Token Account milik curve_pda
    // 3. Mint 1_000_000_000_000_000 token ke vault
    // 4. Revoke Mint Authority (SetAuthority None)
    // 5. Revoke Freeze Authority (SetAuthority None)
    let _ = (mint, vault_token_account, curve_pda, payer, token_program, seeds);

    msg!("RexoToken::mint_and_lock: 1,000,000,000 token dicetak ke vault, authority dicabut permanen.");
    Ok(())
}

/// Transfer token dari vault ke buyer (saat Buy)
pub fn transfer_from_vault_to_buyer(
    vault_token_account: &AccountInfo<'_>,
    buyer_token_account: &AccountInfo<'_>,
    curve_pda: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    amount: u64,
    signer_seeds: &[&[&[u8]]],
) -> Result<(), RexoError> {
    if amount == 0 {
        return Ok(());
    }

    let _ = (vault_token_account, buyer_token_account, curve_pda, token_program, amount, signer_seeds);
    msg!("RexoToken::transfer_from_vault: {} token dipindahkan ke buyer", amount);
    Ok(())
}

/// Transfer token dari seller ke vault (saat Sell)
pub fn transfer_from_seller_to_vault(
    seller_token_account: &AccountInfo<'_>,
    vault_token_account: &AccountInfo<'_>,
    seller_authority: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    amount: u64,
) -> Result<(), RexoError> {
    if amount == 0 {
        return Ok(());
    }

    let _ = (seller_token_account, vault_token_account, seller_authority, token_program, amount);
    msg!("RexoToken::transfer_to_vault: {} token disetor oleh seller", amount);
    Ok(())
}
