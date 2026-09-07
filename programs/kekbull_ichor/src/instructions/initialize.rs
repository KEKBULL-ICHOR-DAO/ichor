use anchor_lang::prelude::*;

use crate::constants::{
    bonding_curve_pda, withdraw_withheld_pda, PUMP_PROGRAM, TOKEN_2022_PROGRAM_ID,
    CREATOR_ESCROW_SEED, WITHDRAW_WITHHELD_SEED,
};
use crate::events::Initialized;
use crate::loader::require_upgrade_authority;
use crate::math::{ratio_frozen_at_initialize, require_creator_decay_secs};
use crate::state::{paused_at_initialize, Config};
use crate::validate::check_initialize;

#[derive(Accounts)]
pub struct Initialize<'info> {
    /// Must be the deployed program's upgrade authority. Not a hardcoded key.
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        init,
        payer = authority,
        space = Config::space(),
        seeds = [Config::SEED],
        bump
    )]
    pub config: Account<'info, Config>,

    /// CHECK: Token-2022 KEKBULL mint. Owner and layout validated in handler.
    pub kekbull_mint: UncheckedAccount<'info>,

    /// CHECK: existing Token-2022 ICHOR mint. Mint authority must already be
    /// the config PDA; freeze none; TransferFeeConfig exactly 25 bps /
    /// u64::MAX with fee authority = config PDA and withdraw-withheld =
    /// the dedicated PDA. Supply must be zero. This instruction never mints.
    pub ichor_mint: UncheckedAccount<'info>,

    /// CHECK: withdraw-withheld PDA. Address is derived; the account need
    /// not exist. Used to bind and store the bump.
    #[account(seeds = [WITHDRAW_WITHHELD_SEED], bump)]
    pub withdraw_withheld_authority: UncheckedAccount<'info>,

    /// CHECK: creator-escrow PDA. Address is derived; the account need
    /// not exist. Used to bind and store the bump. The Token-2022 ATA
    /// owned by this PDA is created by the client before first distribute.
    #[account(seeds = [CREATOR_ESCROW_SEED], bump)]
    pub creator_escrow_authority: UncheckedAccount<'info>,

    /// CHECK: canonical pump.fun bonding-curve PDA for `kekbull_mint`.
    pub bonding_curve: UncheckedAccount<'info>,

    /// CHECK: this program's executable account. Address pinned to declare_id.
    #[account(address = crate::ID)]
    pub program_account: UncheckedAccount<'info>,

    /// CHECK: ProgramData for this program. Loader state validated in handler.
    pub program_data: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handle_initialize(
    ctx: Context<Initialize>,
    emission_numerator: u64,
    emission_denominator: u64,
    ratio_timelock_secs: u64,
    creator_decay_secs: u64,
) -> Result<()> {
    require_creator_decay_secs(creator_decay_secs)?;
    require_upgrade_authority(
        &crate::ID,
        ctx.accounts.program_account.key,
        ctx.accounts.program_account.owner,
        ctx.accounts.program_account.executable,
        &ctx.accounts.program_account.data.borrow(),
        ctx.accounts.program_data.key,
        ctx.accounts.program_data.owner,
        &ctx.accounts.program_data.data.borrow(),
        ctx.accounts.authority.key,
    )?;

    let config_key = ctx.accounts.config.key();
    check_initialize(
        ctx.accounts.kekbull_mint.key,
        ctx.accounts.kekbull_mint.owner,
        &ctx.accounts.kekbull_mint.data.borrow(),
        ctx.accounts.ichor_mint.key,
        ctx.accounts.ichor_mint.owner,
        &ctx.accounts.ichor_mint.data.borrow(),
        &config_key,
        &withdraw_withheld_pda(&crate::ID).0,
        ctx.accounts.bonding_curve.key,
        ctx.accounts.bonding_curve.owner,
        &ctx.accounts.bonding_curve.data.borrow(),
        emission_numerator,
        emission_denominator,
    )?;

    let (canonical, _) = bonding_curve_pda(ctx.accounts.kekbull_mint.key);

    let config = &mut ctx.accounts.config;
    config.authority = ctx.accounts.authority.key();
    config.pending_authority = None;
    config.kekbull_mint = ctx.accounts.kekbull_mint.key();
    config.ichor_mint = ctx.accounts.ichor_mint.key();
    config.kekbull_token_program = TOKEN_2022_PROGRAM_ID;
    config.ichor_token_program = TOKEN_2022_PROGRAM_ID;
    config.pump_program = PUMP_PROGRAM;
    config.bonding_curve = canonical;
    config.emission_numerator = emission_numerator;
    config.emission_denominator = emission_denominator;
    config.pending_emission_numerator = 0;
    config.pending_emission_denominator = 0;
    config.pending_ratio_unlock_ts = 0;
    config.ratio_timelock_secs = ratio_timelock_secs;
    config.paused = paused_at_initialize();
    config.ratio_updates_frozen = ratio_frozen_at_initialize(ratio_timelock_secs);
    config.has_pending_ratio = false;
    config.total_kekbull_burned = 0;
    config.total_ichor_minted = 0;
    config.creator_beneficiary = None;
    config.realms_program = None;
    config.realms_realm = None;
    config.realms_governance = None;
    config.realms_native_treasury = None;
    config.fee_beneficiaries_bound = false;
    config.transfer_fee_authority_revoked = false;
    config.withdraw_withheld_bump = ctx.bumps.withdraw_withheld_authority;
    config.total_fees_withdrawn = 0;
    config.total_fees_to_creator = 0;
    config.total_fees_to_realms = 0;
    config.creator_escrow_bump = ctx.bumps.creator_escrow_authority;
    config.last_creator_claim_ts = Clock::get()?.unix_timestamp;
    config.creator_decay_secs = creator_decay_secs;
    config.total_creator_claimed = 0;
    config.total_creator_swept = 0;
    config.bump = ctx.bumps.config;
    config.reserved = [0u8; 32];

    emit!(Initialized {
        authority: config.authority,
        kekbull_mint: config.kekbull_mint,
        ichor_mint: config.ichor_mint,
        bonding_curve: config.bonding_curve,
        emission_numerator,
        emission_denominator,
        ratio_timelock_secs,
        creator_decay_secs,
    });
    Ok(())
}
