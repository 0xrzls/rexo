// Copyright (c) 2026 Rexo
// SPDX-License-Identifier: Apache-2.0

//! Logika bisnis inti.
//!
//! Semua fungsi di sini adalah fungsi bebas yang menerima `&AccountInfo`
//! secara eksplisit. Tidak ada yang tersembunyi di balik macro. Ini
//! disengaja: bagian inilah yang akan diaudit, dan auditor harus bisa
//! membacanya tanpa memahami DSL Venus lebih dulu.
//!
//! `lib.rs` di atas modul ini tipis — ia hanya menghubungkan DSL ke sini.

use rialo_s_program::{account_info::AccountInfo, entrypoint::ProgramResult, msg, pubkey::Pubkey};

use crate::accounts::{GraduateAccounts, LaunchAccounts, TradeAccounts};
use crate::constants::*;
use crate::errors::RexoError;
use crate::events::*;
use crate::guards;
use crate::state::{narrow, LaunchView};
use crate::{token, vault};

// ---------------------------------------------------------------------------
// Launch
// ---------------------------------------------------------------------------

pub struct LaunchParams {
    pub bond: u64,
    pub dev_buy: u64,
    pub now: u64,
}

pub struct LaunchOutcome {
    pub view: LaunchView,
    pub sealed_until: u64,
    pub dev_tokens: u64,
}

/// Buat token, kunci authority-nya, buka jendela sealed.
///
/// Tier SELALU mulai di Unverified. Ia hanya bisa naik lewat
/// `apply_verification` setelah REX mengembalikan bukti.
pub fn launch(
    program_id: &Pubkey,
    acc: &LaunchAccounts<'_, '_>,
    params: LaunchParams,
) -> Result<LaunchOutcome, ProgramErrorAlias> {
    guards::assert_tier_not_self_assigned(None)?;

    // 1. Vault harus ada sebelum ada dana yang masuk.
    vault::ensure_vault(
        program_id,
        acc.mint.key,
        acc.payer,
        acc.vault,
        acc.system_program,
    )?;

    // 2. Mint + cetak seluruh supply + CABUT authority. Satu transaksi.
    token::create_mint_and_lock(
        program_id,
        acc.payer,
        acc.mint,
        acc.mint_authority,
        acc.curve_token_account,
        acc.token_program,
        acc.system_program,
        TOTAL_SUPPLY,
    )?;

    // 3. Bond masuk vault. Dikembalikan saat lulus, hangus kalau ditinggalkan.
    guards::require_bond(TIER_UNVERIFIED, params.bond)?;
    vault::deposit(acc.payer, acc.vault, acc.system_program, params.bond)?;

    let mut view = LaunchView::genesis(TIER_UNVERIFIED);
    let sealed_until = params.now.saturating_add(SEALED_WINDOW_SECS);

    // 4. Dev buy opsional, dibatasi tier. Dilakukan di transaksi yang sama
    //    dengan pembuatan mint supaya tidak ada celah untuk sniper.
    let mut dev_tokens = 0u64;
    if params.dev_buy > 0 {
        let cfg = view.config();
        let mut curve = view.curve();
        let receipt = curve.buy(&cfg, params.dev_buy as u128, 0)?;
        dev_tokens = narrow(receipt.tokens_out)?;
        guards::require_creator_allocation(TIER_UNVERIFIED, dev_tokens)?;

        view.apply(&curve)?;
        vault::deposit(
            acc.payer,
            acc.vault,
            acc.system_program,
            narrow(receipt.quote_spent)?,
        )?;

        // Sapu fee dev-buy SEKARANG. Kalau tidak, ia mengendap di vault:
        // `real_quote` hanya mencatat bagian net, jadi selisihnya tidak
        // akan pernah ikut terbawa di `graduation_payload` maupun di jalur
        // penjualan. Nilainya kecil per token, tapi bocor di setiap
        // peluncuran dan tidak pernah bisa diambil siapa pun.
        vault::withdraw(acc.vault, acc.treasury, narrow(receipt.fee_protocol)?)?;
        vault::withdraw(acc.vault, acc.creator_vault, narrow(receipt.fee_creator)?)?;

        // Token dev TIDAK langsung dikirim — ia masuk vesting.
        // Lihat unlock_tranche.
    }

    view.status = STATUS_SEALED;

    msg!(
        "rexo::launch mint={} bond={} dev_tokens={} sealed_until={}",
        acc.mint.key,
        params.bond,
        dev_tokens,
        sealed_until
    );

    Ok(LaunchOutcome {
        view,
        sealed_until,
        dev_tokens,
    })
}

