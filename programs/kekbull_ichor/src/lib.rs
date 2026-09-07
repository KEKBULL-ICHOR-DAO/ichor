//! Atomic KEKBULL (Token-2022) burn → ICHOR (Token-2022) mint.
//!
//! Program identity is supplied at build/deploy:
//!
//! ```text
//! declare_id!(env!("KEKBULL_ICHOR_PROGRAM_ID"));
//! ```
//!
//! There is no placeholder program ID and no keypair in this workspace.
//! `initialize` binds an *existing* Token-2022 ICHOR mint whose mint
//! authority is already the config PDA, freeze authority is none, and
//! TransferFeeConfig is exactly 25 bps / `u64::MAX` with fee-setting
//! authority = config PDA and withdraw-withheld authority = the dedicated
//! program PDA. There is no genesis mint, no treasury mint, and no
//! signature-replay account - issuance is the burn CPI succeeding in the
//! same instruction.
//!
//! Conversion is gated on the pump.fun bonding-curve 81-byte core:
//! `complete == true` and `real_token_reserves == 0`.

use anchor_lang::prelude::*;

pub mod constants;
pub mod curve;
pub mod error;
pub mod events;
pub mod extensions;
pub mod instructions;
pub mod loader;
pub mod math;
pub mod mint_layout;
pub mod state;
pub mod validate;

// Anchor 0.32 `#[program]` resolves `__client_accounts_*` / `__cpi_client_accounts_*`
// from the crate root (see solana-foundation/anchor#1871, #3690, #3811).
pub use instructions::*;

// Anchor 0.32.1's `declare_id!` accepts a string literal *or* a `Pubkey`
// expression. `env!` is `&str`, so it is const-converted. There is no
// placeholder program ID - identity is supplied at build/deploy.
declare_id!(
    ::anchor_lang::solana_program::pubkey::Pubkey::from_str_const(env!("KEKBULL_ICHOR_PROGRAM_ID"))
);

#[program]
pub mod kekbull_ichor {
    use super::*;

    /// Bind an existing zero-supply Token-2022 ICHOR mint (authority = config
    /// PDA; TransferFeeConfig exactly as approved) to a Token-2022 KEKBULL
    /// mint. Signer must be this program's upgrade authority. Does not mint.
    pub fn initialize(
        ctx: Context<Initialize>,
        emission_numerator: u64,
        emission_denominator: u64,
        ratio_timelock_secs: u64,
        creator_decay_secs: u64,
    ) -> Result<()> {
        instructions::initialize::handle_initialize(
            ctx,
            emission_numerator,
            emission_denominator,
            ratio_timelock_secs,
            creator_decay_secs,
        )
    }

    pub fn set_paused(ctx: Context<Admin>, paused: bool) -> Result<()> {
        instructions::admin::handle_set_paused(ctx, paused)
    }

    pub fn propose_emission_ratio(
        ctx: Context<Admin>,
        numerator: u64,
        denominator: u64,
    ) -> Result<()> {
        instructions::admin::handle_propose_emission_ratio(ctx, numerator, denominator)
    }

    pub fn apply_emission_ratio(ctx: Context<ApplyEmissionRatio>) -> Result<()> {
        instructions::admin::handle_apply_emission_ratio(ctx)
    }

    pub fn cancel_pending_ratio(ctx: Context<Admin>) -> Result<()> {
        instructions::admin::handle_cancel_pending_ratio(ctx)
    }

    pub fn freeze_ratio_updates(ctx: Context<Admin>) -> Result<()> {
        instructions::admin::handle_freeze_ratio_updates(ctx)
    }

    pub fn increase_ratio_timelock(ctx: Context<Admin>, new_secs: u64) -> Result<()> {
        instructions::admin::handle_increase_ratio_timelock(ctx, new_secs)
    }

    pub fn set_pending_authority(ctx: Context<Admin>, pending: Option<Pubkey>) -> Result<()> {
        instructions::admin::handle_set_pending_authority(ctx, pending)
    }

    pub fn accept_authority(ctx: Context<AcceptAuthority>) -> Result<()> {
        instructions::admin::handle_accept_authority(ctx)
    }

    /// BurnChecked KEKBULL + MintToChecked ICHOR. Both CPIs are Token-2022.
    /// Output is computed before either CPI and rejected if it is below
    /// `min_ichor_amount`.
    pub fn convert(
        ctx: Context<Convert>,
        kekbull_amount: u64,
        min_ichor_amount: u64,
    ) -> Result<()> {
        instructions::convert::handle_convert(ctx, kekbull_amount, min_ichor_amount)
    }

    /// Commit the GovER5 realm, Governance PDA, and native treasury before
    /// fee bind or position-rights handoff. Irreversible. Split weights stay
    /// program constants; this does not bind the creator beneficiary.
    pub fn commit_dao_destination(ctx: Context<CommitDaoDestination>) -> Result<()> {
        instructions::fees::handle_commit_dao_destination(ctx)
    }

    /// Bind the creator wallet to the already-committed Realms destination.
    /// Accounts must match the on-chain commitment. Split weights are program
    /// constants, not caller-selected.
    pub fn set_fee_distribution(
        ctx: Context<SetFeeDistribution>,
        creator_beneficiary: Pubkey,
    ) -> Result<()> {
        instructions::fees::handle_set_fee_distribution(ctx, creator_beneficiary)
    }

    /// Permissionless: withdraw mint-withheld ICHOR via the dedicated PDA,
    /// measure the vault delta, and transfer 25/75 to the bound destinations.
    pub fn distribute_transfer_fees(ctx: Context<DistributeTransferFees>) -> Result<()> {
        instructions::fees::handle_distribute_transfer_fees(ctx)
    }

    /// Pilot-only: reassert the fixed Token-2022 transfer fee at 25 bps /
    /// `u64::MAX`. Fails after `revoke_transfer_fee_authority`.
    pub fn set_transfer_fee(
        ctx: Context<SetTransferFee>,
        transfer_fee_basis_points: u16,
        maximum_fee: u64,
    ) -> Result<()> {
        instructions::fees::handle_set_transfer_fee(ctx, transfer_fee_basis_points, maximum_fee)
    }

    /// Permanently revoke TransferFeeConfig authority after governance
    /// acceptance. Irreversible.
    pub fn revoke_transfer_fee_authority(ctx: Context<RevokeTransferFeeAuthority>) -> Result<()> {
        instructions::fees::handle_revoke_transfer_fee_authority(ctx)
    }

    /// Signed by the current creator beneficiary only. Moves creator-escrow
    /// ICHOR to the beneficiary's canonical Token-2022 ATA and stamps liveness.
    pub fn claim_creator_fees(ctx: Context<ClaimCreatorFees>) -> Result<()> {
        instructions::fees::handle_claim_creator_fees(ctx)
    }

    /// Permissionless after `creator_decay_secs` with no claim. Moves escrow
    /// to the bound Realms native-treasury ATA. Does not change the beneficiary.
    pub fn sweep_unclaimed_creator_fees(ctx: Context<SweepUnclaimedCreatorFees>) -> Result<()> {
        instructions::fees::handle_sweep_unclaimed_creator_fees(ctx)
    }

    /// Signed by the current creator beneficiary only. Never by config.authority.
    /// Rotating the wallet is itself proof of liveness.
    pub fn set_creator_beneficiary(
        ctx: Context<SetCreatorBeneficiary>,
        new_beneficiary: Pubkey,
    ) -> Result<()> {
        instructions::fees::handle_set_creator_beneficiary(ctx, new_beneficiary)
    }
}
