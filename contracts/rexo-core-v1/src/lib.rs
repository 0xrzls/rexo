// Copyright (c) 2026 Rexo
// SPDX-License-Identifier: Apache-2.0

//! # Rexo Core — launchpad meme coin native untuk Rialo
//!
//! ## Aturan menulis di dalam `rialo! { }`
//!
//! Blok macro di bawah HANYA memakai konstruksi yang terbukti diterima
//! parser DSL Venus. Daftar ini disusun dengan membandingkan terhadap file
//! yang berhasil compile:
//!
//! | Konstruksi | Boleh | Catatan |
//! |---|---|---|
//! | `// komentar` | ya | |
//! | `/// doc comment` | **TIDAK** | jadi `#[doc]` di level token; parser berhenti |
//! | `let x = ...;` | ya | |
//! | `self.field = ...;` | ya | |
//! | `if cond { }` / `else { }` | ya | |
//! | `match` | **tidak terbukti** | pakai if/else |
//! | `return` | **tidak terbukti** | pakai if/else |
//! | `AFTER <var> CALL [fn];` | ya | var harus variabel lokal sederhana |
//! | `SEND` / `START` / `EVERY` / `ON` | **tidak terbukti** | lihat TROUBLESHOOT.md |
//! | struct literal `Nama { .. }` | ya | |
//! | `crate::path::fn()` | ya | |
//! | `msg!(..)` | ya | |
//!
//! Kalau build gagal dengan `unexpected token, expected }`, itu parser DSL,
//! bukan rustc. Prosedur bisect ada di TROUBLESHOOT.md.
//!
//! ## Peta arsitektur
//!
//! ```text
//! lib.rs        cangkang DSL. Hanya konstruksi terverifikasi.
//!  ├─ ops.rs        logika bisnis. INI yang diaudit.
//!  ├─ curve.rs      matematika bonding curve (21 test, nol dependency)
//!  ├─ state.rs      jembatan u64 <-> u128 (4 test)
//!  ├─ guards.rs     kontrol akses, status, kolam keluar (10 test)
//!  ├─ vault.rs      pemindahan kelvin lewat CPI
//!  ├─ token.rs      mint / burn / transfer Token-2022
//!  ├─ accounts.rs   parsing & validasi PDA
//!  ├─ hooks.rs      statistik lintas-peluncuran
//!  ├─ events.rs     event terstruktur
//!  ├─ errors.rs     error domain, kode stabil
//!  └─ constants.rs  seluruh angka ekonomi
//! ```

pub mod accounts;
pub mod constants;
pub mod curve;
pub mod errors;
pub mod events;
pub mod guards;
pub mod hooks;
pub mod ops;
pub mod state;
pub mod token;
pub mod vault;

pub use constants::*;

use rialo_venus_proc_macro::rialo;

