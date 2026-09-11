// Copyright (c) 2026 Rexo
// SPDX-License-Identifier: Apache-2.0

//! Logika bisnis, dikelompokkan per peran seperti Meteora DBC.
//!
//! ```text
//! Partner   create_config · update_config · claim_partner_fee
//! Creator   initialize · claim_creator_fee · claim_creator_tokens
//! Trading   buy_exact_in · buy_exact_out · sell_exact_in · sell_exact_out
//! Migrasi   migrate
//! Protokol  claim_protocol_fee
//! ```
//!
//! Semua fungsi menerima `&AccountInfo` eksplisit. Tidak ada yang
//! tersembunyi di balik macro — bagian inilah yang diaudit, dan auditor
//! harus bisa membacanya tanpa memahami DSL Venus lebih dulu.

use rialo_s_program::{account_info::AccountInfo, msg, pubkey::Pubkey};

use crate::config::{ConfigPatch, LaunchConfig, MIGRATE_EXTERNAL};
use crate::errors::RexoError;
use crate::fees::{self, Payee};
use crate::state::{narrow, Launch, STATE_MIGRATED, STATE_MIGRATING};
use crate::{token, vault};

pub type Res<T> = Result<T, rialo_s_program::program_error::ProgramError>;

// ===========================================================================
// PARTNER
// ===========================================================================

/// Buat config. Ini yang membuat satu program melayani banyak launchpad.
pub fn create_config(cfg: &LaunchConfig) -> Res<()> {
    cfg.validate()?;
    msg!(
        "rexo::config created fee_total={} partner={}",
        cfg.fees.total_bps,
        cfg.fees.partner_bps
    );
    Ok(())
}

/// Ubah config. Kurva dan supply ditolak — peluncuran yang sudah jalan
/// memegang salinannya, dan mengubahnya membuat harga historis tidak bisa
/// direproduksi.
pub fn update_config(cfg: &mut LaunchConfig, patch: &ConfigPatch) -> Res<()> {
    cfg.apply_update(patch)?;
    msg!("rexo::config updated");
    Ok(())
}

// ===========================================================================
// CREATOR — peluncuran
// ===========================================================================

pub struct InitAccounts<'a, 'info> {
    pub payer: &'a AccountInfo<'info>,
    pub launch: &'a AccountInfo<'info>,
    pub mint: &'a AccountInfo<'info>,
    pub authority: &'a AccountInfo<'info>,
    pub base_vault: &'a AccountInfo<'info>,
    pub quote_vault: &'a AccountInfo<'info>,
    pub creator_token_account: &'a AccountInfo<'info>,
    pub token_program: &'a AccountInfo<'info>,
    pub system_program: &'a AccountInfo<'info>,
}

pub struct InitResult {
    pub launch: Launch,
    pub creator_tokens: u64,
    pub quote_spent: u64,
}

