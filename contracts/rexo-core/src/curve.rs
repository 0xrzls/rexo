//! # Rexo — matematika bonding curve
//!
//! Nol dependency, nol tipe on-chain. Bisa diuji sekarang juga:
//!
//! ```text
//! rustc --test src/curve.rs -o /tmp/t && /tmp/t
//! ```
//!
//! ## Ruang lingkup
//!
//! Modul ini HANYA matematika AMM: reserve masuk, reserve keluar. Ia tidak
//! tahu apa-apa soal siapa yang menerima fee — pembagian empat arah ada di
//! `fees.rs`, dan parameternya ada di `config.rs`.
//!
//! `fee_bps` tetap dibutuhkan di sini untuk satu hal: menghitung jumlah
//! gross pada pengisian terakhir yang menghasilkan refund.
//!
//! ## Konstanta preset
//!
//! `CurveConfig::pumpfun_like` memakai angka dari `Global` account pump.fun:
//!
//! ```text
//! virtual_token = 1_073_000_000_000_000
//! virtual_quote =        30_000_000_000
//! curve_supply  =   793_100_000_000_000
//! total supply  = 1_000_000_000_000_000
//! ```
//!
//! Bentuk kurva bukan bagian yang rusak dari pump.fun, jadi ia
//! dipertahankan. Yang berubah ada di lapisan lain.
//!
//! ## Aturan pembulatan
//!
//! Setiap pembulatan menguntungkan pool, tidak pernah trader. `ceil_div`
//! dipakai pada PEMBAGI, bukan hasil.

#![allow(dead_code)]

// ---------------------------------------------------------------------------
// Konstanta
// ---------------------------------------------------------------------------

pub const BPS_DENOM: u128 = 10_000;

pub const TOKEN_DECIMALS: u32 = 6;
/// Desimal RLO. ASUMSI: 9 (pola lamport). Belum dikonfirmasi — lihat 04-AUDIT.md.
pub const QUOTE_DECIMALS: u32 = 9;

pub const ONE_TOKEN: u128 = 1_000_000;
pub const ONE_QUOTE: u128 = 1_000_000_000;


// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurveError {
    ZeroAmount,
    CurveComplete,
    CurveNotComplete,
    Overflow,
    SlippageExceeded,
    ExceedsCirculating,
    InvalidConfig,
}

impl core::fmt::Display for CurveError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            CurveError::ZeroAmount => "amount resolves to zero",
            CurveError::CurveComplete => "curve already graduated",
            CurveError::CurveNotComplete => "curve has not graduated yet",
            CurveError::Overflow => "arithmetic overflow",
            CurveError::SlippageExceeded => "slippage limit exceeded",
            CurveError::ExceedsCirculating => "exceeds circulating supply",
            CurveError::InvalidConfig => "invalid curve config",
        })
    }
}

// ---------------------------------------------------------------------------
// Konfigurasi
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CurveConfig {
    pub virtual_quote: u128,
    pub virtual_token: u128,
    pub curve_supply: u128,
    pub lp_reserve: u128,
    /// Fee perdagangan total, bps. Pembagiannya BUKAN urusan modul ini —
    /// lihat `fees.rs`. Di sini fee hanya dibutuhkan untuk menghitung
    /// jumlah gross pada pengisian terakhir yang menghasilkan refund.
    pub fee_bps: u128,
}

impl CurveConfig {
    /// Parameter produksi. Bentuk kurva identik dengan pump.fun.
    pub const fn pumpfun_like(fee_bps: u128) -> Self {
        Self {
            virtual_quote: 30 * ONE_QUOTE,             //          30_000_000_000
            virtual_token: 1_073_000_000 * ONE_TOKEN,  //   1_073_000_000_000_000
            curve_supply: 793_100_000 * ONE_TOKEN,     //     793_100_000_000_000
            lp_reserve: 206_900_000 * ONE_TOKEN,       //     206_900_000_000_000
            fee_bps,
        }
    }

