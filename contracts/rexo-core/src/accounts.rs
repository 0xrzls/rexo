// Copyright (c) 2026 Rexo
// SPDX-License-Identifier: Apache-2.0

//! Parsing dan validasi akun.
//!
//! # Kenapa modul ini ada
//!
//! Manifest program Venus (lihat `wit/*-manifest.json`) mendeklarasikan akun
//! per-instruksi dengan tiga sumber: `parameter`, `derived`, dan
//! `well_known`. Akun yang selalu ada: `payer` (signer, writable),
//! `workflow_pda` (derived, owner = program_id), dan `system_program`.
//!
//! Modul ini mengubah slice `&[AccountInfo]` mentah menjadi struct bernama,
//! dan memverifikasi setiap PDA sebelum dipakai. Pola ini disengaja: dengan
//! begitu satu-satunya bagian yang belum terverifikasi (cara macro Venus
//! menyerahkan slice akun ke badan fungsi) terisolasi di SATU baris di
//! `lib.rs`, bukan tersebar ke seluruh program.
//!
//! Kalau accessor-nya ternyata berbeda, yang perlu kamu ubah cuma baris itu.
//! Seluruh validasi di bawah tetap benar.

use rialo_s_program::{
    account_info::{next_account_info, AccountInfo},
    program_error::ProgramError,
    pubkey::Pubkey,
    system_program,
};

use crate::errors::RexoError;
use crate::{token, vault};

/// Akun untuk `launch`.
pub struct LaunchAccounts<'a, 'info> {
    pub payer: &'a AccountInfo<'info>,
    pub workflow: &'a AccountInfo<'info>,
    pub mint: &'a AccountInfo<'info>,
    pub mint_authority: &'a AccountInfo<'info>,
    pub curve_token_account: &'a AccountInfo<'info>,
    pub vault: &'a AccountInfo<'info>,
    /// Tujuan sapuan fee protokol. Wajib ada di `launch` karena dev-buy
    /// sudah menghasilkan fee di transaksi yang sama — tanpa akun ini fee
    /// itu nyangkut di vault selamanya.
    pub treasury: &'a AccountInfo<'info>,
    pub creator_vault: &'a AccountInfo<'info>,
    pub token_program: &'a AccountInfo<'info>,
    pub system_program: &'a AccountInfo<'info>,
}

impl<'a, 'info> LaunchAccounts<'a, 'info> {
    pub fn parse(
        program_id: &Pubkey,
        accounts: &'a [AccountInfo<'info>],
    ) -> Result<Self, ProgramError> {
        let iter = &mut accounts.iter();
        let me = Self {
            payer: next_account_info(iter)?,
            workflow: next_account_info(iter)?,
            mint: next_account_info(iter)?,
            mint_authority: next_account_info(iter)?,
            curve_token_account: next_account_info(iter)?,
            vault: next_account_info(iter)?,
            treasury: next_account_info(iter)?,
            creator_vault: next_account_info(iter)?,
            token_program: next_account_info(iter)?,
            system_program: next_account_info(iter)?,
        };
        me.validate(program_id)?;
        Ok(me)
    }

    fn validate(&self, program_id: &Pubkey) -> Result<(), ProgramError> {
        if !self.payer.is_signer {
            return Err(RexoError::Unauthorized.into());
        }
        if !system_program::check_id(self.system_program.key) {
            return Err(ProgramError::IncorrectProgramId);
        }
        token::assert_mint_authority(program_id, self.mint.key, self.mint_authority)?;
        // vault belum tentu ada saat launch, jadi cek turunannya saja
        let (expected_vault, _) = vault::derive_vault(program_id, self.mint.key);
        if expected_vault != *self.vault.key {
            return Err(RexoError::InvalidVault.into());
        }
        let (expected_creator_vault, _) = vault::derive_creator_vault(program_id, self.payer.key);
        if expected_creator_vault != *self.creator_vault.key {
            return Err(RexoError::InvalidVault.into());
        }
        Ok(())
    }
}

