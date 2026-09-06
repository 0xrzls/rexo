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

use crate::constants::{MINT_AUTHORITY_SEED, TOKEN_DECIMALS};
use crate::errors::RexoError;

// Alias supaya ganti versi token program cuma menyentuh satu baris.
use rialo_spl_token_2022 as token;

/// Ukuran akun mint Token-2022 tanpa extension.
const MINT_SIZE: usize = 82;

pub fn derive_mint_authority(program_id: &Pubkey, mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[MINT_AUTHORITY_SEED, mint.as_array()], program_id)
}

pub fn assert_mint_authority(
    program_id: &Pubkey,
    mint: &Pubkey,
    authority: &AccountInfo<'_>,
) -> Result<u8, ProgramError> {
    let (expected, bump) = derive_mint_authority(program_id, mint);
    if expected != *authority.key {
        msg!("mint authority mismatch: got {} expected {}", authority.key, expected);
        return Err(RexoError::InvalidMintAuthority.into());
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
    curve_token_account: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    system_program_account: &AccountInfo<'a>,
    total_supply: u64,
) -> ProgramResult {
    let bump = assert_mint_authority(program_id, mint.key, mint_authority)?;
    let seeds: &[&[u8]] = &[MINT_AUTHORITY_SEED, mint.key.as_array(), &[bump]];

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

    // 3. cetak SELURUH supply ke akun token kurva
    invoke_signed(
        &token::instruction::mint_to(
            token_program.key,
            mint.key,
            curve_token_account.key,
            mint_authority.key,
            &[],
            total_supply,
        )?,
        &[
            mint.clone(),
            curve_token_account.clone(),
            mint_authority.clone(),
            token_program.clone(),
        ],
        &[seeds],
    )?;

    // 4. CABUT mint authority — permanen, tidak bisa dibatalkan
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

    // 5. CABUT freeze authority — tanpa ini kreator masih bisa membekukan
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
pub fn transfer_from_curve<'a>(
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
    let bump = assert_mint_authority(program_id, mint.key, mint_authority)?;
    let seeds: &[&[u8]] = &[MINT_AUTHORITY_SEED, mint.key.as_array(), &[bump]];

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
pub fn transfer_to_curve<'a>(
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
pub fn burn_from_curve<'a>(
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
    let bump = assert_mint_authority(program_id, mint.key, mint_authority)?;
    let seeds: &[&[u8]] = &[MINT_AUTHORITY_SEED, mint.key.as_array(), &[bump]];

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