// ---------------------------------------------------------------------------
// Verifikasi
// ---------------------------------------------------------------------------

/// Terapkan hasil verifikasi REX. Satu-satunya jalan tier bisa naik.
pub fn apply_verification(
    view: &mut LaunchView,
    bond: u64,
    members: u64,
    age_days: u64,
    ok: bool,
) -> u8 {
    if !ok || members < MIN_TELEGRAM_MEMBERS || age_days < MIN_X_ACCOUNT_AGE_DAYS {
        view.tier = TIER_UNVERIFIED;
        return TIER_UNVERIFIED;
    }
    let tier = if bond >= MIN_BOND[TIER_COMMITTED as usize] {
        TIER_COMMITTED
    } else if bond >= MIN_BOND[TIER_VERIFIED as usize] {
        TIER_VERIFIED
    } else {
        TIER_UNVERIFIED
    };
    view.tier = tier;
    tier
}

// ---------------------------------------------------------------------------
// Buy
// ---------------------------------------------------------------------------

pub struct TradeOutcome {
    pub token_amount: u64,
    pub quote_amount: u64,
    pub fee_protocol: u64,
    pub fee_creator: u64,
    pub refund: u64,
    pub graduated: bool,
}

/// Beli token dari kurva.
///
/// Urutan penting: hitung dulu, commit state, baru pindahkan dana. Kalau
/// matematikanya menolak, tidak ada satu kelvin pun yang berpindah.
pub fn buy(
    program_id: &Pubkey,
    acc: &TradeAccounts<'_, '_>,
    view: &mut LaunchView,
    quote_in: u64,
    min_tokens_out: u64,
    now: u64,
) -> Result<TradeOutcome, ProgramErrorAlias> {
    guards::require_active(view.status)?;

    let cfg = view.config();
    let mut curve = view.curve();
    let receipt = curve.buy(&cfg, quote_in as u128, min_tokens_out as u128)?;

    let tokens_out = narrow(receipt.tokens_out)?;
    let quote_spent = narrow(receipt.quote_spent)?;
    let fee_protocol = narrow(receipt.fee_protocol)?;
    let fee_creator = narrow(receipt.fee_creator)?;
    let refund = narrow(receipt.refund)?;

    view.apply(&curve)?;

    // -- pemindahan dana --
    // Pembeli mengirim quote_spent (bukan quote_in — sisanya tidak pernah
    // meninggalkan wallet-nya, jadi tidak perlu refund terpisah).
    vault::deposit(acc.trader, acc.vault, acc.system_program, quote_spent)?;

    // Fee keluar dari vault ke tujuan masing-masing.
    vault::withdraw(acc.vault, acc.treasury, fee_protocol)?;
    vault::withdraw(acc.vault, acc.creator_vault, fee_creator)?;

    // Token keluar dari akun kurva ke pembeli.
    token::transfer_from_curve(
        program_id,
        acc.mint,
        acc.curve_token_account,
        acc.trader_token_account,
        acc.mint_authority,
        acc.token_program,
        tokens_out,
    )?;

    if receipt.graduated {
        view.status = STATUS_GRADUATED;
    }

    let progress = curve.progress_bps(&cfg);
    crate::rexo_emit!(Trade {
        mint: *acc.mint.key,
        trader: *acc.trader.key,
        is_buy: true,
        quote_amount: quote_spent,
        token_amount: tokens_out,
        fee_protocol,
        fee_creator,
        virtual_quote: view.virtual_quote,
        virtual_token: view.virtual_token,
        real_quote: view.real_quote,
        real_token: view.real_token,
        progress_bps: progress as u64,
        timestamp: now,
    });

    msg!(
        "rexo::buy tokens={} spent={} fee_p={} fee_c={} progress={}bps graduated={}",
        tokens_out,
        quote_spent,
        fee_protocol,
        fee_creator,
        progress,
        receipt.graduated
    );

    Ok(TradeOutcome {
        token_amount: tokens_out,
        quote_amount: quote_spent,
        fee_protocol,
        fee_creator,
        refund,
        graduated: receipt.graduated,
    })
}

