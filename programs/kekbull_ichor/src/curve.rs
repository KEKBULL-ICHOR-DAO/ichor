//! pump.fun bonding-curve 81-byte core parser and graduation checks.
//!
//! Layout measured in `src/launch/position.rs` and
//! `docs/FINDINGS_20260822_USDC.md`. Discriminator from `docs/idl/pump.json`.

use anchor_lang::prelude::*;

use crate::constants::{
    bonding_curve_pda, BONDING_CURVE_CORE_LEN, BONDING_CURVE_DISCRIMINATOR, CURVE_OFF_COMPLETE,
    CURVE_OFF_CREATOR, CURVE_OFF_DISCRIMINATOR, CURVE_OFF_REAL_QUOTE_RESERVES,
    CURVE_OFF_REAL_TOKEN_RESERVES, CURVE_OFF_TOKEN_TOTAL_SUPPLY, CURVE_OFF_VIRTUAL_QUOTE_RESERVES,
    CURVE_OFF_VIRTUAL_TOKEN_RESERVES, PUMP_PROGRAM,
};
use crate::error::{Check, CheckResult};
use crate::mint_layout::parse_bool;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BondingCurveCore {
    pub virtual_token_reserves: u64,
    pub virtual_quote_reserves: u64,
    pub real_token_reserves: u64,
    pub real_quote_reserves: u64,
    pub token_total_supply: u64,
    pub complete: bool,
    pub creator: Pubkey,
}

fn u64_at(data: &[u8], off: usize) -> u64 {
    u64::from_le_bytes(data[off..off + 8].try_into().unwrap())
}

pub fn parse_bonding_curve_core(data: &[u8]) -> CheckResult<BondingCurveCore> {
    if data.len() < BONDING_CURVE_CORE_LEN {
        return Err(Check::BondingCurveTooShort);
    }
    if data[CURVE_OFF_DISCRIMINATOR..CURVE_OFF_DISCRIMINATOR + 8] != BONDING_CURVE_DISCRIMINATOR {
        return Err(Check::InvalidBondingCurveDiscriminator);
    }
    Ok(BondingCurveCore {
        virtual_token_reserves: u64_at(data, CURVE_OFF_VIRTUAL_TOKEN_RESERVES),
        virtual_quote_reserves: u64_at(data, CURVE_OFF_VIRTUAL_QUOTE_RESERVES),
        real_token_reserves: u64_at(data, CURVE_OFF_REAL_TOKEN_RESERVES),
        real_quote_reserves: u64_at(data, CURVE_OFF_REAL_QUOTE_RESERVES),
        token_total_supply: u64_at(data, CURVE_OFF_TOKEN_TOTAL_SUPPLY),
        complete: parse_bool(data[CURVE_OFF_COMPLETE])?,
        creator: Pubkey::try_from(&data[CURVE_OFF_CREATOR..CURVE_OFF_CREATOR + 32])
            .map_err(|_| Check::BondingCurveTooShort)?,
    })
}

/// Identity of the curve account: canonical PDA and pump.fun owner.
pub fn require_canonical_curve(
    mint: &Pubkey,
    curve_key: &Pubkey,
    curve_owner: &Pubkey,
    expected_curve: Option<&Pubkey>,
) -> CheckResult<()> {
    let (canonical, _) = bonding_curve_pda(mint);
    if *curve_key != canonical {
        return Err(Check::InvalidBondingCurveAddress);
    }
    if let Some(stored) = expected_curve {
        if *curve_key != *stored {
            return Err(Check::InvalidBondingCurveAddress);
        }
    }
    if *curve_owner != PUMP_PROGRAM {
        return Err(Check::InvalidBondingCurveOwner);
    }
    Ok(())
}

/// Conversion gate: graduated and empty of real tokens.
pub fn require_graduated(core: &BondingCurveCore) -> CheckResult<()> {
    if !core.complete {
        return Err(Check::CurveNotComplete);
    }
    if core.real_token_reserves != 0 {
        return Err(Check::CurveStillHasReserves);
    }
    Ok(())
}

