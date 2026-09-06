//! # Coldstart — mesin bonding curve
//!
//! Nol dependency, nol tipe on-chain. Bisa diuji sekarang juga:
//!
//! ```text
//! rustc --test src/curve.rs -o /tmp/curve_test && /tmp/curve_test
//! ```
//!
//! ## Kenapa matematikanya identik dengan pump.fun
//!
//! Konstanta di `CurveConfig::coldstart()` disalin dari `Global` account
//! pump.fun yang sebenarnya (lihat 01-RESEARCH.md untuk sumbernya):
//!
//! ```text
//! initial_virtual_token_reserves = 1_073_000_000_000_000
//! initial_virtual_sol_reserves   =    30_000_000_000
//! initial_real_token_reserves    =   793_100_000_000_000
//! token_total_supply             = 1_000_000_000_000_000
//! selesai ketika real_token_reserves == 0
//! ```
//!
//! Ini disengaja. Bentuk kurva bukan bagian yang rusak dari pump.fun —
//! yang rusak adalah SIAPA yang boleh meluncurkan dan apa yang mereka
//! pertaruhkan. Menjaga kurva tetap identik berarti progress bar, market
//! cap, dan harga bisa dibandingkan langsung dengan token pump.fun, dan
//! trader tidak perlu belajar model baru.
//!
//! ## Yang BERBEDA: tier
//!
//! Total fee selalu 100 bps di semua tier, jadi **pembeli membayar harga
//! yang sama persis di mana pun**. Yang berubah cuma syarat untuk kreator:
//! bagi hasil fee, batas alokasi dev, bond, dan vesting. Tier adalah
//! sinyal, bukan pajak terhadap pembeli. Ada test yang menegakkan ini.
//!
//! ## Aturan pembulatan
//!
//! Setiap pembulatan menguntungkan pool, tidak pernah trader. `ceil_div`
//! dipakai pada PEMBAGI, bukan hasil. Jangan ubah arahnya tanpa
//! menjalankan `many_random_ops_never_break_invariants`.

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

/// Total fee, sama di semua tier. Pembeli tidak pernah membayar lebih
/// karena kreator memilih tier yang lebih rendah.
pub const TOTAL_FEE_BPS: u128 = 100;

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
    /// Alokasi kreator melebihi batas tier.
    CreatorCapExceeded,
    /// Bond di bawah minimum tier.
    BondTooSmall,
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
            CurveError::CreatorCapExceeded => "creator allocation cap exceeded",
            CurveError::BondTooSmall => "launch bond below tier minimum",
        })
    }
}

// ---------------------------------------------------------------------------
// Tier
// ---------------------------------------------------------------------------

/// Tier verifikasi peluncuran.
///
/// Tier ditetapkan oleh hasil verifikasi REX pada saat launch, BUKAN oleh
/// klaim kreator. Program tidak boleh menerima tier sebagai argumen dari
/// pemanggil — lihat catatan di lib.rs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchTier {
    /// Tidak ada sosial terverifikasi. Boleh diluncurkan, tapi kreator
    /// tidak dapat bagi hasil fee dan batas alokasinya paling ketat.
    Unverified,
    /// Kanal sosial terbukti hidup lewat webcall ter-attest validator.
    Verified,
    /// Terverifikasi + bond + menerima vesting reaktif.
    Committed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TierPolicy {
    /// Bagian fee untuk kreator, dalam bps dari perdagangan.
    /// Sisanya (TOTAL_FEE_BPS - creator_fee_bps) ke protokol.
    pub creator_fee_bps: u128,
    /// Batas alokasi kreator, bps dari curve_supply.
    pub max_creator_bps: u128,
    /// Bond minimum, dalam unit quote. Dikembalikan saat lulus,
    /// hangus ke LP kalau ditinggalkan.
    pub min_bond: u128,
    /// Apakah vesting reaktif wajib.
    pub vesting_required: bool,
}

impl LaunchTier {
    pub const fn policy(&self) -> TierPolicy {
        match *self {
            LaunchTier::Unverified => TierPolicy {
                creator_fee_bps: 0,
                max_creator_bps: 100, // 1%
                min_bond: 0,
                vesting_required: false,
            },
            LaunchTier::Verified => TierPolicy {
                creator_fee_bps: 25,
                max_creator_bps: 300, // 3%
                min_bond: 2 * ONE_QUOTE,
                vesting_required: false,
            },
            LaunchTier::Committed => TierPolicy {
                creator_fee_bps: 50,
                max_creator_bps: 500, // 5%
                min_bond: 10 * ONE_QUOTE,
                vesting_required: true,
            },
        }
    }

