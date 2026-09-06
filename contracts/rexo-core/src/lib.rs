// Copyright (c) 2026 Rexo
// SPDX-License-Identifier: Apache-2.0

//! # Rexo Core — launchpad meme coin native untuk Rialo
//!
//! ## Peta arsitektur
//!
//! ```text
//! lib.rs        cangkang DSL Venus. Tipis dengan sengaja.
//!  ├─ ops.rs        logika bisnis. INI yang diaudit.
//!  │   ├─ curve.rs      matematika bonding curve (21 test, nol dependency)
//!  │   ├─ state.rs      jembatan u64 <-> u128 (4 test)
//!  │   ├─ guards.rs     kontrol akses & status (6 test)
//!  │   ├─ vault.rs      pemindahan kelvin lewat CPI
//!  │   ├─ token.rs      mint / burn / transfer Token-2022
//!  │   ├─ accounts.rs   parsing & validasi PDA
//!  │   ├─ events.rs     event terstruktur untuk indexer
//!  │   ├─ errors.rs     error domain dengan kode stabil
//!  │   └─ constants.rs  seluruh angka ekonomi, satu file
//! ```
//!
//! Logikanya sengaja TIDAK hidup di dalam macro. Auditor harus bisa membaca
//! `ops.rs` tanpa memahami DSL Venus lebih dulu, dan `curve.rs` bisa diuji
//! dengan `rustc` biasa tanpa toolchain Rialo sama sekali.
//!
//! ## Yang terverifikasi vs yang belum
//!
//! **Terverifikasi** dari source `rialo-venus` 0.12.2 di docs.rs:
//! - `rialo_s_program::{account_info::AccountInfo, entrypoint::ProgramResult,
//!   msg, program::{invoke, invoke_signed}, program_error::ProgramError,
//!   pubkey::Pubkey, rent::Rent, system_instruction, system_program,
//!   sysvar::Sysvar}`
//! - `AccountInfo::kelvins()` dan `try_borrow_mut_kelvins()` — bukan
//!   `lamports()`. Rialo mengganti nama satuannya.
//! - `Pubkey::as_array()` — bukan `to_bytes()`
//! - state workflow diserialisasi dengan **bincode + serde**
//! - `WORKFLOW_SEED = "rialo_workflow"`, PDA = [seed, payer, nonce]
//! - Pola "vault carry data → kurangi saldo langsung, jangan pakai
//!   system_instruction::transfer" — persis yang dipakai rialo-venus sendiri
//!
//! **BELUM terverifikasi** — satu hal saja, dan sudah diisolasi:
//! bagaimana macro `rialo!` menyerahkan `&[AccountInfo]` ke badan fungsi.
//! Setiap `control fn` di bawah memanggil `self.accounts()` di SATU baris.
//! Kalau accessor-nya ternyata bernama lain, ubah baris itu saja —
//! seluruh `ops.rs`, `accounts.rs`, `vault.rs`, dan `token.rs` tetap benar.
//!
//! Cara memastikannya: buka `venus/` di rialo-examples, cari contoh yang
//! memindahkan kelvin atau token, dan lihat bagaimana ia mengakses akun.

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

pub use constants::*;

use rialo_venus_proc_macro::rialo;

