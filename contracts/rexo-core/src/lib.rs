//! # Coldstart — launchpad yang hanya mungkin ada di Rialo
//!
//! ## STATUS FILE INI — BACA DULU
//!
//! `src/curve.rs` = kode nyata, 20 test, jalan hari ini dengan `rustc`.
//! `src/lib.rs`   = **KERANGKA DESAIN**, bukan kode siap deploy.
//!
//! Struktur workflow di bawah mengikuti aturan yang terverifikasi dari
//! `AGENTS.md`/`CLAUDE.md` resmi di repo `SubzeroLabs/rialo-examples`
//! (peran fungsi, statement async, blok `rex`, larangan loop). Tapi
//! sintaks persis untuk deklarasi state, tipe token, signature webcall,
//! dan API sealed-execution BELUM bisa aku verifikasi — `docs.rialo.io`
//! memblokir akses otomatis dan direktori repo-nya juga.
//!
//! Sebelum menulis baris pertama yang sungguhan, ambil ini:
//!   https://docs.rialo.io/user/latest/llms-full.txt
//! Itu seluruh buku user + referensi DSL Venus + source contoh anchor
//! dalam satu file markdown. Fetch ke editor/agent kamu.
//!
//! ## ATURAN KERAS VENUS DSL (terverifikasi)
//!
//! - Statement async — `AFTER`, `EVERY`, `ON`, `SEND`, `START` — DILARANG
//!   di dalam `for`/`while`/`loop`, closure, dan async block. Untuk
//!   memproses list, simpan indeks di state; handler menaikkan indeks lalu
//!   menerbitkan ulang operasinya.
//! - Fungsi di blok `rex {}` jalan di TEE dan TIDAK menerima workflow
//!   state. Setiap input harus parameter eksplisit.
//! - Peran: `initiating fn` entry klien, `handler fn` penerima hasil async,
//!   `control fn` penyetir workflow berjalan, `terminating fn` penutup
//!   (tidak dapat instruksi lagi, jadi tidak bisa menerbitkan async).
//! - Argumen program = data transaksi PUBLIK. Jangan pernah mengoper
//!   secret lewat `--arg`.

mod curve;

use curve::{CurveConfig, CurveState, LaunchTier};

// ===========================================================================
// KEBIJAKAN
// ===========================================================================

/// Jendela sealed di awal peluncuran. Selama ini order terenkripsi dan
/// tidak ada yang bisa melihat urutan, ukuran, atau pengirimnya.
/// Semua diisi pada satu clearing price yang sama.
const SEALED_WINDOW_SECONDS: u64 = 90;

/// Interval heartbeat: cek ulang kanal sosial masih hidup.
const HEARTBEAT_INTERVAL_SECONDS: u64 = 21_600; // 6 jam

/// Berapa kali heartbeat gagal berturut-turut sebelum token dianggap
/// ditinggalkan dan bond hangus ke LP.
const HEARTBEAT_FAILURES_BEFORE_ABANDON: u32 = 4; // ~24 jam

/// Ambang anggota Telegram minimum untuk lolos tier Verified.
const MIN_TELEGRAM_MEMBERS: u64 = 50;

/// Umur akun X minimum, dalam hari. Menyaring akun sekali pakai.
const MIN_X_ACCOUNT_AGE_DAYS: u64 = 30;