// ---------------------------------------------------------------------------
// Sell
// ---------------------------------------------------------------------------

pub struct SellParams<'k> {
    pub tokens_in: u64,
    pub min_quote_out: u64,
    pub creator: &'k Pubkey,
    pub creator_tranches_unlocked: u8,
    /// Sisa kolam bond yang hangus, dalam kelvin. Nol kalau token sehat.
    pub exit_pool: u64,
    /// Token yang masih beredar saat abandonment terjadi.
    pub exit_base: u64,
    pub now: u64,
}

pub struct SellOutcome {
    pub trade: TradeOutcome,
    /// Bagian bond hangus yang dibayarkan ke penjual ini.
    pub exit_bonus: u64,
    pub exit_pool_left: u64,
    pub exit_base_left: u64,
}

pub fn sell(
    acc: &TradeAccounts<'_, '_>,
    view: &mut LaunchView,
    p: SellParams<'_>,
) -> Result<SellOutcome, ProgramErrorAlias> {
    // `require_exitable`, BUKAN `require_active`.
    //
    // Pemegang token harus selalu bisa keluar — termasuk setelah token
    // ditandai ditinggalkan. Memakai require_active di sini mengunci mereka
    // dan menghukum korban, bukan pelaku.
    guards::require_exitable(view.status)?;

    let cfg = view.config();
    let curve_before = view.curve();
    let progress = curve_before.progress_bps(&cfg) as u64;
    if acc.trader.key == p.creator
        && guards::creator_locked(p.creator_tranches_unlocked, progress, false)
    {
        return Err(RexoError::CreatorLocked.into());
    }

    let mut curve = curve_before;
    let receipt = curve.sell(&cfg, p.tokens_in as u128, p.min_quote_out as u128)?;

    let quote_out = narrow(receipt.quote_out)?;
    let fee_protocol = narrow(receipt.fee_protocol)?;
    let fee_creator = narrow(receipt.fee_creator)?;

    view.apply(&curve)?;

    // Bagian bond yang hangus, dibayar pro-rata saat penjual keluar.
    let (bonus, pool_left, base_left) =
        guards::exit_bonus(p.exit_pool, p.exit_base, p.tokens_in);

    // Token masuk dulu, baru dana keluar. Kalau transfer token gagal,
    // transaksi batal sebelum vault tersentuh.
    token::transfer_to_curve(
        acc.mint,
        acc.trader_token_account,
        acc.curve_token_account,
        acc.trader,
        acc.token_program,
        p.tokens_in,
    )?;

    vault::withdraw(acc.vault, acc.trader, quote_out.saturating_add(bonus))?;
    vault::withdraw(acc.vault, acc.treasury, fee_protocol)?;
    vault::withdraw(acc.vault, acc.creator_vault, fee_creator)?;

    crate::rexo_emit!(Trade {
        mint: *acc.mint.key,
        trader: *acc.trader.key,
        is_buy: false,
        quote_amount: quote_out,
        token_amount: p.tokens_in,
        fee_protocol,
        fee_creator,
        virtual_quote: view.virtual_quote,
        virtual_token: view.virtual_token,
        real_quote: view.real_quote,
        real_token: view.real_token,
        progress_bps: curve.progress_bps(&cfg) as u64,
        timestamp: p.now,
    });

    msg!(
        "rexo::sell tokens={} out={} bonus={} fee_p={} fee_c={}",
        p.tokens_in,
        quote_out,
        bonus,
        fee_protocol,
        fee_creator
    );

    Ok(SellOutcome {
        trade: TradeOutcome {
            token_amount: p.tokens_in,
            quote_amount: quote_out,
            fee_protocol,
            fee_creator,
            refund: 0,
            graduated: false,
        },
        exit_bonus: bonus,
        exit_pool_left: pool_left,
        exit_base_left: base_left,
    })
}

// ---------------------------------------------------------------------------
// Abandonment
// ---------------------------------------------------------------------------

pub struct AbandonOutcome {
    /// Kolam bond yang hangus, dibayar pro-rata saat pemegang keluar.
    pub exit_pool: u64,
    /// Token yang masih beredar dan berhak atas kolam itu.
    pub exit_base: u64,
    pub burned_creator_tokens: u64,
}

