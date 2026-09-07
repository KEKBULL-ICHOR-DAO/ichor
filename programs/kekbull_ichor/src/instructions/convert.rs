use anchor_lang::prelude::*;
use anchor_spl::token_2022::{self, BurnChecked, MintToChecked};

use crate::events::Converted;
use crate::state::{add_totals, Config, ConfigView};
use crate::validate::check_convert;

#[derive(Accounts)]
pub struct Convert<'info> {
    pub burner: Signer<'info>,

    #[account(
        mut,
        seeds = [Config::SEED],
        bump = config.bump
    )]
    pub config: Account<'info, Config>,

    /// CHECK: must be the configured Token-2022 KEKBULL mint.
    #[account(mut)]
    pub kekbull_mint: UncheckedAccount<'info>,

    /// CHECK: must be the configured Token-2022 ICHOR mint.
    #[account(mut)]
    pub ichor_mint: UncheckedAccount<'info>,

    /// CHECK: Token-2022 token account, mint KEKBULL, owner = burner.
    #[account(mut)]
    pub kekbull_from: UncheckedAccount<'info>,

    /// CHECK: Token-2022 token account, mint ICHOR. Owner is the recipient.
    /// The burner signs this transaction and chooses `ichor_to`; the event
    /// records that owner. Arbitrary recipient is intentional.
    #[account(mut)]
    pub ichor_to: UncheckedAccount<'info>,

    /// CHECK: canonical pump.fun bonding-curve PDA; owner and 81-byte core
    /// validated in the handler.
    pub bonding_curve: UncheckedAccount<'info>,

    /// CHECK: must be the Token-2022 program id.
    pub token_2022_program: UncheckedAccount<'info>,

    /// CHECK: must be the Token-2022 program id (ICHOR mint CPI).
    pub token_program: UncheckedAccount<'info>,
}

pub fn handle_convert(
    ctx: Context<Convert>,
    kekbull_amount: u64,
    min_ichor_amount: u64,
) -> Result<()> {
    let view = ConfigView::from(&*ctx.accounts.config);
    let outcome = check_convert(
        &view,
        ctx.accounts.burner.key,
        ctx.accounts.kekbull_mint.key,
        ctx.accounts.kekbull_mint.owner,
        &ctx.accounts.kekbull_mint.data.borrow(),
        ctx.accounts.ichor_mint.key,
        ctx.accounts.ichor_mint.owner,
        &ctx.accounts.ichor_mint.data.borrow(),
        ctx.accounts.kekbull_from.owner,
        &ctx.accounts.kekbull_from.data.borrow(),
        ctx.accounts.ichor_to.owner,
        &ctx.accounts.ichor_to.data.borrow(),
        ctx.accounts.bonding_curve.key,
        ctx.accounts.bonding_curve.owner,
        &ctx.accounts.bonding_curve.data.borrow(),
        ctx.accounts.token_2022_program.key,
        ctx.accounts.token_program.key,
        kekbull_amount,
        min_ichor_amount,
    )?;

    token_2022::burn_checked(
        CpiContext::new(
            ctx.accounts.token_2022_program.to_account_info(),
            BurnChecked {
                mint: ctx.accounts.kekbull_mint.to_account_info(),
                from: ctx.accounts.kekbull_from.to_account_info(),
                authority: ctx.accounts.burner.to_account_info(),
            },
        ),
        outcome.kekbull_amount,
        outcome.kekbull_decimals,
    )?;

    let bump = [ctx.accounts.config.bump];
    let seeds: &[&[u8]] = &[Config::SEED, &bump];
    let signer = &[seeds];

    token_2022::mint_to_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            MintToChecked {
                mint: ctx.accounts.ichor_mint.to_account_info(),
                to: ctx.accounts.ichor_to.to_account_info(),
                authority: ctx.accounts.config.to_account_info(),
            },
            signer,
        ),
        outcome.ichor_amount,
        outcome.ichor_decimals,
    )?;

    let config = &mut ctx.accounts.config;
    let (burned, minted) = add_totals(
        config.total_kekbull_burned,
        config.total_ichor_minted,
        outcome.kekbull_amount,
        outcome.ichor_amount,
    )?;
    config.total_kekbull_burned = burned;
    config.total_ichor_minted = minted;

    emit!(Converted {
        burner: ctx.accounts.burner.key(),
        recipient: outcome.recipient,
        kekbull_mint: config.kekbull_mint,
        ichor_mint: config.ichor_mint,
        kekbull_burned: outcome.kekbull_amount,
        ichor_minted: outcome.ichor_amount,
        min_ichor_amount: outcome.min_ichor_amount,
        ratio_numerator: config.emission_numerator,
        ratio_denominator: config.emission_denominator,
    });
    Ok(())
}
