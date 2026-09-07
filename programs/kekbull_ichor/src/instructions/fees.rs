use anchor_lang::prelude::*;
use anchor_spl::token_2022::{self, SetAuthority, TransferChecked};
use anchor_spl::token_2022_extensions::transfer_fee::{
    self, TransferFeeSetTransferFee, WithdrawWithheldTokensFromMint,
};

use crate::constants::{creator_escrow_pda, TOKEN_2022_PROGRAM_ID, CREATOR_ESCROW_SEED, WITHDRAW_WITHHELD_SEED};
use crate::error::IchorError;
use crate::events::{
    CreatorBeneficiarySet, CreatorFeesClaimed, DaoDestinationCommitted, FeeDistributionBound,
    TransferFeeAuthorityRevoked, TransferFeeUpdated, TransferFeesDistributed,
    UnclaimedCreatorFeesSwept,
};
use crate::extensions::parse_ichor_transfer_fee_config;
use crate::math::split_harvested_fees;
use crate::mint_layout::{parse_mint, parse_token_account};
use crate::state::{add_fee_totals, Config, ConfigView};
use crate::validate::{
    check_claim_creator_fees, check_commit_dao_destination, check_distribute,
    check_revoke_transfer_fee_authority, check_set_creator_beneficiary,
    check_set_fee_distribution, check_set_transfer_fee, check_sweep_unclaimed_creator_fees,
};

#[derive(Accounts)]
pub struct CommitDaoDestination<'info> {
    pub authority: Signer<'info>,

    #[account(
        mut,
        seeds = [Config::SEED],
        bump = config.bump,
        has_one = authority @ IchorError::Unauthorized
    )]
    pub config: Account<'info, Config>,

    /// CHECK: must be exact GovER5 (`GovER5Lthms3bLBqWub97yVrMmEogzX7xNjdXpPPCVZw`).
    pub realms_program: UncheckedAccount<'info>,

    /// CHECK: RealmV1/V2 owned by `realms_program`; community mint = Config ICHOR.
    pub realm: UncheckedAccount<'info>,

    /// CHECK: SPL Governance account. Owner must equal `realms_program`.
    pub governance: UncheckedAccount<'info>,

    /// CHECK: must equal `["native-treasury", governance]` on `realms_program`.
    pub native_treasury: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct SetFeeDistribution<'info> {
    pub authority: Signer<'info>,

    #[account(
        mut,
        seeds = [Config::SEED],
        bump = config.bump,
        has_one = authority @ IchorError::Unauthorized
    )]
    pub config: Account<'info, Config>,

    /// CHECK: must match the committed GovER5 destination (GTesT is refused).
    pub realms_program: UncheckedAccount<'info>,

    /// CHECK: RealmV1/V2 owned by `realms_program`; community mint = Config ICHOR.
    pub realm: UncheckedAccount<'info>,

    /// CHECK: SPL Governance account. Owner must equal `realms_program`.
    pub governance: UncheckedAccount<'info>,

    /// CHECK: must equal `["native-treasury", governance]` on `realms_program`.
    pub native_treasury: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct DistributeTransferFees<'info> {
    /// Permissionless trigger. Recorded only as the transaction signer.
    pub caller: Signer<'info>,

    #[account(
        mut,
        seeds = [Config::SEED],
        bump = config.bump
    )]
    pub config: Account<'info, Config>,

    /// CHECK: configured Token-2022 ICHOR mint.
    #[account(mut)]
    pub ichor_mint: UncheckedAccount<'info>,

    /// CHECK: withdraw-withheld PDA. Signs the withdraw CPI.
    #[account(
        seeds = [WITHDRAW_WITHHELD_SEED],
        bump = config.withdraw_withheld_bump
    )]
    pub withdraw_withheld_authority: UncheckedAccount<'info>,

    /// CHECK: Token-2022 ICHOR account owned by the withdraw PDA.
    #[account(mut)]
    pub fee_vault: UncheckedAccount<'info>,

    /// CHECK: Token-2022 ICHOR ATA owned by the bound creator beneficiary.
    #[account(mut)]
    pub creator_escrow: UncheckedAccount<'info>,

    /// CHECK: Token-2022 ICHOR account owned by the bound Realms treasury.
    #[account(mut)]
    pub realms_destination: UncheckedAccount<'info>,

    /// CHECK: must be the Token-2022 program id.
    pub token_2022_program: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct SetTransferFee<'info> {
    pub authority: Signer<'info>,

    #[account(
        mut,
        seeds = [Config::SEED],
        bump = config.bump,
        has_one = authority @ IchorError::Unauthorized
    )]
    pub config: Account<'info, Config>,

    /// CHECK: configured Token-2022 ICHOR mint.
    #[account(mut)]
    pub ichor_mint: UncheckedAccount<'info>,

    /// CHECK: must be the Token-2022 program id.
    pub token_2022_program: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct RevokeTransferFeeAuthority<'info> {
    pub authority: Signer<'info>,

    #[account(
        mut,
        seeds = [Config::SEED],
        bump = config.bump,
        has_one = authority @ IchorError::Unauthorized
    )]
    pub config: Account<'info, Config>,

    /// CHECK: configured Token-2022 ICHOR mint.
    #[account(mut)]
    pub ichor_mint: UncheckedAccount<'info>,

    /// CHECK: must be the Token-2022 program id.
    pub token_2022_program: UncheckedAccount<'info>,
}

