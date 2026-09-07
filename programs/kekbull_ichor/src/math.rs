//! Integer emission math. No floating point.
//!
//! Human-unit ratio `numerator / denominator` is applied after reading both
//! mint decimals from accounts. Equal decimals (including 255 == 255) scale
//! by 1. Unequal decimals use checked `10^abs(delta)` only. There is no
//! absolute decimal cap - overflow of the required scale or product fails.

use crate::error::{Check, CheckResult};

/// `10^exp` via repeated checked multiply. `10^0 == 1`.
pub fn ten_pow(exp: u8) -> CheckResult<u128> {
    let mut v = 1u128;
    for _ in 0..exp {
        v = v.checked_mul(10).ok_or(Check::ArithmeticOverflow)?;
    }
    Ok(v)
}

pub fn require_ratio(numerator: u64, denominator: u64) -> CheckResult<()> {
    if numerator == 0 || denominator == 0 {
        return Err(Check::ZeroRatio);
    }
    // Human-unit ceiling: numerator/denominator <= 1/1 (finding 2.5).
    let max = crate::constants::MAX_EMISSION_RATIO_NUMERATOR_PER_DENOMINATOR as u128;
    if (numerator as u128) > (denominator as u128).saturating_mul(max) {
        return Err(Check::RatioAboveCeiling);
    }
    Ok(())
}

pub fn require_amount(amount: u64) -> CheckResult<()> {
    if amount == 0 {
        return Err(Check::ZeroAmount);
    }
    Ok(())
}

/// Convert a raw KEKBULL amount into a raw ICHOR amount.
///
/// `ichor_raw = kek_raw * numerator * 10^max(0, i-k)
///              / (denominator * 10^max(0, k-i))`
pub fn ichor_from_kekbull(
    kekbull_amount: u64,
    kekbull_decimals: u8,
    ichor_decimals: u8,
    numerator: u64,
    denominator: u64,
) -> CheckResult<u64> {
    require_amount(kekbull_amount)?;
    require_ratio(numerator, denominator)?;

    let (scale_up, scale_down) = match ichor_decimals.cmp(&kekbull_decimals) {
        core::cmp::Ordering::Equal => (1u128, 1u128),
        core::cmp::Ordering::Greater => (ten_pow(ichor_decimals - kekbull_decimals)?, 1u128),
        core::cmp::Ordering::Less => (1u128, ten_pow(kekbull_decimals - ichor_decimals)?),
    };

    let num = (kekbull_amount as u128)
        .checked_mul(numerator as u128)
        .ok_or(Check::ArithmeticOverflow)?
        .checked_mul(scale_up)
        .ok_or(Check::ArithmeticOverflow)?;
    let den = (denominator as u128)
        .checked_mul(scale_down)
        .ok_or(Check::ArithmeticOverflow)?;

    let minted = num.checked_div(den).ok_or(Check::ArithmeticOverflow)?;
    if minted == 0 {
        return Err(Check::ConversionFloorsToZero);
    }
    u64::try_from(minted).map_err(|_| Check::ArithmeticOverflow)
}

/// Split harvested base units 2500 / 7500 bps. Remainder after the floored
/// creator share is paid to Realms so `creator + realms == amount`.
pub fn split_harvested_fees(amount: u64) -> CheckResult<(u64, u64)> {
    let creator = (amount as u128)
        .checked_mul(crate::constants::FEE_SPLIT_CREATOR_BPS as u128)
        .ok_or(Check::ArithmeticOverflow)?
        .checked_div(crate::constants::FEE_SPLIT_BPS_DENOMINATOR as u128)
        .ok_or(Check::ArithmeticOverflow)?;
    let creator = u64::try_from(creator).map_err(|_| Check::ArithmeticOverflow)?;
    let realms = amount
        .checked_sub(creator)
        .ok_or(Check::ArithmeticOverflow)?;
    Ok((creator, realms))
}

pub fn require_min_ichor(ichor_amount: u64, min_ichor_amount: u64) -> CheckResult<()> {
    if ichor_amount < min_ichor_amount {
        return Err(Check::BelowMinIchorAmount);
    }
    Ok(())
}

/// `timelock_secs == 0` at initialize is terminal: the initialized ratio
/// cannot be proposed, applied, or lengthened.
pub fn ratio_frozen_at_initialize(timelock_secs: u64) -> bool {
    timelock_secs == 0
}

/// Unlock timestamp for a proposed ratio. `timelock_secs == 0` means the
/// initialized ratio is frozen (no proposals).
pub fn pending_unlock_ts(now: i64, timelock_secs: u64) -> CheckResult<i64> {
    if timelock_secs == 0 {
        return Err(Check::RatioLocked);
    }
    let delta = i64::try_from(timelock_secs).map_err(|_| Check::TimestampOverflow)?;
    now.checked_add(delta).ok_or(Check::TimestampOverflow)
}

