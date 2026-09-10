// Copyright (c) 2026 Rexo
// SPDX-License-Identifier: Apache-2.0

//! `FeeLedger` — fee ditarik, bukan didorong.
//!
//! # Kenapa berubah dari v1
//!
//! v1 memindahkan fee ke treasury dan creator vault di **setiap**
//! perdagangan: dua transfer tambahan per trade. Selain boros, itu
//! melahirkan bug akuntansi yang kutemukan sendiri — fee tersapu lebih
//! dulu, sehingga `fees_protocol` di state cuma penghitung seumur hidup
//! dan tidak ada saldo tersisa untuk mendanai apa pun.
//!
//! LaunchLab (`claim_creator_fee`, `claim_platform_fee`) dan Meteora DBC
//! (`claim_trading_fee`) memakai pola tarik. Fee menumpuk di vault quote,
//! ledger mencatat siapa berhak berapa, penerima menariknya saat mau.
//!
//! # Invariant yang dijaga modul ini
//!
//! ```text
//! saldo_vault_quote  ==  reserve_kurva  +  fee_belum_ditarik
//! ```
//!
//! Kalau invariant ini pecah, ada jalur yang memindahkan kelvin tanpa
//! mencatatnya. Ada test khusus untuk itu.

use crate::config::{FeeSplit, BPS_DENOM};
use crate::errors::RexoError;

/// Saldo yang belum ditarik, per penerima.
///
/// `referral` tidak ada di sini: ia dibayar langsung saat perdagangan ke
/// alamat yang dikirim pemanggil, karena penerimanya berbeda tiap trade
/// dan menyimpan ledger per-referrer akan butuh satu akun per alamat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FeeLedger {
    pub protocol: u64,
    pub partner: u64,
    pub creator: u64,
    /// Total yang pernah dibayarkan ke referrer. Hanya statistik.
    pub referral_paid: u64,
}

/// Hasil pembagian satu fee perdagangan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeeShares {
    pub protocol: u64,
    pub partner: u64,
    pub creator: u64,
    /// Dibayar langsung, tidak masuk ledger.
    pub referral: u64,
}

impl FeeShares {
    pub fn total(&self) -> u64 {
        self.protocol + self.partner + self.creator + self.referral
    }
    /// Yang menumpuk di vault dan menunggu ditarik.
    pub fn retained(&self) -> u64 {
        self.protocol + self.partner + self.creator
    }
}

/// Bagi fee sesuai config.
///
/// Pembulatan: tiga penerima pertama dibulatkan ke bawah, **protokol
/// menyerap sisanya**. Dengan begitu jumlah bagian selalu persis sama
/// dengan fee total dan tidak ada satu kelvin pun menguap.
///
/// Kalau `referrer_present` false, bagian referral ikut ke protokol —
/// bukan hangus, dan bukan dikembalikan ke trader (itu akan membuat harga
/// berbeda tergantung ada tidaknya referrer).
pub fn split(fee: u64, cfg: &FeeSplit, referrer_present: bool) -> Result<FeeShares, RexoError> {
    if fee == 0 {
        return Ok(FeeShares {
            protocol: 0,
            partner: 0,
            creator: 0,
            referral: 0,
        });
    }
    let f = fee as u128;
    let t = cfg.total_bps as u128;

    let partner = (f * cfg.partner_bps as u128 / t) as u64;
    let creator = (f * cfg.creator_bps as u128 / t) as u64;
    let referral = if referrer_present {
        (f * cfg.referral_bps as u128 / t) as u64
    } else {
        0
    };

    let assigned = partner
        .checked_add(creator)
        .and_then(|v| v.checked_add(referral))
        .ok_or(RexoError::MathOverflow)?;
    if assigned > fee {
        return Err(RexoError::MathOverflow);
    }
    // Protokol mengambil sisa, termasuk remainder pembulatan.
    let protocol = fee - assigned;

    Ok(FeeShares {
        protocol,
        partner,
        creator,
        referral,
    })
}

impl FeeLedger {
    pub fn accrue(&mut self, s: &FeeShares) -> Result<(), RexoError> {
        self.protocol = self
            .protocol
            .checked_add(s.protocol)
            .ok_or(RexoError::MathOverflow)?;
        self.partner = self
            .partner
            .checked_add(s.partner)
            .ok_or(RexoError::MathOverflow)?;
        self.creator = self
            .creator
            .checked_add(s.creator)
            .ok_or(RexoError::MathOverflow)?;
        self.referral_paid = self
            .referral_paid
            .checked_add(s.referral)
            .ok_or(RexoError::MathOverflow)?;
        Ok(())
    }

