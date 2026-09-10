// Copyright (c) 2026 Rexo
// SPDX-License-Identifier: Apache-2.0

//! Parsing dan validasi akun.
//!
//! Urutan akun adalah bagian dari ABI: `next_account_info` mengambilnya
//! berurutan. Setiap PDA diverifikasi sebelum dipakai — tanpa itu,
//! penyerang bisa mengirim akun miliknya sendiri sebagai "vault".

use rialo_s_program::{
    account_info::{next_account_info, AccountInfo},
    program_error::ProgramError,
    pubkey::Pubkey,
    system_program,
};

use crate::config::LaunchConfig;
use crate::errors::RexoError;
use crate::ops::{InitAccounts, TradeAccounts};
use crate::{token, vault};

/// Baca LaunchConfig dari akun. Deserialisasi memakai bincode+serde,
/// sama seperti state workflow Venus.
pub fn read_config(_account: &AccountInfo<'_>) -> Result<LaunchConfig, ProgramError> {
    // TODO(config-account): saat ini config dibaca sebagai preset default.
    // Layout serialisasinya menunggu konfirmasi bentuk yang dipakai Venus
    // untuk akun non-workflow. Sampai itu jelas, memakai preset lebih jujur
    // daripada mem-parse byte dengan asumsi yang belum diverifikasi.
    Ok(LaunchConfig::pumpfun_like())
}

fn expect_system(a: &AccountInfo<'_>) -> Result<(), ProgramError> {
    if !system_program::check_id(a.key) {
        return Err(ProgramError::IncorrectProgramId);
    }
    Ok(())
}

pub struct InitParsed<'a, 'info> {
    pub inner: InitAccounts<'a, 'info>,
    pub config: &'a AccountInfo<'info>,
}

impl<'a, 'info> core::ops::Deref for InitParsed<'a, 'info> {
    type Target = InitAccounts<'a, 'info>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

pub fn parse_init<'a, 'info>(
    program_id: &Pubkey,
    accounts: &'a [AccountInfo<'info>],
) -> Result<InitParsed<'a, 'info>, ProgramError> {
    let it = &mut accounts.iter();
    let payer = next_account_info(it)?;
    let config = next_account_info(it)?;
    let launch = next_account_info(it)?;
    let mint = next_account_info(it)?;
    let authority = next_account_info(it)?;
    let base_vault = next_account_info(it)?;
    let quote_vault = next_account_info(it)?;
    let creator_token_account = next_account_info(it)?;
    let token_program = next_account_info(it)?;
    let sysprog = next_account_info(it)?;

    if !payer.is_signer {
        return Err(RexoError::Unauthorized.into());
    }
    expect_system(sysprog)?;
    check_pdas(program_id, launch.key, authority, base_vault, quote_vault)?;

    Ok(InitParsed {
        inner: InitAccounts {
            payer,
            launch,
            mint,
            authority,
            base_vault,
            quote_vault,
            creator_token_account,
            token_program,
            system_program: sysprog,
        },
        config,
    })
}

pub fn parse_trade<'a, 'info>(
    program_id: &Pubkey,
    accounts: &'a [AccountInfo<'info>],
) -> Result<TradeAccounts<'a, 'info>, ProgramError> {
    let it = &mut accounts.iter();
    let trader = next_account_info(it)?;
    let launch = next_account_info(it)?;
    let mint = next_account_info(it)?;
    let authority = next_account_info(it)?;
    let base_vault = next_account_info(it)?;
    let quote_vault = next_account_info(it)?;
    let trader_token_account = next_account_info(it)?;
    let token_program = next_account_info(it)?;
    let sysprog = next_account_info(it)?;
    // Referrer opsional: kalau ada akun tersisa, itu dia.
    let referrer = it.next();

    if !trader.is_signer {
        return Err(RexoError::Unauthorized.into());
    }
    expect_system(sysprog)?;
    check_pdas(program_id, launch.key, authority, base_vault, quote_vault)?;

    Ok(TradeAccounts {
        trader,
        launch,
        mint,
        authority,
        base_vault,
        quote_vault,
        trader_token_account,
        referrer,
        token_program,
        system_program: sysprog,
    })
}

pub struct ClaimParsed<'a, 'info> {
    pub signer: &'a AccountInfo<'info>,
    pub launch: &'a AccountInfo<'info>,
    pub quote_vault: &'a AccountInfo<'info>,
    pub recipient: &'a AccountInfo<'info>,
}