fn require_ichor_mint(config: &Config, mint: &Pubkey, mint_owner: &Pubkey) -> Result<()> {
    require!(*mint == config.ichor_mint, IchorError::InvalidIchorMint);
    require!(
        *mint_owner == TOKEN_2022_PROGRAM_ID,
        IchorError::InvalidMintOwner
    );
    Ok(())
}

pub fn handle_commit_dao_destination(ctx: Context<CommitDaoDestination>) -> Result<()> {
    let view = ConfigView::from(&*ctx.accounts.config);
    check_commit_dao_destination(
        &view,
        ctx.accounts.authority.key,
        ctx.accounts.realms_program.key,
        ctx.accounts.realm.key,
        ctx.accounts.realm.owner,
        &ctx.accounts.realm.data.borrow(),
        ctx.accounts.governance.key,
        ctx.accounts.governance.owner,
        &ctx.accounts.governance.data.borrow(),
        ctx.accounts.native_treasury.key,
    )?;
    let config = &mut ctx.accounts.config;
    config.realms_program = Some(*ctx.accounts.realms_program.key);
    config.realms_realm = Some(*ctx.accounts.realm.key);
    config.realms_governance = Some(*ctx.accounts.governance.key);
    config.realms_native_treasury = Some(*ctx.accounts.native_treasury.key);
    emit!(DaoDestinationCommitted {
        authority: ctx.accounts.authority.key(),
        realms_program: *ctx.accounts.realms_program.key,
        realms_realm: *ctx.accounts.realm.key,
        realms_governance: *ctx.accounts.governance.key,
        realms_native_treasury: *ctx.accounts.native_treasury.key,
    });
    Ok(())
}

pub fn handle_set_fee_distribution(
    ctx: Context<SetFeeDistribution>,
    creator_beneficiary: Pubkey,
) -> Result<()> {
    let view = ConfigView::from(&*ctx.accounts.config);
    check_set_fee_distribution(
        &view,
        ctx.accounts.authority.key,
        &creator_beneficiary,
        ctx.accounts.realms_program.key,
        ctx.accounts.realm.key,
        ctx.accounts.realm.owner,
        &ctx.accounts.realm.data.borrow(),
        ctx.accounts.governance.key,
        ctx.accounts.governance.owner,
        &ctx.accounts.governance.data.borrow(),
        ctx.accounts.native_treasury.key,
    )?;
    let config = &mut ctx.accounts.config;
    config.creator_beneficiary = Some(creator_beneficiary);
    config.fee_beneficiaries_bound = true;
    emit!(FeeDistributionBound {
        authority: ctx.accounts.authority.key(),
        creator_beneficiary,
        realms_program: *ctx.accounts.realms_program.key,
        realms_governance: *ctx.accounts.governance.key,
        realms_native_treasury: *ctx.accounts.native_treasury.key,
    });
    Ok(())
}

