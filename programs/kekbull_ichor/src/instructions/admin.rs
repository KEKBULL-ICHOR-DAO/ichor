use anchor_lang::prelude::*;

use crate::error::IchorError;
use crate::events::{
    AuthorityAccepted, EmissionRatioApplied, EmissionRatioCancelled, EmissionRatioProposed,
    PauseSet, PendingAuthoritySet, RatioTimelockIncreased, RatioUpdatesFrozenEvent,
};
use crate::state::{Config, ConfigView};
use crate::validate::{
    check_accept_authority, check_apply_ratio, check_increase_timelock, check_propose_ratio,
    check_set_pending_authority,
};

#[derive(Accounts)]
pub struct Admin<'info> {
    pub authority: Signer<'info>,

    #[account(
        mut,
        seeds = [Config::SEED],
        bump = config.bump,
        has_one = authority @ IchorError::Unauthorized
    )]
    pub config: Account<'info, Config>,
}

#[derive(Accounts)]
pub struct ApplyEmissionRatio<'info> {
    #[account(
        mut,
        seeds = [Config::SEED],
        bump = config.bump
    )]
    pub config: Account<'info, Config>,
}

#[derive(Accounts)]
pub struct AcceptAuthority<'info> {
    pub pending_authority: Signer<'info>,

    #[account(
        mut,
        seeds = [Config::SEED],
        bump = config.bump
    )]
    pub config: Account<'info, Config>,
}

pub fn handle_set_paused(ctx: Context<Admin>, paused: bool) -> Result<()> {
    ctx.accounts.config.paused = paused;
    emit!(PauseSet {
        authority: ctx.accounts.authority.key(),
        paused,
    });
    Ok(())
}

pub fn handle_propose_emission_ratio(
    ctx: Context<Admin>,
    numerator: u64,
    denominator: u64,
) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let view = ConfigView::from(&*ctx.accounts.config);
    let unlock = check_propose_ratio(
        &view,
        ctx.accounts.authority.key,
        numerator,
        denominator,
        now,
    )?;
    let config = &mut ctx.accounts.config;
    config.pending_emission_numerator = numerator;
    config.pending_emission_denominator = denominator;
    config.pending_ratio_unlock_ts = unlock;
    config.has_pending_ratio = true;
    emit!(EmissionRatioProposed {
        authority: ctx.accounts.authority.key(),
        numerator,
        denominator,
        unlock_ts: unlock,
    });
    Ok(())
}

pub fn handle_apply_emission_ratio(ctx: Context<ApplyEmissionRatio>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let view = ConfigView::from(&*ctx.accounts.config);
    let (numerator, denominator) = check_apply_ratio(&view, now)?;
    let config = &mut ctx.accounts.config;
    config.emission_numerator = numerator;
    config.emission_denominator = denominator;
    config.pending_emission_numerator = 0;
    config.pending_emission_denominator = 0;
    config.pending_ratio_unlock_ts = 0;
    config.has_pending_ratio = false;
    emit!(EmissionRatioApplied {
        numerator,
        denominator,
    });
    Ok(())
}

pub fn handle_cancel_pending_ratio(ctx: Context<Admin>) -> Result<()> {
    require!(
        ctx.accounts.config.has_pending_ratio,
        IchorError::NoPendingRatio
    );
    let config = &mut ctx.accounts.config;
    config.pending_emission_numerator = 0;
    config.pending_emission_denominator = 0;
    config.pending_ratio_unlock_ts = 0;
    config.has_pending_ratio = false;
    emit!(EmissionRatioCancelled {
        authority: ctx.accounts.authority.key(),
    });
    Ok(())
}

pub fn handle_freeze_ratio_updates(ctx: Context<Admin>) -> Result<()> {
    ctx.accounts.config.ratio_updates_frozen = true;
    ctx.accounts.config.has_pending_ratio = false;
    ctx.accounts.config.pending_emission_numerator = 0;
    ctx.accounts.config.pending_emission_denominator = 0;
    ctx.accounts.config.pending_ratio_unlock_ts = 0;
    emit!(RatioUpdatesFrozenEvent {
        authority: ctx.accounts.authority.key(),
    });
    Ok(())
}

pub fn handle_increase_ratio_timelock(ctx: Context<Admin>, new_secs: u64) -> Result<()> {
    let view = ConfigView::from(&*ctx.accounts.config);
    check_increase_timelock(&view, ctx.accounts.authority.key, new_secs)?;
    ctx.accounts.config.ratio_timelock_secs = new_secs;
    emit!(RatioTimelockIncreased {
        authority: ctx.accounts.authority.key(),
        ratio_timelock_secs: new_secs,
    });
    Ok(())
}

pub fn handle_set_pending_authority(ctx: Context<Admin>, pending: Option<Pubkey>) -> Result<()> {
    check_set_pending_authority(pending)?;
    ctx.accounts.config.pending_authority = pending;
    emit!(PendingAuthoritySet {
        authority: ctx.accounts.authority.key(),
        pending_authority: pending,
    });
    Ok(())
}

pub fn handle_accept_authority(ctx: Context<AcceptAuthority>) -> Result<()> {
    let view = ConfigView::from(&*ctx.accounts.config);
    check_accept_authority(&view, ctx.accounts.pending_authority.key)?;
    ctx.accounts.config.authority = ctx.accounts.pending_authority.key();
    ctx.accounts.config.pending_authority = None;
    emit!(AuthorityAccepted {
        authority: ctx.accounts.config.authority,
    });
    Ok(())
}