pub fn ratio_unlock_elapsed(now: i64, unlock_ts: i64) -> bool {
    now >= unlock_ts
}

/// `creator_decay_secs` is immutable after initialize and must be > 0.
/// Zero would make a permissionless sweep valid immediately (§0.2 / §0.3).
pub fn require_creator_decay_secs(decay_secs: u64) -> CheckResult<()> {
    if decay_secs == 0 {
        return Err(Check::CreatorDecayZero);
    }
    Ok(())
}

/// Claim/sweep fail closed if the clock moved backwards relative to last claim.
pub fn require_claim_clock(now: i64, last_claim_ts: i64) -> CheckResult<()> {
    if now < last_claim_ts {
        return Err(Check::ClockInversion);
    }
    Ok(())
}

/// Permissionless sweep is valid only when `now - last_claim_ts > decay_secs`.
/// Fail-closed on clock inversion and on i64/u64 conversion failures.
pub fn creator_decay_elapsed(
    now: i64,
    last_claim_ts: i64,
    decay_secs: u64,
) -> CheckResult<bool> {
    require_creator_decay_secs(decay_secs)?;
    require_claim_clock(now, last_claim_ts)?;
    let elapsed_i = now
        .checked_sub(last_claim_ts)
        .ok_or(Check::ClockInversion)?;
    let elapsed = u64::try_from(elapsed_i).map_err(|_| Check::ClockInversion)?;
    Ok(elapsed > decay_secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_to_one_human_units_6_to_9() {
        assert_eq!(ichor_from_kekbull(1_000_000, 6, 9, 1, 1), Ok(1_000_000_000));
    }

    #[test]
    fn one_to_one_human_units_9_to_6() {
        assert_eq!(ichor_from_kekbull(1_000_000_000, 9, 6, 1, 1), Ok(1_000_000));
    }

    #[test]
    fn equal_255_decimals_needs_no_scale() {
        assert_eq!(ichor_from_kekbull(100, 255, 255, 1, 1), Ok(100));
        assert_eq!(ichor_from_kekbull(7, 255, 255, 1, 1), Ok(7));
        assert_eq!(ten_pow(0), Ok(1));
    }

    #[test]
    fn large_decimal_delta_fails_checked_pow10() {
        // 10^255 cannot fit in u128. Reject the scale, not an arbitrary cap.
        assert_eq!(
            ichor_from_kekbull(1, 0, 255, 1, 1),
            Err(Check::ArithmeticOverflow)
        );
        assert_eq!(
            ichor_from_kekbull(1_000, 255, 0, 1, 1),
            Err(Check::ArithmeticOverflow)
        );
        assert!(ten_pow(38).is_ok());
        assert_eq!(ten_pow(39), Err(Check::ArithmeticOverflow));
    }

    #[test]
    fn one_to_one_same_decimals() {
        assert_eq!(ichor_from_kekbull(42, 6, 6, 1, 1), Ok(42));
        assert_eq!(ichor_from_kekbull(7, 0, 0, 1, 1), Ok(7));
    }

    #[test]
    fn two_ichor_per_kekbull_exceeds_ceiling() {
        assert_eq!(
            ichor_from_kekbull(1_000_000, 6, 9, 2, 1),
            Err(Check::RatioAboveCeiling)
        );
    }

    #[test]
    fn one_ichor_per_two_kekbull() {
        assert_eq!(ichor_from_kekbull(2_000_000, 6, 9, 1, 2), Ok(1_000_000_000));
        assert_eq!(
            ichor_from_kekbull(1, 0, 0, 1, 2),
            Err(Check::ConversionFloorsToZero)
        );
        assert_eq!(ichor_from_kekbull(3, 0, 0, 1, 2), Ok(1));
    }

    #[test]
    fn reduced_scale_matches_unreduced_form() {
        let kek = 123_456_789u64;
        let unreduced = {
            let num = (kek as u128) * 1 * 1_000_000_000u128;
            let den = 2u128 * 1_000_000u128;
            num / den
        };
        assert_eq!(
            ichor_from_kekbull(kek, 6, 9, 1, 2).unwrap() as u128,
            unreduced
        );
    }

    #[test]
    fn zero_and_ratio_rejected() {
        assert_eq!(ichor_from_kekbull(0, 6, 9, 1, 1), Err(Check::ZeroAmount));
        assert_eq!(ichor_from_kekbull(1, 6, 9, 0, 1), Err(Check::ZeroRatio));
        assert_eq!(ichor_from_kekbull(1, 6, 9, 1, 0), Err(Check::ZeroRatio));
        assert_eq!(
            ichor_from_kekbull(1, 6, 9, u64::MAX, 1),
            Err(Check::RatioAboveCeiling)
        );
        assert_eq!(require_ratio(1, 1), Ok(()));
        assert_eq!(require_ratio(1, 2), Ok(()));
        assert_eq!(require_ratio(2, 1), Err(Check::RatioAboveCeiling));
    }

    #[test]
    fn overflow_when_result_exceeds_u64() {
        assert_eq!(
            ichor_from_kekbull(u64::MAX, 0, 1, 1, 1),
            Err(Check::ArithmeticOverflow)
        );
    }

    #[test]
    fn dust_that_cannot_scale_down_is_rejected() {
        assert_eq!(
            ichor_from_kekbull(1, 9, 6, 1, 1),
            Err(Check::ConversionFloorsToZero)
        );
        assert_eq!(
            ichor_from_kekbull(999, 9, 6, 1, 1),
            Err(Check::ConversionFloorsToZero)
        );
        assert_eq!(ichor_from_kekbull(1_000, 9, 6, 1, 1), Ok(1));
    }

    #[test]
    fn min_ichor_amount_rejects_below_signed_floor() {
        let minted = ichor_from_kekbull(1_000_000, 6, 9, 1, 1).unwrap();
        assert_eq!(require_min_ichor(minted, minted), Ok(()));
        assert_eq!(require_min_ichor(minted, minted - 1), Ok(()));
        assert_eq!(
            require_min_ichor(minted, minted + 1),
            Err(Check::BelowMinIchorAmount)
        );
    }

    #[test]
    fn timelock_zero_means_frozen() {
        assert!(ratio_frozen_at_initialize(0));
        assert!(!ratio_frozen_at_initialize(1));
        assert_eq!(pending_unlock_ts(100, 0), Err(Check::RatioLocked));
        assert_eq!(pending_unlock_ts(100, 50), Ok(150));
        assert_eq!(
            pending_unlock_ts(i64::MAX, 1),
            Err(Check::TimestampOverflow)
        );
        assert!(ratio_unlock_elapsed(150, 150));
        assert!(!ratio_unlock_elapsed(149, 150));
    }

    #[test]
    fn floor_is_not_round_half_up() {
        assert_eq!(ichor_from_kekbull(5, 0, 0, 1, 3), Ok(1));
    }

    #[test]
    fn splitter_pays_remainder_to_realms() {
        assert_eq!(split_harvested_fees(0), Ok((0, 0)));
        assert_eq!(split_harvested_fees(1), Ok((0, 1)));
        assert_eq!(split_harvested_fees(3), Ok((0, 3)));
        assert_eq!(split_harvested_fees(4), Ok((1, 3)));
        assert_eq!(split_harvested_fees(100), Ok((25, 75)));
        assert_eq!(split_harvested_fees(101), Ok((25, 76)));
        assert_eq!(split_harvested_fees(7), Ok((1, 6)));
        let (c, r) = split_harvested_fees(u64::MAX).unwrap();
        assert_eq!(c.checked_add(r), Some(u64::MAX));
        assert_eq!(c, u64::MAX / 4);
        assert_eq!(r, u64::MAX - c);
    }

    #[test]
    fn initialize_rejects_zero_creator_decay() {
        assert_eq!(require_creator_decay_secs(0), Err(Check::CreatorDecayZero));
        assert_eq!(require_creator_decay_secs(1), Ok(()));
        assert_eq!(
            creator_decay_elapsed(100, 0, 0),
            Err(Check::CreatorDecayZero)
        );
    }

    #[test]
    fn decay_comparison_is_strict_and_fail_closed() {
        assert_eq!(creator_decay_elapsed(100, 0, 100), Ok(false));
        assert_eq!(creator_decay_elapsed(101, 0, 100), Ok(true));
        assert_eq!(creator_decay_elapsed(50, 50, 1), Ok(false));
        assert_eq!(
            creator_decay_elapsed(10, 20, 1),
            Err(Check::ClockInversion)
        );
        assert_eq!(require_claim_clock(10, 20), Err(Check::ClockInversion));
        assert_eq!(require_claim_clock(20, 20), Ok(()));
        assert_eq!(require_claim_clock(21, 20), Ok(()));
        // i64::MIN - (positive last) would overflow checked_sub after the
        // now < last gate; inversion is rejected first.
        assert_eq!(
            creator_decay_elapsed(i64::MIN, 0, 1),
            Err(Check::ClockInversion)
        );
    }
}