/// Luncurkan token dari sebuah config.
///
/// Urutan disengaja: vault dulu, lalu mint + cabut authority, baru
/// pembelian kreator. Pembelian kreator terjadi di transaksi yang SAMA
/// dengan pembuatan mint supaya tidak ada celah bagi sniper untuk masuk
/// di antaranya.
pub fn initialize(
    program_id: &Pubkey,
    acc: &InitAccounts<'_, '_>,
    cfg: &LaunchConfig,
    creator_buy: u64,
    now: u64,
) -> Res<InitResult> {
    cfg.validate()?;
    let mut l = Launch::open(cfg, now)?;

    vault::ensure_quote_vault(
        program_id,
        acc.launch.key,
        acc.payer,
        acc.quote_vault,
        acc.system_program,
    )?;

    // Membuat mint, mencetak seluruh supply ke base_vault, lalu MENCABUT
    // mint authority dan freeze authority. Permanen. Setelah ini supply
    // tetap selamanya dan tidak ada yang bisa membekukan dompet siapa pun.
    token::create_mint_and_lock(
        program_id,
        acc.payer,
        acc.mint,
        acc.authority,
        acc.base_vault,
        acc.token_program,
        acc.system_program,
        cfg.total_supply(),
    )?;

    let mut creator_tokens = 0u64;
    let mut quote_spent = 0u64;

    if creator_buy > 0 {
        let ccfg = l.curve_config();
        let mut c = l.curve();
        let r = c.buy(&ccfg, creator_buy as u128, 0)?;
        creator_tokens = narrow(r.tokens_out)?;

        if creator_tokens > cfg.max_creator_tokens() {
            msg!(
                "rexo::init creator buy {} exceeds cap {}",
                creator_tokens,
                cfg.max_creator_tokens()
            );
            return Err(RexoError::CreatorCapExceeded.into());
        }

        quote_spent = narrow(r.quote_spent)?;
        let fee = narrow(r.fee)?;
        let shares = fees::split(fee, &l.fees, false)?;

        l.apply(&c)?;
        l.ledger.accrue(&shares)?;
        // Referral tidak berlaku di pembelian kreator, jadi bagiannya ikut
        // protokol — `split` dengan referrer_present=false sudah menangani.
        vault::deposit(acc.payer, acc.quote_vault, acc.system_program, quote_spent)?;

        // Token kreator TIDAK langsung dikirim. Ia masuk alokasi yang
        // tunduk pada jadwal vesting dan baru cair setelah migrasi.
        l.creator_allocation = creator_tokens;
    }

    msg!(
        "rexo::init mint={} creator_tokens={} spent={}",
        acc.mint.key,
        creator_tokens,
        quote_spent
    );

    Ok(InitResult {
        launch: l,
        creator_tokens,
        quote_spent,
    })
}

// ===========================================================================
// TRADING
// ===========================================================================

pub struct TradeAccounts<'a, 'info> {
    pub trader: &'a AccountInfo<'info>,
    pub launch: &'a AccountInfo<'info>,
    pub mint: &'a AccountInfo<'info>,
    pub authority: &'a AccountInfo<'info>,
    pub base_vault: &'a AccountInfo<'info>,
    pub quote_vault: &'a AccountInfo<'info>,
    pub trader_token_account: &'a AccountInfo<'info>,
    /// Opsional. Kalau tidak ada, bagian referral ikut ke protokol —
    /// bukan dikembalikan ke trader, supaya harga tidak berbeda tergantung
    /// siapa yang mengirim order.
    pub referrer: Option<&'a AccountInfo<'info>>,
    pub token_program: &'a AccountInfo<'info>,
    pub system_program: &'a AccountInfo<'info>,
}

pub struct TradeResult {
    pub base_amount: u64,
    pub quote_amount: u64,
    pub fee: u64,
    pub refund: u64,
    pub completed_curve: bool,
}

/// Beli dengan jumlah quote yang pasti.
pub fn buy_exact_in(
    program_id: &Pubkey,
    acc: &TradeAccounts<'_, '_>,
    l: &mut Launch,
    quote_in: u64,
    min_base_out: u64,
) -> Res<TradeResult> {
    if !l.is_funding() {
        return Err(RexoError::NotFunding.into());
    }
    let ccfg = l.curve_config();
    let mut c = l.curve();
    let r = c.buy(&ccfg, quote_in as u128, min_base_out as u128)?;

    let base_out = narrow(r.tokens_out)?;
    let spent = narrow(r.quote_spent)?;
    let fee = narrow(r.fee)?;
    let refund = narrow(r.refund)?;
    let shares = fees::split(fee, &l.fees, acc.referrer.is_some())?;

    l.apply(&c)?;
    l.ledger.accrue(&shares)?;

    settle_buy(program_id, acc, spent, base_out, shares.referral)?;

    let completed = l.real_token == 0;
    if completed {
        l.state = STATE_MIGRATING;
    }

    msg!(
        "rexo::buy in={} out={} fee={} refund={}",
        spent,
        base_out,
        fee,
        refund
    );
    Ok(TradeResult {
        base_amount: base_out,
        quote_amount: spent,
        fee,
        refund,
        completed_curve: completed,
    })
}

