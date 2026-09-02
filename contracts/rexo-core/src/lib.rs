//! # Rexo Core — Meme coin launchpad native untuk Rialo
//!
//! Workflow Venus 0.12.2 native dengan callback otomatis, kurva pump.fun terkalibrasi,
//! dan kontrol status on-chain.

pub mod curve;

use rialo_venus_proc_macro::rialo;

pub const STATUS_UNINITIALIZED: u8 = 0;
pub const STATUS_ACTIVE: u8 = 1;
pub const STATUS_GRADUATED: u8 = 2;
pub const STATUS_CANCELLED: u8 = 3;

rialo! {
    workflow {
        state {
            creator: Pubkey,
            mint: Pubkey,
            tier: u8,
            status: u8,
            heartbeat_interval: u64,
            heartbeat_count: u64,
            created_at: u64,
            virtual_quote_kelvins: u64,
            virtual_token_supply: u64,
            real_quote_kelvins: u64,
            real_token_reserves: u64,
            total_fee_collected: u64,
            bond_kelvins: u64,
            graduated_at: u64,
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
                msg!(
                    "RexoCore::launch creator={} mint={} tier={} bond={} interval={}",
                    creator,
                    mint,
                    tier,
                    bond_kelvins,
                    heartbeat_interval
                );

                self.creator = creator;
                self.mint = mint;
                self.tier = tier;
                self.status = crate::STATUS_ACTIVE;
                self.bond_kelvins = bond_kelvins;
                self.heartbeat_interval = heartbeat_interval;
                self.heartbeat_count = 0;
                self.created_at = self.unix_timestamp() as u64;
                self.graduated_at = 0;

                // Konstanta standar bonding curve pump.fun dalam desimal native:
                // Quote: 30 RLO = 30_000_000_000 Kelvins (9 desimal)
                // Token: 1_073_000_000_000_000 (6 desimal)
                self.virtual_quote_kelvins = 30_000_000_000;
                self.virtual_token_supply = 1_073_000_000_000_000;
                self.real_quote_kelvins = 0;
                self.real_token_reserves = 793_100_000_000_000;
                self.total_fee_collected = 0;

                let next_heartbeat = self.created_at + heartbeat_interval;
                AFTER next_heartbeat CALL [on_heartbeat];

                msg!("RexoCore::launched status=active next_heartbeat={}", next_heartbeat);
                Ok(())
            }

            handler fn on_heartbeat(&mut self) -> ProgramResult {
                msg!("RexoCore::on_heartbeat count={}", self.heartbeat_count);
                if self.status == crate::STATUS_ACTIVE {
                    self.heartbeat_count += 1;
                    let now = self.unix_timestamp() as u64;
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
                let _ = (quote_in_kelvins, min_tokens_out);
                msg!("RexoCore::buy quote_in={}", quote_in_kelvins);
                Ok(())
            }

            control fn sell(
                &mut self,
                tokens_in: u64,
                min_quote_out_kelvins: u64,
            ) -> ProgramResult {
                let _ = (tokens_in, min_quote_out_kelvins);
                msg!("RexoCore::sell tokens_in={}", tokens_in);
                Ok(())
            }

            control fn graduate(&mut self) -> ProgramResult {
                msg!("RexoCore::graduate");
                if self.status == crate::STATUS_ACTIVE {
                    self.status = crate::STATUS_GRADUATED;
                    self.graduated_at = self.unix_timestamp() as u64;
                }
                Ok(())
            }

            control fn get_state(&mut self) -> ProgramResult {
                msg!(
                    "RexoCore::state status={} creator={} mint={} quote={} tokens={}",
                    self.status,
                    self.creator,
                    self.mint,
                    self.real_quote_kelvins,
                    self.real_token_reserves
                );
                Ok(())
            }

            terminating fn cancel(&mut self) -> ProgramResult {
                msg!("RexoCore::cancel");
                if self.status == crate::STATUS_ACTIVE {
                    self.status = crate::STATUS_CANCELLED;
                }
                Ok(())
            }

            terminating fn finalize(&mut self) -> ProgramResult {
                msg!("RexoCore::finalize");
                if self.status == crate::STATUS_GRADUATED {
                    msg!("RexoCore::finalized pool completed");
                }
                Ok(())
            }
        }
    }
}
