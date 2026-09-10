// Copyright (c) 2026 Rexo
// SPDX-License-Identifier: Apache-2.0

//! # Rexo v2 — launchpad token untuk Rialo
//!
//! ## Aturan menulis di dalam `rialo! { }`
//!
//! Hanya konstruksi yang TERBUKTI diterima parser DSL. Daftar ini disusun
//! dengan membandingkan terhadap program yang berhasil compile:
//!
//! | Konstruksi | Boleh |
//! |---|---|
//! | `// komentar` | ya |
//! | `/// doc comment` | TIDAK — jadi `#[doc]`, parser berhenti |
//! | `let` / `self.field = ...` / `if` | ya |
//! | `match` / `return` | tidak terbukti |
//! | `AFTER <n> seconds CALL [fn];` | ya — durasi RELATIF |
//! | target `AFTER` | harus `handler fn`, bukan `control fn` |
//! | `msg!` dengan >1 argumen format | tidak terbukti |
//! | `self.accounts` / `self.program_id` | field, bukan method |
//!
//! ## Fitur yang SENGAJA tidak ada
//!
//! Verifikasi sosial REX, lelang tersegel, dan denyut liveness butuh
//! primitif yang bentuk sintaksisnya belum terkonfirmasi. Tidak ada
//! stub, tidak ada timer kosong, tidak ada field yang menjanjikannya.
//! Ditambahkan ketika primitifnya terbukti, bukan sebelumnya.
//!
//! ## Yang khas Rialo dan BISA dipakai hari ini
//!
//! Migrasi otomatis. Saat kurva habis, `AFTER n seconds CALL [migrate]`
//! menjalankan migrasi sebagai transaksi terpisah tanpa keeper. Meteora
//! butuh `dbc-keeper`; LaunchLab butuh seseorang memanggil
//! `migrate_to_amm`. Di sini chain yang melakukannya.

pub mod accounts;
pub mod config;
pub mod curve;
pub mod errors;
pub mod fees;
pub mod ops;
pub mod state;
pub mod token;
pub mod vault;

use rialo_venus_proc_macro::rialo;