rialo! {
    // =======================================================================
    // STATE
    //
    // >>> SINTAKS DEKLARASI STATE ADALAH BAGIAN YANG WAJIB DIKONFIRMASI.
    // >>> Salin bentuk persisnya dari venus/http-fetch/src/lib.rs.
    //
    // Field disimpan datar (skalar saja) karena sampai kamu tahu tipe apa
    // yang didukung struct state Venus, skalar adalah taruhan paling aman.
    // =======================================================================
    struct Launch {
        // --- identitas ---
        creator: Address,
        mint: Address,
        name: String,
        symbol: String,
        metadata_uri: String,

        // --- klaim sosial (mentah, belum diverifikasi) ---
        telegram_handle: String,
        x_handle: String,

        // --- hasil verifikasi (DITULIS OLEH REX, bukan oleh kreator) ---
        tier: u8,                 // 0=Unverified 1=Verified 2=Committed
        verified_at: u64,
        telegram_members: u64,
        x_account_age_days: u64,
        heartbeat_failures: u32,
        abandoned: bool,

        // --- parameter kurva (snapshot saat launch, immutable) ---
        cfg_virtual_quote: u128,
        cfg_virtual_token: u128,
        cfg_curve_supply: u128,
        cfg_lp_reserve: u128,

        // --- state kurva ---
        virtual_quote: u128,
        virtual_token: u128,
        real_quote: u128,
        real_token: u128,
        fees_protocol: u128,
        fees_creator: u128,
        forfeited_quote: u128,
        complete: bool,

        // --- bond & vesting kreator ---
        bond: u128,
        bond_returned: bool,
        creator_tokens_locked: u128,
        creator_tranches_unlocked: u8,

        // --- sealed window ---
        sealed_until: u64,
        sealed_order_count: u32,
        sealed_cursor: u32,       // indeks, BUKAN loop — lihat aturan DSL

        // --- stake-for-service ---
        sfs_position: Address,
        sfs_funded: u128,

        // --- pasca-lulus ---
        dex_pool: String,
    }

    // =======================================================================
    // 1. PELUNCURAN — verifikasi dulu, baru tetapkan syarat
    // =======================================================================

    /// Luncurkan token. Perhatikan yang TIDAK ada di sini: kreator tidak
    /// mengoper tier-nya sendiri. Tier ditulis oleh REX setelah verifikasi
    /// nyata. Kalau tier bisa diklaim, seluruh desain ini tidak ada artinya.
    initiating fn launch(
        workflow_pda_slug: String,
        name: String,
        symbol: String,
        metadata_uri: String,
        telegram_handle: String,
        x_handle: String,
        bond: u128,
    ) {
        // Mulai pesimis. Tier hanya bisa naik lewat bukti.
        let cfg = CurveConfig::coldstart(LaunchTier::Unverified);
        cfg.validate().expect("invalid curve config");
        let st = CurveState::new(&cfg).expect("curve init failed");

        // TODO: buat mint dengan supply = cfg.total_supply(), lalu CABUT
        // mint authority dan freeze authority di transaksi yang sama.
        // Kalau authority masih hidup, seluruh matematika curve.rs tidak
        // ada artinya — deployer bisa mencetak supply tambahan kapan saja.

        state.creator      = ctx.sender();
        state.name         = name;
        state.symbol       = symbol;
        state.metadata_uri = metadata_uri;
        state.telegram_handle = telegram_handle;
        state.x_handle     = x_handle;
        state.bond         = bond;
        state.tier         = 0;
        state.abandoned    = false;
        store_cfg(&cfg);
        store_curve(&st);

        // Jendela sealed dibuka sekarang. Tidak ada yang bisa membeli
        // "lebih dulu" karena di dalam jendela tidak ada urutan.
        state.sealed_until = ctx.now() + SEALED_WINDOW_SECONDS;

        // Webcall ter-attest validator. API key hidup di dalam REX dan
        // tidak pernah muncul on-chain. Inilah yang tidak bisa dilakukan
        // program Solana tanpa oracle khusus per token.
        SEND verify_socials(state.telegram_handle, state.x_handle)
            => on_socials_verified();

        // Tutup jendela sealed secara reaktif. Bukan cron, bukan bot.
        AFTER SEALED_WINDOW_SECONDS => settle_sealed_batch();

        // Heartbeat: kanal sosial harus tetap hidup, bukan cuma ada saat launch.
        EVERY HEARTBEAT_INTERVAL_SECONDS => heartbeat();
    }

    /// Menerima verdict dari REX. REX mengembalikan angka, bukan data mentah.
    handler fn on_socials_verified(members: u64, age_days: u64, ok: bool) {
        state.telegram_members   = members;
        state.x_account_age_days = age_days;
        state.verified_at        = ctx.now();

        if !ok || members < MIN_TELEGRAM_MEMBERS || age_days < MIN_X_ACCOUNT_AGE_DAYS {
            state.tier = 0; // tetap Unverified
            return;
        }

        // Bond menentukan Verified vs Committed.
        let cfg_v = CurveConfig::coldstart(LaunchTier::Verified);
        let cfg_c = CurveConfig::coldstart(LaunchTier::Committed);

        if state.bond >= cfg_c.policy().min_bond {
            state.tier = 2;
            store_cfg(&cfg_c);
        } else if state.bond >= cfg_v.policy().min_bond {
            state.tier = 1;
            store_cfg(&cfg_v);
        } else {
            state.tier = 0;
        }
    }

    // =======================================================================
    // 2. TRADING
    // =======================================================================

    /// Beli. Selama jendela sealed, order tidak diisi langsung — ia
    /// diantre terenkripsi dan diisi pada clearing price batch.
    control fn buy(quote_in: u128, min_tokens_out: u128) {
        if ctx.now() < state.sealed_until {
            // TODO: antrekan sebagai order terenkripsi ke REX.
            // Order TIDAK boleh disimpan plaintext di state — kalau iya,
            // sniper cukup membaca state dan seluruh mekanisme ini bocor.
            state.sealed_order_count += 1;
            return;
        }

        let cfg = load_cfg();
        let mut st = load_curve();
        let receipt = st.buy(&cfg, quote_in, min_tokens_out).expect("buy rejected");
        store_curve(&st);

        // TODO: tarik receipt.quote_spent, kirim receipt.tokens_out,
        //       kembalikan receipt.refund kalau bukan nol,
        //       rutekan receipt.fee_creator dan receipt.fee_protocol.

        route_fees(receipt.fee_protocol, receipt.fee_creator);

        if receipt.graduated {
            START graduate();
        }
    }

    control fn sell(tokens_in: u128, min_quote_out: u128) {
        if ctx.now() < state.sealed_until {
            panic!("tidak bisa jual selama jendela sealed");
        }
        let cfg = load_cfg();
        let mut st = load_curve();

        if ctx.sender() == state.creator && state.creator_tokens_locked > 0 {
            panic!("alokasi kreator masih terkunci");
        }

        let receipt = st.sell(&cfg, tokens_in, min_quote_out).expect("sell rejected");
        store_curve(&st);
        route_fees(receipt.fee_protocol, receipt.fee_creator);
    }

    // =======================================================================
    // 3. SEALED BATCH — dieksekusi sekali, di dalam TEE
    // =======================================================================

    /// Dipicu oleh `AFTER` di launch(). Tidak ada bot yang memanggil ini.
    control fn settle_sealed_batch() {
        if state.sealed_order_count == 0 {
            state.sealed_until = 0;
            return;
        }
        // REX mendekripsi seluruh batch, menghitung SATU clearing price,
        // dan mengisi semua order pro-rata pada harga itu. Menjadi orang
        // pertama tidak memberi keuntungan apa pun.
        SEND clear_sealed_batch(state.sealed_order_count) => on_batch_cleared();
    }

    handler fn on_batch_cleared(total_quote: u128, total_tokens: u128, ok: bool) {
        if !ok {
            // Retry berjangka. Bukan loop — statement async dilarang di loop.
            AFTER 15 => settle_sealed_batch();
            return;
        }
        let cfg = load_cfg();
        let mut st = load_curve();

        // Batch diterapkan ke kurva sebagai SATU pergerakan agregat.
        let receipt = st.buy(&cfg, total_quote, 0).expect("batch fill failed");
        assert!(receipt.tokens_out >= total_tokens, "clearing price tidak konsisten");
        store_curve(&st);

        state.sealed_until = 0;
        state.sealed_cursor = 0;

        // Distribusi ke pembeli dilakukan bertahap lewat kursor, bukan loop.
        START distribute_fills();

        if receipt.graduated {
            START graduate();
        }
    }

    /// Bagikan hasil batch, satu potong per invokasi. Kursor disimpan di
    /// state; handler menaikkannya dan menerbitkan ulang. Inilah pola yang
    /// diwajibkan Venus untuk memproses list.
    control fn distribute_fills() {
        if state.sealed_cursor >= state.sealed_order_count {
            return;
        }
        // TODO: transfer isian untuk order index state.sealed_cursor
        state.sealed_cursor += 1;
        AFTER 0 => distribute_fills();
    }

    // =======================================================================
    // 4. HEARTBEAT — sosial harus tetap hidup, bukan cuma ada saat launch
    // =======================================================================

    /// Dipicu oleh `EVERY`. Di Solana ini butuh keeper yang memantau setiap
    /// token selamanya. Dengan 20.000 peluncuran per hari, keeper itu jadi
    /// bisnis tersendiri — persis biaya middleware yang diserang Rialo.
    control fn heartbeat() {
        if state.complete || state.abandoned {
            return;
        }
        SEND verify_socials(state.telegram_handle, state.x_handle)
            => on_heartbeat();
    }

    handler fn on_heartbeat(members: u64, _age_days: u64, ok: bool) {
        if ok && members >= MIN_TELEGRAM_MEMBERS {
            state.heartbeat_failures = 0;
            state.telegram_members = members;
            return;
        }

        state.heartbeat_failures += 1;
        if state.heartbeat_failures >= HEARTBEAT_FAILURES_BEFORE_ABANDON {
            // Kanal sosial mati. Bond hangus KE LP, bukan ke protokol —
            // yang dirugikan adalah pemegang token, jadi merekalah yang
            // dikompensasi.
            let mut st = load_curve();
            st.forfeit_bond(state.bond).expect("forfeit overflow");
            store_curve(&st);

            state.abandoned = true;
            state.bond = 0;
            state.tier = 0;

            // TODO: burn state.creator_tokens_locked.
            state.creator_tokens_locked = 0;
        }
    }

    // =======================================================================
    // 5. VESTING REAKTIF — predikat, bukan timer
    // =======================================================================

    /// Tranche kreator terbuka berdasarkan KONDISI, bukan waktu saja.
    /// Predikat dievaluasi setiap akhir blok oleh semua validator.
    control fn check_vesting() {
        if state.abandoned || state.creator_tokens_locked == 0 {
            return;
        }
        let cfg = load_cfg();
        let st = load_curve();
        let progress = st.progress_bps(&cfg);

        if state.creator_tranches_unlocked == 0 && progress >= 2_500 {
            release_tranche(1);
        } else if state.creator_tranches_unlocked == 1 && state.complete {
            release_tranche(2);
        }
        // Tranche 3 dibuka 30 hari pasca-lulus DAN harga >= harga lulus.
        // Didaftarkan di graduate() sebagai reactive transaction.
    }

    control fn release_tranche(n: u8) {
        state.creator_tranches_unlocked = n;
        // TODO: buka sepertiga creator_tokens_locked ke kreator
    }

    // =======================================================================
    // 6. GRADUATION — tanpa jendela yang bisa di-front-run
    // =======================================================================

    /// `control fn`, bukan `terminating fn`, karena masih perlu menerbitkan
    /// operasi async. Yang menutup workflow adalah finalize().
    control fn graduate() {
        let cfg = load_cfg();
        let st = load_curve();
        let payload = st.graduation_payload(&cfg).expect("belum lulus");

        // TODO: setor payload.lp_tokens + payload.lp_quote ke pool DEX,
        //       lalu BURN LP token-nya. LP yang bisa ditarik kembali
        //       membuat "graduation" cuma rug pull dengan langkah tambahan.

        // Pool baru dibuka dalam mode sealed juga. Di pump.fun, migrasi
        // adalah momen paling berbahaya: sniper front-run pembuatan LP,
        // lalu menjual ke gelombang pembeli pertama.
        state.sealed_until = ctx.now() + SEALED_WINDOW_SECONDS;

        if !state.abandoned && state.bond > 0 {
            state.bond_returned = true;
            // TODO: kembalikan bond ke kreator — dia menuntaskan janjinya
        }

        // Stake-for-Service: sebagian fee protokol di-stake, dan YIELD-nya
        // membiayai automasi token ini selamanya. Tidak ada top-up.
        // Ini fitur yang paling tidak bisa ditiru chain lain.
        START endow_service_stake(payload.fees_protocol / 2);

        SEND create_pool(payload.lp_tokens, payload.lp_quote) => on_pool_created();
    }

    handler fn on_pool_created(pool_id: String, ok: bool) {
        if !ok {
            AFTER 30 => graduate();
            return;
        }
        state.dex_pool = pool_id;
        finalize();
    }

    // =======================================================================
    // 7. STAKE-FOR-SERVICE — token yang membiayai hidupnya sendiri
    // =======================================================================

    /// Buat posisi SfS. Yield-nya dirutekan ke ServicePaymaster, yang
    /// mencetak service credit untuk membayar gas, storage, dan eksekusi
    /// terjadwal token ini. Selamanya, tanpa isi ulang manual.
    ///
    /// Rialo menyebut pola ini "self-maintaining protocols" dan "bulk
    /// sponsorship of user activity" di whitepaper SfS-nya.
    control fn endow_service_stake(amount: u128) {
        if amount == 0 {
            return;
        }
        state.sfs_funded += amount;
        // TODO: buat posisi SfS dengan routing fraction, simpan alamatnya
        //       di state.sfs_position
    }

    /// Menutup workflow peluncuran. Perdagangan berlanjut di pool DEX.
    terminating fn finalize() {
        state.complete = true;
    }

    // =======================================================================
    // REX — jalan di dalam TEE
    //
    // Fungsi di sini tidak melihat workflow state. Semua input eksplisit.
    // API key dan respons mentah tidak pernah keluar dari enclave; yang
    // keluar hanya verdict berupa angka.
    //
    // Contoh dengan blok rex butuh vendored wit/rex-component.wit —
    // jangan hapus file itu kalau menyalin scaffold dari repo examples.
    // =======================================================================
    rex {
        /// Panggil Telegram Bot API dan X API dengan kredensial tersegel,
        /// kembalikan hanya angka yang dibutuhkan program.
        ///
        /// Whitepaper privasi Rialo menyebut persis pola ini: menyimpan
        /// API key dan memakainya untuk mengambil data dari sistem nyata
        /// tanpa pernah membocorkan kredensial atau data mentahnya.
        fn verify_socials(telegram: String, x_handle: String) -> (u64, u64, bool) {
            // TODO: webcall ter-attest dari dalam TEE.
            //   getChatMemberCount untuk telegram
            //   users/by/username untuk x_handle, ambil created_at
            let _ = (telegram, x_handle);
            (0, 0, false)
        }

        /// Dekripsi order batch, hitung satu clearing price, kembalikan
        /// agregatnya. Isi order individual tidak pernah terlihat on-chain.
        fn clear_sealed_batch(order_count: u32) -> (u128, u128, bool) {
            let _ = order_count;
            (0, 0, false)
        }
    }
}