/// Hanguskan bond dan burn alokasi kreator setelah heartbeat gagal berulang.
///
/// # Kenapa bond TIDAK memakai `CurveState::forfeit_bond`
///
/// `forfeit_bond` menaruh nilainya di `forfeited_quote`, yang hanya dibayar
/// lewat `graduation_payload`. Tapi token yang ditinggalkan **tidak akan
/// pernah lulus** — jadi bond itu akan nyangkut di vault selamanya.
///
/// Yang benar: bond jadi kolam keluar yang dibagikan pro-rata ke pemegang
/// token saat mereka menjual. Merekalah pihak yang dirugikan, jadi
/// merekalah yang dikompensasi — bukan protokol, dan bukan kolam LP yang
/// tidak akan pernah terbentuk.
#[allow(clippy::too_many_arguments)]
pub fn abandon(
    program_id: &Pubkey,
    acc: &TradeAccounts<'_, '_>,
    view: &mut LaunchView,
    bond: u64,
    creator_tokens_locked: u64,
    global_failures: u64,
    global_checks: u64,
    now: u64,
) -> Result<AbandonOutcome, ProgramErrorAlias> {
    guards::abandonment_permitted(global_failures, global_checks)?;
    guards::require_active(view.status)?;

    if creator_tokens_locked > 0 {
        token::burn_from_curve(
            program_id,
            acc.mint,
            acc.curve_token_account,
            acc.mint_authority,
            acc.token_program,
            creator_tokens_locked,
        )?;
    }

    // Token beredar di tangan publik = yang terjual dari kurva, dikurangi
    // alokasi kreator yang baru saja di-burn.
    let sold = INITIAL_REAL_TOKEN.saturating_sub(view.real_token);
    let exit_base = sold.saturating_sub(creator_tokens_locked);

    // Kalau tidak ada pemegang sama sekali, tidak ada yang dirugikan.
    // Bond tetap di vault dan disapu treasury lewat jalur terpisah.
    let exit_pool = if exit_base == 0 { 0 } else { bond };

    view.status = STATUS_ABANDONED;
    view.tier = TIER_UNVERIFIED;

    crate::rexo_emit!(Abandoned {
        mint: *acc.mint.key,
        forfeited_bond: exit_pool,
        burned_creator_tokens: creator_tokens_locked,
        timestamp: now,
    });

    msg!(
        "rexo::abandoned mint={} exit_pool={} exit_base={} burned={}",
        acc.mint.key,
        exit_pool,
        exit_base,
        creator_tokens_locked
    );

    Ok(AbandonOutcome {
        exit_pool,
        exit_base,
        burned_creator_tokens: creator_tokens_locked,
    })
}

// ---------------------------------------------------------------------------
// Graduation
// ---------------------------------------------------------------------------

pub struct GraduationOutcome {
    pub lp_tokens: u64,
    pub lp_quote: u64,
    pub bond_returned: u64,
    pub sfs_endowment: u64,
}

