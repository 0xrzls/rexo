//! # Rexo Core — State Storage & Jembatan u64 <-> u128
//!
//! Serialisasi skalar datar kompatibel bincode + serde untuk runtime Venus.

use rialo_s_program::pubkey::Pubkey;
use serde::{Deserialize, Serialize};

use crate::constants::*;
use crate::errors::RexoError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CurveState {
    pub creator: Pubkey,
    pub mint: Pubkey,
    pub vault: Pubkey,
    pub tier: u8,
    pub status: u8,
    pub heartbeat_interval: u64,
    pub heartbeat_count: u64,
    pub last_heartbeat_at: u64,
    pub created_at: u64,
    pub graduated_at: u64,
    pub virtual_quote_reserves: u64,
    pub virtual_token_reserves: u64,
    pub real_quote_reserves: u64,
    pub real_token_reserves: u64,
    pub fees_protocol_lifetime: u64,
    pub fees_creator_lifetime: u64,
    pub bond_kelvins: u64,
    pub bump_curve: u8,
    pub bump_vault: u8,
}

impl CurveState {
    pub fn new(
        creator: Pubkey,
        mint: Pubkey,
        vault: Pubkey,
        tier: u8,
        bond_kelvins: u64,
        heartbeat_interval: u64,
        now: u64,
        bump_curve: u8,
        bump_vault: u8,
    ) -> Self {
        Self {
            creator,
            mint,
            vault,
            tier,
            status: STATUS_ACTIVE,
            heartbeat_interval,
            heartbeat_count: 0,
            last_heartbeat_at: now,
            created_at: now,
            graduated_at: 0,
            virtual_quote_reserves: INITIAL_VIRTUAL_QUOTE_RESERVES as u64,
            virtual_token_reserves: INITIAL_VIRTUAL_TOKEN_RESERVES as u64,
            real_quote_reserves: 0,
            real_token_reserves: INITIAL_REAL_TOKEN_RESERVES as u64,
            fees_protocol_lifetime: 0,
            fees_creator_lifetime: 0,
            bond_kelvins,
            bump_curve,
            bump_vault,
        }
    }

    /// Progress persentase kelulusan (0..10000 bps)
    pub fn progress_bps(&self) -> u64 {
        let sold = INITIAL_REAL_TOKEN_RESERVES.saturating_sub(self.real_token_reserves as u128);
        ((sold * 10_000) / INITIAL_REAL_TOKEN_RESERVES) as u64
    }

    /// Update state setelah pembelian
    pub fn apply_buy(
        &mut self,
        quote_net: u64,
        tokens_out: u64,
        fee_proto: u64,
        fee_creator: u64,
    ) -> Result<(), RexoError> {
        self.virtual_quote_reserves = self
            .virtual_quote_reserves
            .checked_add(quote_net)
            .ok_or(RexoError::Overflow)?;
        self.real_quote_reserves = self
            .real_quote_reserves
            .checked_add(quote_net)
            .ok_or(RexoError::Overflow)?;

        self.virtual_token_reserves = self
            .virtual_token_reserves
            .checked_sub(tokens_out)
            .ok_or(RexoError::InsufficientLiquidity)?;
        self.real_token_reserves = self
            .real_token_reserves
            .checked_sub(tokens_out)
            .ok_or(RexoError::InsufficientLiquidity)?;

        self.fees_protocol_lifetime = self
            .fees_protocol_lifetime
            .saturating_add(fee_proto);
        self.fees_creator_lifetime = self
            .fees_creator_lifetime
            .saturating_add(fee_creator);

        if self.real_token_reserves == 0 {
            self.status = STATUS_GRADUATED;
        }

        Ok(())
    }

    /// Update state setelah penjualan
    pub fn apply_sell(
        &mut self,
        tokens_in: u64,
        quote_gross: u64,
        fee_proto: u64,
        fee_creator: u64,
    ) -> Result<(), RexoError> {
        self.virtual_token_reserves = self
            .virtual_token_reserves
            .checked_add(tokens_in)
            .ok_or(RexoError::Overflow)?;
        self.real_token_reserves = self
            .real_token_reserves
            .checked_add(tokens_in)
            .ok_or(RexoError::Overflow)?;

        self.virtual_quote_reserves = self
            .virtual_quote_reserves
            .checked_sub(quote_gross)
            .ok_or(RexoError::InsufficientLiquidity)?;
        self.real_quote_reserves = self
            .real_quote_reserves
            .checked_sub(quote_gross)
            .ok_or(RexoError::InsufficientLiquidity)?;

        self.fees_protocol_lifetime = self
            .fees_protocol_lifetime
            .saturating_add(fee_proto);
        self.fees_creator_lifetime = self
            .fees_creator_lifetime
            .saturating_add(fee_creator);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state_and_progress() {
        let dummy_pubkey = Pubkey::default();
        let state = CurveState::new(
            dummy_pubkey,
            dummy_pubkey,
            dummy_pubkey,
            TIER_UNVERIFIED,
            0,
            300,
            1000,
            255,
            254,
        );

        assert_eq!(state.status, STATUS_ACTIVE);
        assert_eq!(state.progress_bps(), 0);
        assert_eq!(state.real_quote_reserves, 0);
        assert_eq!(state.real_token_reserves as u128, INITIAL_REAL_TOKEN_RESERVES);
    }

    #[test]
    fn test_apply_buy_updates_reserves() {
        let dummy = Pubkey::default();
        let mut state = CurveState::new(dummy, dummy, dummy, TIER_COMMUNITY, 500_000_000, 300, 1000, 255, 254);
        
        let buy_quote = 1_000_000_000; // 1 RLO
        let tokens_out = 30_000_000_000;
        state.apply_buy(buy_quote, tokens_out, 5_000_000, 5_000_000).unwrap();

        assert_eq!(state.real_quote_reserves, 1_000_000_000);
        assert_eq!(state.real_token_reserves, (INITIAL_REAL_TOKEN_RESERVES as u64) - tokens_out);
        assert_eq!(state.fees_protocol_lifetime, 5_000_000);
        assert_eq!(state.fees_creator_lifetime, 5_000_000);
        assert!(state.progress_bps() > 0);
    }

    #[test]
    fn test_apply_sell_decreases_quote_reserves() {
        let dummy = Pubkey::default();
        let mut state = CurveState::new(dummy, dummy, dummy, TIER_COMMUNITY, 500_000_000, 300, 1000, 255, 254);
        
        state.apply_buy(2_000_000_000, 50_000_000_000, 10_000_000, 10_000_000).unwrap();
        let prev_tokens = state.real_token_reserves;
        
        // Sell back 10_000_000_000 tokens for 350_000_000 quote
        state.apply_sell(10_000_000_000, 350_000_000, 1_750_000, 1_750_000).unwrap();
        assert_eq!(state.real_token_reserves, prev_tokens + 10_000_000_000);
        assert_eq!(state.real_quote_reserves, 1_650_000_000);
    }

    #[test]
    fn test_graduation_triggers_at_zero_tokens() {
        let dummy = Pubkey::default();
        let mut state = CurveState::new(dummy, dummy, dummy, TIER_COMMUNITY, 500_000_000, 300, 1000, 255, 254);
        
        let all_tokens = state.real_token_reserves;
        state.apply_buy(85_000_000_000, all_tokens, 400_000_000, 400_000_000).unwrap();
        assert_eq!(state.real_token_reserves, 0);
        assert_eq!(state.status, STATUS_GRADUATED);
        assert_eq!(state.progress_bps(), 10_000);
    }
}