    pub fn total_supply(&self) -> u128 {
        self.curve_supply + self.lp_reserve
    }

    /// Panggil di initiating fn sebelum menulis state apa pun.
    pub fn validate(&self) -> Result<(), CurveError> {
        if self.virtual_quote == 0
            || self.virtual_token == 0
            || self.curve_supply == 0
            || self.fee_bps == 0
            || self.fee_bps >= BPS_DENOM
            // virtual_token harus melebihi curve_supply, kalau tidak
            // pembagi jadi nol saat kurva hampir habis.
            || self.virtual_token <= self.curve_supply
        {
            return Err(CurveError::InvalidConfig);
        }
        self.virtual_quote
            .checked_mul(self.virtual_token)
            .ok_or(CurveError::InvalidConfig)?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CurveState {
    pub virtual_quote: u128,
    pub virtual_token: u128,
    /// RLO nyata di vault kurva (di luar fee dan bond).
    pub real_quote: u128,
    /// Token yang masih tersisa untuk dijual.
    pub real_token: u128,
    /// Bond yang hangus karena token ditinggalkan. Masuk LP saat lulus.
    pub forfeited_quote: u128,
    pub complete: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuyReceipt {
    pub tokens_out: u128,
    /// Gross yang dipotong dari pembeli (net + fee).
    pub quote_spent: u128,
    /// Fee total. Pembagiannya dilakukan `fees.rs`.
    pub fee: u128,
    /// Sisa yang dikembalikan saat order terakhir melebihi token tersisa.
    pub refund: u128,
    pub graduated: bool,
}



#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SellReceipt {
    pub tokens_in: u128,
    pub quote_out: u128,
    pub fee: u128,
}



#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraduationPayload {
    /// Token yang disetor ke pool DEX.
    pub lp_tokens: u128,
    /// RLO ke pool: hasil kurva + bond yang hangus.
    pub lp_quote: u128,
}

fn ceil_div(a: u128, b: u128) -> Result<u128, CurveError> {
    if b == 0 {
        return Err(CurveError::Overflow);
    }
    let num = a.checked_add(b - 1).ok_or(CurveError::Overflow)?;
    Ok(num / b)
}

impl CurveState {
    pub fn new(cfg: &CurveConfig) -> Result<Self, CurveError> {
        cfg.validate()?;
        Ok(Self {
            virtual_quote: cfg.virtual_quote,
            virtual_token: cfg.virtual_token,
            real_quote: 0,
            real_token: cfg.curve_supply,
            forfeited_quote: 0,
            complete: false,
        })
    }

    pub fn k(&self) -> Result<u128, CurveError> {
        self.virtual_quote
            .checked_mul(self.virtual_token)
            .ok_or(CurveError::Overflow)
    }

    // -----------------------------------------------------------------------
    // Buy
    // -----------------------------------------------------------------------

