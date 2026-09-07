use anchor_lang::prelude::*;

use crate::constants::CONFIG_SEED;
use crate::error::{Check, CheckResult};

/// Single program config. ICHOR mint authority is this PDA.
#[account]
#[derive(InitSpace)]
pub struct Config {
    pub authority: Pubkey,
    pub pending_authority: Option<Pubkey>,
    pub kekbull_mint: Pubkey,
    pub ichor_mint: Pubkey,
    pub kekbull_token_program: Pubkey,
    pub ichor_token_program: Pubkey,
    pub pump_program: Pubkey,
    pub bonding_curve: Pubkey,
    pub emission_numerator: u64,
    pub emission_denominator: u64,
    pub pending_emission_numerator: u64,
    pub pending_emission_denominator: u64,
    pub pending_ratio_unlock_ts: i64,
    pub ratio_timelock_secs: u64,
    pub paused: bool,
    pub ratio_updates_frozen: bool,
    pub has_pending_ratio: bool,
    pub total_kekbull_burned: u64,
    pub total_ichor_minted: u64,
    pub creator_beneficiary: Option<Pubkey>,
    pub realms_program: Option<Pubkey>,
    pub realms_realm: Option<Pubkey>,
    pub realms_governance: Option<Pubkey>,
    pub realms_native_treasury: Option<Pubkey>,
    pub fee_beneficiaries_bound: bool,
    pub transfer_fee_authority_revoked: bool,
    pub withdraw_withheld_bump: u8,
    /// Gross Token-2022 fees withdrawn from the mint-level accumulator.
    pub total_fees_withdrawn: u64,
    /// Net base units measured into the creator escrow after second-hop fee.
    /// Does not mean units that reached the creator wallet.
    pub total_fees_to_creator: u64,
    /// Net base units measured into the Realms destination after second-hop fee
    /// on the 75% distribute leg (not sweeps of unclaimed creator escrow).
    pub total_fees_to_realms: u64,
    pub creator_escrow_bump: u8,
    /// Last successful creator claim or beneficiary rotation. Set at initialize
    /// from Clock - never left at unix epoch 0.
    pub last_creator_claim_ts: i64,
    /// Immutable after initialize. Must be > 0.
    pub creator_decay_secs: u64,
    /// Net base units claimed from escrow into the creator ATA.
    pub total_creator_claimed: u64,
    /// Net base units swept from escrow into the Realms treasury.
    pub total_creator_swept: u64,
    pub bump: u8,
    pub reserved: [u8; 32],
}

impl Config {
    pub const SEED: &'static [u8] = CONFIG_SEED;

    pub fn space() -> usize {
        8 + Self::INIT_SPACE
    }
}

/// Fields the validators need, without the Anchor account wrapper.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigView {
    pub authority: Pubkey,
    pub pending_authority: Option<Pubkey>,
    pub kekbull_mint: Pubkey,
    pub ichor_mint: Pubkey,
    pub kekbull_token_program: Pubkey,
    pub ichor_token_program: Pubkey,
    pub pump_program: Pubkey,
    pub bonding_curve: Pubkey,
    pub emission_numerator: u64,
    pub emission_denominator: u64,
    pub pending_emission_numerator: u64,
    pub pending_emission_denominator: u64,
    pub pending_ratio_unlock_ts: i64,
    pub ratio_timelock_secs: u64,
    pub paused: bool,
    pub ratio_updates_frozen: bool,
    pub has_pending_ratio: bool,
    pub total_kekbull_burned: u64,
    pub total_ichor_minted: u64,
    pub creator_beneficiary: Option<Pubkey>,
    pub realms_program: Option<Pubkey>,
    pub realms_realm: Option<Pubkey>,
    pub realms_governance: Option<Pubkey>,
    pub realms_native_treasury: Option<Pubkey>,
    pub fee_beneficiaries_bound: bool,
    pub transfer_fee_authority_revoked: bool,
    pub withdraw_withheld_bump: u8,
    pub total_fees_withdrawn: u64,
    pub total_fees_to_creator: u64,
    pub total_fees_to_realms: u64,
    pub creator_escrow_bump: u8,
    pub last_creator_claim_ts: i64,
    pub creator_decay_secs: u64,
    pub total_creator_claimed: u64,
    pub total_creator_swept: u64,
}

