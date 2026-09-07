//! Composed account checks. Each helper returns every applicable [`Check`]
//! so tests can assert the full violation set; instructions fail on the first.

use anchor_lang::prelude::*;

use crate::constants::{
    bonding_curve_pda, creator_escrow_pda, is_commitment_realms_program, native_treasury_pda,
    withdraw_withheld_pda, PUMP_PROGRAM, REALM_ACCOUNT_TYPE_V1, REALM_ACCOUNT_TYPE_V2,
    TOKEN_2022_PROGRAM_ID,
};
use crate::curve::{parse_bonding_curve_core, require_canonical_curve, require_curve_for_convert};
use crate::error::{Check, CheckResult};
use crate::extensions::{
    parse_ichor_transfer_fee_config, require_ichor_transfer_fee_config,
    require_operational_transfer_fee_config, require_pilot_transfer_fee,
};
use crate::math::{
    creator_decay_elapsed, ichor_from_kekbull, pending_unlock_ts, ratio_unlock_elapsed,
    require_claim_clock, require_min_ichor, require_ratio,
    split_harvested_fees,
};
use crate::mint_layout::{
    parse_mint, parse_token_account, require_ichor_authorities, require_initialized_mint,
    require_token_account, MintView,
};
use crate::state::{add_fee_totals, add_totals, require_not_paused, ConfigView};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConvertOk {
    pub kekbull_decimals: u8,
    pub ichor_decimals: u8,
    pub kekbull_amount: u64,
    pub ichor_amount: u64,
    pub min_ichor_amount: u64,
    pub recipient: Pubkey,
}

pub fn collect(mut into: Vec<Check>, result: CheckResult<()>) -> Vec<Check> {
    if let Err(e) = result {
        into.push(e);
    }
    into
}

pub fn require_token_program(actual: &Pubkey, expected: &Pubkey) -> CheckResult<()> {
    if actual != expected {
        return Err(Check::InvalidTokenProgram);
    }
    Ok(())
}

pub fn require_mint_owner(owner: &Pubkey, expected_program: &Pubkey) -> CheckResult<()> {
    if owner != expected_program {
        return Err(Check::InvalidMintOwner);
    }
    Ok(())
}

pub fn require_configured_mints(
    config: &ConfigView,
    kekbull_mint: &Pubkey,
    ichor_mint: &Pubkey,
) -> CheckResult<()> {
    if *kekbull_mint != config.kekbull_mint {
        return Err(Check::InvalidKekbullMint);
    }
    if *ichor_mint != config.ichor_mint {
        return Err(Check::InvalidIchorMint);
    }
    Ok(())
}

pub fn require_distinct_mints(kekbull: &Pubkey, ichor: &Pubkey) -> CheckResult<()> {
    if kekbull == ichor {
        return Err(Check::MintsMustDiffer);
    }
    Ok(())
}

/// Initialize-time checks against raw account bytes and owners.
pub fn check_initialize(
    kekbull_mint: &Pubkey,
    kekbull_owner: &Pubkey,
    kekbull_data: &[u8],
    ichor_mint: &Pubkey,
    ichor_owner: &Pubkey,
    ichor_data: &[u8],
    config_pda: &Pubkey,
    withdraw_withheld_pda: &Pubkey,
    curve_key: &Pubkey,
    curve_owner: &Pubkey,
    curve_data: &[u8],
    numerator: u64,
    denominator: u64,
) -> CheckResult<(MintView, MintView)> {
    require_distinct_mints(kekbull_mint, ichor_mint)?;
    require_ratio(numerator, denominator)?;
    if *kekbull_owner != TOKEN_2022_PROGRAM_ID {
        return Err(Check::KekbullNotToken2022);
    }
    if *ichor_owner != TOKEN_2022_PROGRAM_ID {
        return Err(Check::IchorNotToken2022);
    }
    let kek = parse_mint(kekbull_data)?;
    require_initialized_mint(&kek)?;
    let ichor = parse_mint(ichor_data)?;
    require_ichor_authorities(&ichor, config_pda)?;
    if ichor.supply != 0 {
        return Err(Check::IchorSupplyMustBeZero);
    }
    let fee = parse_ichor_transfer_fee_config(ichor_data)?;
    require_ichor_transfer_fee_config(&fee, config_pda, withdraw_withheld_pda)?;
    require_canonical_curve(kekbull_mint, curve_key, curve_owner, None)?;
    // Layout/discriminator only - graduation is a convert-time gate.
    // Owner is already required to be PUMP_PROGRAM by require_canonical_curve.
    let _ = parse_bonding_curve_core(curve_data)?;
    Ok((kek, ichor))
}

pub fn check_propose_ratio(
    config: &ConfigView,
    signer: &Pubkey,
    numerator: u64,
    denominator: u64,
    now: i64,
) -> CheckResult<i64> {
    crate::state::require_authority(config, signer)?;
    if config.ratio_updates_frozen {
        return Err(Check::RatioUpdatesFrozen);
    }
    require_ratio(numerator, denominator)?;
    pending_unlock_ts(now, config.ratio_timelock_secs)
}

pub fn check_apply_ratio(config: &ConfigView, now: i64) -> CheckResult<(u64, u64)> {
    if config.ratio_updates_frozen {
        return Err(Check::RatioUpdatesFrozen);
    }
    if !config.has_pending_ratio {
        return Err(Check::NoPendingRatio);
    }
    if !ratio_unlock_elapsed(now, config.pending_ratio_unlock_ts) {
        return Err(Check::RatioTimelockNotElapsed);
    }
    require_ratio(
        config.pending_emission_numerator,
        config.pending_emission_denominator,
    )?;
    Ok((
        config.pending_emission_numerator,
        config.pending_emission_denominator,
    ))
}

pub fn check_increase_timelock(
    config: &ConfigView,
    signer: &Pubkey,
    new_secs: u64,
) -> CheckResult<()> {
    crate::state::require_authority(config, signer)?;
    if config.ratio_timelock_secs == 0 {
        return Err(Check::RatioLocked);
    }
    if new_secs < config.ratio_timelock_secs {
        return Err(Check::TimelockCannotDecrease);
    }
    Ok(())
}

pub fn check_accept_authority(config: &ConfigView, signer: &Pubkey) -> CheckResult<()> {
    match config.pending_authority {
        None => Err(Check::NoPendingAuthority),
        Some(pending) if pending == *signer => Ok(()),
        Some(_) => Err(Check::PendingAuthorityMismatch),
    }
}

pub fn check_set_pending_authority(pending: Option<Pubkey>) -> CheckResult<()> {
    if let Some(key) = pending {
        if key == Pubkey::default() {
            return Err(Check::InvalidPendingAuthority);
        }
    }
    Ok(())
}

pub fn check_set_transfer_fee(
    config: &ConfigView,
    signer: &Pubkey,
    basis_points: u16,
    maximum_fee: u64,
) -> CheckResult<()> {
    crate::state::require_authority(config, signer)?;
    if config.transfer_fee_authority_revoked {
        return Err(Check::FeeAuthorityRevoked);
    }
    require_pilot_transfer_fee(basis_points, maximum_fee)
}

pub fn check_revoke_transfer_fee_authority(
    config: &ConfigView,
    signer: &Pubkey,
) -> CheckResult<()> {
    crate::state::require_authority(config, signer)?;
    if config.transfer_fee_authority_revoked {
        return Err(Check::FeeAuthorityRevoked);
    }
    Ok(())
}

/// GovER5 Realms proof: realm community mint = ICHOR, Governance PDA, and
/// native treasury. GTesT is a published test instance, not a commitment
/// target. Does not write Config.
pub fn check_realms_destination_proof(
    ichor_mint: &Pubkey,
    realms_program: &Pubkey,
    realm_key: &Pubkey,
    realm_owner: &Pubkey,
    realm_data: &[u8],
    governance_key: &Pubkey,
    governance_owner: &Pubkey,
    governance_data: &[u8],
    treasury: &Pubkey,
) -> CheckResult<()> {
    if *realms_program == Pubkey::default()
        || *governance_key == Pubkey::default()
        || *realm_key == Pubkey::default()
    {
        return Err(Check::InvalidRealmsGovernance);
    }
    if !is_commitment_realms_program(realms_program) {
        return Err(Check::InvalidRealmsGovernance);
    }
    if governance_owner != realms_program {
        return Err(Check::InvalidRealmsGovernance);
    }
    if governance_data.len() < 65 || !(18..=21).contains(&governance_data[0]) {
        return Err(Check::InvalidRealmsGovernance);
    }
    let realm = Pubkey::try_from(&governance_data[1..33])
        .map_err(|_| Check::InvalidRealmsGovernance)?;
    let governed_account = Pubkey::try_from(&governance_data[33..65])
        .map_err(|_| Check::InvalidRealmsGovernance)?;
    if realm == Pubkey::default() || governed_account == Pubkey::default() {
        return Err(Check::InvalidRealmsGovernance);
    }
    if realm != *realm_key {
        return Err(Check::InvalidRealmsGovernance);
    }
    if realm_owner != realms_program {
        return Err(Check::InvalidRealmsGovernance);
    }
    if realm_data.len() < 33
        || (realm_data[0] != REALM_ACCOUNT_TYPE_V1 && realm_data[0] != REALM_ACCOUNT_TYPE_V2)
    {
        return Err(Check::InvalidRealmsGovernance);
    }
    let realm_community_mint = Pubkey::try_from(&realm_data[1..33])
        .map_err(|_| Check::InvalidRealmsGovernance)?;
    if realm_community_mint != *ichor_mint {
        return Err(Check::InvalidRealmsGovernance);
    }
    let prefix: &[u8] = match governance_data[0] {
        18 => b"account-governance",
        19 => b"program-governance",
        20 => b"mint-governance",
        21 => b"token-governance",
        _ => return Err(Check::InvalidRealmsGovernance),
    };
    let (expected_governance, _) = Pubkey::find_program_address(
        &[prefix, realm.as_ref(), governed_account.as_ref()],
        realms_program,
    );
    if expected_governance != *governance_key {
        return Err(Check::InvalidRealmsGovernance);
    }
    let (expected, _) = native_treasury_pda(realms_program, governance_key);
    if *treasury != expected || *treasury == Pubkey::default() {
        return Err(Check::InvalidRealmsTreasury);
    }
    Ok(())
}