    /// Batas alokasi kreator dalam unit token mentah.
    pub fn max_creator_tokens(&self, cfg: &CurveConfig) -> u128 {
        cfg.curve_supply * self.policy().max_creator_bps / BPS_DENOM
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
    pub tier: LaunchTier,
}

impl CurveConfig {
    /// Parameter produksi. Bentuk kurva identik dengan pump.fun.
    pub const fn coldstart(tier: LaunchTier) -> Self {
        Self {
            virtual_quote: 30 * ONE_QUOTE,             //          30_000_000_000
            virtual_token: 1_073_000_000 * ONE_TOKEN,  //   1_073_000_000_000_000
            curve_supply: 793_100_000 * ONE_TOKEN,     //     793_100_000_000_000
            lp_reserve: 206_900_000 * ONE_TOKEN,       //     206_900_000_000_000
            tier,
        }
    }

    pub fn total_supply(&self) -> u128 {
        self.curve_supply + self.lp_reserve
    }

    pub fn policy(&self) -> TierPolicy {
        self.tier.policy()
    }

    /// Panggil di initiating fn sebelum menulis state apa pun.
    pub fn validate(&self) -> Result<(), CurveError> {
        let p = self.policy();
        if self.virtual_quote == 0
            || self.virtual_token == 0
            || self.curve_supply == 0
            || p.creator_fee_bps > TOTAL_FEE_BPS
            || p.max_creator_bps > BPS_DENOM
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
    /// Fee bagian protokol.
    pub fees_protocol: u128,
    /// Fee bagian kreator.
    pub fees_creator: u128,
    /// Bond yang hangus karena token ditinggalkan. Masuk LP saat lulus.
    pub forfeited_quote: u128,
    pub complete: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuyReceipt {
    pub tokens_out: u128,
    /// Gross yang dipotong dari pembeli (net + fee).
    pub quote_spent: u128,
    pub fee_protocol: u128,
    pub fee_creator: u128,
    /// Sisa yang dikembalikan saat order terakhir melebihi token tersisa.
    pub refund: u128,
    pub graduated: bool,
}

impl BuyReceipt {
    pub fn fee_total(&self) -> u128 {
        self.fee_protocol + self.fee_creator
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SellReceipt {
    pub tokens_in: u128,
    pub quote_out: u128,
    pub fee_protocol: u128,
    pub fee_creator: u128,
}

impl SellReceipt {
    pub fn fee_total(&self) -> u128 {
        self.fee_protocol + self.fee_creator
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraduationPayload {
    /// Token yang disetor ke pool DEX.
    pub lp_tokens: u128,
    /// RLO ke pool: hasil kurva + bond yang hangus.
    pub lp_quote: u128,
    pub fees_protocol: u128,
    pub fees_creator: u128,
}

fn ceil_div(a: u128, b: u128) -> Result<u128, CurveError> {
    if b == 0 {
        return Err(CurveError::Overflow);
    }
    let num = a.checked_add(b - 1).ok_or(CurveError::Overflow)?;
    Ok(num / b)
}

/// Bagi fee sesuai tier. Kreator dibulatkan ke bawah; protokol menyerap sisa,
/// jadi jumlahnya selalu persis sama dengan fee total.
fn split_fee(fee: u128, cfg: &CurveConfig) -> Result<(u128, u128), CurveError> {
    let creator = fee
        .checked_mul(cfg.policy().creator_fee_bps)
        .ok_or(CurveError::Overflow)?
        / TOTAL_FEE_BPS;
    Ok((fee - creator, creator))
}

impl CurveState {
    pub fn new(cfg: &CurveConfig) -> Result<Self, CurveError> {
        cfg.validate()?;
        Ok(Self {
            virtual_quote: cfg.virtual_quote,
            virtual_token: cfg.virtual_token,
            real_quote: 0,
            real_token: cfg.curve_supply,
            fees_protocol: 0,
            fees_creator: 0,
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
            .checked_mul(TOTAL_FEE_BPS)
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
                BPS_DENOM - TOTAL_FEE_BPS,
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

        let (fee_protocol, fee_creator) = split_fee(fee, cfg)?;

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
        self.fees_protocol = self
            .fees_protocol
            .checked_add(fee_protocol)
            .ok_or(CurveError::Overflow)?;
        self.fees_creator = self
            .fees_creator
            .checked_add(fee_creator)
            .ok_or(CurveError::Overflow)?;

        if self.real_token == 0 {
            self.complete = true;
        }

        Ok(BuyReceipt {
            tokens_out,
            quote_spent: net_in + fee,
            fee_protocol,
            fee_creator,
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
            .checked_mul(TOTAL_FEE_BPS)
            .ok_or(CurveError::Overflow)?
            / BPS_DENOM;
        let quote_out = gross_out - fee;
        if quote_out < min_quote_out {
            return Err(CurveError::SlippageExceeded);
        }

        let (fee_protocol, fee_creator) = split_fee(fee, cfg)?;

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
        self.fees_protocol = self
            .fees_protocol
            .checked_add(fee_protocol)
            .ok_or(CurveError::Overflow)?;
        self.fees_creator = self
            .fees_creator
            .checked_add(fee_creator)
            .ok_or(CurveError::Overflow)?;

        Ok(SellReceipt {
            tokens_in,
            quote_out,
            fee_protocol,
            fee_creator,
        })
    }

    // -----------------------------------------------------------------------
    // Bond & alokasi kreator
    // -----------------------------------------------------------------------

    /// Validasi bond peluncuran terhadap minimum tier.
    pub fn check_bond(cfg: &CurveConfig, bond: u128) -> Result<(), CurveError> {
        if bond < cfg.policy().min_bond {
            return Err(CurveError::BondTooSmall);
        }
        Ok(())
    }

    /// Validasi alokasi kreator terhadap batas tier.
    pub fn check_creator_allocation(
        cfg: &CurveConfig,
        tokens: u128,
    ) -> Result<(), CurveError> {
        if tokens > cfg.tier.max_creator_tokens(cfg) {
            return Err(CurveError::CreatorCapExceeded);
        }
        Ok(())
    }

    /// Bond hangus karena token ditinggalkan. Masuk ke LP saat lulus,
    /// bukan ke protokol — supaya pemegang token yang dapat kompensasi.
    ///
    /// Ini TIDAK menyentuh reserve kurva. Menambah `real_quote` tanpa
    /// menambah `virtual_quote` akan merusak invariant harga.
    pub fn forfeit_bond(&mut self, bond: u128) -> Result<(), CurveError> {
        self.forfeited_quote = self
            .forfeited_quote
            .checked_add(bond)
            .ok_or(CurveError::Overflow)?;
        Ok(())
    }

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
            BPS_DENOM - TOTAL_FEE_BPS,
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
            fees_protocol: self.fees_protocol,
            fees_creator: self.fees_creator,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh(tier: LaunchTier) -> (CurveConfig, CurveState) {
        let cfg = CurveConfig::coldstart(tier);
        let st = CurveState::new(&cfg).unwrap();
        (cfg, st)
    }

    // -- Paritas dengan pump.fun ------------------------------------------

    #[test]
    fn constants_match_pumpfun_global_account() {
        let cfg = CurveConfig::coldstart(LaunchTier::Verified);
        assert_eq!(cfg.virtual_token, 1_073_000_000_000_000);
        assert_eq!(cfg.virtual_quote, 30_000_000_000);
        assert_eq!(cfg.curve_supply, 793_100_000_000_000);
        assert_eq!(cfg.total_supply(), 1_000_000_000_000_000);
        // "gap" pump.fun yang menjaga kurva tetap likuid setelah lulus
        assert_eq!(cfg.virtual_token - cfg.curve_supply, 279_900_000_000_000);
    }

    #[test]
    fn config_valid_and_k_fits_u128() {
        for t in [
            LaunchTier::Unverified,
            LaunchTier::Verified,
            LaunchTier::Committed,
        ] {
            let cfg = CurveConfig::coldstart(t);
            assert!(cfg.validate().is_ok());
            assert_eq!(
                cfg.virtual_quote * cfg.virtual_token,
                32_190_000_000_000_000_000_000_000u128
            );
        }
    }

    #[test]
    fn rejects_bad_config() {
        let mut cfg = CurveConfig::coldstart(LaunchTier::Verified);
        cfg.virtual_token = cfg.curve_supply; // pembagi bisa jadi nol
        assert_eq!(cfg.validate(), Err(CurveError::InvalidConfig));
    }

    // -- Invariant lintas tier --------------------------------------------

    #[test]
    fn buyer_pays_identical_price_across_all_tiers() {
        let mut outs = vec![];
        let mut spends = vec![];
        for t in [
            LaunchTier::Unverified,
            LaunchTier::Verified,
            LaunchTier::Committed,
        ] {
            let (cfg, mut st) = fresh(t);
            let r = st.buy(&cfg, ONE_QUOTE, 0).unwrap();
            outs.push(r.tokens_out);
            spends.push(r.quote_spent);
            // total fee selalu sama, apa pun tier-nya
            assert_eq!(r.fee_total(), 10_000_000);
        }
        assert_eq!(outs[0], outs[1]);
        assert_eq!(outs[1], outs[2]);
        assert_eq!(spends[0], spends[2]);
    }

    #[test]
    fn fee_split_is_exact_and_tier_dependent() {
        let cases = [
            (LaunchTier::Unverified, 0u128, 10_000_000u128),
            (LaunchTier::Verified, 2_500_000, 7_500_000),
            (LaunchTier::Committed, 5_000_000, 5_000_000),
        ];
        for (tier, creator, protocol) in cases {
            let (cfg, mut st) = fresh(tier);
            let r = st.buy(&cfg, ONE_QUOTE, 0).unwrap();
            assert_eq!(r.fee_creator, creator, "{:?}", tier);
            assert_eq!(r.fee_protocol, protocol, "{:?}", tier);
            // tidak ada unit yang hilang saat pembagian
            assert_eq!(r.fee_creator + r.fee_protocol, 10_000_000);
        }
    }

    #[test]
    fn tier_caps_creator_allocation() {
        let cfg0 = CurveConfig::coldstart(LaunchTier::Unverified);
        let cfg2 = CurveConfig::coldstart(LaunchTier::Committed);
        assert_eq!(
            LaunchTier::Unverified.max_creator_tokens(&cfg0),
            7_931_000_000_000
        );
        assert_eq!(
            LaunchTier::Committed.max_creator_tokens(&cfg2),
            39_655_000_000_000
        );
        assert_eq!(
            CurveState::check_creator_allocation(&cfg0, 7_931_000_000_001),
            Err(CurveError::CreatorCapExceeded)
        );
        assert!(CurveState::check_creator_allocation(&cfg0, 7_931_000_000_000).is_ok());
    }

    #[test]
    fn tier_enforces_minimum_bond() {
        let cfg = CurveConfig::coldstart(LaunchTier::Committed);
        assert_eq!(
            CurveState::check_bond(&cfg, 9 * ONE_QUOTE),
            Err(CurveError::BondTooSmall)
        );
        assert!(CurveState::check_bond(&cfg, 10 * ONE_QUOTE).is_ok());
        // tier tanpa verifikasi tidak butuh bond
        let cfg0 = CurveConfig::coldstart(LaunchTier::Unverified);
        assert!(CurveState::check_bond(&cfg0, 0).is_ok());
    }

    // -- Mekanika kurva ----------------------------------------------------

    #[test]
    fn genesis_state() {
        let (cfg, st) = fresh(LaunchTier::Verified);
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
        let (cfg, mut st) = fresh(LaunchTier::Verified);
        let r = st.buy(&cfg, ONE_QUOTE, 0).unwrap();
        assert_eq!(r.tokens_out, 34_277_831_558_567);
        assert_eq!(r.fee_total(), 10_000_000);
        assert_eq!(r.quote_spent, ONE_QUOTE);
        assert_eq!(r.refund, 0);
        assert!(!r.graduated);
        assert_eq!(st.progress_bps(&cfg), 432);
    }

    #[test]
    fn round_trip_only_loses_fees() {
        let (cfg, mut st) = fresh(LaunchTier::Verified);
        let buy = st.buy(&cfg, ONE_QUOTE, 0).unwrap();
        let sell = st.sell(&cfg, buy.tokens_out, 0).unwrap();
        assert_eq!(sell.quote_out, 980_100_000); // ~98.01% = dua kali 1%
        assert!(sell.quote_out < buy.quote_spent);
        assert_eq!(st.real_token, cfg.curve_supply);
        assert!(st.real_quote <= 1); // pembulatan tidak menguras pool
    }

    #[test]
    fn price_is_monotonic_and_curve_graduates() {
        let (cfg, mut st) = fresh(LaunchTier::Committed);
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
        // fee dibagi rata di tier Committed
        assert_eq!(st.fees_creator + st.fees_protocol, 858_639_991);
    }

    #[test]
    fn oversized_buy_fills_exactly_and_refunds() {
        let (cfg, mut st) = fresh(LaunchTier::Verified);
        let r = st.buy(&cfg, 200 * ONE_QUOTE, 0).unwrap();
        assert!(r.graduated);
        assert_eq!(r.tokens_out, cfg.curve_supply);
        assert_eq!(r.refund, 114_136_000_952);
        assert_eq!(r.quote_spent + r.refund, 200 * ONE_QUOTE);
        assert_eq!(st.real_quote, 85_005_359_057);
        assert_eq!(r.fee_creator, 214_659_997);
        assert_eq!(r.fee_protocol, 643_979_994);
        assert_eq!(r.fee_total(), 858_639_991);
    }

    #[test]
    fn quote_to_graduate_is_accurate() {
        let (cfg, mut st) = fresh(LaunchTier::Verified);
        let needed = st.quote_to_graduate(&cfg).unwrap();
        let r = st.buy(&cfg, needed, 0).unwrap();
        assert!(r.graduated);
        assert_eq!(r.refund, 0);
    }

    #[test]
    fn no_trading_after_graduation() {
        let (cfg, mut st) = fresh(LaunchTier::Verified);
        st.buy(&cfg, 200 * ONE_QUOTE, 0).unwrap();
        assert_eq!(st.buy(&cfg, ONE_QUOTE, 0), Err(CurveError::CurveComplete));
        assert_eq!(st.sell(&cfg, ONE_TOKEN, 0), Err(CurveError::CurveComplete));
    }

    #[test]
    fn cannot_sell_more_than_circulating() {
        let (cfg, mut st) = fresh(LaunchTier::Verified);
        st.buy(&cfg, ONE_QUOTE, 0).unwrap();
        assert_eq!(
            st.sell(&cfg, cfg.curve_supply, 0),
            Err(CurveError::ExceedsCirculating)
        );
    }

    #[test]
    fn slippage_guard_blocks_bad_fill_without_mutating_state() {
        let (cfg, mut st) = fresh(LaunchTier::Verified);
        let before = st;
        assert_eq!(
            st.buy(&cfg, ONE_QUOTE, 34_277_831_558_568),
            Err(CurveError::SlippageExceeded)
        );
        assert_eq!(st, before, "state berubah walaupun buy gagal");
    }

    #[test]
    fn zero_and_dust_amounts() {
        let (cfg, mut st) = fresh(LaunchTier::Verified);
        assert_eq!(st.buy(&cfg, 0, 0), Err(CurveError::ZeroAmount));
        assert_eq!(st.sell(&cfg, 0, 0), Err(CurveError::ZeroAmount));
        let r = st.buy(&cfg, 1000, 0).unwrap();
        assert_eq!(r.tokens_out, 35_408_998);
        assert_eq!(r.fee_total(), 10);
    }

    // -- Bond hangus -------------------------------------------------------

    #[test]
    fn forfeited_bond_goes_to_lp_not_protocol() {
        let (cfg, mut st) = fresh(LaunchTier::Committed);
        st.forfeit_bond(10 * ONE_QUOTE).unwrap();
        // bond hangus tidak boleh menggeser harga
        let price_before = st.price_per_token().unwrap();
        assert_eq!(price_before, 27);
        assert_eq!(st.real_quote, 0);

        st.buy(&cfg, 200 * ONE_QUOTE, 0).unwrap();
        let p = st.graduation_payload(&cfg).unwrap();
        assert_eq!(p.lp_quote, 85_005_359_057 + 10 * ONE_QUOTE);
        assert_eq!(p.lp_tokens, cfg.lp_reserve);
        // fee tidak ikut bertambah dari bond
        assert_eq!(p.fees_protocol + p.fees_creator, 858_639_991);
    }

    #[test]
    fn graduation_payload_is_gated() {
        let (cfg, st) = fresh(LaunchTier::Verified);
        assert_eq!(
            st.graduation_payload(&cfg),
            Err(CurveError::CurveNotComplete)
        );
    }

    // -- Property test -----------------------------------------------------

    #[test]
    fn many_random_ops_never_break_invariants() {
        let (cfg, mut st) = fresh(LaunchTier::Committed);
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
        let (cfg, mut st) = fresh(LaunchTier::Committed);
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