rialo! {
    workflow {
        state {
            creator: Pubkey,
            mint: Pubkey,
            treasury: Pubkey,
            name: String,
            symbol: String,
            metadata_uri: String,

            telegram_handle: String,
            x_handle: String,

            tier: u8,
            verified_at: u64,
            telegram_members: u64,
            x_account_age_days: u64,
            heartbeat_count: u64,
            heartbeat_failures: u32,

            status: u8,
            created_at: u64,
            sealed_until: u64,
            graduated_at: u64,

            virtual_quote: u64,
            virtual_token: u64,
            real_quote: u64,
            real_token: u64,
            fees_protocol: u64,
            fees_creator: u64,
            forfeited_quote: u64,

            bond: u64,
            bond_settled: bool,
            creator_tokens_locked: u64,
            creator_tranches_unlocked: u8,

            exit_pool: u64,
            exit_base: u64,

            sealed_order_count: u32,
            sealed_cursor: u32,

            sfs_funded: u64,
            dex_pool: String,
        }

        program {
            use rialo_s_program::{entrypoint::ProgramResult, msg, pubkey::Pubkey};

            // ===============================================================
            // 1. LAUNCH
            //
            // Perhatikan yang TIDAK ada di parameter: `tier`. Kreator tidak
            // pernah menetapkan tier-nya sendiri.
            // ===============================================================
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

                // AKSES AKUN — lihat CATATAN AKSES AKUN di bawah blok macro.
                // Kalau baris ini masih error, jalankan `cargo expand` dan
                // baca definisi `struct Program` yang digenerate.
                let accs = self.accounts;
                let acc = crate::accounts::LaunchAccounts::parse(self.program_id, accs)?;

                let params = crate::ops::LaunchParams { bond, dev_buy, now };
                let out = crate::ops::launch(self.program_id, &acc, params)?;

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

                self.virtual_quote = out.view.virtual_quote;
                self.virtual_token = out.view.virtual_token;
                self.real_quote = out.view.real_quote;
                self.real_token = out.view.real_token;
                self.fees_protocol = out.view.fees_protocol;
                self.fees_creator = out.view.fees_creator;
                self.forfeited_quote = out.view.forfeited_quote;

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

                // Tutup jendela sealed. Reaktif, bukan cron.
                // AFTER memakai timestamp ABSOLUT — terkonfirmasi dari file
                // yang berhasil compile (created_at + interval).
                let settle_at = out.sealed_until;
                AFTER settle_at CALL [settle_sealed_batch];

                let first_beat = now + crate::HEARTBEAT_INTERVAL_SECS;
                AFTER first_beat CALL [heartbeat];

                // ---------------------------------------------------------
                // VERIFIKASI SOSIAL LEWAT REX — BELUM AKTIF
                //
                // Bentuk statement webcall Venus belum terverifikasi. Yang
                // terbukti diterima parser hanya AFTER..CALL. Bentuk yang
                // kutebak sebelumnya membuat macro gagal parse:
                //
                //   SEND verify_socials(a, b) CALL [on_socials_verified];
                //
                // Cari bentuk aslinya di venus/http-fetch atau
                // venus/rex-wasm-pipeline, lalu aktifkan di sini.
                //
                // Sampai itu terjadi, on_socials_verified tidak pernah
                // dipanggil dan tier tetap 0 (Unverified). Itu default yang
                // AMAN: tier paling ketat, kreator tidak dapat bagi fee.
                // ---------------------------------------------------------

                msg!("rexo::launched sealed_until={}", settle_at);
                msg!("rexo::launched next_beat={}", first_beat);
                Ok(())
            }

            // ===============================================================
            // 2. VERIFIKASI
            // ===============================================================
            // on_socials_verified DIHAPUS.
            //
            // Macro membangun enum `interface::Instruction` hanya dari entry
            // point yang benar-benar bisa dipanggil. Handler tanpa operasi
            // async yang menargetkannya tidak masuk enum itu, sehingga kode
            // dispatch yang digenerate merujuk varian yang tidak ada:
            //
            //   error[E0599]: no variant named `OnSocialsVerified`
            //                 found for enum `interface::Instruction`
            //
            // Kembalikan fungsi ini BERSAMAAN dengan statement webcall yang
            // menargetkannya. Handler dan pemanggilnya satu paket.

            // ===============================================================
            // 3. TRADING
            // ===============================================================
            control fn buy(&mut self, quote_in: u64, min_tokens_out: u64) -> ProgramResult {
                let now = self.unix_timestamp() as u64;
                let in_window = self.status == crate::STATUS_SEALED
                    && now < self.sealed_until;

                if in_window {
                    // Order diantre, tidak diisi berurutan. Tidak ada
                    // keuntungan menjadi pertama.
                    //
                    // TODO(sealed): antrekan sebagai order TERENKRIPSI ke
                    // REX. Order tidak boleh tersimpan plaintext di state.
                    self.sealed_order_count += 1;
                    msg!("rexo::buy queued n={}", self.sealed_order_count);
                } else {
                    let accs = self.accounts;
                    let acc = crate::accounts::TradeAccounts::parse(
                        self.program_id, accs,
                    )?;
                    let mut view = crate::state::view_from(
                        self.tier,
                        self.status,
                        self.virtual_quote,
                        self.virtual_token,
                        self.real_quote,
                        self.real_token,
                        self.fees_protocol,
                        self.fees_creator,
                        self.forfeited_quote,
                    );

                    let out = crate::ops::buy(
                        self.program_id, &acc, &mut view, quote_in, min_tokens_out, now,
                    )?;

                    self.status = view.status;
                    self.virtual_quote = view.virtual_quote;
                    self.virtual_token = view.virtual_token;
                    self.real_quote = view.real_quote;
                    self.real_token = view.real_token;
                    self.fees_protocol = view.fees_protocol;
                    self.fees_creator = view.fees_creator;
                    self.forfeited_quote = view.forfeited_quote;

                    if out.graduated {
                        self.graduated_at = now;
                        // Dijadwalkan, bukan dipanggil langsung: graduate
                        // butuh set akun berbeda, jadi ia harus jalan
                        // sebagai transaksi tersendiri.
                        let grad_at = now + 1;
                        AFTER grad_at CALL [graduate];
                    }
                }
                Ok(())
            }

            control fn sell(&mut self, tokens_in: u64, min_quote_out: u64) -> ProgramResult {
                let now = self.unix_timestamp() as u64;
                let accs = self.accounts;
                let acc = crate::accounts::TradeAccounts::parse(self.program_id, accs)?;
                let mut view = crate::state::view_from(
                    self.tier,
                    self.status,
                    self.virtual_quote,
                    self.virtual_token,
                    self.real_quote,
                    self.real_token,
                    self.fees_protocol,
                    self.fees_creator,
                    self.forfeited_quote,
                );

                let params = crate::ops::SellParams {
                    tokens_in,
                    min_quote_out,
                    creator: &self.creator,
                    creator_tranches_unlocked: self.creator_tranches_unlocked,
                    exit_pool: self.exit_pool,
                    exit_base: self.exit_base,
                    now,
                };
                let out = crate::ops::sell(&acc, &mut view, params)?;

                self.virtual_quote = view.virtual_quote;
                self.virtual_token = view.virtual_token;
                self.real_quote = view.real_quote;
                self.real_token = view.real_token;
                self.fees_protocol = view.fees_protocol;
                self.fees_creator = view.fees_creator;

                // Kolam keluar menyusut seiring pemegang token keluar.
                self.exit_pool = out.exit_pool_left;
                self.exit_base = out.exit_base_left;
                Ok(())
            }

            // ===============================================================
            // 4. SEALED BATCH
            // ===============================================================
            handler fn settle_sealed_batch(&mut self) -> ProgramResult {
                if self.status == crate::STATUS_SEALED {
                    if self.sealed_order_count == 0 {
                        self.status = crate::STATUS_ACTIVE;
                        self.sealed_until = 0;
                        msg!("rexo::sealed empty, active");
                    } else {
                        // TODO(sealed): panggil REX untuk mendekripsi batch
                        // dan menghitung satu clearing price. Butuh bentuk
                        // statement webcall yang belum terverifikasi.
                        //
                        // Sementara: buka perdagangan normal supaya alur
                        // tidak macet. Ini MELEMAHKAN proteksi sniper —
                        // jangan dibiarkan begini di mainnet.
                        self.status = crate::STATUS_ACTIVE;
                        self.sealed_until = 0;
                        msg!("rexo::sealed fallback n={}", self.sealed_order_count);
                        AFTER 1 seconds CALL [distribute_fills];
                    }
                }
                Ok(())
            }

            handler fn distribute_fills(&mut self) -> ProgramResult {
                if self.sealed_cursor < self.sealed_order_count {
                    // TODO(sealed): transfer isian untuk sealed_cursor
                    self.sealed_cursor += 1;
                    // +1 detik, bukan "sekarang": menjadwalkan pada
                    // timestamp saat ini berisiko dieksekusi ulang di blok
                    // yang sama dan menghabiskan compute budget.
                    //
                    // Ini bukan loop — ini rekursi lewat state cursor.
                    // Venus melarang statement async di dalam for/while.
                    let next = self.unix_timestamp() as u64 + 1;
                    AFTER next CALL [distribute_fills];
                }
                Ok(())
            }

            // ===============================================================
            // 5. HEARTBEAT
            // ===============================================================
            handler fn heartbeat(&mut self) -> ProgramResult {
                let running = self.status == crate::STATUS_ACTIVE
                    || self.status == crate::STATUS_SEALED;

                if running {
                    self.heartbeat_count += 1;
                    let now = self.unix_timestamp() as u64;

                    // TODO(rex): di sini seharusnya webcall verifikasi
                    // sosial. Tanpa bentuk statement yang terverifikasi,
                    // heartbeat hanya berdetak tanpa memeriksa apa pun.
                    let next_tick = now + crate::HEARTBEAT_INTERVAL_SECS;
                    AFTER next_tick CALL [heartbeat];

                    msg!("rexo::beat n={}", self.heartbeat_count);
                    msg!("rexo::beat next={}", next_tick);
                }
                Ok(())
            }

            // on_heartbeat DIHAPUS — alasan sama seperti di atas.
            //
            // Konsekuensinya: `heartbeat` sekarang berdetak tanpa memeriksa
            // apa pun, dan `try_abandon` tidak pernah terpicu otomatis.
            // Tidak ada bond yang bisa hangus. Itu default yang aman.

            control fn try_abandon(&mut self) -> ProgramResult {
                let now = self.unix_timestamp() as u64;
                let stats = crate::hooks::global_failure_stats();

                // Pengaman diperiksa DULU, di luar ops::abandon, supaya
                // penekanan tidak membatalkan transaksi dan kita masih bisa
                // menjadwalkan ulang heartbeat.
                let permitted =
                    crate::guards::abandonment_permitted(stats.0, stats.1).is_ok();

                if permitted {
                    let accs = self.accounts;
                    let acc = crate::accounts::TradeAccounts::parse(
                        self.program_id, accs,
                    )?;
                    let mut view = crate::state::view_from(
                        self.tier,
                        self.status,
                        self.virtual_quote,
                        self.virtual_token,
                        self.real_quote,
                        self.real_token,
                        self.fees_protocol,
                        self.fees_creator,
                        self.forfeited_quote,
                    );

                    let out = crate::ops::abandon(
                        self.program_id,
                        &acc,
                        &mut view,
                        self.bond,
                        self.creator_tokens_locked,
                        stats.0,
                        stats.1,
                        now,
                    )?;

                    self.status = crate::STATUS_ABANDONED;
                    self.tier = crate::TIER_UNVERIFIED;
                    self.bond = 0;
                    self.bond_settled = true;
                    self.creator_tokens_locked = 0;

                    // Bond hangus jadi kolam yang dibayar pro-rata ke
                    // pemegang token saat mereka keluar. Menjual TETAP
                    // diizinkan setelah abandonment.
                    self.exit_pool = out.exit_pool;
                    self.exit_base = out.exit_base;

                    msg!("rexo::abandoned pool={}", out.exit_pool);
                    msg!("rexo::abandoned base={}", out.exit_base);
                } else {
                    // Kegagalan sistemik: ini masalah kita, bukan mereka.
                    self.heartbeat_failures = 0;
                    let next_tick = now + crate::HEARTBEAT_INTERVAL_SECS;
                    AFTER next_tick CALL [heartbeat];
                    msg!("rexo::abandon suppressed (correlated failure)");
                }
                Ok(())
            }

            // ===============================================================
            // 6. VESTING
            // ===============================================================
            control fn unlock_tranche(&mut self) -> ProgramResult {
                let eligible = self.status != crate::STATUS_ABANDONED
                    && self.creator_tokens_locked > 0;

                if eligible {
                    let view = crate::state::view_from(
                        self.tier,
                        self.status,
                        self.virtual_quote,
                        self.virtual_token,
                        self.real_quote,
                        self.real_token,
                        self.fees_protocol,
                        self.fees_creator,
                        self.forfeited_quote,
                    );
                    let cfg = view.config();
                    let progress = view.curve().progress_bps(&cfg) as u64;
                    let graduated = self.status == crate::STATUS_GRADUATED
                        || self.status == crate::STATUS_FINALIZED;

                    let locked = crate::guards::creator_locked(
                        self.creator_tranches_unlocked, progress, graduated,
                    );

                    if !locked {
                        self.creator_tranches_unlocked += 1;
                        let amount = self.creator_tokens_locked / 3;
                        // TODO(vesting): transfer `amount` ke kreator
                        msg!("rexo::vest t={}", self.creator_tranches_unlocked);
                        msg!("rexo::vest amt={}", amount);
                    }
                }
                Ok(())
            }

            // ===============================================================
            // 7. GRADUATION
            // ===============================================================
            handler fn graduate(&mut self) -> ProgramResult {
                let now = self.unix_timestamp() as u64;
                let accs = self.accounts;
                let acc = crate::accounts::GraduateAccounts::parse(self.program_id, accs)?;
                let view = crate::state::view_from(
                    self.tier,
                    self.status,
                    self.virtual_quote,
                    self.virtual_token,
                    self.real_quote,
                    self.real_token,
                    self.fees_protocol,
                    self.fees_creator,
                    self.forfeited_quote,
                );

                let out = crate::ops::graduate(
                    self.program_id, &acc, &view, self.bond, self.bond_settled, now,
                )?;

                self.bond_settled = true;
                self.sfs_funded = out.sfs_endowment;
                self.graduated_at = now;
                self.status = crate::STATUS_FINALIZED;

                // TODO(pool): buat pool lalu BURN LP token. LP yang bisa
                // ditarik kembali membuat "graduation" cuma rug pull dengan
                // langkah tambahan.
                // TODO(sfs): buat posisi Stake-for-Service dari treasury.

                msg!("rexo::graduated lp_q={}", out.lp_quote);
                msg!("rexo::graduated sfs={}", out.sfs_endowment);
                Ok(())
            }

            // ===============================================================
            // 8. VIEW
            // ===============================================================
            control fn get_state(&mut self) -> ProgramResult {
                let view = crate::state::view_from(
                    self.tier,
                    self.status,
                    self.virtual_quote,
                    self.virtual_token,
                    self.real_quote,
                    self.real_token,
                    self.fees_protocol,
                    self.fees_creator,
                    self.forfeited_quote,
                );
                let cfg = view.config();
                let st = view.curve();
                let progress = st.progress_bps(&cfg) as u64;
                let price = st.price_per_token().unwrap_or(0) as u64;
                let mcap = st.market_cap(&cfg).unwrap_or(0) as u64;

                msg!("rexo::state status={}", self.status);
                msg!("rexo::state tier={}", self.tier);
                msg!("rexo::state progress_bps={}", progress);
                msg!("rexo::state price={}", price);
                msg!("rexo::state mcap={}", mcap);
                msg!("rexo::res vq={}", self.virtual_quote);
                msg!("rexo::res vt={}", self.virtual_token);
                msg!("rexo::res rq={}", self.real_quote);
                msg!("rexo::res rt={}", self.real_token);
                msg!("rexo::res exit_pool={}", self.exit_pool);
                msg!("rexo::res exit_base={}", self.exit_base);
                Ok(())
            }

            // ===============================================================
            // 9. TERMINATING
            // ===============================================================
            terminating fn finalize(&mut self) -> ProgramResult {
                self.status = crate::STATUS_FINALIZED;
                msg!("rexo::finalized pool={}", self.dex_pool);
                Ok(())
            }

            terminating fn cancel(&mut self) -> ProgramResult {
                // Setelah ada pembeli, membatalkan sama dengan mencuri.
                if self.real_quote == 0 {
                    self.status = crate::STATUS_ABANDONED;
                    msg!("rexo::cancelled");
                } else {
                    msg!("rexo::cancel rejected rq={}", self.real_quote);
                }
                Ok(())
            }
        }
    }
}