    /// Tukar `quote_in` RLO menjadi token.
    ///
    /// `min_tokens_out` adalah proteksi slippage. Untuk order yang masuk
    /// sealed window, nilai ini dievaluasi terhadap clearing price batch,
    /// bukan harga tick-per-tick — lihat 02-ARCHITECTURE.md.
    pub fn buy(
        &mut self,
        cfg: &CurveConfig,
        quote_in: u128,
        min_tokens_out: u128,
    ) -> Result<BuyReceipt, CurveError> {
        if self.complete {
            return Err(CurveError::CurveComplete);
        }
        if quote_in == 0 {
            return Err(CurveError::ZeroAmount);
        }

        let mut fee = quote_in
            .checked_mul(cfg.fee_bps)
            .ok_or(CurveError::Overflow)?
            / BPS_DENOM;
        let mut net_in = quote_in - fee;
        if net_in == 0 {
            return Err(CurveError::ZeroAmount);
        }

        let k = self.k()?;
        let new_vq = self
            .virtual_quote
            .checked_add(net_in)
            .ok_or(CurveError::Overflow)?;

        // ceil pada pembagi -> tokens_out dibulatkan ke BAWAH. Pool menang.
        let mut tokens_out = self
            .virtual_token
            .checked_sub(ceil_div(k, new_vq)?)
            .ok_or(CurveError::Overflow)?;
        let mut refund = 0u128;

        if tokens_out >= self.real_token {
            // Order terakhir: isi persis sisa token, kembalikan selebihnya.
            tokens_out = self.real_token;
            let new_vt = self
                .virtual_token
                .checked_sub(tokens_out)
                .ok_or(CurveError::Overflow)?;
            let required_vq = ceil_div(k, new_vt)?;
            net_in = required_vq
                .checked_sub(self.virtual_quote)
                .ok_or(CurveError::Overflow)?;

            let mut gross = ceil_div(
                net_in.checked_mul(BPS_DENOM).ok_or(CurveError::Overflow)?,
                BPS_DENOM - cfg.fee_bps,
            )?;
            // Pembulatan bisa mendorong gross 1 unit melewati input; clamp
            // supaya pembeli tidak pernah ditagih lebih dari yang dikirim.
            if gross > quote_in {
                gross = quote_in;
            }
            fee = gross - net_in; // aman: net_in <= gross secara konstruksi
            refund = quote_in - gross;
        }

        if tokens_out == 0 {
            return Err(CurveError::ZeroAmount);
        }
        if tokens_out < min_tokens_out {
            return Err(CurveError::SlippageExceeded);
        }


        self.virtual_quote = self
            .virtual_quote
            .checked_add(net_in)
            .ok_or(CurveError::Overflow)?;
        self.virtual_token = self
            .virtual_token
            .checked_sub(tokens_out)
            .ok_or(CurveError::Overflow)?;
        self.real_quote = self
            .real_quote
            .checked_add(net_in)
            .ok_or(CurveError::Overflow)?;
        self.real_token = self
            .real_token
            .checked_sub(tokens_out)
            .ok_or(CurveError::Overflow)?;

        if self.real_token == 0 {
            self.complete = true;
        }

        Ok(BuyReceipt {
            tokens_out,
            quote_spent: net_in + fee,
            fee,
            refund,
            graduated: self.complete,
        })
    }

    // -----------------------------------------------------------------------
    // Sell
    // -----------------------------------------------------------------------

    pub fn sell(
        &mut self,
        cfg: &CurveConfig,
        tokens_in: u128,
        min_quote_out: u128,
    ) -> Result<SellReceipt, CurveError> {
        if self.complete {
            return Err(CurveError::CurveComplete);
        }
        if tokens_in == 0 {
            return Err(CurveError::ZeroAmount);
        }

        let token_back = self
            .real_token
            .checked_add(tokens_in)
            .ok_or(CurveError::Overflow)?;
        if token_back > cfg.curve_supply {
            return Err(CurveError::ExceedsCirculating);
        }

        let k = self.k()?;
        let new_vt = self
            .virtual_token
            .checked_add(tokens_in)
            .ok_or(CurveError::Overflow)?;

        let gross_out = self
            .virtual_quote
            .checked_sub(ceil_div(k, new_vt)?)
            .ok_or(CurveError::Overflow)?;
        if gross_out == 0 {
            return Err(CurveError::ZeroAmount);
        }

        let fee = gross_out
            .checked_mul(cfg.fee_bps)
            .ok_or(CurveError::Overflow)?
            / BPS_DENOM;
        let quote_out = gross_out - fee;
        if quote_out < min_quote_out {
            return Err(CurveError::SlippageExceeded);
        }


        self.virtual_quote = self
            .virtual_quote
            .checked_sub(gross_out)
            .ok_or(CurveError::Overflow)?;
        self.virtual_token = new_vt;
        self.real_quote = self
            .real_quote
            .checked_sub(gross_out)
            .ok_or(CurveError::Overflow)?;
        self.real_token = token_back;

        Ok(SellReceipt {
            tokens_in,
            quote_out,
            fee,
        })
    }

