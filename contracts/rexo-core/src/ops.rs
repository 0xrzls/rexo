//! # Rexo Core — Logika Bisnis Utama
//!
//! Seluruh aturan bisnis, pembagian fee, bonding curve execution, dan proteksi tier
//! hidup di modul ini secara modular tanpa bergantung pada macro DSL Venus.

use rialo_s_program::{account_info::AccountInfo, msg, pubkey::Pubkey};

use crate::constants::*;
use crate::curve;
use crate::errors::RexoError;
use crate::events::*;
use crate::guards::*;
use crate::state::CurveState;
use crate::token;
use crate::vault;

/// Eksekusi Peluncuran Token
pub fn launch<'a>(
    state: &mut CurveState,
    creator_info: &AccountInfo<'a>,
    vault_info: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    mint_pubkey: Pubkey,
    requested_tier: u8,
    bond_kelvins: u64,
    heartbeat_interval: u64,
    now: u64,
) -> Result<(), RexoError> {
    assert_signer(creator_info)?;
    assert_tier_not_self_assigned(requested_tier)?;
    assert_bond_meets_tier_requirement(requested_tier, bond_kelvins)?;

    let interval = if heartbeat_interval < MIN_HEARTBEAT_INTERVAL {
        DEFAULT_HEARTBEAT_INTERVAL
    } else {
        heartbeat_interval
    };

    // Setor bond kreator ke vault (jika ada)
    if bond_kelvins > 0 {
        vault::deposit_kelvins(creator_info, vault_info, system_program, bond_kelvins)?;
    }

    *state = CurveState::new(
        *creator_info.key,
        mint_pubkey,
        *vault_info.key,
        requested_tier,
        bond_kelvins,
        interval,
        now,
        0,
        0,
    );

    emit_event(
        "Launch",
        &LaunchEvent {
            creator: *creator_info.key,
            mint: mint_pubkey,
            tier: requested_tier,
            bond_kelvins,
            heartbeat_interval: interval,
            timestamp: now,
        },
    );

    msg!(
        "RexoOps::launch: Berhasil diluncurkan. Creator={} Mint={} Bond={} Interval={}",
        creator_info.key,
        mint_pubkey,
        bond_kelvins,
        interval
    );

    Ok(())
}

/// Eksekusi Pembelian Token (Buy)
pub fn buy<'a>(
    state: &mut CurveState,
    trader_info: &AccountInfo<'a>,
    vault_info: &AccountInfo<'a>,
    treasury_info: &AccountInfo<'a>,
    creator_info: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    quote_in_kelvins: u64,
    min_tokens_out: u64,
    now: u64,
) -> Result<u64, RexoError> {
    assert_signer(trader_info)?;
    assert_active(state.status)?;

    if quote_in_kelvins == 0 {
        return Err(RexoError::ZeroAmount);
    }

    // Hitung pembagian fee flat 100 bps
    let total_fee = (quote_in_kelvins as u128 * TOTAL_FEE_BPS / BPS_DENOM) as u64;
    let proto_fee = total_fee / 2;
    let creator_fee = total_fee.saturating_sub(proto_fee);
    let quote_net = quote_in_kelvins
        .checked_sub(total_fee)
        .ok_or(RexoError::Overflow)?;

    // Hitung output token berdasarkan constant product bonding curve
    let (tokens_out, _new_virtual_sol, _new_virtual_token) = curve::calculate_buy(
        quote_net as u128,
        state.virtual_quote_reserves as u128,
        state.virtual_token_reserves as u128,
        state.real_token_reserves as u128,
    )
    .map_err(|_| RexoError::InsufficientLiquidity)?;

    let tokens_out_u64 = tokens_out as u64;
    if tokens_out_u64 < min_tokens_out {
        return Err(RexoError::SlippageExceeded);
    }

    // Setor quote currency dari trader ke vault
    vault::deposit_kelvins(trader_info, vault_info, system_program, quote_in_kelvins)?;

    // Fee disapu segera dari vault ke treasury & creator
    if proto_fee > 0 {
        vault::withdraw_kelvins(vault_info, treasury_info, proto_fee)?;
    }
    if creator_fee > 0 && state.creator == *creator_info.key {
        vault::withdraw_kelvins(vault_info, creator_info, creator_fee)?;
    }

    // Update state kurva
    state.apply_buy(quote_net, tokens_out_u64, proto_fee, creator_fee)?;

    emit_event(
        "Buy",
        &BuyEvent {
            buyer: *trader_info.key,
            mint: state.mint,
            quote_in_kelvins,
            tokens_out: tokens_out_u64,
            protocol_fee_kelvins: proto_fee,
            creator_fee_kelvins: creator_fee,
            real_quote_kelvins: state.real_quote_reserves,
            real_token_reserves: state.real_token_reserves,
            timestamp: now,
        },
    );

    msg!(
        "RexoOps::buy: Sukses! in={} kelvins, out={} tokens, progress={}%",
        quote_in_kelvins,
        tokens_out_u64,
        state.progress_bps() / 100
    );

    Ok(tokens_out_u64)
}