// ===========================================================================
// CATATAN AKSES AKUN
//
// Error sebelumnya memberi tahu tipe `self` di dalam macro:
//
//   &mut Program<'program, 'account_info>
//
// Tipe itu DIGENERATE macro, jadi tidak ada di docs.rs dan tidak bisa
// dicari. Yang bisa dipastikan dari pesan error:
//
//   error[E0599]: no method named `accounts`   -> 5x, sama dengan jumlah
//   error[E0599]: no method named `program_id` -> 9x  call site di kode
//
// Korelasinya persis, jadi keduanya bukan method. Kode sekarang memakai
// akses FIELD (`self.accounts`, `self.program_id`), yang merupakan bentuk
// paling umum untuk struct hasil generate dengan dua lifetime seperti itu.
//
// KALAU MASIH ERROR, jangan menebak lagi — baca definisinya langsung:
//
//   cargo install cargo-expand
//   cargo expand --lib > /tmp/expanded.rs
//   grep -n "struct Program" -A 25 /tmp/expanded.rs
//   grep -n "impl.*Program" -A 40 /tmp/expanded.rs
//
// Itu akan menampilkan nama field dan method yang sebenarnya. Kandidat
// lain yang mungkin, urut dari yang paling masuk akal:
//
//   self.account_infos          self.accounts()
//   self.ctx.accounts           self.infos
//   self.id                     self.key
//
// Ganti di 5 baris `let accs = ...` dan 9 baris `self.program_id`.
// Seluruh modul lain (ops, accounts, vault, token) tidak perlu disentuh.
// ===========================================================================