/// Beli sejumlah token yang pasti, dengan batas atas belanja.
///
/// Tanpa varian ini kamu tidak bisa bilang "aku mau tepat 1 juta token".
/// LaunchLab punya `buy_exact_out`; v1 Rexo tidak, dan itu kekurangan
/// nyata bukan sekadar kenyamanan.
pub fn buy_exact_out(
    program_id: &Pubkey,
    acc: &TradeAccounts<'_, '_>,
    l: &mut Launch,
    base_out: u64,
    max_quote_in: u64,
) -> Res<TradeResult> {
    if !l.is_funding() {
        return Err(RexoError::NotFunding.into());
    }
    let ccfg = l.curve_config();
    let c = l.curve();
    let gross = narrow(c.quote_in_for_tokens_out(&ccfg, base_out as u128)?)?;
    if gross > max_quote_in {
        msg!("rexo::buy_exact_out needs {} max {}", gross, max_quote_in);
        return Err(RexoError::ExceedsMaxIn.into());
    }
    // Dieksekusi lewat jalur exact_in dengan jumlah yang sudah dihitung,
    // sehingga hanya ada SATU implementasi matematika kurva yang dipakai
    // kedua arah. Dua implementasi paralel adalah cara termudah agar
    // keduanya perlahan menyimpang.
    buy_exact_in(program_id, acc, l, gross, base_out)
}

pub fn sell_exact_in(
    program_id: &Pubkey,
    acc: &TradeAccounts<'_, '_>,
    l: &mut Launch,
    base_in: u64,
    min_quote_out: u64,
) -> Res<TradeResult> {
    if !l.is_funding() {
        return Err(RexoError::NotFunding.into());
    }
    let ccfg = l.curve_config();
    let mut c = l.curve();
    let r = c.sell(&ccfg, base_in as u128, min_quote_out as u128)?;

    let quote_out = narrow(r.quote_out)?;
    let fee = narrow(r.fee)?;
    let shares = fees::split(fee, &l.fees, acc.referrer.is_some())?;

    l.apply(&c)?;
    l.ledger.accrue(&shares)?;

    settle_sell(program_id, acc, base_in, quote_out, shares.referral)?;

    msg!("rexo::sell in={} out={} fee={}", base_in, quote_out, fee);
    Ok(TradeResult {
        base_amount: base_in,
        quote_amount: quote_out,
        fee,
        refund: 0,
        completed_curve: false,
    })
}

/// Jual sampai menerima sejumlah quote yang pasti.
pub fn sell_exact_out(
    program_id: &Pubkey,
    acc: &TradeAccounts<'_, '_>,
    l: &mut Launch,
    quote_out: u64,
    max_base_in: u64,
) -> Res<TradeResult> {
    if !l.is_funding() {
        return Err(RexoError::NotFunding.into());
    }
    let ccfg = l.curve_config();
    let c = l.curve();
    let base_in = narrow(c.tokens_in_for_quote_out(&ccfg, quote_out as u128)?)?;
    if base_in > max_base_in {
        msg!("rexo::sell_exact_out needs {} max {}", base_in, max_base_in);
        return Err(RexoError::ExceedsMaxIn.into());
    }
    sell_exact_in(program_id, acc, l, base_in, quote_out)
}

// -- pemindahan dana ------------------------------------------------------
//
// Fee TIDAK dipindahkan di sini. Ia menumpuk di quote_vault dan dicatat di
// ledger; penerima menariknya lewat claim_*. Hanya referral yang dibayar
// seketika, karena penerimanya berbeda tiap perdagangan.

fn settle_buy(
    program_id: &Pubkey,
    acc: &TradeAccounts<'_, '_>,
    quote_spent: u64,
    base_out: u64,
    referral: u64,
) -> Res<()> {
    vault::deposit(
        acc.trader,
        acc.quote_vault,
        acc.system_program,
        quote_spent,
    )?;
    if let Some(r) = acc.referrer {
        vault::withdraw(acc.quote_vault, r, referral)?;
    }
    token::transfer_out(
        program_id,
        acc.mint,
        acc.base_vault,
        acc.trader_token_account,
        acc.authority,
        acc.token_program,
        base_out,
    )
}