rialo! {
    workflow {
        state {
            // -- identitas --
            creator: Pubkey,
            mint: Pubkey,
            treasury: Pubkey,
            name: String,
            symbol: String,
            metadata_uri: String,

            // -- klaim sosial (mentah, belum diverifikasi) --
            telegram_handle: String,
            x_handle: String,

            // -- hasil verifikasi (DITULIS REX, bukan kreator) --
            tier: u8,
            verified_at: u64,
            telegram_members: u64,
            x_account_age_days: u64,
            heartbeat_count: u64,
            heartbeat_failures: u32,

            // -- status & waktu --
            status: u8,
            created_at: u64,
            sealed_until: u64,
            graduated_at: u64,

            // -- state kurva (u64; matematikanya u128 di curve.rs) --
            virtual_quote: u64,
            virtual_token: u64,
            real_quote: u64,
            real_token: u64,
            fees_protocol: u64,
            fees_creator: u64,
            forfeited_quote: u64,

            // -- bond & vesting --
            bond: u64,
            bond_settled: bool,
            creator_tokens_locked: u64,
            creator_tranches_unlocked: u8,

            // -- kolam keluar (diisi saat abandonment) --
            exit_pool: u64,
            exit_base: u64,

            // -- sealed batch --
            sealed_order_count: u32,
            sealed_cursor: u32,

            // -- stake-for-service --
            sfs_funded: u64,

            // -- pasca-lulus --
            dex_pool: String,
        }

        program {
            use rialo_s_program::{
                entrypoint::ProgramResult,
                msg,
                pubkey::Pubkey,
            };

            // ===============================================================
            // 1. LAUNCH
            // ===============================================================

            /// Luncurkan token.
            ///
            /// Perhatikan yang TIDAK ada di daftar parameter: `tier`.
            /// Kreator tidak pernah menetapkan tier-nya sendiri. Kalau tier
            /// bisa diklaim, seluruh sistem verifikasi runtuh.
            initiating fn launch(
                &mut self,
                creator: Pubkey,
                mint: Pubkey,
                treasury: Pubkey,
                name: String,
                symbol: String,
                metadata_uri: String,
                telegram_handle: String,
                x_handle: String,
                bond: u64,
                dev_buy: u64,
            ) -> ProgramResult {
                let now = self.unix_timestamp() as u64;

                // >>> SATU-SATUNYA BARIS YANG BELUM TERVERIFIKASI <<<
                let accs = self.accounts();
                let acc = crate::accounts::LaunchAccounts::parse(self.program_id(), accs)?;

                let out = crate::ops::launch(
                    self.program_id(),
                    &acc,
                    crate::ops::LaunchParams { bond, dev_buy, now },
                )?;

                self.creator = creator;
                self.mint = mint;
                self.treasury = treasury;
                self.name = name;
                self.symbol = symbol;
                self.metadata_uri = metadata_uri;
                self.telegram_handle = telegram_handle;
                self.x_handle = x_handle;

                self.tier = out.view.tier;
                self.status = out.view.status;
                self.created_at = now;
                self.sealed_until = out.sealed_until;
                self.graduated_at = 0;

                self.store_view(&out.view);
                self.bond = bond;
                self.bond_settled = false;
                self.creator_tokens_locked = out.dev_tokens;
                self.creator_tranches_unlocked = 0;
                self.exit_pool = 0;
                self.exit_base = 0;
                self.sealed_order_count = 0;
                self.sealed_cursor = 0;
                self.heartbeat_count = 0;
                self.heartbeat_failures = 0;
                self.sfs_funded = 0;

                // Verifikasi sosial lewat REX. API key hidup di TEE.
                SEND verify_socials(self.telegram_handle, self.x_handle)
                    CALL [on_socials_verified];

                // Tutup jendela sealed. Reaktif, bukan cron.
                //
                // CATATAN: `AFTER` di sini memakai timestamp ABSOLUT
                // (created_at + window). Kalau ternyata Venus mengharapkan
                // durasi RELATIF dalam detik, ganti ke `SEALED_WINDOW_SECS`
                // saja — memakai absolut di API relatif menjadwalkan ini
                // ~56 tahun ke depan dan gejalanya adalah "tidak terjadi
                // apa-apa". Cek `venus/price-alert` untuk memastikan.
                AFTER out.sealed_until CALL [settle_sealed_batch];

                // Heartbeat: sosial harus tetap hidup, bukan cuma ada saat launch.
                AFTER now + crate::HEARTBEAT_INTERVAL_SECS CALL [heartbeat];

                msg!("rexo::launched mint={} sealed_until={}", mint, out.sealed_until);
                Ok(())
            }

            // ===============================================================
            // 2. VERIFIKASI
            // ===============================================================

            handler fn on_socials_verified(
                &mut self,
                members: u64,
                age_days: u64,
                ok: bool,
            ) -> ProgramResult {
                let mut view = self.load_view();
                let tier = crate::ops::apply_verification(
                    &mut view, self.bond, members, age_days, ok,
                );

                self.tier = tier;
                self.telegram_members = members;
                self.x_account_age_days = age_days;
                self.verified_at = self.unix_timestamp() as u64;
                self.store_view(&view);

                msg!("rexo::verified tier={} members={} age={}", tier, members, age_days);
                Ok(())
            }

            // ===============================================================
            // 3. TRADING
            // ===============================================================

            control fn buy(
                &mut self,
                quote_in: u64,
                min_tokens_out: u64,
            ) -> ProgramResult {
                let now = self.unix_timestamp() as u64;

                // Selama jendela sealed, order diantre terenkripsi, tidak
                // diisi berurutan. Tidak ada keuntungan menjadi pertama.
                if self.status == crate::STATUS_SEALED
                    && !crate::guards::sealed_window_closed(now, self.sealed_until)
                {
                    self.sealed_order_count += 1;
                    msg!("rexo::buy queued in sealed window n={}", self.sealed_order_count);
                    // TODO(sealed): antrekan sebagai order TERENKRIPSI ke REX.
                    // Order TIDAK boleh tersimpan plaintext di state —
                    // kalau iya, sniper cukup membaca state dan seluruh
                    // mekanisme ini bocor.
                    return Ok(());
                }

                let accs = self.accounts();
                let acc = crate::accounts::TradeAccounts::parse(self.program_id(), accs)?;
                let mut view = self.load_view();

                let out = crate::ops::buy(
                    self.program_id(), &acc, &mut view, quote_in, min_tokens_out, now,
                )?;

                self.store_view(&view);
                self.status = view.status;

                if out.graduated {
                    self.graduated_at = now;
                    START graduate();
                }
                Ok(())
            }

            control fn sell(
                &mut self,
                tokens_in: u64,
                min_quote_out: u64,
            ) -> ProgramResult {
                let now = self.unix_timestamp() as u64;
                let accs = self.accounts();
                let acc = crate::accounts::TradeAccounts::parse(self.program_id(), accs)?;
                let mut view = self.load_view();

                let out = crate::ops::sell(
                    &acc,
                    &mut view,
                    crate::ops::SellParams {
                        tokens_in,
                        min_quote_out,
                        creator: &self.creator,
                        creator_tranches_unlocked: self.creator_tranches_unlocked,
                        exit_pool: self.exit_pool,
                        exit_base: self.exit_base,
                        now,
                    },
                )?;

                self.store_view(&view);
                // Kolam keluar menyusut seiring pemegang token keluar.
                self.exit_pool = out.exit_pool_left;
                self.exit_base = out.exit_base_left;
                Ok(())
            }

            // ===============================================================
            // 4. SEALED BATCH
            // ===============================================================

            control fn settle_sealed_batch(&mut self) -> ProgramResult {
                if self.status != crate::STATUS_SEALED {
                    return Ok(());
                }
                if self.sealed_order_count == 0 {
                    self.status = crate::STATUS_ACTIVE;
                    self.sealed_until = 0;
                    msg!("rexo::sealed_batch empty, going active");
                    return Ok(());
                }
                SEND clear_sealed_batch(self.sealed_order_count) CALL [on_batch_cleared];
                Ok(())
            }

            handler fn on_batch_cleared(
                &mut self,
                total_quote: u64,
                total_tokens: u64,
                ok: bool,
            ) -> ProgramResult {
                if !ok {
                    // Retry berjangka. BUKAN loop — statement async dilarang
                    // di dalam for/while/loop.
                    AFTER self.unix_timestamp() as u64 + 15 CALL [settle_sealed_batch];
                    return Ok(());
                }

                let now = self.unix_timestamp() as u64;
                let accs = self.accounts();
                let acc = crate::accounts::TradeAccounts::parse(self.program_id(), accs)?;
                let mut view = self.load_view();
                view.status = crate::STATUS_ACTIVE;

                // Batch diterapkan sebagai SATU pergerakan agregat.
                let out = crate::ops::buy(
                    self.program_id(), &acc, &mut view, total_quote, total_tokens, now,
                )?;

                self.store_view(&view);
                self.status = view.status;
                self.sealed_until = 0;
                self.sealed_cursor = 0;

                START distribute_fills();
                if out.graduated {
                    self.graduated_at = now;
                    START graduate();
                }
                Ok(())
            }

            /// Bagikan isian batch satu per satu lewat kursor.
            ///
            /// Ini bukan loop — ini rekursi lewat state. Venus melarang
            /// statement async di dalam loop, jadi inilah pola yang wajib
            /// dipakai untuk memproses list.
            control fn distribute_fills(&mut self) -> ProgramResult {
                if self.sealed_cursor >= self.sealed_order_count {
                    msg!("rexo::distribute complete n={}", self.sealed_order_count);
                    return Ok(());
                }
                // TODO(sealed): transfer isian untuk order index sealed_cursor
                self.sealed_cursor += 1;
                // +1 detik, bukan "sekarang". Menjadwalkan pada timestamp
                // saat ini berisiko dieksekusi ulang di blok yang sama dan
                // menghabiskan compute budget dalam satu transaksi.
                AFTER self.unix_timestamp() as u64 + 1 CALL [distribute_fills];
                Ok(())
            }

            // ===============================================================
            // 5. HEARTBEAT
            // ===============================================================

            control fn heartbeat(&mut self) -> ProgramResult {
                if self.status == crate::STATUS_GRADUATED
                    || self.status == crate::STATUS_FINALIZED
                    || self.status == crate::STATUS_ABANDONED
                {
                    return Ok(());
                }
                self.heartbeat_count += 1;
                SEND verify_socials(self.telegram_handle, self.x_handle)
                    CALL [on_heartbeat];
                Ok(())
            }

            handler fn on_heartbeat(
                &mut self,
                members: u64,
                _age_days: u64,
                ok: bool,
            ) -> ProgramResult {
                let now = self.unix_timestamp() as u64;

                if ok && members >= crate::MIN_TELEGRAM_MEMBERS {
                    self.heartbeat_failures = 0;
                    self.telegram_members = members;
                } else {
                    self.heartbeat_failures += 1;
                }

                if self.heartbeat_failures >= crate::HEARTBEAT_FAILURES_BEFORE_ABANDON {
                    let accs = self.accounts();
                    let acc = crate::accounts::TradeAccounts::parse(self.program_id(), accs)?;
                    let mut view = self.load_view();

                    // Pengaman kegagalan terkorelasi. Kalau Telegram down,
                    // JANGAN hanguskan bond ribuan token tak bersalah.
                    // TODO(global): ganti dua angka ini dengan akumulator
                    // global yang di-update tiap siklus heartbeat.
                    let (global_failures, global_checks) = self.global_failure_stats();

                    match crate::ops::abandon(
                        self.program_id(),
                        &acc,
                        &mut view,
                        self.bond,
                        self.creator_tokens_locked,
                        global_failures,
                        global_checks,
                        now,
                    ) {
                        Ok(out) => {
                            self.store_view(&view);
                            self.status = crate::STATUS_ABANDONED;
                            self.tier = crate::TIER_UNVERIFIED;
                            self.bond = 0;
                            self.bond_settled = true;
                            self.creator_tokens_locked = 0;
                            // Bond hangus jadi kolam yang dibayar pro-rata
                            // ke pemegang token saat mereka keluar.
                            self.exit_pool = out.exit_pool;
                            self.exit_base = out.exit_base;
                            return Ok(());
                        }
                        Err(e) => {
                            // Ditekan karena sistemik. Reset penghitung
                            // supaya token ini tidak langsung dihukum lagi
                            // begitu API pulih.
                            msg!("rexo::abandon suppressed: {:?}", e);
                            self.heartbeat_failures = 0;
                        }
                    }
                }

                AFTER now + crate::HEARTBEAT_INTERVAL_SECS CALL [heartbeat];
                Ok(())
            }

            // ===============================================================
            // 6. VESTING
            // ===============================================================

            control fn unlock_tranche(&mut self) -> ProgramResult {
                if self.status == crate::STATUS_ABANDONED
                    || self.creator_tokens_locked == 0
                {
                    return Ok(());
                }
                let view = self.load_view();
                let cfg = view.config();
                let progress = view.curve().progress_bps(&cfg) as u64;
                let graduated = self.status == crate::STATUS_GRADUATED
                    || self.status == crate::STATUS_FINALIZED;

                if !crate::guards::creator_locked(
                    self.creator_tranches_unlocked, progress, graduated,
                ) {
                    self.creator_tranches_unlocked += 1;
                    let amount = self.creator_tokens_locked / 3;
                    // TODO(vesting): transfer `amount` ke creator
                    msg!(
                        "rexo::vesting tranche={} amount={}",
                        self.creator_tranches_unlocked,
                        amount
                    );
                }
                Ok(())
            }

            // ===============================================================
            // 7. GRADUATION
            // ===============================================================

            control fn graduate(&mut self) -> ProgramResult {
                let now = self.unix_timestamp() as u64;
                let accs = self.accounts();
                let acc = crate::accounts::GraduateAccounts::parse(self.program_id(), accs)?;
                let view = self.load_view();

                let out = crate::ops::graduate(
                    self.program_id(), &acc, &view, self.bond, self.bond_settled, now,
                )?;

                self.bond_settled = true;
                self.sfs_funded = out.sfs_endowment;
                self.graduated_at = now;

                // Pool baru dibuka dalam mode sealed juga. Di pump.fun,
                // migrasi adalah momen paling berbahaya: sniper front-run
                // pembuatan LP lalu menjual ke gelombang pembeli pertama.
                self.sealed_until = now + crate::SEALED_WINDOW_SECS;

                SEND create_pool(out.lp_tokens, out.lp_quote) CALL [on_pool_created];
                Ok(())
            }

            handler fn on_pool_created(
                &mut self,
                pool_id: String,
                ok: bool,
            ) -> ProgramResult {
                if !ok {
                    AFTER self.unix_timestamp() as u64 + 30 CALL [graduate];
                    return Ok(());
                }
                self.dex_pool = pool_id;
                finalize();
                Ok(())
            }

            // ===============================================================
            // 8. VIEW
            // ===============================================================

            control fn get_state(&mut self) -> ProgramResult {
                let view = self.load_view();
                let cfg = view.config();
                let st = view.curve();

                msg!(
                    "rexo::state status={} tier={} progress={}bps price={} mcap={} to_grad={}",
                    self.status,
                    self.tier,
                    st.progress_bps(&cfg) as u64,
                    st.price_per_token().unwrap_or(0) as u64,
                    st.market_cap(&cfg).unwrap_or(0) as u64,
                    st.quote_to_graduate(&cfg).unwrap_or(0) as u64
                );
                msg!(
                    "rexo::reserves vq={} vt={} rq={} rt={} fee_p={} fee_c={} bond={}",
                    self.virtual_quote,
                    self.virtual_token,
                    self.real_quote,
                    self.real_token,
                    self.fees_protocol,
                    self.fees_creator,
                    self.bond
                );
                Ok(())
            }

            // ===============================================================
            // 9. TERMINATING
            // ===============================================================

            terminating fn finalize(&mut self) -> ProgramResult {
                self.status = crate::STATUS_FINALIZED;
                msg!("rexo::finalized mint={} pool={}", self.mint, self.dex_pool);
                Ok(())
            }

            terminating fn cancel(&mut self) -> ProgramResult {
                // Hanya boleh dibatalkan sebelum ada perdagangan nyata.
                // Setelah ada pembeli, membatalkan = mencuri.
                if self.real_quote > 0 {
                    msg!("rexo::cancel rejected real_quote={}", self.real_quote);
                    return Err(crate::errors::RexoError::WrongStatus.into());
                }
                self.status = crate::STATUS_ABANDONED;
                Ok(())
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Jembatan state <-> LaunchView
//
// CATATAN: sebelumnya ini ditulis sebagai `trait ViewBridge` tanpa
// implementasi — itu tidak akan compile. Sekarang ditulis sebagai method
// biasa yang HARUS diletakkan di dalam blok `program { }`, bersama
// initiating/handler/control/terminating fn.
//
// Kalau macro Venus mengizinkan `impl` untuk tipe state yang dihasilkannya,
// kamu bisa memindahkannya ke luar. Kalau tidak, tempelkan keempat method
// ini ke dalam blok `program { }` apa adanya.
// ---------------------------------------------------------------------------
//
// fn load_view(&self) -> crate::state::LaunchView {
//     crate::state::LaunchView {
//         tier: self.tier,
//         status: self.status,
//         virtual_quote: self.virtual_quote,
//         virtual_token: self.virtual_token,
//         real_quote: self.real_quote,
//         real_token: self.real_token,
//         fees_protocol: self.fees_protocol,
//         fees_creator: self.fees_creator,
//         forfeited_quote: self.forfeited_quote,
//     }
// }
//
// fn store_view(&mut self, v: &crate::state::LaunchView) {
//     self.tier = v.tier;
//     self.status = v.status;
//     self.virtual_quote = v.virtual_quote;
//     self.virtual_token = v.virtual_token;
//     self.real_quote = v.real_quote;
//     self.real_token = v.real_token;
//     self.fees_protocol = v.fees_protocol;
//     self.fees_creator = v.fees_creator;
//     self.forfeited_quote = v.forfeited_quote;
// }
//
// /// TODO(global): akumulator kegagalan lintas token. Default (0, 1)
// /// berarti "0% gagal" dan MENGIZINKAN abandonment — itu default yang
// /// berbahaya. Ganti sebelum heartbeat pertama jalan di jaringan publik.
// fn global_failure_stats(&self) -> (u64, u64) {
//     (0, 1)
// }
