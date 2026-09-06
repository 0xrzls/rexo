// Copyright (c) 2026 Rexo
// SPDX-License-Identifier: Apache-2.0

//! Pemindahan RLO (kelvin) masuk dan keluar vault PDA.
//!
//! # Dua arah, dua mekanisme berbeda
//!
//! **Masuk** (user → vault): pakai `system_instruction::transfer` lewat
//! `invoke`. User menandatangani, jadi CPI biasa cukup.
//!
//! **Keluar** (vault → user): TIDAK boleh pakai `system_instruction::transfer`.
//! Vault PDA dimiliki program kita dan membawa data, dan System Program
//! menolak transfer dari akun yang membawa data ("Transfer: `from` must not
//! carry data"). Yang benar adalah memanipulasi saldo langsung lewat
//! `try_borrow_mut_kelvins`.
//!
//! Ini bukan tebakan. Pola ini persis yang dipakai `rialo_venus` sendiri di
//! `write_to_storage` (baris 220-229) dan `close_account` (baris 298-307)
//! pada versi 0.12.2, dengan komentar yang sama.

use rialo_s_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, msg, program::invoke,
    program_error::ProgramError, pubkey::Pubkey, rent::Rent, system_instruction,
    system_program, sysvar::Sysvar,
};

use crate::constants::VAULT_SEED;
use crate::errors::RexoError;

/// Turunkan alamat vault untuk sebuah mint.
pub fn derive_vault(program_id: &Pubkey, mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[VAULT_SEED, mint.as_array()], program_id)
}

/// Pastikan akun vault yang dikirim benar-benar PDA yang kita harapkan.
///
/// Tanpa cek ini, penyerang bisa mengirim akun miliknya sendiri sebagai
/// "vault" dan menarik seluruh hasil kurva. Ini kelas bug paling umum di
/// program bergaya Solana.
pub fn assert_vault(
    program_id: &Pubkey,
    mint: &Pubkey,
    vault: &AccountInfo<'_>,
) -> Result<u8, ProgramError> {
    let (expected, bump) = derive_vault(program_id, mint);
    if expected != *vault.key {
        msg!(
            "vault mismatch: got {} expected {}",
            vault.key,
            expected
        );
        return Err(RexoError::InvalidVault.into());
    }
    if vault.owner != program_id {
        msg!("vault not owned by program");
        return Err(RexoError::InvalidVault.into());
    }
    Ok(bump)
}

/// Buat vault kalau belum ada. Idempoten.
pub fn ensure_vault<'a>(
    program_id: &Pubkey,
    mint: &Pubkey,
    payer: &AccountInfo<'a>,
    vault: &AccountInfo<'a>,
    system_program_account: &AccountInfo<'a>,
) -> ProgramResult {
    if !system_program::check_id(system_program_account.key) {
        return Err(ProgramError::IncorrectProgramId);
    }
    let (expected, bump) = derive_vault(program_id, mint);
    if expected != *vault.key {
        return Err(RexoError::InvalidVault.into());
    }
    if vault.kelvins() > 0 {
        return Ok(()); // sudah ada
    }

    // Vault menyimpan 1 byte penanda supaya ia "carry data" dan karenanya
    // hanya bisa dikurangi lewat program kita, tidak lewat System Program.
    let space: usize = 1;
    let rent = Rent::get()?;
    let lamports = rent.minimum_balance(space);

    let seeds: &[&[u8]] = &[VAULT_SEED, mint.as_array(), &[bump]];
    rialo_s_program::program::invoke_signed(
        &system_instruction::create_account(
            payer.key,
            vault.key,
            lamports,
            space as u64,
            program_id,
        ),
        &[payer.clone(), vault.clone(), system_program_account.clone()],
        &[seeds],
    )?;
    msg!("vault {} created with {} kelvin rent", vault.key, lamports);
    Ok(())
}

/// User → vault. User harus signer.
pub fn deposit<'a>(
    from: &AccountInfo<'a>,
    vault: &AccountInfo<'a>,
    system_program_account: &AccountInfo<'a>,
    kelvins: u64,
) -> ProgramResult {
    if kelvins == 0 {
        return Ok(());
    }
    if !from.is_signer {
        return Err(RexoError::Unauthorized.into());
    }
    invoke(
        &system_instruction::transfer(from.key, vault.key, kelvins),
        &[from.clone(), vault.clone(), system_program_account.clone()],
    )
}

/// Vault → user. Manipulasi saldo langsung; lihat catatan modul.
///
/// Menyisakan rent-exempt minimum di vault. Menguras vault sampai nol akan
/// membuatnya bisa di-garbage-collect dan seluruh peluncuran hilang.
pub fn withdraw<'a>(
    vault: &AccountInfo<'a>,
    to: &AccountInfo<'a>,
    kelvins: u64,
) -> ProgramResult {
    if kelvins == 0 {
        return Ok(());
    }
    let rent = Rent::get()?;
    let reserve = rent.minimum_balance(vault.data_len());
    let balance = vault.kelvins();

    let available = balance.saturating_sub(reserve);
    if available < kelvins {
        msg!(
            "vault {} balance {} reserve {} available {} requested {}",
            vault.key,
            balance,
            reserve,
            available,
            kelvins
        );
        return Err(RexoError::InsufficientVaultBalance.into());
    }

    **vault.try_borrow_mut_kelvins()? -= kelvins;
    **to.try_borrow_mut_kelvins()? += kelvins;
    Ok(())
}

/// Saldo yang bisa dipakai, di luar cadangan rent.
pub fn spendable(vault: &AccountInfo<'_>) -> Result<u64, ProgramError> {
    let rent = Rent::get()?;
    Ok(vault
        .kelvins()
        .saturating_sub(rent.minimum_balance(vault.data_len())))
}