    /// Total yang masih tersimpan di vault dan menunggu ditarik.
    pub fn outstanding(&self) -> u64 {
        self.protocol + self.partner + self.creator
    }

    /// Kosongkan satu kanal dan kembalikan jumlahnya.
    pub fn take(&mut self, who: Payee) -> u64 {
        let slot = match who {
            Payee::Protocol => &mut self.protocol,
            Payee::Partner => &mut self.partner,
            Payee::Creator => &mut self.creator,
        };
        core::mem::take(slot)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Payee {
    Protocol,
    Partner,
    Creator,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> FeeSplit {
        FeeSplit {
            total_bps: 100,
            protocol_bps: 40,
            partner_bps: 30,
            creator_bps: 20,
            referral_bps: 10,
        }
    }

    #[test]
    fn split_is_exact_no_kelvin_lost() {
        for fee in [1u64, 7, 99, 100, 10_000_000, 858_639_991] {
            let s = split(fee, &cfg(), true).unwrap();
            assert_eq!(s.total(), fee, "fee={}", fee);
        }
    }

    #[test]
    fn split_matches_configured_ratios() {
        let s = split(10_000_000, &cfg(), true).unwrap();
        assert_eq!(s.protocol, 4_000_000);
        assert_eq!(s.partner, 3_000_000);
        assert_eq!(s.creator, 2_000_000);
        assert_eq!(s.referral, 1_000_000);
    }

    #[test]
    fn without_referrer_the_share_goes_to_protocol_not_the_trader() {
        // Penting: total fee harus IDENTIK dengan atau tanpa referrer,
        // kalau tidak harga jadi berbeda tergantung siapa yang mengirim
        // order — dan itu bisa diarbitrase.
        let with = split(10_000_000, &cfg(), true).unwrap();
        let without = split(10_000_000, &cfg(), false).unwrap();
        assert_eq!(with.total(), without.total());
        assert_eq!(without.referral, 0);
        assert_eq!(without.protocol, 5_000_000); // 40 + 10
    }

    #[test]
    fn rounding_remainder_always_lands_on_protocol() {
        // 7 kelvin tidak bisa dibagi rapi 40/30/20/10
        let s = split(7, &cfg(), true).unwrap();
        assert_eq!(s.partner, 2); // 7*30/100 = 2.1 -> 2
        assert_eq!(s.creator, 1); // 7*20/100 = 1.4 -> 1
        assert_eq!(s.referral, 0); // 7*10/100 = 0.7 -> 0
        assert_eq!(s.protocol, 4); // sisanya
        assert_eq!(s.total(), 7);
    }

    #[test]
    fn ledger_accrues_and_drains_exactly() {
        let mut l = FeeLedger::default();
        for _ in 0..50 {
            l.accrue(&split(10_000_000, &cfg(), true).unwrap()).unwrap();
        }
        assert_eq!(l.protocol, 200_000_000);
        assert_eq!(l.partner, 150_000_000);
        assert_eq!(l.creator, 100_000_000);
        assert_eq!(l.referral_paid, 50_000_000);
        assert_eq!(l.outstanding(), 450_000_000);

        assert_eq!(l.take(Payee::Partner), 150_000_000);
        assert_eq!(l.partner, 0);
        assert_eq!(l.outstanding(), 300_000_000);
        // menarik dua kali tidak menghasilkan apa-apa
        assert_eq!(l.take(Payee::Partner), 0);
    }

    #[test]
    fn vault_invariant_holds_across_many_trades() {
        // Simulasi: tiap trade menambah `net` ke reserve dan `fee` ke
        // ledger. Saldo vault harus selalu = reserve + outstanding.
        let mut reserve = 0u64;
        let mut ledger = FeeLedger::default();
        let mut vault = 0u64;

        for i in 1..=200u64 {
            let gross = i * 1_000_000;
            let fee = gross / 100;
            let net = gross - fee;
            let s = split(fee, &cfg(), i % 3 == 0).unwrap();

            // yang benar-benar masuk vault: net + bagian yang ditahan.
            // referral keluar seketika.
            vault += net + s.retained();
            reserve += net;
            ledger.accrue(&s).unwrap();

            assert_eq!(
                vault,
                reserve + ledger.outstanding(),
                "invariant pecah di trade {}",
                i
            );
        }

        // tarik semuanya, vault harus tersisa persis reserve
        vault -= ledger.take(Payee::Protocol);
        vault -= ledger.take(Payee::Partner);
        vault -= ledger.take(Payee::Creator);
        assert_eq!(vault, reserve);
        assert_eq!(ledger.outstanding(), 0);
    }

    #[test]
    fn zero_fee_is_a_no_op() {
        let s = split(0, &cfg(), true).unwrap();
        assert_eq!(s.total(), 0);
    }
}