pub fn handle_distribute_transfer_fees(ctx: Context<DistributeTransferFees>) -> Result<()> {
    let view = ConfigView::from(&*ctx.accounts.config);
    let (expected_withdraw, _) = crate::constants::withdraw_withheld_pda(&crate::ID);
    let creator = ctx
        .accounts
        .config
        .creator_beneficiary
        .ok_or(IchorError::FeeBeneficiariesUnbound)?;
    let treasury = ctx
        .accounts
        .config
        .realms_native_treasury
        .ok_or(IchorError::FeeBeneficiariesUnbound)?;
    let expected_vault =
        anchor_spl::associated_token::get_associated_token_address_with_program_id(
            &expected_withdraw,
            ctx.accounts.ichor_mint.key,
            &TOKEN_2022_PROGRAM_ID,
        );
    let expected_creator =
        anchor_spl::associated_token::get_associated_token_address_with_program_id(
            &creator,
            ctx.accounts.ichor_mint.key,
            &TOKEN_2022_PROGRAM_ID,
        );
    let expected_realms =
        anchor_spl::associated_token::get_associated_token_address_with_program_id(
            &treasury,
            ctx.accounts.ichor_mint.key,
            &TOKEN_2022_PROGRAM_ID,
        );
    require_keys_eq!(
        ctx.accounts.fee_vault.key(),
        expected_vault,
        IchorError::FeeDestinationMismatch
    );
    require_keys_eq!(
        ctx.accounts.creator_escrow.key(),
        expected_creator,
        IchorError::FeeDestinationMismatch
    );
    require_keys_eq!(
        ctx.accounts.realms_destination.key(),
        expected_realms,
        IchorError::FeeDestinationMismatch
    );
    let outcome = check_distribute(
        &view,
        ctx.accounts.ichor_mint.key,
        ctx.accounts.ichor_mint.owner,
        &ctx.accounts.ichor_mint.data.borrow(),
        ctx.accounts.withdraw_withheld_authority.key,
        &expected_withdraw,
        ctx.accounts.fee_vault.owner,
        &ctx.accounts.fee_vault.data.borrow(),
        ctx.accounts.creator_escrow.owner,
        &ctx.accounts.creator_escrow.data.borrow(),
        ctx.accounts.realms_destination.owner,
        &ctx.accounts.realms_destination.data.borrow(),
        ctx.accounts.token_2022_program.key,
    )?;

    if outcome.withheld_before > 0 {
        let withdraw_bump = [ctx.accounts.config.withdraw_withheld_bump];
        let withdraw_seeds: &[&[u8]] = &[WITHDRAW_WITHHELD_SEED, &withdraw_bump];
        transfer_fee::withdraw_withheld_tokens_from_mint(CpiContext::new_with_signer(
            ctx.accounts.token_2022_program.to_account_info(),
            WithdrawWithheldTokensFromMint {
                token_program_id: ctx.accounts.token_2022_program.to_account_info(),
                mint: ctx.accounts.ichor_mint.to_account_info(),
                destination: ctx.accounts.fee_vault.to_account_info(),
                authority: ctx.accounts.withdraw_withheld_authority.to_account_info(),
            },
            &[withdraw_seeds],
        ))?;
    }

    let vault_after = parse_token_account(&ctx.accounts.fee_vault.data.borrow())?;
    let withdrawn = vault_after
        .amount
        .checked_sub(outcome.vault_before)
        .ok_or(IchorError::ArithmeticOverflow)?;
    let (creator_amount, realms_amount) = split_harvested_fees(vault_after.amount)?;

    let withdraw_bump = [ctx.accounts.config.withdraw_withheld_bump];
    let withdraw_seeds: &[&[u8]] = &[WITHDRAW_WITHHELD_SEED, &withdraw_bump];
    let withdraw_signer = &[withdraw_seeds];

    if creator_amount > 0 {
        token_2022::transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_2022_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.fee_vault.to_account_info(),
                    mint: ctx.accounts.ichor_mint.to_account_info(),
                    to: ctx.accounts.creator_escrow.to_account_info(),
                    authority: ctx.accounts.withdraw_withheld_authority.to_account_info(),
                },
                withdraw_signer,
            ),
            creator_amount,
            outcome.ichor_decimals,
        )?;
    }
    if realms_amount > 0 {
        token_2022::transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_2022_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.fee_vault.to_account_info(),
                    mint: ctx.accounts.ichor_mint.to_account_info(),
                    to: ctx.accounts.realms_destination.to_account_info(),
                    authority: ctx.accounts.withdraw_withheld_authority.to_account_info(),
                },
                withdraw_signer,
            ),
            realms_amount,
            outcome.ichor_decimals,
        )?;
    }

    let creator_after = parse_token_account(&ctx.accounts.creator_escrow.data.borrow())?;
    let realms_after = parse_token_account(&ctx.accounts.realms_destination.data.borrow())?;
    let creator_received = creator_after
        .amount
        .checked_sub(outcome.creator_before)
        .ok_or(IchorError::ArithmeticOverflow)?;
    let realms_received = realms_after
        .amount
        .checked_sub(outcome.realms_before)
        .ok_or(IchorError::ArithmeticOverflow)?;

    let config = &mut ctx.accounts.config;
    let (withdrawn_total, creator_total, realms_total) = add_fee_totals(
        config.total_fees_withdrawn,
        config.total_fees_to_creator,
        config.total_fees_to_realms,
        withdrawn,
        creator_received,
        realms_received,
    )?;
    config.total_fees_withdrawn = withdrawn_total;
    config.total_fees_to_creator = creator_total;
    config.total_fees_to_realms = realms_total;

    let remainder_recipient = config
        .realms_native_treasury
        .ok_or(IchorError::FeeBeneficiariesUnbound)?;
    emit!(TransferFeesDistributed {
        caller: ctx.accounts.caller.key(),
        withdrawn,
        vault_distributed: vault_after.amount,
        creator_amount,
        realms_amount,
        creator_received,
        realms_received,
        remainder_recipient,
        creator_destination: ctx.accounts.creator_escrow.key(),
        realms_destination: ctx.accounts.realms_destination.key(),
    });
    Ok(())
}