/// Akun untuk `buy` dan `sell`.
pub struct TradeAccounts<'a, 'info> {
    pub trader: &'a AccountInfo<'info>,
    pub workflow: &'a AccountInfo<'info>,
    pub mint: &'a AccountInfo<'info>,
    pub mint_authority: &'a AccountInfo<'info>,
    pub curve_token_account: &'a AccountInfo<'info>,
    pub trader_token_account: &'a AccountInfo<'info>,
    pub vault: &'a AccountInfo<'info>,
    pub treasury: &'a AccountInfo<'info>,
    pub creator_vault: &'a AccountInfo<'info>,
    pub token_program: &'a AccountInfo<'info>,
    pub system_program: &'a AccountInfo<'info>,
}

impl<'a, 'info> TradeAccounts<'a, 'info> {
    pub fn parse(
        program_id: &Pubkey,
        accounts: &'a [AccountInfo<'info>],
    ) -> Result<Self, ProgramError> {
        let iter = &mut accounts.iter();
        let me = Self {
            trader: next_account_info(iter)?,
            workflow: next_account_info(iter)?,
            mint: next_account_info(iter)?,
            mint_authority: next_account_info(iter)?,
            curve_token_account: next_account_info(iter)?,
            trader_token_account: next_account_info(iter)?,
            vault: next_account_info(iter)?,
            treasury: next_account_info(iter)?,
            creator_vault: next_account_info(iter)?,
            token_program: next_account_info(iter)?,
            system_program: next_account_info(iter)?,
        };
        me.validate(program_id)?;
        Ok(me)
    }

    fn validate(&self, program_id: &Pubkey) -> Result<(), ProgramError> {
        if !self.trader.is_signer {
            return Err(RexoError::Unauthorized.into());
        }
        if !system_program::check_id(self.system_program.key) {
            return Err(ProgramError::IncorrectProgramId);
        }
        // INI cek yang mencegah pencurian vault. Tanpa ini, penyerang
        // mengirim akun miliknya sebagai "vault" dan menguras hasil kurva.
        vault::assert_vault(program_id, self.mint.key, self.vault)?;
        token::assert_mint_authority(program_id, self.mint.key, self.mint_authority)?;
        Ok(())
    }
}

/// Akun untuk `graduate`.
pub struct GraduateAccounts<'a, 'info> {
    pub payer: &'a AccountInfo<'info>,
    pub workflow: &'a AccountInfo<'info>,
    pub mint: &'a AccountInfo<'info>,
    pub mint_authority: &'a AccountInfo<'info>,
    pub curve_token_account: &'a AccountInfo<'info>,
    pub vault: &'a AccountInfo<'info>,
    pub pool_token_account: &'a AccountInfo<'info>,
    pub pool_quote_account: &'a AccountInfo<'info>,
    pub creator: &'a AccountInfo<'info>,
    pub token_program: &'a AccountInfo<'info>,
    pub system_program: &'a AccountInfo<'info>,
}

impl<'a, 'info> GraduateAccounts<'a, 'info> {
    pub fn parse(
        program_id: &Pubkey,
        accounts: &'a [AccountInfo<'info>],
    ) -> Result<Self, ProgramError> {
        let iter = &mut accounts.iter();
        let me = Self {
            payer: next_account_info(iter)?,
            workflow: next_account_info(iter)?,
            mint: next_account_info(iter)?,
            mint_authority: next_account_info(iter)?,
            curve_token_account: next_account_info(iter)?,
            vault: next_account_info(iter)?,
            pool_token_account: next_account_info(iter)?,
            pool_quote_account: next_account_info(iter)?,
            creator: next_account_info(iter)?,
            token_program: next_account_info(iter)?,
            system_program: next_account_info(iter)?,
        };
        vault::assert_vault(program_id, me.mint.key, me.vault)?;
        token::assert_mint_authority(program_id, me.mint.key, me.mint_authority)?;
        Ok(me)
    }
}