pub fn require_curve_for_convert(
    mint: &Pubkey,
    curve_key: &Pubkey,
    curve_owner: &Pubkey,
    expected_curve: &Pubkey,
    data: &[u8],
) -> CheckResult<BondingCurveCore> {
    require_canonical_curve(mint, curve_key, curve_owner, Some(expected_curve))?;
    let core = parse_bonding_curve_core(data)?;
    require_graduated(&core)?;
    Ok(core)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::{
        BONDING_CURVE_CORE_LEN, BONDING_CURVE_DISCRIMINATOR, CURVE_OFF_COMPLETE, CURVE_OFF_CREATOR,
        CURVE_OFF_REAL_QUOTE_RESERVES, CURVE_OFF_REAL_TOKEN_RESERVES, CURVE_OFF_TOKEN_TOTAL_SUPPLY,
        CURVE_OFF_VIRTUAL_QUOTE_RESERVES, CURVE_OFF_VIRTUAL_TOKEN_RESERVES, PUMP_PROGRAM,
    };

    fn core_bytes(disc: [u8; 8], real_token: u64, complete: bool, extra: usize) -> Vec<u8> {
        let mut d = vec![0u8; BONDING_CURVE_CORE_LEN + extra];
        d[0..8].copy_from_slice(&disc);
        d[CURVE_OFF_VIRTUAL_TOKEN_RESERVES..CURVE_OFF_VIRTUAL_TOKEN_RESERVES + 8]
            .copy_from_slice(&1_073_000_000_000_000u64.to_le_bytes());
        d[CURVE_OFF_VIRTUAL_QUOTE_RESERVES..CURVE_OFF_VIRTUAL_QUOTE_RESERVES + 8]
            .copy_from_slice(&30_000_000_000u64.to_le_bytes());
        d[CURVE_OFF_REAL_TOKEN_RESERVES..CURVE_OFF_REAL_TOKEN_RESERVES + 8]
            .copy_from_slice(&real_token.to_le_bytes());
        d[CURVE_OFF_REAL_QUOTE_RESERVES..CURVE_OFF_REAL_QUOTE_RESERVES + 8]
            .copy_from_slice(&85_000_000_000u64.to_le_bytes());
        d[CURVE_OFF_TOKEN_TOTAL_SUPPLY..CURVE_OFF_TOKEN_TOTAL_SUPPLY + 8]
            .copy_from_slice(&1_000_000_000_000_000u64.to_le_bytes());
        d[CURVE_OFF_COMPLETE] = u8::from(complete);
        d[CURVE_OFF_CREATOR..CURVE_OFF_CREATOR + 32].copy_from_slice(&[0xAB; 32]);
        d
    }

    #[test]
    fn parse_81_byte_core() {
        let d = core_bytes(BONDING_CURVE_DISCRIMINATOR, 0, true, 0);
        assert_eq!(d.len(), 81);
        let c = parse_bonding_curve_core(&d).unwrap();
        assert!(c.complete);
        assert_eq!(c.real_token_reserves, 0);
        assert_eq!(c.virtual_quote_reserves, 30_000_000_000);
        assert_eq!(c.creator, Pubkey::new_from_array([0xAB; 32]));
        assert_eq!(require_graduated(&c), Ok(()));
    }

    #[test]
    fn parse_151_byte_account_ignores_trailing_fields() {
        let d = core_bytes(BONDING_CURVE_DISCRIMINATOR, 0, true, 70);
        assert_eq!(d.len(), 151);
        let c = parse_bonding_curve_core(&d).unwrap();
        assert_eq!(require_graduated(&c), Ok(()));
    }

    #[test]
    fn rejects_short_account_and_wrong_discriminator() {
        assert_eq!(
            parse_bonding_curve_core(&[0u8; 80]),
            Err(Check::BondingCurveTooShort)
        );
        let d = core_bytes([0u8; 8], 0, true, 0);
        assert_eq!(
            parse_bonding_curve_core(&d),
            Err(Check::InvalidBondingCurveDiscriminator)
        );
        let mut almost = core_bytes(BONDING_CURVE_DISCRIMINATOR, 0, true, 0);
        almost[7] ^= 1;
        assert_eq!(
            parse_bonding_curve_core(&almost),
            Err(Check::InvalidBondingCurveDiscriminator)
        );
    }

    #[test]
    fn complete_flag_must_be_exactly_0_or_1() {
        let mut d = core_bytes(BONDING_CURVE_DISCRIMINATOR, 0, true, 0);
        d[CURVE_OFF_COMPLETE] = 2;
        assert_eq!(parse_bonding_curve_core(&d), Err(Check::InvalidBool));
        d[CURVE_OFF_COMPLETE] = 0;
        assert!(!parse_bonding_curve_core(&d).unwrap().complete);
    }

    #[test]
    fn complete_false_is_rejected_even_with_zero_reserves() {
        let c = parse_bonding_curve_core(&core_bytes(BONDING_CURVE_DISCRIMINATOR, 0, false, 0))
            .unwrap();
        assert_eq!(require_graduated(&c), Err(Check::CurveNotComplete));
    }

    #[test]
    fn leftover_real_reserves_rejected_even_when_complete() {
        let c =
            parse_bonding_curve_core(&core_bytes(BONDING_CURVE_DISCRIMINATOR, 1, true, 0)).unwrap();
        assert_eq!(require_graduated(&c), Err(Check::CurveStillHasReserves));
    }

    #[test]
    fn canonical_pda_and_owner_are_required() {
        let mint = Pubkey::new_from_array([5u8; 32]);
        let (canonical, _) = bonding_curve_pda(&mint);
        assert_eq!(
            require_canonical_curve(&mint, &canonical, &PUMP_PROGRAM, Some(&canonical)),
            Ok(())
        );
        assert_eq!(
            require_canonical_curve(
                &mint,
                &Pubkey::new_from_array([6u8; 32]),
                &PUMP_PROGRAM,
                Some(&canonical)
            ),
            Err(Check::InvalidBondingCurveAddress)
        );
        assert_eq!(
            require_canonical_curve(
                &mint,
                &canonical,
                &crate::constants::TOKEN_PROGRAM_ID,
                Some(&canonical)
            ),
            Err(Check::InvalidBondingCurveOwner)
        );
        // Stored PDA disagrees with the derived one - treat as wrong address.
        let other = Pubkey::new_from_array([9u8; 32]);
        assert_eq!(
            require_canonical_curve(&mint, &canonical, &PUMP_PROGRAM, Some(&other)),
            Err(Check::InvalidBondingCurveAddress)
        );
    }

    #[test]
    fn convert_helper_requires_graduation_and_identity() {
        let mint = Pubkey::new_from_array([5u8; 32]);
        let (canonical, _) = bonding_curve_pda(&mint);
        let data = core_bytes(BONDING_CURVE_DISCRIMINATOR, 0, true, 0);
        assert!(
            require_curve_for_convert(&mint, &canonical, &PUMP_PROGRAM, &canonical, &data).is_ok()
        );
        let incomplete = core_bytes(BONDING_CURVE_DISCRIMINATOR, 0, false, 0);
        assert_eq!(
            require_curve_for_convert(&mint, &canonical, &PUMP_PROGRAM, &canonical, &incomplete)
                .unwrap_err(),
            Check::CurveNotComplete
        );
    }
}