impl From<&Config> for ConfigView {
    fn from(c: &Config) -> Self {
        Self {
            authority: c.authority,
            pending_authority: c.pending_authority,
            kekbull_mint: c.kekbull_mint,
            ichor_mint: c.ichor_mint,
            kekbull_token_program: c.kekbull_token_program,
            ichor_token_program: c.ichor_token_program,
            pump_program: c.pump_program,
            bonding_curve: c.bonding_curve,
            emission_numerator: c.emission_numerator,
            emission_denominator: c.emission_denominator,
            pending_emission_numerator: c.pending_emission_numerator,
            pending_emission_denominator: c.pending_emission_denominator,
            pending_ratio_unlock_ts: c.pending_ratio_unlock_ts,
            ratio_timelock_secs: c.ratio_timelock_secs,
            paused: c.paused,
            ratio_updates_frozen: c.ratio_updates_frozen,
            has_pending_ratio: c.has_pending_ratio,
            total_kekbull_burned: c.total_kekbull_burned,
            total_ichor_minted: c.total_ichor_minted,
            creator_beneficiary: c.creator_beneficiary,
            realms_program: c.realms_program,
            realms_realm: c.realms_realm,
            realms_governance: c.realms_governance,
            realms_native_treasury: c.realms_native_treasury,
            fee_beneficiaries_bound: c.fee_beneficiaries_bound,
            transfer_fee_authority_revoked: c.transfer_fee_authority_revoked,
            withdraw_withheld_bump: c.withdraw_withheld_bump,
            total_fees_withdrawn: c.total_fees_withdrawn,
            total_fees_to_creator: c.total_fees_to_creator,
            total_fees_to_realms: c.total_fees_to_realms,
            creator_escrow_bump: c.creator_escrow_bump,
            last_creator_claim_ts: c.last_creator_claim_ts,
            creator_decay_secs: c.creator_decay_secs,
            total_creator_claimed: c.total_creator_claimed,
            total_creator_swept: c.total_creator_swept,
        }
    }
}

pub fn require_authority(config: &ConfigView, signer: &Pubkey) -> CheckResult<()> {
    if config.authority != *signer {
        return Err(Check::Unauthorized);
    }
    Ok(())
}

/// Whether `initialize` leaves the program paused. Always true.
///
/// `convert` is gated on the pump.fun curve alone (`require_graduated`), so it
/// becomes callable by anyone the instant the curve completes. PLAN §9 creates
/// and permanently locks the ICHOR/SOL pool at step 10, six steps after the
/// step-7 minimum convert - a window in which a holder could irreversibly burn
/// KEKBULL for ICHOR that has no market to sell into. `Config` carries no pool
/// field, so the program cannot verify a market even in principle.
///
/// Shipping paused makes the safe state the default and the market the thing
/// that has to be proven. Deliberately takes no argument: the safe state is not
/// a caller's choice. Unpause is a separate authority-signed `set_paused(false)`
/// once the pool exists and its lock is verified from chain.
pub fn paused_at_initialize() -> bool {
    true
}

pub fn require_not_paused(config: &ConfigView) -> CheckResult<()> {
    if config.paused {
        return Err(Check::Paused);
    }
    Ok(())
}

pub fn add_totals(
    burned: u64,
    minted: u64,
    add_burned: u64,
    add_minted: u64,
) -> CheckResult<(u64, u64)> {
    let burned = burned
        .checked_add(add_burned)
        .ok_or(Check::ArithmeticOverflow)?;
    let minted = minted
        .checked_add(add_minted)
        .ok_or(Check::ArithmeticOverflow)?;
    Ok((burned, minted))
}

