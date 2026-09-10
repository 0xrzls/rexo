// Copyright (c) 2026 Rexo
// SPDX-License-Identifier: Apache-2.0

//! Operasi token lewat `rialo-spl-token-2022`.
//!
//! # Yang paling penting di file ini
//!
//! `create_mint` MENCABUT mint authority dan freeze authority di transaksi
//! yang sama dengan pembuatan mint. Ini bukan opsional dan bukan langkah
//! yang bisa ditunda.
//!
//! Kalau mint authority masih hidup, seluruh matematika di `curve.rs` tidak
//! ada artinya — deployer bisa mencetak supply tambahan kapan saja dan
//! setiap invariant konservasi token yang kita uji jadi bohong. Ini satu-
//! satunya bug di repo ini yang bisa menghapus seluruh nilai token dalam
//! satu instruksi.
//!
//! pump.fun sendiri tidak memberi kreator kemampuan mencabut authority
//! (lihat 01-RESEARCH.md). Kita melakukannya otomatis untuk semua.

use rialo_s_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, msg,
    program::{invoke, invoke_signed}, program_error::ProgramError, pubkey::Pubkey,
    rent::Rent, system_instruction, sysvar::Sysvar,
};

use crate::config::{AUTHORITY_SEED, BASE_VAULT_SEED};

/// Desimal token yang diluncurkan.
pub const TOKEN_DECIMALS: u8 = 6;
use crate::errors::RexoError;

// Alias supaya ganti versi token program cuma menyentuh satu baris.
use rialo_spl_token_2022 as token;

/// Ukuran akun mint Token-2022 tanpa extension.
const MINT_SIZE: usize = 82;
/// Ukuran akun token Token-2022 tanpa extension.
const TOKEN_ACCOUNT_SIZE: usize = 165;

pub fn derive_base_vault(program_id: &Pubkey, launch: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[BASE_VAULT_SEED, launch.as_array()], program_id)
}

pub fn derive_mint_authority(program_id: &Pubkey, mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[AUTHORITY_SEED, mint.as_array()], program_id)
}

pub fn assert_authority(
    program_id: &Pubkey,
    mint: &Pubkey,
    authority: &AccountInfo<'_>,
) -> Result<u8, ProgramError> {
    let (expected, bump) = derive_mint_authority(program_id, mint);
    if expected != *authority.key {
        msg!("mint authority mismatch: got {} expected {}", authority.key, expected);
        return Err(RexoError::InvalidAuthority.into());
    }
    Ok(bump)
}

/// Buat mint, cetak seluruh supply ke akun kurva, lalu CABUT kedua authority.
///
/// Setelah fungsi ini kembali, supply token itu tetap selamanya.
#[allow(clippy::too_many_arguments)]
pub fn create_mint_and_lock<'a>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    mint_authority: &AccountInfo<'a>,
    base_vault: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    system_program_account: &AccountInfo<'a>,
    total_supply: u64,
) -> ProgramResult {
    let bump = assert_authority(program_id, mint.key, mint_authority)?;
    let seeds: &[&[u8]] = &[AUTHORITY_SEED, mint.key.as_array(), &[bump]];

    // 1. alokasikan akun mint
    let rent = Rent::get()?;
    invoke(
        &system_instruction::create_account(
            payer.key,
            mint.key,
            rent.minimum_balance(MINT_SIZE),
            MINT_SIZE as u64,
            token_program.key,
        ),
        &[payer.clone(), mint.clone(), system_program_account.clone()],
    )?;

    // 2. inisialisasi mint dengan authority sementara = PDA kita
    invoke(
        &token::instruction::initialize_mint2(
            token_program.key,
            mint.key,
            mint_authority.key,
            Some(mint_authority.key), // freeze authority, dicabut di langkah 5
            TOKEN_DECIMALS,
        )?,
        &[mint.clone(), token_program.clone()],
    )?;

    // 3. buat akun token kurva SEBAGAI PDA MILIK PROGRAM.
    //
    // Ini yang hilang sebelumnya, dan tanpanya `mint_to` di langkah 4
    // gagal — akun token yang belum diinisialisasi tidak bisa menerima
    // hasil cetak. Akibatnya `launch` gagal 100%.
    let (expected_ta, ta_bump) = derive_base_vault(program_id, mint.key);
    if expected_ta != *base_vault.key {
        msg!("curve token account mismatch");
        return Err(RexoError::InvalidVault.into());
    }
    let ta_seeds: &[&[u8]] = &[BASE_VAULT_SEED, mint.key.as_array(), &[ta_bump]];

    invoke_signed(
        &system_instruction::create_account(
            payer.key,
            base_vault.key,
            rent.minimum_balance(TOKEN_ACCOUNT_SIZE),
            TOKEN_ACCOUNT_SIZE as u64,
            token_program.key,
        ),
        &[
            payer.clone(),
            base_vault.clone(),
            system_program_account.clone(),
        ],
        &[ta_seeds],
    )?;

    // Pemiliknya PDA mint_authority, bukan payer. Kreator tidak pernah
    // bisa menyentuh supply kurva secara langsung.
    invoke(
        &token::instruction::initialize_account3(
            token_program.key,
            base_vault.key,
            mint.key,
            mint_authority.key,
        )?,
        &[
            base_vault.clone(),
            mint.clone(),
            token_program.clone(),
        ],
    )?;

    // 4. cetak SELURUH supply ke akun token kurva
    invoke_signed(
        &token::instruction::mint_to(
            token_program.key,
            mint.key,
            base_vault.key,
            mint_authority.key,
            &[],
            total_supply,
        )?,
        &[
            mint.clone(),
            base_vault.clone(),
            mint_authority.clone(),
            token_program.clone(),
        ],
        &[seeds],
    )?;

    // 5. CABUT mint authority — permanen, tidak bisa dibatalkan
    invoke_signed(
        &token::instruction::set_authority(
            token_program.key,
            mint.key,
            None, // None = cabut
            token::instruction::AuthorityType::MintTokens,
            mint_authority.key,
            &[],
        )?,
        &[mint.clone(), mint_authority.clone(), token_program.clone()],
        &[seeds],
    )?;

    // 6. CABUT freeze authority — tanpa ini kreator masih bisa membekukan
    //    wallet pemegang token, yang secara efektif adalah rug pull.
    invoke_signed(
        &token::instruction::set_authority(
            token_program.key,
            mint.key,
            None,
            token::instruction::AuthorityType::FreezeAccount,
            mint_authority.key,
            &[],
        )?,
        &[mint.clone(), mint_authority.clone(), token_program.clone()],
        &[seeds],
    )?;

    msg!(
        "mint {} created, {} supply minted, mint+freeze authority REVOKED",
        mint.key,
        total_supply
    );
    Ok(())
}