fn settle_sell(
    program_id: &Pubkey,
    acc: &TradeAccounts<'_, '_>,
    base_in: u64,
    quote_out: u64,
    referral: u64,
) -> Res<()> {
    // Token masuk dulu. Kalau transfer ini gagal, transaksi batal sebelum
    // satu kelvin pun meninggalkan vault.
    token::transfer_in(
        acc.mint,
        acc.trader_token_account,
        acc.base_vault,
        acc.trader,
        acc.token_program,
        base_in,
    )?;
    vault::withdraw(acc.quote_vault, acc.trader, quote_out)?;
    if let Some(r) = acc.referrer {
        vault::withdraw(acc.quote_vault, r, referral)?;
    }
    let _ = program_id;
    Ok(())
}

// ===========================================================================
// KLAIM FEE — pola tarik
// ===========================================================================

pub fn claim_fee<'a>(
    l: &mut Launch,
    who: Payee,
    quote_vault: &AccountInfo<'a>,
    recipient: &AccountInfo<'a>,
) -> Res<u64> {
    let amount = l.ledger.take(who);
    if amount == 0 {
        return Err(RexoError::NoFeesToClaim.into());
    }
    vault::withdraw(quote_vault, recipient, amount)?;
    msg!("rexo::claim {:?} amount={}", who, amount);
    Ok(amount)
}

// ===========================================================================
// KLAIM TOKEN KREATOR — tunduk vesting
// ===========================================================================

pub fn claim_creator_tokens<'a>(
    program_id: &Pubkey,
    l: &mut Launch,
    _launch_key: &Pubkey,
    mint: &AccountInfo<'a>,
    base_vault: &AccountInfo<'a>,
    creator_token_account: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    now: u64,
) -> Res<u64> {
    let claimable = l.creator_claimable(now);
    if claimable == 0 {
        return Err(RexoError::NothingToClaim.into());
    }
    l.creator_claimed = l
        .creator_claimed
        .checked_add(claimable)
        .ok_or(RexoError::MathOverflow)?;

    token::transfer_out(
        program_id,
        mint,
        base_vault,
        creator_token_account,
        authority,
        token_program,
        claimable,
    )?;
    msg!("rexo::creator_claim amount={}", claimable);
    Ok(claimable)
}

// ===========================================================================
// MIGRASI
// ===========================================================================

pub struct MigrateResult {
    pub lp_base: u64,
    pub lp_quote: u64,
}

/// Pindahkan likuiditas keluar dari kurva.
///
/// Dipicu `AFTER n seconds CALL [migrate]` yang didaftarkan saat kurva
/// habis. **Tidak ada keeper.** Meteora butuh `dbc-keeper` untuk langkah
/// ini; LaunchLab butuh seseorang memanggil `migrate_to_amm`. Ini satu
/// keunggulan Rialo yang bisa dipakai hari ini dengan sintaks yang sudah
/// terbukti.
pub fn migrate<'a>(
    program_id: &Pubkey,
    l: &mut Launch,
    _launch_key: &Pubkey,
    mint: &AccountInfo<'a>,
    base_vault: &AccountInfo<'a>,
    quote_vault: &AccountInfo<'a>,
    lp_base_dest: &AccountInfo<'a>,
    lp_quote_dest: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    now: u64,
) -> Res<MigrateResult> {
    if !l.is_migrating() {
        return Err(RexoError::NotMigrating.into());
    }
    // Sabuk dan bretel: state MIGRATING harus konsisten dengan reserve.
    if l.real_token != 0 {
        msg!("rexo::migrate rejected real_token={}", l.real_token);
        return Err(RexoError::NotMigrating.into());
    }

    let lp_base = l.cfg_lp_reserve;
    // Hanya reserve kurva yang pindah. Fee yang belum ditarik tetap di
    // vault — menariknya ke pool akan mencuri hak partner dan kreator.
    let lp_quote = l.real_quote;

    token::transfer_out(
        program_id,
        mint,
        base_vault,
        lp_base_dest,
        authority,
        token_program,
        lp_base,
    )?;
    vault::withdraw(quote_vault, lp_quote_dest, lp_quote)?;

    l.real_quote = 0;
    l.state = STATE_MIGRATED;
    l.migrated_at = now;

    if l.migrate_target == MIGRATE_EXTERNAL {
        // Tujuan eksternal berarti akun tujuan sudah menerima aset dan
        // integrator yang membuat pool-nya. Program ini tidak berpura-pura
        // tahu cara membuat pool di DEX yang belum ada di Rialo.
        msg!("rexo::migrate external base={} quote={}", lp_base, lp_quote);
    } else {
        msg!("rexo::migrate hold base={} quote={}", lp_base, lp_quote);
    }

    Ok(MigrateResult { lp_base, lp_quote })
}

