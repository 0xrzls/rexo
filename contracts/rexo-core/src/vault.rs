//! # Rexo Core — Pengelolaan Vault & Transfer Kelvin
//!
//! Rialo menggunakan satuan terkecil `kelvin`. Akun vault yang membawa data program
//! memindahkan saldo via mutasi `try_borrow_mut_kelvins()` sesuai pola rialo-venus (baris 220–229).

use rialo_s_program::{
    account_info::AccountInfo,
    program::invoke,
    system_instruction,
};

use crate::errors::RexoError;

/// Memindahkan kelvin dari akun pembayar (signer) ke vault
pub fn deposit_kelvins<'a>(
    from: &AccountInfo<'a>,
    to_vault: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    amount: u64,
) -> Result<(), RexoError> {
    if amount == 0 {
        return Ok(());
    }

    let ix = system_instruction::transfer(from.key, to_vault.key, amount);
    invoke(&ix, &[from.clone(), to_vault.clone(), system_program.clone()])
        .map_err(|_| RexoError::InsufficientLiquidity)?;

    Ok(())
}

/// Menarik kelvin dari vault ke akun tujuan.
/// Akun vault yang membawa state program tidak bisa memanggil `system_instruction::transfer`
/// sehingga pemindahan saldo dilakukan via manipulasi saldo aman `try_borrow_mut_kelvins()`.
pub fn withdraw_kelvins<'a>(
    vault: &AccountInfo<'a>,
    recipient: &AccountInfo<'a>,
    amount: u64,
) -> Result<(), RexoError> {
    if amount == 0 {
        return Ok(());
    }

    let mut vault_kelvins = vault
        .try_borrow_mut_kelvins()
        .map_err(|_| RexoError::InsufficientLiquidity)?;
    let mut recipient_kelvins = recipient
        .try_borrow_mut_kelvins()
        .map_err(|_| RexoError::InsufficientLiquidity)?;

    if **vault_kelvins < amount {
        return Err(RexoError::InsufficientLiquidity);
    }

    **vault_kelvins = (**vault_kelvins)
        .checked_sub(amount)
        .ok_or(RexoError::Overflow)?;

    **recipient_kelvins = (**recipient_kelvins)
        .checked_add(amount)
        .ok_or(RexoError::Overflow)?;

    Ok(())
}