/// Kirim token dari akun kurva ke pembeli. Ditandatangani PDA.
#[allow(clippy::too_many_arguments)]
pub fn transfer_out<'a>(
    program_id: &Pubkey,
    mint: &AccountInfo<'a>,
    from: &AccountInfo<'a>,
    to: &AccountInfo<'a>,
    mint_authority: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    amount: u64,
) -> ProgramResult {
    if amount == 0 {
        return Ok(());
    }
    let bump = assert_authority(program_id, mint.key, mint_authority)?;
    let seeds: &[&[u8]] = &[AUTHORITY_SEED, mint.key.as_array(), &[bump]];

    // transfer_checked, bukan transfer: ia memverifikasi mint dan desimal,
    // jadi akun token dengan mint yang salah tidak bisa diselipkan.
    invoke_signed(
        &token::instruction::transfer_checked(
            token_program.key,
            from.key,
            mint.key,
            to.key,
            mint_authority.key,
            &[],
            amount,
            TOKEN_DECIMALS,
        )?,
        &[
            from.clone(),
            mint.clone(),
            to.clone(),
            mint_authority.clone(),
            token_program.clone(),
        ],
        &[seeds],
    )
}

/// Terima token dari penjual kembali ke akun kurva. Penjual menandatangani.
#[allow(clippy::too_many_arguments)]
pub fn transfer_in<'a>(
    mint: &AccountInfo<'a>,
    from: &AccountInfo<'a>,
    to: &AccountInfo<'a>,
    seller: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    amount: u64,
) -> ProgramResult {
    if amount == 0 {
        return Ok(());
    }
    if !seller.is_signer {
        return Err(RexoError::Unauthorized.into());
    }
    invoke(
        &token::instruction::transfer_checked(
            token_program.key,
            from.key,
            mint.key,
            to.key,
            seller.key,
            &[],
            amount,
            TOKEN_DECIMALS,
        )?,
        &[
            from.clone(),
            mint.clone(),
            to.clone(),
            seller.clone(),
            token_program.clone(),
        ],
    )
}

/// Burn token. Dipakai untuk alokasi kreator yang hangus saat abandonment.
pub fn burn_from_vault<'a>(
    program_id: &Pubkey,
    mint: &AccountInfo<'a>,
    from: &AccountInfo<'a>,
    mint_authority: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    amount: u64,
) -> ProgramResult {
    if amount == 0 {
        return Ok(());
    }
    let bump = assert_authority(program_id, mint.key, mint_authority)?;
    let seeds: &[&[u8]] = &[AUTHORITY_SEED, mint.key.as_array(), &[bump]];

    invoke_signed(
        &token::instruction::burn(
            token_program.key,
            from.key,
            mint.key,
            mint_authority.key,
            &[],
            amount,
        )?,
        &[
            from.clone(),
            mint.clone(),
            mint_authority.clone(),
            token_program.clone(),
        ],
        &[seeds],
    )?;
    msg!("burned {} tokens from curve", amount);
    Ok(())
}