// ===========================================================================
// HELPER
// ===========================================================================

fn tier_from_u8(t: u8) -> LaunchTier {
    match t {
        2 => LaunchTier::Committed,
        1 => LaunchTier::Verified,
        _ => LaunchTier::Unverified,
    }
}

fn load_cfg() -> CurveConfig {
    CurveConfig {
        virtual_quote: state.cfg_virtual_quote,
        virtual_token: state.cfg_virtual_token,
        curve_supply: state.cfg_curve_supply,
        lp_reserve: state.cfg_lp_reserve,
        tier: tier_from_u8(state.tier),
    }
}

fn store_cfg(cfg: &CurveConfig) {
    state.cfg_virtual_quote = cfg.virtual_quote;
    state.cfg_virtual_token = cfg.virtual_token;
    state.cfg_curve_supply = cfg.curve_supply;
    state.cfg_lp_reserve = cfg.lp_reserve;
}

fn load_curve() -> CurveState {
    CurveState {
        virtual_quote: state.virtual_quote,
        virtual_token: state.virtual_token,
        real_quote: state.real_quote,
        real_token: state.real_token,
        fees_protocol: state.fees_protocol,
        fees_creator: state.fees_creator,
        forfeited_quote: state.forfeited_quote,
        complete: state.complete,
    }
}

fn store_curve(st: &CurveState) {
    state.virtual_quote = st.virtual_quote;
    state.virtual_token = st.virtual_token;
    state.real_quote = st.real_quote;
    state.real_token = st.real_token;
    state.fees_protocol = st.fees_protocol;
    state.fees_creator = st.fees_creator;
    state.forfeited_quote = st.forfeited_quote;
    state.complete = st.complete;
}

fn route_fees(protocol: u128, creator: u128) {
    // TODO: transfer ke treasury dan creator vault.
    // Tier Unverified punya creator == 0 secara konstruksi (curve.rs).
    let _ = (protocol, creator);
}
