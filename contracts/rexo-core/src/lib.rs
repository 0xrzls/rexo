//! # Rexo Core — Meme coin launchpad native untuk Rialo
//!
//! Cangkang tipis DSL Venus 0.12.2 / 0.20.0-alpha.0. Seluruh logika bisnis
//! diisolasi di `ops.rs`, matematika di `curve.rs`, dan pengaman di `guards.rs`.

pub mod accounts;
pub mod constants;
pub mod curve;
pub mod errors;
pub mod events;
pub mod guards;
pub mod ops;
pub mod state;
pub mod token;
pub mod vault;

use rialo_venus_proc_macro::rialo;

pub use constants::*;
pub use errors::RexoError;
pub use state::CurveState;

rialo! {
    workflow {
        state {
            creator: Pubkey,
            mint: Pubkey,
            vault: Pubkey,
            tier: u8,
            status: u8,
            heartbeat_interval: u64,
            heartbeat_count: u64,
            last_heartbeat_at: u64,
            created_at: u64,
            graduated_at: u64,
            virtual_quote_reserves: u64,
            virtual_token_reserves: u64,
            real_quote_reserves: u64,
            real_token_reserves: u64,
            fees_protocol_lifetime: u64,
            fees_creator_lifetime: u64,
            bond_kelvins: u64,
        }

        program {
            use rialo_s_program::{
                entrypoint::ProgramResult,
                msg,
                pubkey::Pubkey,
            };

            initiating fn launch(
                &mut self,
                creator: Pubkey,
                mint: Pubkey,
                tier: u8,
                bond_kelvins: u64,
                heartbeat_interval: u64,
            ) -> ProgramResult {
                let now = self.unix_timestamp() as u64;

                // Akses akun yang dipass oleh runtime Venus (ditandai jelas):
                let accounts = self.accounts;
                let parsed = crate::accounts::LaunchAccounts::parse(accounts)
                    .map_err(|e| rialo_s_program::program_error::ProgramError::Custom(e as u32))?;

                let mut curve_state = crate::state::CurveState::new(
                    creator,
                    mint,
                    *parsed.vault.key,
                    tier,
                    bond_kelvins,
                    heartbeat_interval,
                    now,
                    0,
                    0,
                );

                crate::ops::launch(
                    &mut curve_state,
                    parsed.creator,
                    parsed.vault,
                    parsed.system_program,
                    mint,
                    tier,
                    bond_kelvins,
                    heartbeat_interval,
                    now,
                ).map_err(|e| rialo_s_program::program_error::ProgramError::Custom(e as u32))?;

                // Salin state ke storage workflow
                self.creator = curve_state.creator;
                self.mint = curve_state.mint;
                self.vault = curve_state.vault;
                self.tier = curve_state.tier;
                self.status = curve_state.status;
                self.heartbeat_interval = curve_state.heartbeat_interval;
                self.heartbeat_count = curve_state.heartbeat_count;
                self.last_heartbeat_at = curve_state.last_heartbeat_at;
                self.created_at = curve_state.created_at;
                self.graduated_at = 0;
                self.virtual_quote_reserves = curve_state.virtual_quote_reserves;
                self.virtual_token_reserves = curve_state.virtual_token_reserves;
                self.real_quote_reserves = curve_state.real_quote_reserves;
                self.real_token_reserves = curve_state.real_token_reserves;
                self.fees_protocol_lifetime = 0;
                self.fees_creator_lifetime = 0;
                self.bond_kelvins = curve_state.bond_kelvins;

                let next_heartbeat = now + self.heartbeat_interval;
                AFTER next_heartbeat CALL [on_heartbeat];

                msg!("RexoCore::launched status=active next_heartbeat={}", next_heartbeat);
                Ok(())
            }

            handler fn on_heartbeat(&mut self) -> ProgramResult {
                let now = self.unix_timestamp() as u64;
                let mut curve_state = self.to_curve_state();

                crate::ops::on_heartbeat(&mut curve_state, now)
                    .map_err(|e| rialo_s_program::program_error::ProgramError::Custom(e as u32))?;

                self.heartbeat_count = curve_state.heartbeat_count;
                self.last_heartbeat_at = curve_state.last_heartbeat_at;

                if self.status == crate::constants::STATUS_ACTIVE {
                    let next_tick = now + self.heartbeat_interval;
                    AFTER next_tick CALL [on_heartbeat];
                }
                Ok(())
            }

            control fn buy(
                &mut self,
                quote_in_kelvins: u64,
                min_tokens_out: u64,
            ) -> ProgramResult {
                let now = self.unix_timestamp() as u64;
                let accounts = self.accounts;
                let parsed = crate::accounts::TradeAccounts::parse(accounts)
                    .map_err(|e| rialo_s_program::program_error::ProgramError::Custom(e as u32))?;

                let mut curve_state = self.to_curve_state();

                crate::ops::buy(
                    &mut curve_state,
                    parsed.trader,
                    parsed.vault,
                    parsed.treasury,
                    parsed.creator,
                    parsed.system_program,
                    quote_in_kelvins,
                    min_tokens_out,
                    now,
                ).map_err(|e| rialo_s_program::program_error::ProgramError::Custom(e as u32))?;

                self.sync_from_curve_state(&curve_state);
                Ok(())
            }

            control fn sell(
                &mut self,
                tokens_in: u64,
                min_quote_out_kelvins: u64,
            ) -> ProgramResult {
                let now = self.unix_timestamp() as u64;
                let accounts = self.accounts;
                let parsed = crate::accounts::TradeAccounts::parse(accounts)
                    .map_err(|e| rialo_s_program::program_error::ProgramError::Custom(e as u32))?;

                let mut curve_state = self.to_curve_state();

                crate::ops::sell(
                    &mut curve_state,
                    parsed.trader,
                    parsed.vault,
                    parsed.treasury,
                    parsed.creator,
                    tokens_in,
                    min_quote_out_kelvins,
                    now,
                ).map_err(|e| rialo_s_program::program_error::ProgramError::Custom(e as u32))?;

                self.sync_from_curve_state(&curve_state);
                Ok(())
            }

            control fn graduate(&mut self) -> ProgramResult {
                let now = self.unix_timestamp() as u64;
                let accounts = self.accounts;
                let vault_info = &accounts[0];

                let mut curve_state = self.to_curve_state();
                crate::ops::graduate(&mut curve_state, vault_info, now)
                    .map_err(|e| rialo_s_program::program_error::ProgramError::Custom(e as u32))?;

                self.sync_from_curve_state(&curve_state);
                Ok(())
            }

            internal fn to_curve_state(&self) -> CurveState {
                CurveState {
                    creator: self.creator,
                    mint: self.mint,
                    vault: self.vault,
                    tier: self.tier,
                    status: self.status,
                    heartbeat_interval: self.heartbeat_interval,
                    heartbeat_count: self.heartbeat_count,
                    last_heartbeat_at: self.last_heartbeat_at,
                    created_at: self.created_at,
                    graduated_at: self.graduated_at,
                    virtual_quote_reserves: self.virtual_quote_reserves,
                    virtual_token_reserves: self.virtual_token_reserves,
                    real_quote_reserves: self.real_quote_reserves,
                    real_token_reserves: self.real_token_reserves,
                    fees_protocol_lifetime: self.fees_protocol_lifetime,
                    fees_creator_lifetime: self.fees_creator_lifetime,
                    bond_kelvins: self.bond_kelvins,
                    bump_curve: 0,
                    bump_vault: 0,
                }
            }

            internal fn sync_from_curve_state(&mut self, state: &CurveState) {
                self.status = state.status;
                self.virtual_quote_reserves = state.virtual_quote_reserves;
                self.virtual_token_reserves = state.virtual_token_reserves;
                self.real_quote_reserves = state.real_quote_reserves;
                self.real_token_reserves = state.real_token_reserves;
                self.fees_protocol_lifetime = state.fees_protocol_lifetime;
                self.fees_creator_lifetime = state.fees_creator_lifetime;
                self.graduated_at = state.graduated_at;
            }
        }
    }
}