rialo! {
    workflow {
        state {
            partner: Pubkey,
            creator: Pubkey,
            mint: Pubkey,
            protocol_treasury: Pubkey,

            name: String,
            symbol: String,
            metadata_uri: String,

            lifecycle: u8,

            cfg_virtual_quote: u64,
            cfg_virtual_token: u64,
            cfg_total_base_sell: u64,
            cfg_lp_reserve: u64,

            fee_total_bps: u64,
            fee_protocol_bps: u64,
            fee_partner_bps: u64,
            fee_creator_bps: u64,
            fee_referral_bps: u64,

            vest_bps: u64,
            vest_cliff_secs: u64,
            vest_duration_secs: u64,

            virtual_quote: u64,
            virtual_token: u64,
            real_quote: u64,
            real_token: u64,

            ledger_protocol: u64,
            ledger_partner: u64,
            ledger_creator: u64,
            ledger_referral_paid: u64,

            creator_allocation: u64,
            creator_claimed: u64,

            migrate_target: u8,
            created_at: u64,
            migrated_at: u64,
        }

        program {
            use rialo_s_program::{entrypoint::ProgramResult, msg, pubkey::Pubkey};

            // ===============================================================
            // CREATOR — luncurkan token dari sebuah config
            //
            // Parameter kurva dan fee datang dari LaunchConfig milik
            // partner, bukan dari konstanta di kode. Itu yang membuat satu
            // program melayani banyak launchpad.
            // ===============================================================
            initiating fn initialize(
                &mut self,
                partner: Pubkey,
                creator: Pubkey,
                mint: Pubkey,
                protocol_treasury: Pubkey,
                name: String,
                symbol: String,
                metadata_uri: String,
                creator_buy: u64,
            ) -> ProgramResult {
                let now = self.unix_timestamp() as u64;
                let accs = self.accounts;
                let acc = crate::accounts::parse_init(self.program_id, accs)?;

                let cfg = crate::accounts::read_config(acc.config)?;
                let out = crate::ops::initialize(
                    self.program_id, &acc, &cfg, creator_buy, now,
                )?;
                let l = out.launch;

                self.partner = partner;
                self.creator = creator;
                self.mint = mint;
                self.protocol_treasury = protocol_treasury;
                self.name = name;
                self.symbol = symbol;
                self.metadata_uri = metadata_uri;

                self.lifecycle = l.state;
                self.cfg_virtual_quote = l.cfg_virtual_quote;
                self.cfg_virtual_token = l.cfg_virtual_token;
                self.cfg_total_base_sell = l.cfg_total_base_sell;
                self.cfg_lp_reserve = l.cfg_lp_reserve;

                self.fee_total_bps = l.fees.total_bps;
                self.fee_protocol_bps = l.fees.protocol_bps;
                self.fee_partner_bps = l.fees.partner_bps;
                self.fee_creator_bps = l.fees.creator_bps;
                self.fee_referral_bps = l.fees.referral_bps;

                self.vest_bps = l.vesting.vested_bps;
                self.vest_cliff_secs = l.vesting.cliff_secs;
                self.vest_duration_secs = l.vesting.duration_secs;

                self.virtual_quote = l.virtual_quote;
                self.virtual_token = l.virtual_token;
                self.real_quote = l.real_quote;
                self.real_token = l.real_token;

                self.ledger_protocol = l.ledger.protocol;
                self.ledger_partner = l.ledger.partner;
                self.ledger_creator = l.ledger.creator;
                self.ledger_referral_paid = l.ledger.referral_paid;

                self.creator_allocation = l.creator_allocation;
                self.creator_claimed = 0;
                self.migrate_target = l.migrate_target;
                self.created_at = now;
                self.migrated_at = 0;

                msg!("rexo::init supply_minted={}", self.cfg_total_base_sell);
                Ok(())
            }

            // ===============================================================
            // TRADING — empat instruksi, mengikuti LaunchLab
            // ===============================================================
            control fn buy_exact_in(&mut self, quote_in: u64, min_base_out: u64)
                -> ProgramResult
            {
                let accs = self.accounts;
                let acc = crate::accounts::parse_trade(self.program_id, accs)?;
                let mut l = self.load();
                let r = crate::ops::buy_exact_in(
                    self.program_id, &acc, &mut l, quote_in, min_base_out,
                )?;
                self.store(&l);

                if r.completed_curve {
                    // Migrasi otomatis. Tidak ada keeper, tidak ada bot.
                    AFTER 1 seconds CALL [migrate];
                    msg!("rexo::curve_complete quote={}", self.real_quote);
                }
                Ok(())
            }

            control fn buy_exact_out(&mut self, base_out: u64, max_quote_in: u64)
                -> ProgramResult
            {
                let accs = self.accounts;
                let acc = crate::accounts::parse_trade(self.program_id, accs)?;
                let mut l = self.load();
                let r = crate::ops::buy_exact_out(
                    self.program_id, &acc, &mut l, base_out, max_quote_in,
                )?;
                self.store(&l);

                if r.completed_curve {
                    AFTER 1 seconds CALL [migrate];
                }
                Ok(())
            }

            control fn sell_exact_in(&mut self, base_in: u64, min_quote_out: u64)
                -> ProgramResult
            {
                let accs = self.accounts;
                let acc = crate::accounts::parse_trade(self.program_id, accs)?;
                let mut l = self.load();
                crate::ops::sell_exact_in(
                    self.program_id, &acc, &mut l, base_in, min_quote_out,
                )?;
                self.store(&l);
                Ok(())
            }

            control fn sell_exact_out(&mut self, quote_out: u64, max_base_in: u64)
                -> ProgramResult
            {
                let accs = self.accounts;
                let acc = crate::accounts::parse_trade(self.program_id, accs)?;
                let mut l = self.load();
                crate::ops::sell_exact_out(
                    self.program_id, &acc, &mut l, quote_out, max_base_in,
                )?;
                self.store(&l);
                Ok(())
            }

            // ===============================================================
            // KLAIM FEE — pola tarik
            //
            // Fee menumpuk di quote_vault selama perdagangan dan ditarik di
            // sini. v1 memindahkannya tiap trade: dua transfer tambahan per
            // perdagangan, dan tidak ada saldo tersisa untuk apa pun.
            // ===============================================================
            control fn claim_protocol_fee(&mut self) -> ProgramResult {
                let accs = self.accounts;
                let acc = crate::accounts::parse_claim(self.program_id, accs)?;
                let mut l = self.load();
                let amt = crate::ops::claim_fee(
                    &mut l, crate::fees::Payee::Protocol, acc.quote_vault, acc.recipient,
                )?;
                self.store(&l);
                msg!("rexo::claim_protocol={}", amt);
                Ok(())
            }

            control fn claim_partner_fee(&mut self) -> ProgramResult {
                let accs = self.accounts;
                let acc = crate::accounts::parse_claim(self.program_id, accs)?;
                let mut l = self.load();
                let amt = crate::ops::claim_fee(
                    &mut l, crate::fees::Payee::Partner, acc.quote_vault, acc.recipient,
                )?;
                self.store(&l);
                msg!("rexo::claim_partner={}", amt);
                Ok(())
            }

            control fn claim_creator_fee(&mut self) -> ProgramResult {
                let accs = self.accounts;
                let acc = crate::accounts::parse_claim(self.program_id, accs)?;
                let mut l = self.load();
                let amt = crate::ops::claim_fee(
                    &mut l, crate::fees::Payee::Creator, acc.quote_vault, acc.recipient,
                )?;
                self.store(&l);
                msg!("rexo::claim_creator={}", amt);
                Ok(())
            }

            // ===============================================================
            // KLAIM TOKEN KREATOR — tunduk jadwal vesting
            //
            // Tidak ada yang cair sebelum migrasi. Kreator ikut menanggung
            // risiko sampai kurva benar-benar selesai.
            // ===============================================================
            control fn claim_creator_tokens(&mut self) -> ProgramResult {
                let now = self.unix_timestamp() as u64;
                let accs = self.accounts;
                let acc = crate::accounts::parse_creator_claim(self.program_id, accs)?;
                let mut l = self.load();
                let amt = crate::ops::claim_creator_tokens(
                    self.program_id,
                    &mut l,
                    acc.launch.key,
                    acc.mint,
                    acc.base_vault,
                    acc.creator_token_account,
                    acc.authority,
                    acc.token_program,
                    now,
                )?;
                self.store(&l);
                msg!("rexo::creator_tokens={}", amt);
                Ok(())
            }

            // ===============================================================
            // MIGRASI — dipicu chain, bukan klien
            //
            // handler fn, bukan control fn: target AFTER wajib handler.
            // ===============================================================
            handler fn migrate(&mut self) -> ProgramResult {
                let now = self.unix_timestamp() as u64;
                let accs = self.accounts;
                let acc = crate::accounts::parse_migrate(self.program_id, accs)?;
                let mut l = self.load();
                let r = crate::ops::migrate(
                    self.program_id,
                    &mut l,
                    acc.launch.key,
                    acc.mint,
                    acc.base_vault,
                    acc.quote_vault,
                    acc.lp_base_dest,
                    acc.lp_quote_dest,
                    acc.authority,
                    acc.token_program,
                    now,
                )?;
                self.store(&l);
                self.migrated_at = now;
                msg!("rexo::migrated quote={}", r.lp_quote);
                Ok(())
            }

            // ===============================================================
            // VIEW
            // ===============================================================
            control fn get_state(&mut self) -> ProgramResult {
                let l = self.load();
                let cfg = l.curve_config();
                let c = l.curve();
                msg!("rexo::lifecycle={}", self.lifecycle);
                msg!("rexo::progress_bps={}", c.progress_bps(&cfg) as u64);
                msg!("rexo::price={}", c.price_per_token().unwrap_or(0) as u64);
                msg!("rexo::mcap={}", c.market_cap(&cfg).unwrap_or(0) as u64);
                msg!("rexo::real_quote={}", self.real_quote);
                msg!("rexo::real_token={}", self.real_token);
                msg!("rexo::fees_outstanding={}", l.ledger.outstanding());
                Ok(())
            }

            fn load(&self) -> crate::state::Launch {
                crate::state::Launch {
                    state: self.lifecycle,
                    cfg_virtual_quote: self.cfg_virtual_quote,
                    cfg_virtual_token: self.cfg_virtual_token,
                    cfg_total_base_sell: self.cfg_total_base_sell,
                    cfg_lp_reserve: self.cfg_lp_reserve,
                    fees: crate::config::FeeSplit {
                        total_bps: self.fee_total_bps,
                        protocol_bps: self.fee_protocol_bps,
                        partner_bps: self.fee_partner_bps,
                        creator_bps: self.fee_creator_bps,
                        referral_bps: self.fee_referral_bps,
                    },
                    vesting: crate::config::VestingSchedule {
                        vested_bps: self.vest_bps,
                        cliff_secs: self.vest_cliff_secs,
                        duration_secs: self.vest_duration_secs,
                    },
                    migrate_target: self.migrate_target,
                    migrate_delay_secs: 1,
                    virtual_quote: self.virtual_quote,
                    virtual_token: self.virtual_token,
                    real_quote: self.real_quote,
                    real_token: self.real_token,
                    ledger: crate::fees::FeeLedger {
                        protocol: self.ledger_protocol,
                        partner: self.ledger_partner,
                        creator: self.ledger_creator,
                        referral_paid: self.ledger_referral_paid,
                    },
                    creator_allocation: self.creator_allocation,
                    creator_claimed: self.creator_claimed,
                    created_at: self.created_at,
                    migrated_at: self.migrated_at,
                }
            }

            fn store(&mut self, l: &crate::state::Launch) {
                self.lifecycle = l.state;
                self.virtual_quote = l.virtual_quote;
                self.virtual_token = l.virtual_token;
                self.real_quote = l.real_quote;
                self.real_token = l.real_token;
                self.ledger_protocol = l.ledger.protocol;
                self.ledger_partner = l.ledger.partner;
                self.ledger_creator = l.ledger.creator;
                self.ledger_referral_paid = l.ledger.referral_paid;
                self.creator_allocation = l.creator_allocation;
                self.creator_claimed = l.creator_claimed;
                self.migrated_at = l.migrated_at;
            }
        }
    }
}