pub fn handle_set_transfer_fee(
    ctx: Context<SetTransferFee>,
    transfer_fee_basis_points: u16,
    maximum_fee: u64,
) -> Result<()> {
    require_ichor_mint(
        &ctx.accounts.config,
        ctx.accounts.ichor_mint.key,
        ctx.accounts.ichor_mint.owner,
    )?;
    require!(
        *ctx.accounts.token_2022_program.key == TOKEN_2022_PROGRAM_ID,
        IchorError::InvalidTokenProgram
    );
    let view = ConfigView::from(&*ctx.accounts.config);
    check_set_transfer_fee(
        &view,
        ctx.accounts.authority.key,
        transfer_fee_basis_points,
        maximum_fee,
    )?;
    let _ = parse_ichor_transfer_fee_config(&ctx.accounts.ichor_mint.data.borrow())?;

    let bump = [ctx.accounts.config.bump];
    let seeds: &[&[u8]] = &[Config::SEED, &bump];
    transfer_fee::transfer_fee_set(
        CpiContext::new_with_signer(
            ctx.accounts.token_2022_program.to_account_info(),
            TransferFeeSetTransferFee {
                token_program_id: ctx.accounts.token_2022_program.to_account_info(),
                mint: ctx.accounts.ichor_mint.to_account_info(),
                authority: ctx.accounts.config.to_account_info(),
            },
            &[seeds],
        ),
        transfer_fee_basis_points,
        maximum_fee,
    )?;

    emit!(TransferFeeUpdated {
        authority: ctx.accounts.authority.key(),
        transfer_fee_basis_points,
        maximum_fee,
    });
    Ok(())
}

