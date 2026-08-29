//! # Coldstart — Meme coin launchpad native untuk Rialo

pub mod curve;

use rialo_venus_proc_macro::rialo;
use rialo_s_program::entrypoint::ProgramResult;
use rialo_s_program::pubkey::Pubkey;

rialo! {
    state {
        creator: Pubkey,
        mint: Pubkey,
        name: String,
        symbol: String,
        metadata_uri: String,
        telegram_handle: String,
        x_handle: String,
        tier: u8,
        verified_at: u64,
        telegram_members: u64,
        x_account_age_days: u64,
        heartbeat_failures: u32,
        abandoned: bool,
        cfg_virtual_quote: u128,
        cfg_virtual_token: u128,
        cfg_curve_supply: u128,
        cfg_lp_reserve: u128,
        virtual_quote: u128,
        virtual_token: u128,
        real_quote: u128,
        real_token: u128,
        fees_protocol: u128,
        fees_creator: u128,
        forfeited_quote: u128,
        complete: bool,
        bond: u128,
        bond_returned: bool,
        creator_tokens_locked: u128,
        creator_tranches_unlocked: u8,
        sealed_until: u64,
        sealed_order_count: u32,
        sealed_cursor: u32,
        sfs_position: Pubkey,
        sfs_funded: u128,
        dex_pool: String,
    }

    program {
        initiating fn launch(
            &mut self,
            name: String,
            symbol: String,
            metadata_uri: String,
            telegram_handle: String,
            x_handle: String,
            bond: u128,
        ) -> ProgramResult {
            self.creator = Pubkey::default();
            self.mint = Pubkey::default();
            self.sfs_position = Pubkey::default();
            self.name = name;
            self.symbol = symbol;
            self.metadata_uri = metadata_uri;
            self.telegram_handle = telegram_handle;
            self.x_handle = x_handle;
            self.tier = 0;
            self.verified_at = 0;
            self.telegram_members = 0;
            self.x_account_age_days = 0;
            self.heartbeat_failures = 0;
            self.abandoned = false;
            self.cfg_virtual_quote = 30_000_000_000;
            self.cfg_virtual_token = 1_073_000_000_000_000;
            self.cfg_curve_supply = 793_100_000_000_000;
            self.cfg_lp_reserve = 206_900_000_000_000;
            self.virtual_quote = 30_000_000_000;
            self.virtual_token = 1_073_000_000_000_000;
            self.real_quote = 0;
            self.real_token = 793_100_000_000_000;
            self.fees_protocol = 0;
            self.fees_creator = 0;
            self.forfeited_quote = 0;
            self.complete = false;
            self.bond = bond;
            self.bond_returned = false;
            self.creator_tokens_locked = 0;
            self.creator_tranches_unlocked = 0;
            self.sealed_until = 0;
            self.sealed_order_count = 0;
            self.sealed_cursor = 0;
            self.sfs_funded = 0;
            self.dex_pool = String::new();

            Ok(())
        }

        control fn buy(&mut self, quote_in: u128, min_tokens_out: u128) -> ProgramResult {
            let _ = (quote_in, min_tokens_out);
            Ok(())
        }

        control fn sell(&mut self, tokens_in: u128, min_quote_out: u128) -> ProgramResult {
            let _ = (tokens_in, min_quote_out);
            Ok(())
        }

        control fn settle_sealed_batch(&mut self) -> ProgramResult {
            self.sealed_until = 0;
            Ok(())
        }

        control fn heartbeat(&mut self) -> ProgramResult {
            Ok(())
        }

        control fn check_vesting(&mut self) -> ProgramResult {
            Ok(())
        }

        control fn graduate(&mut self) -> ProgramResult {
            Ok(())
        }

        terminating fn finalize(&mut self) -> ProgramResult {
            self.complete = true;
            Ok(())
        }
    }
}