    // -----------------------------------------------------------------------
    // Bond & alokasi kreator
    // -----------------------------------------------------------------------

    // -----------------------------------------------------------------------
    // Views
    // -----------------------------------------------------------------------

    /// Harga spot: unit quote per 1 token utuh.
    pub fn price_per_token(&self) -> Result<u128, CurveError> {
        Ok(self
            .virtual_quote
            .checked_mul(ONE_TOKEN)
            .ok_or(CurveError::Overflow)?
            / self.virtual_token)
    }

    /// Market cap dalam unit quote mentah.
    /// Rumus identik dengan pump.fun:
    /// `virtual_sol_reserves * token_total_supply / virtual_token_reserves`
    pub fn market_cap(&self, cfg: &CurveConfig) -> Result<u128, CurveError> {
        Ok(self
            .virtual_quote
            .checked_mul(cfg.total_supply())
            .ok_or(CurveError::Overflow)?
            / self.virtual_token)
    }

    /// Progress menuju graduation, 0..=10_000 bps.
    pub fn progress_bps(&self, cfg: &CurveConfig) -> u128 {
        if cfg.curve_supply == 0 {
            return BPS_DENOM;
        }
        let sold = cfg.curve_supply.saturating_sub(self.real_token);
        sold.saturating_mul(BPS_DENOM) / cfg.curve_supply
    }

    /// Gross quote yang dibutuhkan untuk menerima persis `tokens_out`.
    ///
    /// Ini pasangan `exact_out` dari `buy`. Dibutuhkan supaya pengguna bisa
    /// meminta jumlah token yang pasti, bukan hanya membelanjakan jumlah
    /// quote yang pasti.
    pub fn quote_in_for_tokens_out(
        &self,
        cfg: &CurveConfig,
        tokens_out: u128,
    ) -> Result<u128, CurveError> {
        if self.complete {
            return Err(CurveError::CurveComplete);
        }
        if tokens_out == 0 {
            return Err(CurveError::ZeroAmount);
        }
        if tokens_out > self.real_token {
            return Err(CurveError::ExceedsCirculating);
        }
        let k = self.k()?;
        let new_vt = self
            .virtual_token
            .checked_sub(tokens_out)
            .ok_or(CurveError::Overflow)?;
        let net = ceil_div(k, new_vt)?
            .checked_sub(self.virtual_quote)
            .ok_or(CurveError::Overflow)?;
        // ceil di kedua langkah: pembeli membayar sedikit lebih, tidak
        // pernah kurang. Pool tidak boleh rugi karena pembulatan.
        ceil_div(
            net.checked_mul(BPS_DENOM).ok_or(CurveError::Overflow)?,
            BPS_DENOM - cfg.fee_bps,
        )
    }

    /// Token yang harus dijual untuk menerima persis `quote_out` bersih.
    pub fn tokens_in_for_quote_out(
        &self,
        cfg: &CurveConfig,
        quote_out: u128,
    ) -> Result<u128, CurveError> {
        if self.complete {
            return Err(CurveError::CurveComplete);
        }
        if quote_out == 0 {
            return Err(CurveError::ZeroAmount);
        }
        // quote_out adalah nilai SETELAH fee, jadi naikkan ke gross dulu.
        let gross = ceil_div(
            quote_out.checked_mul(BPS_DENOM).ok_or(CurveError::Overflow)?,
            BPS_DENOM - cfg.fee_bps,
        )?;
        let k = self.k()?;
        let new_vq = self
            .virtual_quote
            .checked_sub(gross)
            .ok_or(CurveError::Overflow)?;
        if new_vq == 0 {
            return Err(CurveError::Overflow);
        }
        let tokens_in = ceil_div(k, new_vq)?
            .checked_sub(self.virtual_token)
            .ok_or(CurveError::Overflow)?;
        if self.real_token + tokens_in > cfg.curve_supply {
            return Err(CurveError::ExceedsCirculating);
        }
        Ok(tokens_in)
    }