pub fn handle_revoke_transfer_fee_authority(
    ctx: Context<RevokeTransferFeeAuthority>,
) -> Result<()> {
    require_ichor_mint(
        &ctx.accounts.config,
        ctx.accounts.ichor_mint.key,
        ctx.accounts.ichor_mint.owner,
    )?;
    require!(
        *ctx.accounts.token_2022_program.key == TOKEN_2022_PROGRAM_ID,
        IchorError::InvalidTokenProgram
    );
    let view = ConfigView::from(&*ctx.accounts.config);
    check_revoke_transfer_fee_authority(&view, ctx.accounts.authority.key)?;
    let _ = parse_ichor_transfer_fee_config(&ctx.accounts.ichor_mint.data.borrow())?;

    let bump = [ctx.accounts.config.bump];
    let seeds: &[&[u8]] = &[Config::SEED, &bump];
    token_2022::set_authority(
        CpiContext::new_with_signer(
            ctx.accounts.token_2022_program.to_account_info(),
            SetAuthority {
                current_authority: ctx.accounts.config.to_account_info(),
                account_or_mint: ctx.accounts.ichor_mint.to_account_info(),
            },
            &[seeds],
        ),
        anchor_spl::token_2022::spl_token_2022::instruction::AuthorityType::TransferFeeConfig,
        None,
    )?;

    ctx.accounts.config.transfer_fee_authority_revoked = true;
    emit!(TransferFeeAuthorityRevoked {
        authority: ctx.accounts.authority.key(),
    });
    Ok(())
}


#[derive(Accounts)]
pub struct ClaimCreatorFees<'info> {
    /// Current bound creator beneficiary. Only signer allowed.
    pub creator_beneficiary: Signer<'info>,

    #[account(
        mut,
        seeds = [Config::SEED],
        bump = config.bump
    )]
    pub config: Account<'info, Config>,

    /// CHECK: configured Token-2022 ICHOR mint.
    #[account(mut)]
    pub ichor_mint: UncheckedAccount<'info>,

    /// CHECK: creator-escrow PDA. Signs the transfer CPI.
    #[account(
        seeds = [CREATOR_ESCROW_SEED],
        bump = config.creator_escrow_bump
    )]
    pub creator_escrow_authority: UncheckedAccount<'info>,

    /// CHECK: Token-2022 ICHOR ATA owned by the escrow PDA.
    #[account(mut)]
    pub creator_escrow: UncheckedAccount<'info>,

    /// CHECK: Token-2022 ICHOR ATA owned by the current beneficiary.
    #[account(mut)]
    pub creator_destination: UncheckedAccount<'info>,

    /// CHECK: must be the Token-2022 program id.
    pub token_2022_program: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct SweepUnclaimedCreatorFees<'info> {
    /// Permissionless trigger.
    pub caller: Signer<'info>,

    #[account(
        mut,
        seeds = [Config::SEED],
        bump = config.bump
    )]
    pub config: Account<'info, Config>,

    /// CHECK: configured Token-2022 ICHOR mint.
    #[account(mut)]
    pub ichor_mint: UncheckedAccount<'info>,

    /// CHECK: creator-escrow PDA. Signs the transfer CPI.
    #[account(
        seeds = [CREATOR_ESCROW_SEED],
        bump = config.creator_escrow_bump
    )]
    pub creator_escrow_authority: UncheckedAccount<'info>,

    /// CHECK: Token-2022 ICHOR ATA owned by the escrow PDA.
    #[account(mut)]
    pub creator_escrow: UncheckedAccount<'info>,

    /// CHECK: Token-2022 ICHOR ATA owned by the bound Realms treasury.
    #[account(mut)]
    pub realms_destination: UncheckedAccount<'info>,

    /// CHECK: must be the Token-2022 program id.
    pub token_2022_program: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct SetCreatorBeneficiary<'info> {
    /// Current bound creator beneficiary. Config authority cannot sign this.
    pub creator_beneficiary: Signer<'info>,

    #[account(
        mut,
        seeds = [Config::SEED],
        bump = config.bump
    )]
    pub config: Account<'info, Config>,
}