/// (realm, program, governance, treasury) once all four Options are Some.
pub fn committed_dao_destination(
    config: &ConfigView,
) -> CheckResult<(Pubkey, Pubkey, Pubkey, Pubkey)> {
    let realm = config.realms_realm.ok_or(Check::DaoDestinationUncommitted)?;
    let program = config
        .realms_program
        .ok_or(Check::DaoDestinationUncommitted)?;
    let governance = config
        .realms_governance
        .ok_or(Check::DaoDestinationUncommitted)?;
    let treasury = config
        .realms_native_treasury
        .ok_or(Check::DaoDestinationUncommitted)?;
    if realm == Pubkey::default()
        || program == Pubkey::default()
        || governance == Pubkey::default()
        || treasury == Pubkey::default()
    {
        return Err(Check::DaoDestinationUncommitted);
    }
    if !is_commitment_realms_program(&program) {
        return Err(Check::InvalidRealmsGovernance);
    }
    Ok((realm, program, governance, treasury))
}

pub fn require_accounts_match_committed_dao_destination(
    config: &ConfigView,
    realms_program: &Pubkey,
    realm_key: &Pubkey,
    governance_key: &Pubkey,
    treasury: &Pubkey,
) -> CheckResult<(Pubkey, Pubkey, Pubkey, Pubkey)> {
    let committed = committed_dao_destination(config)?;
    if *realm_key != committed.0
        || *realms_program != committed.1
        || *governance_key != committed.2
        || *treasury != committed.3
    {
        return Err(Check::DaoDestinationMismatch);
    }
    Ok(committed)
}

pub fn check_commit_dao_destination(
    config: &ConfigView,
    signer: &Pubkey,
    realms_program: &Pubkey,
    realm_key: &Pubkey,
    realm_owner: &Pubkey,
    realm_data: &[u8],
    governance_key: &Pubkey,
    governance_owner: &Pubkey,
    governance_data: &[u8],
    treasury: &Pubkey,
) -> CheckResult<()> {
    crate::state::require_authority(config, signer)?;
    if config.fee_beneficiaries_bound {
        return Err(Check::FeeBeneficiariesAlreadyBound);
    }
    if config.realms_program.is_some()
        || config.realms_realm.is_some()
        || config.realms_governance.is_some()
        || config.realms_native_treasury.is_some()
    {
        return Err(Check::DaoDestinationAlreadyCommitted);
    }
    if !is_commitment_realms_program(realms_program) {
        return Err(Check::InvalidRealmsGovernance);
    }
    check_realms_destination_proof(
        &config.ichor_mint,
        realms_program,
        realm_key,
        realm_owner,
        realm_data,
        governance_key,
        governance_owner,
        governance_data,
        treasury,
    )
}