    /// Gross RLO untuk menuntaskan kurva dari posisi sekarang.
    pub fn quote_to_graduate(&self, _cfg: &CurveConfig) -> Result<u128, CurveError> {
        if self.complete {
            return Ok(0);
        }
        let k = self.k()?;
        let new_vt = self
            .virtual_token
            .checked_sub(self.real_token)
            .ok_or(CurveError::Overflow)?;
        let net = ceil_div(k, new_vt)?
            .checked_sub(self.virtual_quote)
            .ok_or(CurveError::Overflow)?;
        ceil_div(
            net.checked_mul(BPS_DENOM).ok_or(CurveError::Overflow)?,
            BPS_DENOM - cfg.fee_bps,
        )
    }

    pub fn graduation_payload(
        &self,
        cfg: &CurveConfig,
    ) -> Result<GraduationPayload, CurveError> {
        if !self.complete {
            return Err(CurveError::CurveNotComplete);
        }
        Ok(GraduationPayload {
            lp_tokens: cfg.lp_reserve,
            lp_quote: self
                .real_quote
                .checked_add(self.forfeited_quote)
                .ok_or(CurveError::Overflow)?,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const FEE: u128 = 100;

    fn fresh() -> (CurveConfig, CurveState) {
        let cfg = CurveConfig::pumpfun_like(FEE);
        let st = CurveState::new(&cfg).unwrap();
        (cfg, st)
    }

    // -- Paritas dengan pump.fun ------------------------------------------

    #[test]
    fn constants_match_pumpfun_global_account() {
        let cfg = CurveConfig::pumpfun_like(FEE);
        assert_eq!(cfg.virtual_token, 1_073_000_000_000_000);
        assert_eq!(cfg.virtual_quote, 30_000_000_000);
        assert_eq!(cfg.curve_supply, 793_100_000_000_000);
        assert_eq!(cfg.total_supply(), 1_000_000_000_000_000);
        // "gap" pump.fun yang menjaga kurva tetap likuid setelah lulus
        assert_eq!(cfg.virtual_token - cfg.curve_supply, 279_900_000_000_000);
    }

    #[test]
    fn k_fits_u128_with_room() {
        let cfg = CurveConfig::pumpfun_like(FEE);
        assert!(cfg.validate().is_ok());
        assert_eq!(
            cfg.virtual_quote * cfg.virtual_token,
            32_190_000_000_000_000_000_000_000u128
        );
    }

    #[test]
    fn rejects_zero_or_full_fee() {
        let mut cfg = CurveConfig::pumpfun_like(0);
        assert_eq!(cfg.validate(), Err(CurveError::InvalidConfig));
        cfg = CurveConfig::pumpfun_like(10_000);
        assert_eq!(cfg.validate(), Err(CurveError::InvalidConfig));
    }

    #[test]
    fn rejects_bad_config() {
        let mut cfg = CurveConfig::pumpfun_like(FEE);
        cfg.virtual_token = cfg.curve_supply; // pembagi bisa jadi nol
        assert_eq!(cfg.validate(), Err(CurveError::InvalidConfig));
    }

    // -- Mekanika kurva ----------------------------------------------------

    #[test]
    fn genesis_state() {
        let (cfg, st) = fresh();
        assert_eq!(st.real_token, cfg.curve_supply);
        assert_eq!(st.real_quote, 0);
        assert!(!st.complete);
        assert_eq!(st.progress_bps(&cfg), 0);
        assert_eq!(st.price_per_token().unwrap(), 27);
        // ~27.96 RLO, memakai rumus mcap pump.fun persis
        assert_eq!(st.market_cap(&cfg).unwrap(), 27_958_993_476);
    }

    #[test]
    fn buy_one_rlo_matches_reference() {
        let (cfg, mut st) = fresh();
        let r = st.buy(&cfg, ONE_QUOTE, 0).unwrap();
        assert_eq!(r.tokens_out, 34_277_831_558_567);
        assert_eq!(r.fee, 10_000_000);
        assert_eq!(r.quote_spent, ONE_QUOTE);
        assert_eq!(r.refund, 0);
        assert!(!r.graduated);
        assert_eq!(st.progress_bps(&cfg), 432);
    }

    #[test]
    fn round_trip_only_loses_fees() {
        let (cfg, mut st) = fresh();
        let buy = st.buy(&cfg, ONE_QUOTE, 0).unwrap();
        let sell = st.sell(&cfg, buy.tokens_out, 0).unwrap();
        assert_eq!(sell.quote_out, 980_100_000); // ~98.01% = dua kali 1%
        assert!(sell.quote_out < buy.quote_spent);
        assert_eq!(st.real_token, cfg.curve_supply);
        assert!(st.real_quote <= 1); // pembulatan tidak menguras pool
    }

    #[test]
    fn price_is_monotonic_and_curve_graduates() {
        let (cfg, mut st) = fresh();
        let mut last = st.price_per_token().unwrap();
        let mut buys = 0u32;
        let mut spent = 0u128;
        while !st.complete {
            let r = st.buy(&cfg, ONE_QUOTE, 0).unwrap();
            spent += r.quote_spent;
            let p = st.price_per_token().unwrap();
            assert!(p >= last, "harga turun saat beli");
            last = p;
            buys += 1;
            assert!(buys < 1000);
        }
        assert_eq!(buys, 86);
        assert_eq!(spent, 85_863_999_048);
        assert_eq!(st.real_token, 0);
        assert_eq!(st.progress_bps(&cfg), BPS_DENOM);
    }

    #[test]
    fn oversized_buy_fills_exactly_and_refunds() {
        let (cfg, mut st) = fresh();
        let r = st.buy(&cfg, 200 * ONE_QUOTE, 0).unwrap();
        assert!(r.graduated);
        assert_eq!(r.tokens_out, cfg.curve_supply);
        assert_eq!(r.refund, 114_136_000_952);
        assert_eq!(r.quote_spent + r.refund, 200 * ONE_QUOTE);
        assert_eq!(st.real_quote, 85_005_359_057);
        assert_eq!(r.fee_creator, 214_659_997);
        assert_eq!(r.fee_protocol, 643_979_994);
        assert_eq!(r.fee, 858_639_991);
    }

    #[test]
    fn quote_to_graduate_is_accurate() {
        let (cfg, mut st) = fresh();
        let needed = st.quote_to_graduate(&cfg).unwrap();
        let r = st.buy(&cfg, needed, 0).unwrap();
        assert!(r.graduated);
        assert_eq!(r.refund, 0);
    }

    #[test]
    fn no_trading_after_graduation() {
        let (cfg, mut st) = fresh();
        st.buy(&cfg, 200 * ONE_QUOTE, 0).unwrap();
        assert_eq!(st.buy(&cfg, ONE_QUOTE, 0), Err(CurveError::CurveComplete));
        assert_eq!(st.sell(&cfg, ONE_TOKEN, 0), Err(CurveError::CurveComplete));
    }

    #[test]
    fn cannot_sell_more_than_circulating() {
        let (cfg, mut st) = fresh();
        st.buy(&cfg, ONE_QUOTE, 0).unwrap();
        assert_eq!(
            st.sell(&cfg, cfg.curve_supply, 0),
            Err(CurveError::ExceedsCirculating)
        );
    }

    #[test]
    fn slippage_guard_blocks_bad_fill_without_mutating_state() {
        let (cfg, mut st) = fresh();
        let before = st;
        assert_eq!(
            st.buy(&cfg, ONE_QUOTE, 34_277_831_558_568),
            Err(CurveError::SlippageExceeded)
        );
        assert_eq!(st, before, "state berubah walaupun buy gagal");
    }

    #[test]
    fn zero_and_dust_amounts() {
        let (cfg, mut st) = fresh();
        assert_eq!(st.buy(&cfg, 0, 0), Err(CurveError::ZeroAmount));
        assert_eq!(st.sell(&cfg, 0, 0), Err(CurveError::ZeroAmount));
        let r = st.buy(&cfg, 1000, 0).unwrap();
        assert_eq!(r.tokens_out, 35_408_998);
        assert_eq!(r.fee, 10);
    }

    // -- Bond hangus -------------------------------------------------------

    #[test]
    fn graduation_payload_is_gated() {
        let (cfg, st) = fresh();
        assert_eq!(
            st.graduation_payload(&cfg),
            Err(CurveError::CurveNotComplete)
        );
    }

    // -- Property test -----------------------------------------------------

    #[test]
    fn exact_out_round_trips_with_exact_in() {
        let (cfg, mut st) = fresh();
        // minta persis 34.277.831.558.567 token
        let want = 34_277_831_558_567u128;
        let need = st.quote_in_for_tokens_out(&cfg, want).unwrap();
        let r = st.buy(&cfg, need, want).unwrap();
        assert!(r.tokens_out >= want, "exact_out kurang dari yang diminta");
        assert_eq!(r.refund, 0);
    }

    #[test]
    fn exact_out_rejects_more_than_available() {
        let (cfg, st) = fresh();
        assert_eq!(
            st.quote_in_for_tokens_out(&cfg, cfg.curve_supply + 1),
            Err(CurveError::ExceedsCirculating)
        );
        assert_eq!(
            st.quote_in_for_tokens_out(&cfg, 0),
            Err(CurveError::ZeroAmount)
        );
    }

    #[test]
    fn sell_exact_out_delivers_at_least_requested() {
        let (cfg, mut st) = fresh();
        let b = st.buy(&cfg, 10 * ONE_QUOTE, 0).unwrap();
        let want = ONE_QUOTE; // mau terima 1 RLO bersih
        let tokens = st.tokens_in_for_quote_out(&cfg, want).unwrap();
        assert!(tokens <= b.tokens_out, "butuh lebih dari yang dipegang");
        let r = st.sell(&cfg, tokens, 0).unwrap();
        assert!(r.quote_out >= want, "menerima kurang dari yang diminta");
    }

    #[test]
    fn many_random_ops_never_break_invariants() {
        let (cfg, mut st) = fresh();
        let mut held = 0u128;
        let mut seed = 0x2545F4914F6CDD1Du64;
        for _ in 0..2000 {
            if st.complete {
                break;
            }
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let k_before = st.k().unwrap();
            if seed % 3 == 0 && held > ONE_TOKEN {
                let amt = held / 2;
                st.sell(&cfg, amt, 0).unwrap();
                held -= amt;
            } else {
                let amt = (seed as u128 % (5 * ONE_QUOTE)) + 1000;
                let r = st.buy(&cfg, amt, 0).unwrap();
                held += r.tokens_out;
            }
            // k menyusut = kebocoran akibat pembulatan
            assert!(st.k().unwrap() >= k_before, "k menyusut");
            assert!(st.real_token <= cfg.curve_supply);
            // konservasi token: yang keluar kurva = yang dipegang
            assert_eq!(st.real_token + held, cfg.curve_supply);
        }
    }

    #[test]
    fn fees_never_touch_curve_reserves() {
        let (cfg, mut st) = fresh();
        let mut gross_in = 0u128;
        for _ in 0..40 {
            let r = st.buy(&cfg, 2 * ONE_QUOTE, 0).unwrap();
            gross_in += r.quote_spent;
            if r.graduated {
                break;
            }
        }
        // Setiap RLO masuk harus terhitung: kurva + fee. Tidak ada yang hilang.
        assert_eq!(
            gross_in,
            st.real_quote + st.fees_protocol + st.fees_creator
        );
    }
}