/// Eksekusi Penjualan Token (Sell)
pub fn sell<'a>(
    state: &mut CurveState,
    trader_info: &AccountInfo<'a>,
    vault_info: &AccountInfo<'a>,
    treasury_info: &AccountInfo<'a>,
    creator_info: &AccountInfo<'a>,
    tokens_in: u64,
    min_quote_out_kelvins: u64,
    now: u64,
) -> Result<u64, RexoError> {
    assert_signer(trader_info)?;
    assert_active(state.status)?;

    if tokens_in == 0 {
        return Err(RexoError::ZeroAmount);
    }

    // Hitung output quote kotor dari curve
    let (quote_gross, _new_virtual_sol, _new_virtual_token) = curve::calculate_sell(
        tokens_in as u128,
        state.virtual_quote_reserves as u128,
        state.virtual_token_reserves as u128,
    )
    .map_err(|_| RexoError::InsufficientLiquidity)?;

    let quote_gross_u64 = quote_gross as u64;

    // Hitung fee flat 100 bps
    let total_fee = (quote_gross * TOTAL_FEE_BPS / BPS_DENOM) as u64;
    let proto_fee = total_fee / 2;
    let creator_fee = total_fee.saturating_sub(proto_fee);
    let quote_net = quote_gross_u64
        .checked_sub(total_fee)
        .ok_or(RexoError::Overflow)?;

    if quote_net < min_quote_out_kelvins {
        return Err(RexoError::SlippageExceeded);
    }

    // Tarik payout quote net dari vault ke seller
    vault::withdraw_kelvins(vault_info, trader_info, quote_net)?;

    // Fee disapu segera dari vault ke treasury & creator
    if proto_fee > 0 {
        vault::withdraw_kelvins(vault_info, treasury_info, proto_fee)?;
    }
    if creator_fee > 0 && state.creator == *creator_info.key {
        vault::withdraw_kelvins(vault_info, creator_info, creator_fee)?;
    }

    // Update state kurva
    state.apply_sell(tokens_in, quote_gross_u64, proto_fee, creator_fee)?;

    emit_event(
        "Sell",
        &SellEvent {
            seller: *trader_info.key,
            mint: state.mint,
            tokens_in,
            quote_out_kelvins: quote_net,
            protocol_fee_kelvins: proto_fee,
            creator_fee_kelvins: creator_fee,
            real_quote_kelvins: state.real_quote_reserves,
            real_token_reserves: state.real_token_reserves,
            timestamp: now,
        },
    );

    msg!(
        "RexoOps::sell: Sukses! in={} tokens, out={} kelvins",
        tokens_in,
        quote_net
    );

    Ok(quote_net)
}

/// Handler Autonomous Heartbeat
pub fn on_heartbeat(state: &mut CurveState, now: u64) -> Result<(), RexoError> {
    if state.status == STATUS_ACTIVE {
        state.heartbeat_count = state.heartbeat_count.saturating_add(1);
        state.last_heartbeat_at = now;

        emit_event(
            "Heartbeat",
            &HeartbeatEvent {
                mint: state.mint,
                heartbeat_count: state.heartbeat_count,
                next_expected: now + state.heartbeat_interval,
                timestamp: now,
            },
        );

        msg!(
            "RexoOps::on_heartbeat: count={} next_tick={}",
            state.heartbeat_count,
            now + state.heartbeat_interval
        );
    }
    Ok(())
}

/// Peningkatan Tier Terverifikasi (Hanya dapat dipanggil oleh attestation REX/Validator)
pub fn apply_verification(
    state: &mut CurveState,
    new_tier: u8,
    proof_hash: [u8; 32],
    now: u64,
) -> Result<(), RexoError> {
    if new_tier <= state.tier || new_tier > TIER_REX_GUARANTEED {
        return Err(RexoError::InvalidAccountData);
    }

    let old_tier = state.tier;
    state.tier = new_tier;

    emit_event(
        "Verification",
        &VerificationEvent {
            mint: state.mint,
            old_tier,
            new_tier,
            proof_hash,
            timestamp: now,
        },
    );

    msg!(
        "RexoOps::apply_verification: Tier token diupgrade dari {} ke {}",
        old_tier,
        new_tier
    );
    Ok(())
}