pub fn graduate(
    program_id: &Pubkey,
    acc: &GraduateAccounts<'_, '_>,
    view: &LaunchView,
    bond: u64,
    bond_already_settled: bool,
    now: u64,
) -> Result<GraduationOutcome, ProgramErrorAlias> {
    guards::require_graduated(view.status)?;

    // Sabuk dan bretel: status GRADUATED harus konsisten dengan reserve.
    // Kalau tidak, ada jalur lain yang menyetel status tanpa menguras kurva.
    if view.real_token != 0 {
        msg!("real_token={} tapi status graduated", view.real_token);
        return Err(RexoError::WrongStatus.into());
    }

    let cfg = view.config();
    let curve = view.curve();
    let payload = curve.graduation_payload(&cfg)?;

    let lp_tokens = narrow(payload.lp_tokens)?;
    let lp_quote = narrow(payload.lp_quote)?;

    // Likuiditas pindah ke pool.
    token::transfer_from_curve(
        program_id,
        acc.mint,
        acc.curve_token_account,
        acc.pool_token_account,
        acc.mint_authority,
        acc.token_program,
        lp_tokens,
    )?;
    vault::withdraw(acc.vault, acc.pool_quote_account, lp_quote)?;

    // TODO(pool): setelah pool dibuat, LP token WAJIB di-burn atau di-lock.
    // LP yang bisa ditarik kembali membuat "graduation" cuma rug pull
    // dengan langkah tambahan. Ini bukan detail — ini syarat.

    // Bond kembali: kreator menuntaskan janjinya.
    let mut bond_returned = 0u64;
    if !bond_already_settled && bond > 0 {
        vault::withdraw(acc.vault, acc.creator, bond)?;
        bond_returned = bond;
    }

    // Stake-for-Service: sebagian fee protokol di-stake, dan YIELD-nya
    // membiayai automasi token ini selamanya. Tidak ada top-up, tidak ada
    // bot yang kehabisan saldo. Ini yang tidak bisa ditiru chain lain.
    //
    // AKUNTANSI — baca ini sebelum mengubah apa pun:
    // fee disapu dari vault ke treasury SEGERA di setiap trade (lihat
    // `buy`/`sell`). Artinya `view.fees_protocol` adalah PENGHITUNG SEUMUR
    // HIDUP, bukan saldo yang masih tersimpan di vault. Endowment di bawah
    // karena itu harus didanai dari akun TREASURY, bukan dari vault kurva.
    // Menariknya dari vault akan menguras likuiditas LP.
    let sfs_endowment = view
        .fees_protocol
        .saturating_mul(SFS_ENDOWMENT_BPS)
        / BPS_DENOM;
    // TODO(sfs): buat posisi SfS dengan routing fraction dan arahkan
    // yield-nya ke ServicePaymaster untuk membiayai heartbeat token ini.

    crate::rexo_emit!(Graduated {
        mint: *acc.mint.key,
        lp_tokens,
        lp_quote,
        fees_protocol: view.fees_protocol,
        fees_creator: view.fees_creator,
        bond_returned,
        sfs_endowment,
        timestamp: now,
    });

    msg!(
        "rexo::graduate lp_tokens={} lp_quote={} bond_returned={} sfs={}",
        lp_tokens,
        lp_quote,
        bond_returned,
        sfs_endowment
    );

    Ok(GraduationOutcome {
        lp_tokens,
        lp_quote,
        bond_returned,
        sfs_endowment,
    })
}

// Alias supaya signature tetap pendek dan konversi error otomatis jalan.
pub type ProgramErrorAlias = rialo_s_program::program_error::ProgramError;

// ---------------------------------------------------------------------------
// Tests logika murni (tanpa AccountInfo)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verification_cannot_promote_without_socials() {
        let mut v = LaunchView::genesis(TIER_UNVERIFIED);
        // bond besar tapi sosial gagal -> tetap Unverified
        let t = apply_verification(&mut v, 100 * ONE_RLO, 0, 0, false);
        assert_eq!(t, TIER_UNVERIFIED);
        assert_eq!(v.tier, TIER_UNVERIFIED);
    }

    #[test]
    fn verification_cannot_promote_without_bond() {
        let mut v = LaunchView::genesis(TIER_UNVERIFIED);
        // sosial lolos tapi tanpa bond -> tetap Unverified
        let t = apply_verification(&mut v, 0, 500, 365, true);
        assert_eq!(t, TIER_UNVERIFIED);
    }

    #[test]
    fn verification_tiers_follow_bond_thresholds() {
        let mut v = LaunchView::genesis(TIER_UNVERIFIED);
        assert_eq!(
            apply_verification(&mut v, 2 * ONE_RLO, 500, 365, true),
            TIER_VERIFIED
        );
        assert_eq!(
            apply_verification(&mut v, 10 * ONE_RLO, 500, 365, true),
            TIER_COMMITTED
        );
    }

    #[test]
    fn social_thresholds_are_enforced_at_the_boundary() {
        let mut v = LaunchView::genesis(TIER_UNVERIFIED);
        // satu anggota kurang -> gagal
        assert_eq!(
            apply_verification(&mut v, 10 * ONE_RLO, MIN_TELEGRAM_MEMBERS - 1, 365, true),
            TIER_UNVERIFIED
        );
        // pas di ambang -> lolos
        assert_eq!(
            apply_verification(&mut v, 10 * ONE_RLO, MIN_TELEGRAM_MEMBERS, 365, true),
            TIER_COMMITTED
        );
        // umur akun satu hari kurang -> gagal
        assert_eq!(
            apply_verification(
                &mut v,
                10 * ONE_RLO,
                500,
                MIN_X_ACCOUNT_AGE_DAYS - 1,
                true
            ),
            TIER_UNVERIFIED
        );
    }
}