pub fn handle_claim_creator_fees(ctx: Context<ClaimCreatorFees>) -> Result<()> {
    let view = ConfigView::from(&*ctx.accounts.config);
    let now = Clock::get()?.unix_timestamp;
    let amount = check_claim_creator_fees(
        &view,
        ctx.accounts.creator_beneficiary.key,
        now,
        ctx.accounts.ichor_mint.key,
        ctx.accounts.creator_escrow.owner,
        &ctx.accounts.creator_escrow.data.borrow(),
        ctx.accounts.creator_destination.owner,
        &ctx.accounts.creator_destination.data.borrow(),
        ctx.accounts.token_2022_program.key,
    )?;

    let (escrow_pda, escrow_bump) = creator_escrow_pda(&crate::ID);
    require!(
        escrow_bump == ctx.accounts.config.creator_escrow_bump,
        IchorError::FeeDestinationMismatch
    );
    let expected_escrow =
        anchor_spl::associated_token::get_associated_token_address_with_program_id(
            &escrow_pda,
            ctx.accounts.ichor_mint.key,
            &TOKEN_2022_PROGRAM_ID,
        );
    let creator = ctx
        .accounts
        .config
        .creator_beneficiary
        .ok_or(IchorError::FeeBeneficiariesUnbound)?;
    let expected_dest =
        anchor_spl::associated_token::get_associated_token_address_with_program_id(
            &creator,
            ctx.accounts.ichor_mint.key,
            &TOKEN_2022_PROGRAM_ID,
        );
    require_keys_eq!(
        ctx.accounts.creator_escrow.key(),
        expected_escrow,
        IchorError::FeeDestinationMismatch
    );
    require_keys_eq!(
        ctx.accounts.creator_destination.key(),
        expected_dest,
        IchorError::FeeDestinationMismatch
    );

    let dest_before = parse_token_account(&ctx.accounts.creator_destination.data.borrow())?.amount;
    let mint_view = parse_mint(&ctx.accounts.ichor_mint.data.borrow())?;
    let bump = [ctx.accounts.config.creator_escrow_bump];
    let seeds: &[&[u8]] = &[CREATOR_ESCROW_SEED, &bump];
    token_2022::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_2022_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.creator_escrow.to_account_info(),
                mint: ctx.accounts.ichor_mint.to_account_info(),
                to: ctx.accounts.creator_destination.to_account_info(),
                authority: ctx.accounts.creator_escrow_authority.to_account_info(),
            },
            &[seeds],
        ),
        amount,
        mint_view.decimals,
    )?;

    let dest_after = parse_token_account(&ctx.accounts.creator_destination.data.borrow())?.amount;
    let received = dest_after
        .checked_sub(dest_before)
        .ok_or(IchorError::ArithmeticOverflow)?;
    let config = &mut ctx.accounts.config;
    config.total_creator_claimed = config
        .total_creator_claimed
        .checked_add(received)
        .ok_or(IchorError::ArithmeticOverflow)?;
    config.last_creator_claim_ts = now;
    emit!(CreatorFeesClaimed {
        beneficiary: creator,
        amount,
        received,
        destination: ctx.accounts.creator_destination.key(),
        last_creator_claim_ts: now,
    });
    Ok(())
}