pub fn check_set_fee_distribution(
    config: &ConfigView,
    signer: &Pubkey,
    creator: &Pubkey,
    realms_program: &Pubkey,
    realm_key: &Pubkey,
    realm_owner: &Pubkey,
    realm_data: &[u8],
    governance_key: &Pubkey,
    governance_owner: &Pubkey,
    governance_data: &[u8],
    treasury: &Pubkey,
) -> CheckResult<()> {
    crate::state::require_authority(config, signer)?;
    if config.fee_beneficiaries_bound {
        return Err(Check::FeeBeneficiariesAlreadyBound);
    }
    require_accounts_match_committed_dao_destination(
        config,
        realms_program,
        realm_key,
        governance_key,
        treasury,
    )?;
    let (withdraw_authority, _) = withdraw_withheld_pda(&crate::ID);
    if *creator == Pubkey::default()
        || *creator == config.ichor_mint
        || *creator == withdraw_authority
        || *creator == *realms_program
        || *creator == *governance_key
        || *creator == *treasury
        || *creator == *realm_key
    {
        return Err(Check::InvalidFeeBeneficiary);
    }
    check_realms_destination_proof(
        &config.ichor_mint,
        realms_program,
        realm_key,
        realm_owner,
        realm_data,
        governance_key,
        governance_owner,
        governance_data,
        treasury,
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DistributeOk {
    pub withheld_before: u64,
    pub vault_before: u64,
    pub creator_before: u64,
    pub realms_before: u64,
    pub ichor_decimals: u8,
}

pub fn bound_fee_destinations(config: &ConfigView) -> CheckResult<(Pubkey, Pubkey, Pubkey)> {
    if !config.fee_beneficiaries_bound {
        return Err(Check::FeeBeneficiariesUnbound);
    }
    let creator = config
        .creator_beneficiary
        .ok_or(Check::FeeBeneficiariesUnbound)?;
    let treasury = config
        .realms_native_treasury
        .ok_or(Check::FeeBeneficiariesUnbound)?;
    let realms_program = config
        .realms_program
        .ok_or(Check::FeeBeneficiariesUnbound)?;
    Ok((creator, treasury, realms_program))
}

pub fn check_distribute(
    config: &ConfigView,
    ichor_mint: &Pubkey,
    ichor_mint_owner: &Pubkey,
    ichor_mint_data: &[u8],
    withdraw_authority: &Pubkey,
    expected_withdraw_authority: &Pubkey,
    vault_owner_program: &Pubkey,
    vault_data: &[u8],
    creator_dest_owner_program: &Pubkey,
    creator_dest_data: &[u8],
    realms_dest_owner_program: &Pubkey,
    realms_dest_data: &[u8],
    token_2022_program: &Pubkey,
) -> CheckResult<DistributeOk> {
    let (creator, treasury, _) = bound_fee_destinations(config)?;
    if *ichor_mint != config.ichor_mint {
        return Err(Check::InvalidIchorMint);
    }
    require_token_program(token_2022_program, &TOKEN_2022_PROGRAM_ID)?;
    require_token_program(token_2022_program, &config.ichor_token_program)?;
    require_mint_owner(ichor_mint_owner, &TOKEN_2022_PROGRAM_ID)?;
    if *withdraw_authority != *expected_withdraw_authority {
        return Err(Check::WithdrawWithheldAuthorityMismatch);
    }
    if *vault_owner_program != TOKEN_2022_PROGRAM_ID
        || *creator_dest_owner_program != TOKEN_2022_PROGRAM_ID
        || *realms_dest_owner_program != TOKEN_2022_PROGRAM_ID
    {
        return Err(Check::InvalidTokenAccountOwner);
    }

    let mint = parse_mint(ichor_mint_data)?;
    require_initialized_mint(&mint)?;
    let fee = parse_ichor_transfer_fee_config(ichor_mint_data)?;
    let (fee_authority, _) = crate::constants::config_pda(&crate::ID);
    require_operational_transfer_fee_config(
        &fee,
        &fee_authority,
        expected_withdraw_authority,
        config.transfer_fee_authority_revoked,
    )?;

    let vault = parse_token_account(vault_data)?;
    require_token_account(&vault, ichor_mint, Some(expected_withdraw_authority))?;
    let creator_dest = parse_token_account(creator_dest_data)?;
    require_token_account(&creator_dest, ichor_mint, Some(&creator))?;
    let realms_dest = parse_token_account(realms_dest_data)?;
    require_token_account(&realms_dest, ichor_mint, Some(&treasury))?;

    let withheld_before = fee.withheld_amount;
    let to_distribute = vault
        .amount
        .checked_add(withheld_before)
        .ok_or(Check::ArithmeticOverflow)?;
    let (creator_amount, realms_amount) = split_harvested_fees(to_distribute)?;
    add_fee_totals(
        config.total_fees_withdrawn,
        config.total_fees_to_creator,
        config.total_fees_to_realms,
        withheld_before,
        creator_amount,
        realms_amount,
    )?;

    Ok(DistributeOk {
        withheld_before,
        vault_before: vault.amount,
        creator_before: creator_dest.amount,
        realms_before: realms_dest.amount,
        ichor_decimals: mint.decimals,
    })
}


pub fn check_claim_creator_fees(
    config: &ConfigView,
    signer: &Pubkey,
    now: i64,
    ichor_mint: &Pubkey,
    escrow_ata_owner_program: &Pubkey,
    escrow_ata_data: &[u8],
    creator_ata_owner_program: &Pubkey,
    creator_ata_data: &[u8],
    token_2022_program: &Pubkey,
) -> CheckResult<u64> {
    let (creator, _treasury, _) = bound_fee_destinations(config)?;
    if *signer != creator {
        return Err(Check::UnauthorizedCreatorClaim);
    }
    require_claim_clock(now, config.last_creator_claim_ts)?;
    require_token_program(token_2022_program, &TOKEN_2022_PROGRAM_ID)?;
    if *ichor_mint != config.ichor_mint {
        return Err(Check::InvalidIchorMint);
    }
    if *escrow_ata_owner_program != TOKEN_2022_PROGRAM_ID
        || *creator_ata_owner_program != TOKEN_2022_PROGRAM_ID
    {
        return Err(Check::InvalidTokenAccountOwner);
    }
    let (escrow_pda, escrow_bump) = creator_escrow_pda(&crate::ID);
    if escrow_bump != config.creator_escrow_bump {
        return Err(Check::FeeDestinationMismatch);
    }
    let escrow_ata = parse_token_account(escrow_ata_data)?;
    require_token_account(&escrow_ata, ichor_mint, Some(&escrow_pda))?;
    let creator_ata = parse_token_account(creator_ata_data)?;
    require_token_account(&creator_ata, ichor_mint, Some(&creator))?;
    if escrow_ata.amount == 0 {
        return Err(Check::ZeroAmount);
    }
    Ok(escrow_ata.amount)
}

pub fn check_sweep_unclaimed_creator_fees(
    config: &ConfigView,
    now: i64,
    ichor_mint: &Pubkey,
    escrow_ata_owner_program: &Pubkey,
    escrow_ata_data: &[u8],
    realms_ata_owner_program: &Pubkey,
    realms_ata_data: &[u8],
    token_2022_program: &Pubkey,
) -> CheckResult<u64> {
    let (_creator, treasury, _) = bound_fee_destinations(config)?;
    if !creator_decay_elapsed(now, config.last_creator_claim_ts, config.creator_decay_secs)? {
        return Err(Check::CreatorDecayNotElapsed);
    }
    require_token_program(token_2022_program, &TOKEN_2022_PROGRAM_ID)?;
    if *ichor_mint != config.ichor_mint {
        return Err(Check::InvalidIchorMint);
    }
    if *escrow_ata_owner_program != TOKEN_2022_PROGRAM_ID
        || *realms_ata_owner_program != TOKEN_2022_PROGRAM_ID
    {
        return Err(Check::InvalidTokenAccountOwner);
    }
    let (escrow_pda, escrow_bump) = creator_escrow_pda(&crate::ID);
    if escrow_bump != config.creator_escrow_bump {
        return Err(Check::FeeDestinationMismatch);
    }
    let escrow_ata = parse_token_account(escrow_ata_data)?;
    require_token_account(&escrow_ata, ichor_mint, Some(&escrow_pda))?;
    let realms_ata = parse_token_account(realms_ata_data)?;
    require_token_account(&realms_ata, ichor_mint, Some(&treasury))?;
    if escrow_ata.amount == 0 {
        return Err(Check::ZeroAmount);
    }
    Ok(escrow_ata.amount)
}

pub fn check_set_creator_beneficiary(
    config: &ConfigView,
    signer: &Pubkey,
    new_beneficiary: &Pubkey,
) -> CheckResult<()> {
    let (current, _treasury, _) = bound_fee_destinations(config)?;
    if *signer != current {
        return Err(Check::Unauthorized);
    }
    if *new_beneficiary == Pubkey::default() {
        return Err(Check::InvalidFeeBeneficiary);
    }
    Ok(())
}

pub fn check_convert(
    config: &ConfigView,
    burner: &Pubkey,
    kekbull_mint: &Pubkey,
    kekbull_mint_owner: &Pubkey,
    kekbull_mint_data: &[u8],
    ichor_mint: &Pubkey,
    ichor_mint_owner: &Pubkey,
    ichor_mint_data: &[u8],
    kekbull_from_owner_program: &Pubkey,
    kekbull_from_data: &[u8],
    ichor_to_owner_program: &Pubkey,
    ichor_to_data: &[u8],
    curve_key: &Pubkey,
    curve_owner: &Pubkey,
    curve_data: &[u8],
    token_2022_program: &Pubkey,
    token_program: &Pubkey,
    kekbull_amount: u64,
    min_ichor_amount: u64,
) -> CheckResult<ConvertOk> {
    require_not_paused(config)?;
    require_configured_mints(config, kekbull_mint, ichor_mint)?;
    require_token_program(token_2022_program, &TOKEN_2022_PROGRAM_ID)?;
    require_token_program(token_program, &TOKEN_2022_PROGRAM_ID)?;
    require_token_program(token_2022_program, &config.kekbull_token_program)?;
    require_token_program(token_program, &config.ichor_token_program)?;
    require_mint_owner(kekbull_mint_owner, &TOKEN_2022_PROGRAM_ID)?;
    require_mint_owner(ichor_mint_owner, &TOKEN_2022_PROGRAM_ID)?;
    if *kekbull_from_owner_program != TOKEN_2022_PROGRAM_ID {
        return Err(Check::InvalidTokenAccountOwner);
    }
    if *ichor_to_owner_program != TOKEN_2022_PROGRAM_ID {
        return Err(Check::InvalidTokenAccountOwner);
    }

    let kek_mint = parse_mint(kekbull_mint_data)?;
    require_initialized_mint(&kek_mint)?;
    let ichor_mint_view = parse_mint(ichor_mint_data)?;
    require_initialized_mint(&ichor_mint_view)?;
    let fee = parse_ichor_transfer_fee_config(ichor_mint_data)?;
    let (fee_authority, _) = crate::constants::config_pda(&crate::ID);
    let (withdraw_authority, _) = crate::constants::withdraw_withheld_pda(&crate::ID);
    require_operational_transfer_fee_config(
        &fee,
        &fee_authority,
        &withdraw_authority,
        config.transfer_fee_authority_revoked,
    )?;

    let from = parse_token_account(kekbull_from_data)?;
    require_token_account(&from, kekbull_mint, Some(burner))?;
    let to = parse_token_account(ichor_to_data)?;
    require_token_account(&to, ichor_mint, None)?;

    require_curve_for_convert(
        kekbull_mint,
        curve_key,
        curve_owner,
        &config.bonding_curve,
        curve_data,
    )?;

    let ichor_amount = ichor_from_kekbull(
        kekbull_amount,
        kek_mint.decimals,
        ichor_mint_view.decimals,
        config.emission_numerator,
        config.emission_denominator,
    )?;
    require_min_ichor(ichor_amount, min_ichor_amount)?;
    add_totals(
        config.total_kekbull_burned,
        config.total_ichor_minted,
        kekbull_amount,
        ichor_amount,
    )?;

    Ok(ConvertOk {
        kekbull_decimals: kek_mint.decimals,
        ichor_decimals: ichor_mint_view.decimals,
        kekbull_amount,
        ichor_amount,
        min_ichor_amount,
        recipient: to.owner,
    })
}

/// All convert violations, not only the first. Used by unit tests.
pub fn collect_convert_violations(
    config: &ConfigView,
    burner: &Pubkey,
    kekbull_mint: &Pubkey,
    kekbull_mint_owner: &Pubkey,
    kekbull_mint_data: &[u8],
    ichor_mint: &Pubkey,
    ichor_mint_owner: &Pubkey,
    ichor_mint_data: &[u8],
    kekbull_from_owner_program: &Pubkey,
    kekbull_from_data: &[u8],
    ichor_to_owner_program: &Pubkey,
    ichor_to_data: &[u8],
    curve_key: &Pubkey,
    curve_owner: &Pubkey,
    curve_data: &[u8],
    token_2022_program: &Pubkey,
    token_program: &Pubkey,
    kekbull_amount: u64,
    min_ichor_amount: u64,
) -> Vec<Check> {
    let mut v = Vec::new();
    v = collect(v, require_not_paused(config));
    v = collect(
        v,
        require_configured_mints(config, kekbull_mint, ichor_mint),
    );
    v = collect(
        v,
        require_token_program(token_2022_program, &TOKEN_2022_PROGRAM_ID),
    );
    v = collect(
        v,
        require_token_program(token_program, &TOKEN_2022_PROGRAM_ID),
    );
    v = collect(
        v,
        require_mint_owner(kekbull_mint_owner, &TOKEN_2022_PROGRAM_ID),
    );
    v = collect(
        v,
        require_mint_owner(ichor_mint_owner, &TOKEN_2022_PROGRAM_ID),
    );
    if *kekbull_from_owner_program != TOKEN_2022_PROGRAM_ID {
        v.push(Check::InvalidTokenAccountOwner);
    }
    if *ichor_to_owner_program != TOKEN_2022_PROGRAM_ID {
        v.push(Check::InvalidTokenAccountOwner);
    }
    if let Ok(m) = parse_mint(kekbull_mint_data) {
        v = collect(v, require_initialized_mint(&m));
    } else if let Err(e) = parse_mint(kekbull_mint_data) {
        v.push(e);
    }
    if let Ok(m) = parse_mint(ichor_mint_data) {
        v = collect(v, require_initialized_mint(&m));
    } else if let Err(e) = parse_mint(ichor_mint_data) {
        v.push(e);
    }
    match parse_token_account(kekbull_from_data) {
        Ok(from) => v = collect(v, require_token_account(&from, kekbull_mint, Some(burner))),
        Err(e) => v.push(e),
    }
    match parse_token_account(ichor_to_data) {
        Ok(to) => v = collect(v, require_token_account(&to, ichor_mint, None)),
        Err(e) => v.push(e),
    }
    let canonical_curve = bonding_curve_pda(kekbull_mint).0;
    if *curve_key != canonical_curve || *curve_key != config.bonding_curve {
        v.push(Check::InvalidBondingCurveAddress);
    }
    if *curve_owner != PUMP_PROGRAM {
        v.push(Check::InvalidBondingCurveOwner);
    }
    match parse_bonding_curve_core(curve_data) {
        Ok(core) => {
            if !core.complete {
                v.push(Check::CurveNotComplete);
            }
            if core.real_token_reserves != 0 {
                v.push(Check::CurveStillHasReserves);
            }
        }
        Err(e) => v.push(e),
    }
    if let (Ok(kek), Ok(ichor)) = (parse_mint(kekbull_mint_data), parse_mint(ichor_mint_data)) {
        match ichor_from_kekbull(
            kekbull_amount,
            kek.decimals,
            ichor.decimals,
            config.emission_numerator,
            config.emission_denominator,
        ) {
            Ok(ichor_amount) => {
                v = collect(v, require_min_ichor(ichor_amount, min_ichor_amount));
                v = collect(
                    v,
                    add_totals(
                        config.total_kekbull_burned,
                        config.total_ichor_minted,
                        kekbull_amount,
                        ichor_amount,
                    )
                    .map(|_| ()),
                );
            }
            Err(e) => v.push(e),
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::require_creator_decay_secs;
    use crate::constants::{
        bonding_curve_pda, creator_escrow_pda, native_treasury_pda, withdraw_withheld_pda, BONDING_CURVE_CORE_LEN,
        BONDING_CURVE_DISCRIMINATOR, COPTION_NONE, COPTION_SOME, CURVE_OFF_COMPLETE,
        CURVE_OFF_REAL_TOKEN_RESERVES, EXTENSION_TYPE_TRANSFER_FEE_CONFIG,
        EXTENSION_TYPE_TRANSFER_HOOK, MINT_BASE_LEN, MINT_OFF_AUTHORITY, MINT_OFF_DECIMALS,
        MINT_OFF_FREEZE_AUTHORITY, MINT_OFF_IS_INITIALIZED, MINT_OFF_SUPPLY, PUMP_PROGRAM,
        REALM_ACCOUNT_TYPE_V2, REALMS_PROGRAM, TOKEN_2022_ACCOUNT_TYPE_MINT,
        TOKEN_2022_ACCOUNT_TYPE_OFFSET, TOKEN_2022_PROGRAM_ID, TOKEN_ACCOUNT_BASE_LEN,
        TOKEN_ACCOUNT_OFF_STATE, TOKEN_ACCOUNT_STATE_INITIALIZED, TOKEN_PROGRAM_ID,
        TRANSFER_FEE_BASIS_POINTS, TRANSFER_FEE_CONFIG_LEN, TRANSFER_FEE_CONFIG_OFF_AUTHORITY,
        TRANSFER_FEE_CONFIG_OFF_NEWER, TRANSFER_FEE_CONFIG_OFF_OLDER,
        TRANSFER_FEE_CONFIG_OFF_WITHDRAW, TRANSFER_FEE_MAXIMUM_FEE, TRANSFER_FEE_OFF_BASIS_POINTS,
        TRANSFER_FEE_OFF_MAXIMUM_FEE,
    };

    fn pk(b: u8) -> Pubkey {
        Pubkey::new_from_array([b; 32])
    }

    fn mint(authority: Option<Pubkey>, decimals: u8) -> Vec<u8> {
        let mut d = vec![0u8; MINT_BASE_LEN];
        let tag = if authority.is_some() {
            COPTION_SOME
        } else {
            COPTION_NONE
        };
        d[MINT_OFF_AUTHORITY..MINT_OFF_AUTHORITY + 4].copy_from_slice(&tag.to_le_bytes());
        if let Some(a) = authority {
            d[MINT_OFF_AUTHORITY + 4..MINT_OFF_AUTHORITY + 36].copy_from_slice(a.as_ref());
        }
        d[MINT_OFF_DECIMALS] = decimals;
        d[MINT_OFF_IS_INITIALIZED] = 1;
        d[MINT_OFF_FREEZE_AUTHORITY..MINT_OFF_FREEZE_AUTHORITY + 4]
            .copy_from_slice(&COPTION_NONE.to_le_bytes());
        d
    }

    fn token2022_ichor(authority: Pubkey, withdraw: Pubkey, decimals: u8) -> Vec<u8> {
        let mut d = vec![0u8; TOKEN_2022_ACCOUNT_TYPE_OFFSET + 1];
        d[MINT_OFF_AUTHORITY..MINT_OFF_AUTHORITY + 4].copy_from_slice(&COPTION_SOME.to_le_bytes());
        d[MINT_OFF_AUTHORITY + 4..MINT_OFF_AUTHORITY + 36].copy_from_slice(authority.as_ref());
        d[MINT_OFF_DECIMALS] = decimals;
        d[MINT_OFF_IS_INITIALIZED] = 1;
        d[MINT_OFF_FREEZE_AUTHORITY..MINT_OFF_FREEZE_AUTHORITY + 4]
            .copy_from_slice(&COPTION_NONE.to_le_bytes());
        d[TOKEN_2022_ACCOUNT_TYPE_OFFSET] = TOKEN_2022_ACCOUNT_TYPE_MINT;
        let mut fee = vec![0u8; TRANSFER_FEE_CONFIG_LEN];
        fee[TRANSFER_FEE_CONFIG_OFF_AUTHORITY..TRANSFER_FEE_CONFIG_OFF_AUTHORITY + 32]
            .copy_from_slice(authority.as_ref());
        fee[TRANSFER_FEE_CONFIG_OFF_WITHDRAW..TRANSFER_FEE_CONFIG_OFF_WITHDRAW + 32]
            .copy_from_slice(withdraw.as_ref());
        fee[TRANSFER_FEE_CONFIG_OFF_OLDER + TRANSFER_FEE_OFF_MAXIMUM_FEE
            ..TRANSFER_FEE_CONFIG_OFF_OLDER + TRANSFER_FEE_OFF_MAXIMUM_FEE + 8]
            .copy_from_slice(&TRANSFER_FEE_MAXIMUM_FEE.to_le_bytes());
        fee[TRANSFER_FEE_CONFIG_OFF_OLDER + TRANSFER_FEE_OFF_BASIS_POINTS
            ..TRANSFER_FEE_CONFIG_OFF_OLDER + TRANSFER_FEE_OFF_BASIS_POINTS + 2]
            .copy_from_slice(&TRANSFER_FEE_BASIS_POINTS.to_le_bytes());
        fee[TRANSFER_FEE_CONFIG_OFF_NEWER + TRANSFER_FEE_OFF_MAXIMUM_FEE
            ..TRANSFER_FEE_CONFIG_OFF_NEWER + TRANSFER_FEE_OFF_MAXIMUM_FEE + 8]
            .copy_from_slice(&TRANSFER_FEE_MAXIMUM_FEE.to_le_bytes());
        fee[TRANSFER_FEE_CONFIG_OFF_NEWER + TRANSFER_FEE_OFF_BASIS_POINTS
            ..TRANSFER_FEE_CONFIG_OFF_NEWER + TRANSFER_FEE_OFF_BASIS_POINTS + 2]
            .copy_from_slice(&TRANSFER_FEE_BASIS_POINTS.to_le_bytes());
        d.extend_from_slice(&EXTENSION_TYPE_TRANSFER_FEE_CONFIG.to_le_bytes());
        d.extend_from_slice(&(TRANSFER_FEE_CONFIG_LEN as u16).to_le_bytes());
        d.extend_from_slice(&fee);
        d
    }

    fn token(mint: Pubkey, owner: Pubkey) -> Vec<u8> {
        let mut d = vec![0u8; TOKEN_ACCOUNT_BASE_LEN];
        d[0..32].copy_from_slice(mint.as_ref());
        d[32..64].copy_from_slice(owner.as_ref());
        d[64..72].copy_from_slice(&1_000_000u64.to_le_bytes());
        d[TOKEN_ACCOUNT_OFF_STATE] = TOKEN_ACCOUNT_STATE_INITIALIZED;
        d
    }

    fn curve(complete: bool, real_token: u64) -> Vec<u8> {
        let mut d = vec![0u8; BONDING_CURVE_CORE_LEN];
        d[0..8].copy_from_slice(&BONDING_CURVE_DISCRIMINATOR);
        d[CURVE_OFF_REAL_TOKEN_RESERVES..CURVE_OFF_REAL_TOKEN_RESERVES + 8]
            .copy_from_slice(&real_token.to_le_bytes());
        d[CURVE_OFF_COMPLETE] = u8::from(complete);
        d
    }

    fn graduated_config(
        kek: Pubkey,
        ichor: Pubkey,
        burner: Pubkey,
    ) -> (
        ConfigView,
        Pubkey,
        Vec<u8>,
        Vec<u8>,
        Vec<u8>,
        Vec<u8>,
        Vec<u8>,
    ) {
        let (curve_pda, _) = bonding_curve_pda(&kek);
        let (_, withdraw_bump) = withdraw_withheld_pda(&crate::ID);
        let config = ConfigView {
            authority: pk(1),
            pending_authority: None,
            kekbull_mint: kek,
            ichor_mint: ichor,
            kekbull_token_program: TOKEN_2022_PROGRAM_ID,
            ichor_token_program: TOKEN_2022_PROGRAM_ID,
            pump_program: PUMP_PROGRAM,
            bonding_curve: curve_pda,
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
            withdraw_withheld_bump: withdraw_bump,
            total_fees_withdrawn: 0,
            total_fees_to_creator: 0,
            total_fees_to_realms: 0,
            creator_escrow_bump: 0,
            last_creator_claim_ts: 1,
            creator_decay_secs: 86_400,
            total_creator_claimed: 0,
            total_creator_swept: 0,
        };
        let pda = crate::constants::config_pda(&crate::ID).0;
        let withdraw = withdraw_withheld_pda(&crate::ID).0;
        (
            config,
            curve_pda,
            mint(None, 6),
            token2022_ichor(pda, withdraw, 9),
            token(kek, burner),
            token(ichor, burner),
            curve(true, 0),
        )
    }

    #[test]
    fn happy_path_convert() {
        let kek = pk(2);
        let ichor = pk(3);
        let burner = pk(4);
        let (config, curve_pda, kdata, idata, from, to, cdata) =
            graduated_config(kek, ichor, burner);
        let ok = check_convert(
            &config,
            &burner,
            &kek,
            &TOKEN_2022_PROGRAM_ID,
            &kdata,
            &ichor,
            &TOKEN_2022_PROGRAM_ID,
            &idata,
            &TOKEN_2022_PROGRAM_ID,
            &from,
            &TOKEN_2022_PROGRAM_ID,
            &to,
            &curve_pda,
            &PUMP_PROGRAM,
            &cdata,
            &TOKEN_2022_PROGRAM_ID,
            &TOKEN_2022_PROGRAM_ID,
            1_000_000,
            0,
        )
        .unwrap();
        assert_eq!(ok.ichor_amount, 1_000_000_000);
        assert_eq!(ok.min_ichor_amount, 0);
        assert_eq!(ok.recipient, burner);
        assert_eq!(ok.kekbull_decimals, 6);
        assert_eq!(ok.ichor_decimals, 9);
    }

    #[test]
    fn collect_reports_pause_wrong_mint_and_incomplete_together() {
        let kek = pk(2);
        let ichor = pk(3);
        let burner = pk(4);
        let (mut config, curve_pda, kdata, idata, from, to, _) =
            graduated_config(kek, ichor, burner);
        config.paused = true;
        let wrong_mint = pk(8);
        let incomplete = curve(false, 5);
        let violations = collect_convert_violations(
            &config,
            &burner,
            &wrong_mint,
            &TOKEN_PROGRAM_ID, // also wrong owner
            &kdata,
            &ichor,
            &TOKEN_2022_PROGRAM_ID, // also wrong
            &idata,
            &TOKEN_2022_PROGRAM_ID,
            &from,
            &TOKEN_2022_PROGRAM_ID,
            &to,
            &curve_pda,
            &TOKEN_PROGRAM_ID, // wrong curve owner
            &incomplete,
            &TOKEN_2022_PROGRAM_ID,
            &TOKEN_2022_PROGRAM_ID,
            1_000_000,
            0,
        );
        assert!(violations.contains(&Check::Paused));
        assert!(violations.contains(&Check::InvalidKekbullMint));
        assert!(violations.contains(&Check::InvalidMintOwner));
        assert!(violations.contains(&Check::CurveNotComplete));
        assert!(violations.contains(&Check::CurveStillHasReserves));
        assert!(violations.contains(&Check::InvalidBondingCurveOwner));
        assert!(violations.len() >= 6);
    }

    #[test]
    fn initialize_accepts_incomplete_curve_and_requires_pda_authority() {
        let kek = pk(2);
        let ichor = pk(3);
        let pda = pk(9);
        let withdraw = pk(10);
        let (curve_pda, _) = bonding_curve_pda(&kek);
        let kdata = mint(None, 6);
        let idata = token2022_ichor(pda, withdraw, 9);
        let cdata = curve(false, 793_100_000_000_000);
        assert!(check_initialize(
            &kek,
            &TOKEN_2022_PROGRAM_ID,
            &kdata,
            &ichor,
            &TOKEN_2022_PROGRAM_ID,
            &idata,
            &pda,
            &withdraw,
            &curve_pda,
            &PUMP_PROGRAM,
            &cdata,
            1,
            1,
        )
        .is_ok());
        assert_eq!(
            check_initialize(
                &kek,
                &TOKEN_PROGRAM_ID,
                &kdata,
                &ichor,
                &TOKEN_2022_PROGRAM_ID,
                &idata,
                &pda,
                &withdraw,
                &curve_pda,
                &PUMP_PROGRAM,
                &cdata,
                1,
                1,
            ),
            Err(Check::KekbullNotToken2022)
        );
        assert_eq!(
            check_initialize(
                &kek,
                &TOKEN_2022_PROGRAM_ID,
                &kdata,
                &ichor,
                &TOKEN_PROGRAM_ID,
                &idata,
                &pda,
                &withdraw,
                &curve_pda,
                &PUMP_PROGRAM,
                &cdata,
                1,
                1,
            ),
            Err(Check::IchorNotToken2022)
        );
        assert_eq!(
            check_initialize(
                &kek,
                &TOKEN_2022_PROGRAM_ID,
                &kdata,
                &kek,
                &TOKEN_2022_PROGRAM_ID,
                &idata,
                &pda,
                &withdraw,
                &curve_pda,
                &PUMP_PROGRAM,
                &cdata,
                1,
                1,
            ),
            Err(Check::MintsMustDiffer)
        );
    }

    #[test]
    fn ratio_propose_apply_and_authority() {
        let mut config = ConfigView {
            authority: pk(1),
            pending_authority: Some(pk(2)),
            kekbull_mint: pk(3),
            ichor_mint: pk(4),
            kekbull_token_program: TOKEN_2022_PROGRAM_ID,
            ichor_token_program: TOKEN_2022_PROGRAM_ID,
            pump_program: PUMP_PROGRAM,
            bonding_curve: pk(5),
            emission_numerator: 1,
            emission_denominator: 1,
            pending_emission_numerator: 1,
            pending_emission_denominator: 2,
            pending_ratio_unlock_ts: 1_000,
            ratio_timelock_secs: 100,
            paused: false,
            ratio_updates_frozen: false,
            has_pending_ratio: true,
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
        };
        assert_eq!(check_propose_ratio(&config, &pk(1), 1, 2, 50), Ok(150));
        assert_eq!(
            check_propose_ratio(&config, &pk(1), 3, 1, 50),
            Err(Check::RatioAboveCeiling)
        );
        assert_eq!(
            check_propose_ratio(&config, &pk(9), 1, 2, 50),
            Err(Check::Unauthorized)
        );
        assert_eq!(
            check_apply_ratio(&config, 999),
            Err(Check::RatioTimelockNotElapsed)
        );
        assert_eq!(check_apply_ratio(&config, 1_000), Ok((1, 2)));
        config.ratio_updates_frozen = true;
        assert_eq!(
            check_propose_ratio(&config, &pk(1), 1, 2, 50),
            Err(Check::RatioUpdatesFrozen)
        );
        config.ratio_updates_frozen = false;
        config.ratio_timelock_secs = 0;
        assert_eq!(
            check_propose_ratio(&config, &pk(1), 1, 2, 50),
            Err(Check::RatioLocked)
        );
        assert_eq!(
            check_increase_timelock(&config, &pk(1), 1),
            Err(Check::RatioLocked)
        );
        assert_eq!(check_accept_authority(&config, &pk(2)), Ok(()));
        assert_eq!(
            check_accept_authority(&config, &pk(1)),
            Err(Check::PendingAuthorityMismatch)
        );
        config.pending_authority = None;
        assert_eq!(
            check_accept_authority(&config, &pk(2)),
            Err(Check::NoPendingAuthority)
        );
        config.ratio_timelock_secs = 100;
        assert_eq!(
            check_increase_timelock(&config, &pk(1), 50),
            Err(Check::TimelockCannotDecrease)
        );
        assert_eq!(check_increase_timelock(&config, &pk(1), 100), Ok(()));
        assert_eq!(check_increase_timelock(&config, &pk(1), 200), Ok(()));
    }

    #[test]
    fn wrong_token_programs_on_convert() {
        let kek = pk(2);
        let ichor = pk(3);
        let burner = pk(4);
        let (config, curve_pda, kdata, idata, from, to, cdata) =
            graduated_config(kek, ichor, burner);
        assert_eq!(
            check_convert(
                &config,
                &burner,
                &kek,
                &TOKEN_2022_PROGRAM_ID,
                &kdata,
                &ichor,
                &TOKEN_2022_PROGRAM_ID,
                &idata,
                &TOKEN_2022_PROGRAM_ID,
                &from,
                &TOKEN_2022_PROGRAM_ID,
                &to,
                &curve_pda,
                &PUMP_PROGRAM,
                &cdata,
                &TOKEN_PROGRAM_ID,
                &TOKEN_PROGRAM_ID,
                1_000_000,
                0,
            ),
            Err(Check::InvalidTokenProgram)
        );
    }

    #[test]
    fn nonzero_ichor_supply_is_rejected_at_initialize() {
        let kek = pk(2);
        let ichor = pk(3);
        let pda = pk(9);
        let withdraw = pk(10);
        let (curve_pda, _) = bonding_curve_pda(&kek);
        let kdata = mint(None, 6);
        let mut idata = token2022_ichor(pda, withdraw, 9);
        idata[MINT_OFF_SUPPLY..MINT_OFF_SUPPLY + 8].copy_from_slice(&1u64.to_le_bytes());
        let cdata = curve(false, 0);
        assert_eq!(
            check_initialize(
                &kek,
                &TOKEN_2022_PROGRAM_ID,
                &kdata,
                &ichor,
                &TOKEN_2022_PROGRAM_ID,
                &idata,
                &pda,
                &withdraw,
                &curve_pda,
                &PUMP_PROGRAM,
                &cdata,
                1,
                1,
            ),
            Err(Check::IchorSupplyMustBeZero)
        );
        let legacy = mint(Some(pda), 9);
        assert_eq!(
            check_initialize(
                &kek,
                &TOKEN_2022_PROGRAM_ID,
                &kdata,
                &ichor,
                &TOKEN_2022_PROGRAM_ID,
                &legacy,
                &pda,
                &withdraw,
                &curve_pda,
                &PUMP_PROGRAM,
                &cdata,
                1,
                1,
            ),
            Err(Check::TransferFeeConfigMissing)
        );
    }

    #[test]
    fn min_ichor_amount_fails_before_any_cpi() {
        let kek = pk(2);
        let ichor = pk(3);
        let burner = pk(4);
        let (config, curve_pda, kdata, idata, from, to, cdata) =
            graduated_config(kek, ichor, burner);
        assert_eq!(
            check_convert(
                &config,
                &burner,
                &kek,
                &TOKEN_2022_PROGRAM_ID,
                &kdata,
                &ichor,
                &TOKEN_2022_PROGRAM_ID,
                &idata,
                &TOKEN_2022_PROGRAM_ID,
                &from,
                &TOKEN_2022_PROGRAM_ID,
                &to,
                &curve_pda,
                &PUMP_PROGRAM,
                &cdata,
                &TOKEN_2022_PROGRAM_ID,
                &TOKEN_2022_PROGRAM_ID,
                1_000_000,
                1_000_000_001,
            ),
            Err(Check::BelowMinIchorAmount)
        );
    }

    #[test]
    fn convert_event_fields_record_recipient_and_min() {
        let kek = pk(2);
        let ichor = pk(3);
        let burner = pk(4);
        let recipient = pk(11);
        let (config, curve_pda, kdata, idata, from, _, cdata) =
            graduated_config(kek, ichor, burner);
        let to = token(ichor, recipient);
        let ok = check_convert(
            &config,
            &burner,
            &kek,
            &TOKEN_2022_PROGRAM_ID,
            &kdata,
            &ichor,
            &TOKEN_2022_PROGRAM_ID,
            &idata,
            &TOKEN_2022_PROGRAM_ID,
            &from,
            &TOKEN_2022_PROGRAM_ID,
            &to,
            &curve_pda,
            &PUMP_PROGRAM,
            &cdata,
            &TOKEN_2022_PROGRAM_ID,
            &TOKEN_2022_PROGRAM_ID,
            1_000_000,
            500_000_000,
        )
        .unwrap();
        assert_eq!(ok.recipient, recipient);
        assert_ne!(ok.recipient, burner);
        assert_eq!(ok.ichor_amount, 1_000_000_000);
        assert_eq!(ok.min_ichor_amount, 500_000_000);
        assert_eq!(ok.kekbull_amount, 1_000_000);
    }

    #[test]
    fn default_pending_authority_is_rejected() {
        assert_eq!(check_set_pending_authority(None), Ok(()));
        assert_eq!(check_set_pending_authority(Some(pk(2))), Ok(()));
        assert_eq!(
            check_set_pending_authority(Some(Pubkey::default())),
            Err(Check::InvalidPendingAuthority)
        );
    }

    fn governance_fixture(realms: &Pubkey) -> (Pubkey, Vec<u8>) {
        let realm = pk(30);
        let governed = pk(31);
        let governance = Pubkey::find_program_address(
            &[b"account-governance", realm.as_ref(), governed.as_ref()],
            realms,
        )
        .0;
        let mut data = vec![0u8; 65];
        data[0] = 18;
        data[1..33].copy_from_slice(realm.as_ref());
        data[33..65].copy_from_slice(governed.as_ref());
        (governance, data)
    }

    fn realm_account_data(community_mint: &Pubkey) -> Vec<u8> {
        let mut data = vec![0u8; 33];
        data[0] = REALM_ACCOUNT_TYPE_V2;
        data[1..33].copy_from_slice(community_mint.as_ref());
        data
    }

    fn bound_config(_program: Pubkey) -> (ConfigView, Pubkey, Pubkey, Pubkey, Pubkey) {
        let creator = pk(20);
        let realms = REALMS_PROGRAM;
        let governance = governance_fixture(&realms).0;
        let treasury = native_treasury_pda(&realms, &governance).0;
        let withdraw = withdraw_withheld_pda(&crate::ID).0;
        let mut config = graduated_config(pk(2), pk(3), pk(4)).0;
        config.creator_beneficiary = Some(creator);
        config.realms_program = Some(realms);
        config.realms_realm = Some(pk(30));
        config.realms_governance = Some(governance);
        config.realms_native_treasury = Some(treasury);
        config.fee_beneficiaries_bound = true;
        let (escrow_pda, escrow_bump) = crate::constants::creator_escrow_pda(&crate::ID);
        let _ = escrow_pda;
        config.creator_escrow_bump = escrow_bump;
        config.last_creator_claim_ts = 1_000;
        config.creator_decay_secs = 100;
        (config, creator, treasury, withdraw, realms)
    }

    #[test]
    fn convert_rejects_legacy_spl_ichor_accounts() {
        let kek = pk(2);
        let ichor = pk(3);
        let burner = pk(4);
        let (config, curve_pda, kdata, idata, from, to, cdata) =
            graduated_config(kek, ichor, burner);
        assert_eq!(
            check_convert(
                &config,
                &burner,
                &kek,
                &TOKEN_2022_PROGRAM_ID,
                &kdata,
                &ichor,
                &TOKEN_PROGRAM_ID,
                &idata,
                &TOKEN_2022_PROGRAM_ID,
                &from,
                &TOKEN_2022_PROGRAM_ID,
                &to,
                &curve_pda,
                &PUMP_PROGRAM,
                &cdata,
                &TOKEN_2022_PROGRAM_ID,
                &TOKEN_2022_PROGRAM_ID,
                1_000_000,
                0,
            ),
            Err(Check::InvalidMintOwner)
        );
        assert_eq!(
            check_convert(
                &config,
                &burner,
                &kek,
                &TOKEN_2022_PROGRAM_ID,
                &kdata,
                &ichor,
                &TOKEN_2022_PROGRAM_ID,
                &idata,
                &TOKEN_2022_PROGRAM_ID,
                &from,
                &TOKEN_PROGRAM_ID,
                &to,
                &curve_pda,
                &PUMP_PROGRAM,
                &cdata,
                &TOKEN_2022_PROGRAM_ID,
                &TOKEN_2022_PROGRAM_ID,
                1_000_000,
                0,
            ),
            Err(Check::InvalidTokenAccountOwner)
        );
    }

    #[test]
    fn fee_authority_revocation_and_pilot_cap() {
        let (mut config, _, _, _, _) = bound_config(pk(1));
        config.authority = pk(1);
        assert_eq!(
            check_set_transfer_fee(&config, &pk(1), 25, u64::MAX),
            Ok(())
        );
        assert_eq!(
            check_set_transfer_fee(&config, &pk(1), 0, 0),
            Err(Check::TransferFeeExceedsPilotCap)
        );
        assert_eq!(
            check_set_transfer_fee(&config, &pk(1), 26, u64::MAX),
            Err(Check::TransferFeeExceedsPilotCap)
        );
        assert_eq!(
            check_set_transfer_fee(&config, &pk(9), 10, 1),
            Err(Check::Unauthorized)
        );
        assert_eq!(check_revoke_transfer_fee_authority(&config, &pk(1)), Ok(()));
        config.transfer_fee_authority_revoked = true;
        assert_eq!(
            check_revoke_transfer_fee_authority(&config, &pk(1)),
            Err(Check::FeeAuthorityRevoked)
        );
        assert_eq!(
            check_set_transfer_fee(&config, &pk(1), 10, 1),
            Err(Check::FeeAuthorityRevoked)
        );
    }

    #[test]
    fn fee_distribution_requires_proof_treasury() {
        let (config, creator, treasury, _, realms) = bound_config(pk(1));
        let governance = config.realms_governance.unwrap();
        let (_, governance_data) = governance_fixture(&realms);
        let realm = pk(30);
        let realm_data = realm_account_data(&config.ichor_mint);
        let mut unbound = config.clone();
        unbound.fee_beneficiaries_bound = false;
        assert_eq!(
            check_set_fee_distribution(
                &unbound,
                &unbound.authority,
                &creator,
                &realms,
                &realm,
                &realms,
                &realm_data,
                &governance,
                &realms,
                &governance_data,
                &treasury,
            ),
            Ok(())
        );
        assert_eq!(
            check_set_fee_distribution(
                &unbound,
                &unbound.authority,
                &Pubkey::default(),
                &realms,
                &realm,
                &realms,
                &realm_data,
                &governance,
                &realms,
                &governance_data,
                &treasury,
            ),
            Err(Check::InvalidFeeBeneficiary)
        );
        assert_eq!(
            check_set_fee_distribution(
                &unbound,
                &unbound.authority,
                &creator,
                &realms,
                &realm,
                &realms,
                &realm_data,
                &governance,
                &pk(99),
                &governance_data,
                &treasury,
            ),
            Err(Check::InvalidRealmsGovernance)
        );
        assert_eq!(
            check_set_fee_distribution(
                &unbound,
                &unbound.authority,
                &creator,
                &pk(21),
                &realm,
                &pk(21),
                &realm_data,
                &governance_fixture(&pk(21)).0,
                &pk(21),
                &governance_fixture(&pk(21)).1,
                &native_treasury_pda(&pk(21), &governance_fixture(&pk(21)).0).0,
            ),
            Err(Check::DaoDestinationMismatch)
        );
        let wrong_mint_realm = realm_account_data(&pk(99));
        assert_eq!(
            check_set_fee_distribution(
                &unbound,
                &unbound.authority,
                &creator,
                &realms,
                &realm,
                &realms,
                &wrong_mint_realm,
                &governance,
                &realms,
                &governance_data,
                &treasury,
            ),
            Err(Check::InvalidRealmsGovernance)
        );
        assert_eq!(
            check_set_fee_distribution(
                &unbound,
                &unbound.authority,
                &creator,
                &realms,
                &realm,
                &realms,
                &realm_data,
                &governance,
                &realms,
                &governance_data,
                &pk(99),
            ),
            Err(Check::DaoDestinationMismatch)
        );
        let mut wrong_data = governance_data.clone();
        wrong_data[0] = 16;
        assert_eq!(
            check_set_fee_distribution(
                &unbound,
                &unbound.authority,
                &creator,
                &realms,
                &realm,
                &realms,
                &realm_data,
                &governance,
                &realms,
                &wrong_data,
                &treasury,
            ),
            Err(Check::InvalidRealmsGovernance)
        );
        assert_eq!(
            check_set_fee_distribution(
                &config,
                &config.authority,
                &creator,
                &realms,
                &realm,
                &realms,
                &realm_data,
                &governance,
                &realms,
                &governance_data,
                &treasury,
            ),
            Err(Check::FeeBeneficiariesAlreadyBound)
        );
    }

    #[test]
    fn dao_destination_must_be_committed_before_bind_and_rejects_substitutes() {
        let (config, creator, treasury, _, realms) = bound_config(pk(1));
        let governance = config.realms_governance.unwrap();
        let (_, governance_data) = governance_fixture(&realms);
        let realm = pk(30);
        let realm_data = realm_account_data(&config.ichor_mint);
        let mut uncommitted = config.clone();
        uncommitted.fee_beneficiaries_bound = false;
        uncommitted.realms_program = None;
        uncommitted.realms_realm = None;
        uncommitted.realms_governance = None;
        uncommitted.realms_native_treasury = None;
        uncommitted.creator_beneficiary = None;
        assert_eq!(
            check_set_fee_distribution(
                &uncommitted,
                &uncommitted.authority,
                &creator,
                &realms,
                &realm,
                &realms,
                &realm_data,
                &governance,
                &realms,
                &governance_data,
                &treasury,
            ),
            Err(Check::DaoDestinationUncommitted)
        );
        assert_eq!(
            check_commit_dao_destination(
                &uncommitted,
                &uncommitted.authority,
                &realms,
                &realm,
                &realms,
                &realm_data,
                &governance,
                &realms,
                &governance_data,
                &treasury,
            ),
            Ok(())
        );
        let mut committed = uncommitted.clone();
        committed.realms_program = Some(realms);
        committed.realms_realm = Some(realm);
        committed.realms_governance = Some(governance);
        committed.realms_native_treasury = Some(treasury);
        assert_eq!(
            check_commit_dao_destination(
                &committed,
                &committed.authority,
                &realms,
                &realm,
                &realms,
                &realm_data,
                &governance,
                &realms,
                &governance_data,
                &treasury,
            ),
            Err(Check::DaoDestinationAlreadyCommitted)
        );
        assert_eq!(
            check_set_fee_distribution(
                &committed,
                &committed.authority,
                &creator,
                &realms,
                &realm,
                &realms,
                &realm_data,
                &governance,
                &realms,
                &governance_data,
                &treasury,
            ),
            Ok(())
        );
        let other_realm = pk(31);
        assert_eq!(
            require_accounts_match_committed_dao_destination(
                &committed,
                &realms,
                &other_realm,
                &governance,
                &treasury,
            ),
            Err(Check::DaoDestinationMismatch)
        );
        let other_governed = pk(32);
        let other_gov = Pubkey::find_program_address(
            &[b"account-governance", realm.as_ref(), other_governed.as_ref()],
            &realms,
        )
        .0;
        assert_eq!(
            require_accounts_match_committed_dao_destination(
                &committed,
                &realms,
                &realm,
                &other_gov,
                &treasury,
            ),
            Err(Check::DaoDestinationMismatch)
        );
        let gtest = crate::constants::REALMS_TEST_PROGRAM;
        let gtest_gov = governance_fixture(&gtest);
        let gtest_treasury = native_treasury_pda(&gtest, &gtest_gov.0).0;
        assert_eq!(
            require_accounts_match_committed_dao_destination(
                &committed,
                &gtest,
                &realm,
                &governance,
                &treasury,
            ),
            Err(Check::DaoDestinationMismatch)
        );
        assert_eq!(
            require_accounts_match_committed_dao_destination(
                &committed,
                &gtest,
                &pk(30),
                &gtest_gov.0,
                &gtest_treasury,
            ),
            Err(Check::DaoDestinationMismatch)
        );
        let mut gtest_committed = uncommitted.clone();
        gtest_committed.realms_program = Some(gtest);
        gtest_committed.realms_realm = Some(pk(30));
        gtest_committed.realms_governance = Some(gtest_gov.0);
        gtest_committed.realms_native_treasury = Some(gtest_treasury);
        assert_eq!(
            committed_dao_destination(&gtest_committed),
            Err(Check::InvalidRealmsGovernance)
        );
        assert_eq!(
            require_accounts_match_committed_dao_destination(
                &gtest_committed,
                &gtest,
                &pk(30),
                &gtest_gov.0,
                &gtest_treasury,
            ),
            Err(Check::InvalidRealmsGovernance)
        );
        assert_eq!(
            check_commit_dao_destination(
                &uncommitted,
                &uncommitted.authority,
                &gtest,
                &pk(30),
                &gtest,
                &realm_data,
                &gtest_gov.0,
                &gtest,
                &gtest_gov.1,
                &gtest_treasury,
            ),
            Err(Check::InvalidRealmsGovernance)
        );
        assert_eq!(
            check_realms_destination_proof(
                &uncommitted.ichor_mint,
                &gtest,
                &pk(30),
                &gtest,
                &realm_data,
                &gtest_gov.0,
                &gtest,
                &gtest_gov.1,
                &gtest_treasury,
            ),
            Err(Check::InvalidRealmsGovernance)
        );
        assert_eq!(
            require_accounts_match_committed_dao_destination(
                &committed,
                &realms,
                &realm,
                &governance,
                &native_treasury_pda(&realms, &other_gov).0,
            ),
            Err(Check::DaoDestinationMismatch)
        );
        assert_eq!(
            require_accounts_match_committed_dao_destination(
                &committed,
                &realms,
                &realm,
                &governance,
                &treasury,
            ),
            Ok((realm, realms, governance, treasury))
        );
        let wrong_mint = realm_account_data(&pk(99));
        assert_eq!(
            check_commit_dao_destination(
                &uncommitted,
                &uncommitted.authority,
                &realms,
                &realm,
                &realms,
                &wrong_mint,
                &governance,
                &realms,
                &governance_data,
                &treasury,
            ),
            Err(Check::InvalidRealmsGovernance)
        );
    }

    #[test]
    fn unauthorized_destinations_and_unbound_fees() {
        let program = pk(1);
        let (mut config, creator, treasury, withdraw, _) = bound_config(program);
        let ichor = config.ichor_mint;
        let mint_data =
            token2022_ichor(crate::constants::config_pda(&crate::ID).0, withdraw, 9);
        let vault = token(ichor, withdraw);
        let creator_dest = token(ichor, creator);
        let realms_dest = token(ichor, treasury);

        assert!(check_distribute(
            &config,
            &ichor,
            &TOKEN_2022_PROGRAM_ID,
            &mint_data,
            &withdraw,
            &withdraw,
            &TOKEN_2022_PROGRAM_ID,
            &vault,
            &TOKEN_2022_PROGRAM_ID,
            &creator_dest,
            &TOKEN_2022_PROGRAM_ID,
            &realms_dest,
            &TOKEN_2022_PROGRAM_ID,
        )
        .is_ok());

        let wrong_creator = token(ichor, pk(30));
        assert_eq!(
            check_distribute(
                &config,
                &ichor,
                &TOKEN_2022_PROGRAM_ID,
                &mint_data,
                &withdraw,
                &withdraw,
                &TOKEN_2022_PROGRAM_ID,
                &vault,
                &TOKEN_2022_PROGRAM_ID,
                &wrong_creator,
                &TOKEN_2022_PROGRAM_ID,
                &realms_dest,
                &TOKEN_2022_PROGRAM_ID,
            ),
            Err(Check::TokenAccountOwnerMismatch)
        );
        let wrong_realms = token(ichor, pk(31));
        assert_eq!(
            check_distribute(
                &config,
                &ichor,
                &TOKEN_2022_PROGRAM_ID,
                &mint_data,
                &withdraw,
                &withdraw,
                &TOKEN_2022_PROGRAM_ID,
                &vault,
                &TOKEN_2022_PROGRAM_ID,
                &creator_dest,
                &TOKEN_2022_PROGRAM_ID,
                &wrong_realms,
                &TOKEN_2022_PROGRAM_ID,
            ),
            Err(Check::TokenAccountOwnerMismatch)
        );
        let wrong_vault = token(ichor, pk(32));
        assert_eq!(
            check_distribute(
                &config,
                &ichor,
                &TOKEN_2022_PROGRAM_ID,
                &mint_data,
                &withdraw,
                &withdraw,
                &TOKEN_2022_PROGRAM_ID,
                &wrong_vault,
                &TOKEN_2022_PROGRAM_ID,
                &creator_dest,
                &TOKEN_2022_PROGRAM_ID,
                &realms_dest,
                &TOKEN_2022_PROGRAM_ID,
            ),
            Err(Check::TokenAccountOwnerMismatch)
        );

        config.fee_beneficiaries_bound = false;
        assert_eq!(
            check_distribute(
                &config,
                &ichor,
                &TOKEN_2022_PROGRAM_ID,
                &mint_data,
                &withdraw,
                &withdraw,
                &TOKEN_2022_PROGRAM_ID,
                &vault,
                &TOKEN_2022_PROGRAM_ID,
                &creator_dest,
                &TOKEN_2022_PROGRAM_ID,
                &realms_dest,
                &TOKEN_2022_PROGRAM_ID,
            ),
            Err(Check::FeeBeneficiariesUnbound)
        );
    }

    #[test]
    fn zero_fee_distribution_is_allowed() {
        let program = pk(1);
        let (config, creator, treasury, withdraw, _) = bound_config(program);
        let ichor = config.ichor_mint;
        let mint_data =
            token2022_ichor(crate::constants::config_pda(&crate::ID).0, withdraw, 9);
        let mut vault = token(ichor, withdraw);
        vault[64..72].copy_from_slice(&0u64.to_le_bytes());
        let ok = check_distribute(
            &config,
            &ichor,
            &TOKEN_2022_PROGRAM_ID,
            &mint_data,
            &withdraw,
            &withdraw,
            &TOKEN_2022_PROGRAM_ID,
            &vault,
            &TOKEN_2022_PROGRAM_ID,
            &token(ichor, creator),
            &TOKEN_2022_PROGRAM_ID,
            &token(ichor, treasury),
            &TOKEN_2022_PROGRAM_ID,
        )
        .unwrap();
        assert_eq!(ok.withheld_before, 0);
        assert_eq!(ok.vault_before, 0);
        assert_eq!(split_harvested_fees(0), Ok((0, 0)));
    }

    #[test]
    fn initialize_rejects_extra_extension_and_wrong_fee_rate() {
        let kek = pk(2);
        let ichor = pk(3);
        let pda = pk(9);
        let withdraw = pk(10);
        let (curve_pda, _) = bonding_curve_pda(&kek);
        let kdata = mint(None, 6);
        let mut extra = token2022_ichor(pda, withdraw, 9);
        extra.extend_from_slice(&EXTENSION_TYPE_TRANSFER_HOOK.to_le_bytes());
        extra.extend_from_slice(&32u16.to_le_bytes());
        extra.extend_from_slice(&[0u8; 32]);
        let cdata = curve(false, 0);
        assert_eq!(
            check_initialize(
                &kek,
                &TOKEN_2022_PROGRAM_ID,
                &kdata,
                &ichor,
                &TOKEN_2022_PROGRAM_ID,
                &extra,
                &pda,
                &withdraw,
                &curve_pda,
                &PUMP_PROGRAM,
                &cdata,
                1,
                1,
            ),
            Err(Check::UnexpectedMintExtension)
        );

        let mut wrong_rate = token2022_ichor(pda, withdraw, 9);
        let bps_off = TOKEN_2022_ACCOUNT_TYPE_OFFSET
            + 1
            + 4
            + TRANSFER_FEE_CONFIG_OFF_NEWER
            + TRANSFER_FEE_OFF_BASIS_POINTS;
        wrong_rate[bps_off..bps_off + 2].copy_from_slice(&100u16.to_le_bytes());
        assert_eq!(
            check_initialize(
                &kek,
                &TOKEN_2022_PROGRAM_ID,
                &kdata,
                &ichor,
                &TOKEN_2022_PROGRAM_ID,
                &wrong_rate,
                &pda,
                &withdraw,
                &curve_pda,
                &PUMP_PROGRAM,
                &cdata,
                1,
                1,
            ),
            Err(Check::TransferFeeRateMismatch)
        );
    }

    #[test]
    fn fee_counter_overflow_is_rejected_before_cpi() {
        let program = pk(1);
        let (mut config, creator, treasury, withdraw, _) = bound_config(program);
        config.total_fees_withdrawn = u64::MAX;
        let ichor = config.ichor_mint;
        let mut mint_data =
            token2022_ichor(crate::constants::config_pda(&crate::ID).0, withdraw, 9);
        let withheld_off = TOKEN_2022_ACCOUNT_TYPE_OFFSET + 1 + 4 + 64;
        mint_data[withheld_off..withheld_off + 8].copy_from_slice(&1u64.to_le_bytes());
        assert_eq!(
            check_distribute(
                &config,
                &ichor,
                &TOKEN_2022_PROGRAM_ID,
                &mint_data,
                &withdraw,
                &withdraw,
                &TOKEN_2022_PROGRAM_ID,
                &token(ichor, withdraw),
                &TOKEN_2022_PROGRAM_ID,
                &token(ichor, creator),
                &TOKEN_2022_PROGRAM_ID,
                &token(ichor, treasury),
                &TOKEN_2022_PROGRAM_ID,
            ),
            Err(Check::ArithmeticOverflow)
        );
    }

    #[test]
    fn initialize_rejects_zero_creator_decay_via_helper() {
        assert_eq!(require_creator_decay_secs(0), Err(Check::CreatorDecayZero));
        assert_eq!(require_creator_decay_secs(7), Ok(()));
    }

    #[test]
    fn claim_rejects_unauthorized_signer() {
        let program = pk(1);
        let (config, creator, treasury, withdraw, _) = bound_config(program);
        let _ = (treasury, withdraw);
        let ichor = config.ichor_mint;
        let escrow = creator_escrow_pda(&crate::ID).0;
        let escrow_ata = token(ichor, escrow);
        // put amount 1 into escrow bytes
        let mut escrow_ata = escrow_ata;
        escrow_ata[64..72].copy_from_slice(&1u64.to_le_bytes());
        let creator_ata = token(ichor, creator);
        assert_eq!(
            check_claim_creator_fees(
                &config,
                &pk(99),
                2_000,
                &ichor,
                &TOKEN_2022_PROGRAM_ID,
                &escrow_ata,
                &TOKEN_2022_PROGRAM_ID,
                &creator_ata,
                &TOKEN_2022_PROGRAM_ID,
            ),
            Err(Check::UnauthorizedCreatorClaim)
        );
        assert_eq!(
            check_claim_creator_fees(
                &config,
                &creator,
                2_000,
                &ichor,
                &TOKEN_2022_PROGRAM_ID,
                &escrow_ata,
                &TOKEN_2022_PROGRAM_ID,
                &creator_ata,
                &TOKEN_2022_PROGRAM_ID,
            ),
            Ok(1)
        );
        assert_eq!(
            check_claim_creator_fees(
                &config,
                &creator,
                0,
                &ichor,
                &TOKEN_2022_PROGRAM_ID,
                &escrow_ata,
                &TOKEN_2022_PROGRAM_ID,
                &creator_ata,
                &TOKEN_2022_PROGRAM_ID,
            ),
            Err(Check::ClockInversion)
        );
    }

    #[test]
    fn sweep_before_and_after_decay() {
        let program = pk(1);
        let (config, _creator, treasury, _withdraw, _) = bound_config(program);
        let ichor = config.ichor_mint;
        let escrow = creator_escrow_pda(&crate::ID).0;
        let mut escrow_ata = token(ichor, escrow);
        escrow_ata[64..72].copy_from_slice(&9u64.to_le_bytes());
        let realms_ata = token(ichor, treasury);
        // last_claim=1000, decay=100 → elapsed at 1100 is false, at 1101 is true
        assert_eq!(
            check_sweep_unclaimed_creator_fees(
                &config,
                1_100,
                &ichor,
                &TOKEN_2022_PROGRAM_ID,
                &escrow_ata,
                &TOKEN_2022_PROGRAM_ID,
                &realms_ata,
                &TOKEN_2022_PROGRAM_ID,
            ),
            Err(Check::CreatorDecayNotElapsed)
        );
        assert_eq!(
            check_sweep_unclaimed_creator_fees(
                &config,
                1_101,
                &ichor,
                &TOKEN_2022_PROGRAM_ID,
                &escrow_ata,
                &TOKEN_2022_PROGRAM_ID,
                &realms_ata,
                &TOKEN_2022_PROGRAM_ID,
            ),
            Ok(9)
        );
    }

    #[test]
    fn authority_cannot_set_creator_beneficiary() {
        let program = pk(1);
        let (config, creator, treasury, withdraw, _) = bound_config(program);
        let _ = (treasury, withdraw);
        assert_eq!(
            check_set_creator_beneficiary(&config, &config.authority, &pk(40)),
            Err(Check::Unauthorized)
        );
        assert_eq!(
            check_set_creator_beneficiary(&config, &creator, &pk(40)),
            Ok(())
        );
        assert_eq!(
            check_set_creator_beneficiary(&config, &creator, &Pubkey::default()),
            Err(Check::InvalidFeeBeneficiary)
        );
        let mut unbound = config.clone();
        unbound.fee_beneficiaries_bound = false;
        assert_eq!(
            check_set_creator_beneficiary(&unbound, &creator, &pk(40)),
            Err(Check::FeeBeneficiariesUnbound)
        );
    }

}