/// Kelulusan ke DEX AMM Pool (Graduation)
pub fn graduate<'a>(
    state: &mut CurveState,
    _vault_info: &AccountInfo<'a>,
    now: u64,
) -> Result<(), RexoError> {
    assert_graduated(state.status)?;
    state.graduated_at = now;

    emit_event(
        "Graduate",
        &GraduateEvent {
            mint: state.mint,
            final_quote_kelvins: state.real_quote_reserves,
            burned_lp: true,
            timestamp: now,
        },
    );

    msg!(
        "RexoOps::graduate: Target bonding tercapai. {} kelvins siap dialokasikan ke LP pool.",
        state.real_quote_reserves
    );
    Ok(())
}

/// Pembatalan / Abandonment jika terjadi rug pull / dev tidak hadir
pub fn abandon<'a>(
    state: &mut CurveState,
    vault_info: &AccountInfo<'a>,
    lp_reserve_info: &AccountInfo<'a>,
    reason: u8,
    failed_heartbeats: u64,
    global_failures: u64,
    global_total: u64,
    now: u64,
) -> Result<(), RexoError> {
    assert_active(state.status)?;
    let permitted = abandonment_permitted(failed_heartbeats, global_failures, global_total)?;
    if !permitted {
        return Err(RexoError::InvalidStatus);
    }

    state.status = STATUS_ABANDONED;

    // Bond hangus ke likuiditas pengguna/LP (bukan ke protokol!)
    let forfeited = state.bond_kelvins;
    if forfeited > 0 {
        vault::withdraw_kelvins(vault_info, lp_reserve_info, forfeited)?;
    }

    emit_event(
        "Abandon",
        &AbandonEvent {
            mint: state.mint,
            reason,
            bond_forfeited_kelvins: forfeited,
            timestamp: now,
        },
    );

    msg!(
        "RexoOps::abandon: Token ditandai ditinggalkan. Bond {} kelvins dialihkan ke pemegang token/LP.",
        forfeited
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier_cannot_be_self_assigned_at_launch() {
        let dummy = Pubkey::default();
        let mut state = CurveState::new(dummy, dummy, dummy, 0, 0, 300, 100, 0, 0);
        
        // Meminta tier di atas Unverified harus ditolak
        assert_eq!(
            assert_tier_not_self_assigned(TIER_COMMUNITY),
            Err(RexoError::SelfAssignedTierForbidden)
        );
        assert_eq!(
            assert_tier_not_self_assigned(TIER_REX_GUARANTEED),
            Err(RexoError::SelfAssignedTierForbidden)
        );
        assert!(assert_tier_not_self_assigned(TIER_UNVERIFIED).is_ok());
    }

    #[test]
    fn test_tier_upgrades_via_verification() {
        let dummy = Pubkey::default();
        let mut state = CurveState::new(dummy, dummy, dummy, TIER_UNVERIFIED, 0, 300, 100, 0, 0);

        let proof = [0u8; 32];
        assert!(apply_verification(&mut state, TIER_COMMUNITY, proof, 200).is_ok());
        assert_eq!(state.tier, TIER_COMMUNITY);

        assert!(apply_verification(&mut state, TIER_VERIFIED_DEV, proof, 300).is_ok());
        assert_eq!(state.tier, TIER_VERIFIED_DEV);

        // Downgrade atau tier sama ditolak
        assert!(apply_verification(&mut state, TIER_COMMUNITY, proof, 400).is_err());
    }

    #[test]
    fn test_heartbeat_advances_count() {
        let dummy = Pubkey::default();
        let mut state = CurveState::new(dummy, dummy, dummy, TIER_UNVERIFIED, 0, 300, 100, 0, 0);
        assert_eq!(state.heartbeat_count, 0);

        on_heartbeat(&mut state, 400).unwrap();
        assert_eq!(state.heartbeat_count, 1);
        assert_eq!(state.last_heartbeat_at, 400);

        on_heartbeat(&mut state, 700).unwrap();
        assert_eq!(state.heartbeat_count, 2);
    }

    #[test]
    fn test_abandon_forfeits_bond_to_lp() {
        let dummy = Pubkey::default();
        let mut state = CurveState::new(dummy, dummy, dummy, TIER_COMMUNITY, 500_000_000, 300, 100, 0, 0);

        // 3 kali terlambat heartbeat, rasio kegagalan global 1% -> Diizinkan abandon
        let is_ok = abandonment_permitted(3, 1, 100).unwrap();
        assert!(is_ok);
        assert_eq!(state.status, STATUS_ACTIVE);
    }
}