pub fn add_fee_totals(
    withdrawn: u64,
    to_creator: u64,
    to_realms: u64,
    add_withdrawn: u64,
    add_creator: u64,
    add_realms: u64,
) -> CheckResult<(u64, u64, u64)> {
    let withdrawn = withdrawn
        .checked_add(add_withdrawn)
        .ok_or(Check::ArithmeticOverflow)?;
    let to_creator = to_creator
        .checked_add(add_creator)
        .ok_or(Check::ArithmeticOverflow)?;
    let to_realms = to_realms
        .checked_add(add_realms)
        .ok_or(Check::ArithmeticOverflow)?;
    Ok((withdrawn, to_creator, to_realms))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view() -> ConfigView {
        ConfigView {
            authority: Pubkey::new_from_array([1u8; 32]),
            pending_authority: None,
            kekbull_mint: Pubkey::new_from_array([2u8; 32]),
            ichor_mint: Pubkey::new_from_array([3u8; 32]),
            kekbull_token_program: crate::constants::TOKEN_2022_PROGRAM_ID,
            ichor_token_program: crate::constants::TOKEN_2022_PROGRAM_ID,
            pump_program: crate::constants::PUMP_PROGRAM,
            bonding_curve: Pubkey::new_from_array([4u8; 32]),
            emission_numerator: 1,
            emission_denominator: 1,
            pending_emission_numerator: 0,
            pending_emission_denominator: 0,
            pending_ratio_unlock_ts: 0,
            ratio_timelock_secs: 86_400,
            paused: false,
            ratio_updates_frozen: false,
            has_pending_ratio: false,
            total_kekbull_burned: 0,
            total_ichor_minted: 0,
            creator_beneficiary: None,
            realms_program: None,
            realms_realm: None,
            realms_governance: None,
            realms_native_treasury: None,
            fee_beneficiaries_bound: false,
            transfer_fee_authority_revoked: false,
            withdraw_withheld_bump: 0,
            total_fees_withdrawn: 0,
            total_fees_to_creator: 0,
            total_fees_to_realms: 0,
            creator_escrow_bump: 0,
            last_creator_claim_ts: 1,
            creator_decay_secs: 86_400,
            total_creator_claimed: 0,
            total_creator_swept: 0,
        }
    }

    #[test]
    fn space_includes_discriminator() {
        assert!(Config::space() > Config::INIT_SPACE);
        assert_eq!(Config::space() - Config::INIT_SPACE, 8);
    }

    #[test]
    fn authority_and_pause_gates() {
        let mut c = view();
        assert_eq!(require_authority(&c, &c.authority), Ok(()));
        assert_eq!(
            require_authority(&c, &Pubkey::new_from_array([9u8; 32])),
            Err(Check::Unauthorized)
        );
        assert_eq!(require_not_paused(&c), Ok(()));
        c.paused = true;
        assert_eq!(require_not_paused(&c), Err(Check::Paused));
    }

    #[test]
    fn initialize_ships_paused() {
        assert!(
            paused_at_initialize(),
            "convert must not be reachable before the ICHOR pool exists and is locked"
        );
        // A config in the state `initialize` writes refuses convert.
        let mut c = view();
        c.paused = paused_at_initialize();
        assert_eq!(require_not_paused(&c), Err(Check::Paused));
    }

    #[test]
    fn totals_use_checked_add() {
        assert_eq!(add_totals(10, 20, 3, 4), Ok((13, 24)));
        assert_eq!(
            add_totals(u64::MAX, 0, 1, 0),
            Err(Check::ArithmeticOverflow)
        );
        assert_eq!(
            add_totals(0, u64::MAX, 0, 1),
            Err(Check::ArithmeticOverflow)
        );
    }

    #[test]
    fn fee_totals_use_checked_add() {
        assert_eq!(add_fee_totals(1, 2, 3, 4, 5, 6), Ok((5, 7, 9)));
        assert_eq!(
            add_fee_totals(u64::MAX, 0, 0, 1, 0, 0),
            Err(Check::ArithmeticOverflow)
        );
        assert_eq!(
            add_fee_totals(0, u64::MAX, 0, 0, 1, 0),
            Err(Check::ArithmeticOverflow)
        );
        assert_eq!(
            add_fee_totals(0, 0, u64::MAX, 0, 0, 1),
            Err(Check::ArithmeticOverflow)
        );
    }
}