pub fn parse_claim<'a, 'info>(
    program_id: &Pubkey,
    accounts: &'a [AccountInfo<'info>],
) -> Result<ClaimParsed<'a, 'info>, ProgramError> {
    let it = &mut accounts.iter();
    let signer = next_account_info(it)?;
    let launch = next_account_info(it)?;
    let quote_vault = next_account_info(it)?;
    let recipient = next_account_info(it)?;

    if !signer.is_signer {
        return Err(RexoError::Unauthorized.into());
    }
    vault::assert_quote_vault(program_id, launch.key, quote_vault)?;
    Ok(ClaimParsed {
        signer,
        launch,
        quote_vault,
        recipient,
    })
}

pub struct CreatorClaimParsed<'a, 'info> {
    pub signer: &'a AccountInfo<'info>,
    pub launch: &'a AccountInfo<'info>,
    pub mint: &'a AccountInfo<'info>,
    pub authority: &'a AccountInfo<'info>,
    pub base_vault: &'a AccountInfo<'info>,
    pub creator_token_account: &'a AccountInfo<'info>,
    pub token_program: &'a AccountInfo<'info>,
}

pub fn parse_creator_claim<'a, 'info>(
    program_id: &Pubkey,
    accounts: &'a [AccountInfo<'info>],
) -> Result<CreatorClaimParsed<'a, 'info>, ProgramError> {
    let it = &mut accounts.iter();
    let signer = next_account_info(it)?;
    let launch = next_account_info(it)?;
    let mint = next_account_info(it)?;
    let authority = next_account_info(it)?;
    let base_vault = next_account_info(it)?;
    let creator_token_account = next_account_info(it)?;
    let token_program = next_account_info(it)?;

    if !signer.is_signer {
        return Err(RexoError::Unauthorized.into());
    }
    token::assert_authority(program_id, launch.key, authority)?;
    let (expected, _) = token::derive_base_vault(program_id, launch.key);
    if expected != *base_vault.key {
        return Err(RexoError::InvalidVault.into());
    }
    Ok(CreatorClaimParsed {
        signer,
        launch,
        mint,
        authority,
        base_vault,
        creator_token_account,
        token_program,
    })
}

pub struct MigrateParsed<'a, 'info> {
    pub launch: &'a AccountInfo<'info>,
    pub mint: &'a AccountInfo<'info>,
    pub authority: &'a AccountInfo<'info>,
    pub base_vault: &'a AccountInfo<'info>,
    pub quote_vault: &'a AccountInfo<'info>,
    pub lp_base_dest: &'a AccountInfo<'info>,
    pub lp_quote_dest: &'a AccountInfo<'info>,
    pub token_program: &'a AccountInfo<'info>,
}

pub fn parse_migrate<'a, 'info>(
    program_id: &Pubkey,
    accounts: &'a [AccountInfo<'info>],
) -> Result<MigrateParsed<'a, 'info>, ProgramError> {
    let it = &mut accounts.iter();
    let launch = next_account_info(it)?;
    let mint = next_account_info(it)?;
    let authority = next_account_info(it)?;
    let base_vault = next_account_info(it)?;
    let quote_vault = next_account_info(it)?;
    let lp_base_dest = next_account_info(it)?;
    let lp_quote_dest = next_account_info(it)?;
    let token_program = next_account_info(it)?;

    check_pdas(program_id, launch.key, authority, base_vault, quote_vault)?;
    Ok(MigrateParsed {
        launch,
        mint,
        authority,
        base_vault,
        quote_vault,
        lp_base_dest,
        lp_quote_dest,
        token_program,
    })
}

/// Cek yang mencegah pencurian vault. Tanpa ini, penyerang mengirim akun
/// miliknya sebagai vault dan menguras hasil kurva.
fn check_pdas(
    program_id: &Pubkey,
    launch: &Pubkey,
    authority: &AccountInfo<'_>,
    base_vault: &AccountInfo<'_>,
    quote_vault: &AccountInfo<'_>,
) -> Result<(), ProgramError> {
    token::assert_authority(program_id, launch, authority)?;
    let (eb, _) = token::derive_base_vault(program_id, launch);
    if eb != *base_vault.key {
        return Err(RexoError::InvalidVault.into());
    }
    let (eq, _) = vault::derive_quote_vault(program_id, launch);
    if eq != *quote_vault.key {
        return Err(RexoError::InvalidVault.into());
    }
    Ok(())
}