// ===========================================================================
// Tests logika murni
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{LaunchConfig, ONE_QUOTE};
    use crate::fees::Payee;

    fn launch() -> Launch {
        Launch::open(&LaunchConfig::pumpfun_like(), 0).unwrap()
    }

    /// Simulasi murni: perdagangan menambah reserve dan ledger, saldo vault
    /// harus selalu sama dengan reserve + fee yang belum ditarik.
    #[test]
    fn vault_invariant_survives_mixed_trading() {
        let mut l = launch();
        let ccfg = l.curve_config();
        let mut vault: u64 = 0;
        let mut held: u128 = 0;

        for i in 1..=60u64 {
            let mut c = l.curve();
            if i % 4 == 0 && held > 1_000_000 {
                let amt = held / 4;
                let r = c.sell(&ccfg, amt, 0).unwrap();
                let fee = narrow(r.fee).unwrap();
                let out = narrow(r.quote_out).unwrap();
                let s = fees::split(fee, &l.fees, false).unwrap();
                l.apply(&c).unwrap();
                l.ledger.accrue(&s).unwrap();
                vault -= out + s.referral;
                held -= amt;
            } else {
                let q = i * ONE_QUOTE / 4;
                let r = c.buy(&ccfg, q as u128, 0).unwrap();
                let fee = narrow(r.fee).unwrap();
                let spent = narrow(r.quote_spent).unwrap();
                let s = fees::split(fee, &l.fees, false).unwrap();
                l.apply(&c).unwrap();
                l.ledger.accrue(&s).unwrap();
                vault += spent - s.referral;
                held += r.tokens_out;
            }
            assert_eq!(
                vault,
                l.expected_quote_balance(),
                "invariant pecah di langkah {}",
                i
            );
        }

        // tarik semua fee, sisa vault harus persis reserve kurva
        for w in [Payee::Protocol, Payee::Partner, Payee::Creator] {
            vault -= l.ledger.take(w);
        }
        assert_eq!(vault, l.real_quote);
        assert_eq!(l.ledger.outstanding(), 0);
    }

    #[test]
    fn migration_leaves_unclaimed_fees_behind() {
        let mut l = launch();
        let ccfg = l.curve_config();
        let mut c = l.curve();
        let r = c.buy(&ccfg, 200 * ONE_QUOTE as u128, 0).unwrap();
        let fee = narrow(r.fee).unwrap();
        l.apply(&c).unwrap();
        l.ledger.accrue(&fees::split(fee, &l.fees, false).unwrap())
            .unwrap();
        l.state = STATE_MIGRATING;

        let reserve = l.real_quote;
        let owed = l.ledger.outstanding();
        assert!(owed > 0);
        // Yang pindah ke pool hanya reserve. Fee tetap bisa ditarik.
        assert_eq!(reserve, 85_005_359_057);
        assert_eq!(l.expected_quote_balance(), reserve + owed);
    }

    #[test]
    fn trading_blocked_once_curve_completes() {
        let mut l = launch();
        l.state = STATE_MIGRATING;
        assert!(!l.is_funding());
    }
}