pub fn handle_sweep_unclaimed_creator_fees(ctx: Context<SweepUnclaimedCreatorFees>) -> Result<()> {
    let view = ConfigView::from(&*ctx.accounts.config);
    let now = Clock::get()?.unix_timestamp;
    let amount = check_sweep_unclaimed_creator_fees(
        &view,
        now,
        ctx.accounts.ichor_mint.key,
        ctx.accounts.creator_escrow.owner,
        &ctx.accounts.creator_escrow.data.borrow(),
        ctx.accounts.realms_destination.owner,
        &ctx.accounts.realms_destination.data.borrow(),
        ctx.accounts.token_2022_program.key,
    )?;

    let (escrow_pda, escrow_bump) = creator_escrow_pda(&crate::ID);
    require!(
        escrow_bump == ctx.accounts.config.creator_escrow_bump,
        IchorError::FeeDestinationMismatch
    );
    let expected_escrow =
        anchor_spl::associated_token::get_associated_token_address_with_program_id(
            &escrow_pda,
            ctx.accounts.ichor_mint.key,
            &TOKEN_2022_PROGRAM_ID,
        );
    let treasury = ctx
        .accounts
        .config
        .realms_native_treasury
        .ok_or(IchorError::FeeBeneficiariesUnbound)?;
    let expected_realms =
        anchor_spl::associated_token::get_associated_token_address_with_program_id(
            &treasury,
            ctx.accounts.ichor_mint.key,
            &TOKEN_2022_PROGRAM_ID,
        );
    require_keys_eq!(
        ctx.accounts.creator_escrow.key(),
        expected_escrow,
        IchorError::FeeDestinationMismatch
    );
    require_keys_eq!(
        ctx.accounts.realms_destination.key(),
        expected_realms,
        IchorError::FeeDestinationMismatch
    );

    let dest_before = parse_token_account(&ctx.accounts.realms_destination.data.borrow())?.amount;
    let mint_view = parse_mint(&ctx.accounts.ichor_mint.data.borrow())?;
    let bump = [ctx.accounts.config.creator_escrow_bump];
    let seeds: &[&[u8]] = &[CREATOR_ESCROW_SEED, &bump];
    token_2022::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_2022_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.creator_escrow.to_account_info(),
                mint: ctx.accounts.ichor_mint.to_account_info(),
                to: ctx.accounts.realms_destination.to_account_info(),
                authority: ctx.accounts.creator_escrow_authority.to_account_info(),
            },
            &[seeds],
        ),
        amount,
        mint_view.decimals,
    )?;

    let dest_after = parse_token_account(&ctx.accounts.realms_destination.data.borrow())?.amount;
    let received = dest_after
        .checked_sub(dest_before)
        .ok_or(IchorError::ArithmeticOverflow)?;
    let config = &mut ctx.accounts.config;
    config.total_creator_swept = config
        .total_creator_swept
        .checked_add(received)
        .ok_or(IchorError::ArithmeticOverflow)?;
    emit!(UnclaimedCreatorFeesSwept {
        caller: ctx.accounts.caller.key(),
        amount,
        received,
        realms_destination: ctx.accounts.realms_destination.key(),
        last_creator_claim_ts: config.last_creator_claim_ts,
    });
    Ok(())
}

pub fn handle_set_creator_beneficiary(
    ctx: Context<SetCreatorBeneficiary>,
    new_beneficiary: Pubkey,
) -> Result<()> {
    let view = ConfigView::from(&*ctx.accounts.config);
    check_set_creator_beneficiary(
        &view,
        ctx.accounts.creator_beneficiary.key,
        &new_beneficiary,
    )?;
    let now = Clock::get()?.unix_timestamp;
    crate::math::require_claim_clock(now, ctx.accounts.config.last_creator_claim_ts)?;
    let previous = ctx
        .accounts
        .config
        .creator_beneficiary
        .ok_or(IchorError::FeeBeneficiariesUnbound)?;
    let config = &mut ctx.accounts.config;
    config.creator_beneficiary = Some(new_beneficiary);
    config.last_creator_claim_ts = now;
    emit!(CreatorBeneficiarySet {
        previous_beneficiary: previous,
        new_beneficiary,
        last_creator_claim_ts: now,
    });
    Ok(())
}

