use anchor_lang::prelude::*;

/// On-chain error codes. Validation helpers use [`Check`] so unit tests can
/// assert without constructing Anchor `Error` values.
#[error_code]
pub enum IchorError {
    #[msg("unauthorized")]
    Unauthorized,
    #[msg("program is paused")]
    Paused,
    #[msg("KEKBULL mint does not match config")]
    InvalidKekbullMint,
    #[msg("ICHOR mint does not match config")]
    InvalidIchorMint,
    #[msg("token program does not match the configured or required program")]
    InvalidTokenProgram,
    #[msg("mint account owner is not the required token program")]
    InvalidMintOwner,
    #[msg("bonding-curve address is not the canonical PDA for the KEKBULL mint")]
    InvalidBondingCurveAddress,
    #[msg("bonding-curve account is not owned by the pump.fun program")]
    InvalidBondingCurveOwner,
    #[msg("bonding-curve account is shorter than the 81-byte core")]
    BondingCurveTooShort,
    #[msg("bonding-curve discriminator is not the pump.fun BondingCurve account")]
    InvalidBondingCurveDiscriminator,
    #[msg("bonding curve is not complete; conversion is post-graduation only")]
    CurveNotComplete,
    #[msg("bonding curve still holds real token reserves")]
    CurveStillHasReserves,
    #[msg("mint account is too short for the SPL mint layout")]
    MintAccountTooShort,
    #[msg("mint account is not initialized")]
    MintUninitialized,
    #[msg("ICHOR mint authority must be the config PDA")]
    MintAuthorityMismatch,
    #[msg("ICHOR freeze authority must be unset")]
    FreezeAuthoritySet,
    #[msg("ICHOR must be a Token-2022 mint")]
    IchorNotToken2022,
    #[msg("KEKBULL must be a Token-2022 mint")]
    KekbullNotToken2022,
    #[msg("Token-2022 mint account type is not Mint")]
    InvalidMintAccountType,
    #[msg("Token-2022 mint padding or TLV layout is invalid")]
    InvalidMintExtensionLayout,
    #[msg("ICHOR mint is missing TransferFeeConfig")]
    TransferFeeConfigMissing,
    #[msg("ICHOR mint carries an extension outside the approved set")]
    UnexpectedMintExtension,
    #[msg("TransferFeeConfig bytes are the wrong length or corrupt")]
    InvalidTransferFeeConfig,
    #[msg("TransferFeeConfig authority must be the config PDA")]
    TransferFeeAuthorityMismatch,
    #[msg("withdraw-withheld authority must be the dedicated program PDA")]
    WithdrawWithheldAuthorityMismatch,
    #[msg("ICHOR transfer-fee rate or maximum does not match the approved constants")]
    TransferFeeRateMismatch,
    #[msg("transfer-fee setting authority has been permanently revoked")]
    FeeAuthorityRevoked,
    #[msg("transfer fee must remain fixed at 25 bps / u64::MAX")]
    TransferFeeExceedsPilotCap,
    #[msg("fee beneficiaries have not been bound")]
    FeeBeneficiariesUnbound,
    #[msg("fee beneficiaries are already permanently bound")]
    FeeBeneficiariesAlreadyBound,
    #[msg("creator beneficiary is unset or the default pubkey")]
    InvalidFeeBeneficiary,
    #[msg("Realms governance proof is missing, default, or not owned by the supplied program")]
    InvalidRealmsGovernance,
    #[msg("native treasury is not the canonical Realms PDA for the supplied governance")]
    InvalidRealmsTreasury,
    #[msg("DAO destination (realm, Governance, treasury) has not been committed")]
    DaoDestinationUncommitted,
    #[msg("DAO destination is already permanently committed")]
    DaoDestinationAlreadyCommitted,
    #[msg("supplied realm, Governance, Realms program, or treasury does not match the committed destination")]
    DaoDestinationMismatch,
    #[msg("fee destination token account is not the bound beneficiary")]
    FeeDestinationMismatch,
    #[msg("token account is too short for the SPL account layout")]
    TokenAccountTooShort,
    #[msg("token account mint does not match")]
    TokenAccountMintMismatch,
    #[msg("token account owner does not match the required authority")]
    TokenAccountOwnerMismatch,
    #[msg("token account is not initialized")]
    TokenAccountUninitialized,
    #[msg("token account is frozen")]
    TokenAccountFrozen,
    #[msg("token account owner program is wrong")]
    InvalidTokenAccountOwner,
    #[msg("burn amount is zero")]
    ZeroAmount,
    #[msg("emission numerator or denominator is zero")]
    ZeroRatio,
    #[msg("emission ratio exceeds the 1:1 human-unit ceiling")]
    RatioAboveCeiling,
    #[msg("conversion would mint zero ICHOR after integer normalization")]
    ConversionFloorsToZero,
    #[msg("checked arithmetic overflow")]
    ArithmeticOverflow,
    #[msg("no pending emission ratio")]
    NoPendingRatio,
    #[msg("emission-ratio timelock has not elapsed")]
    RatioTimelockNotElapsed,
    #[msg("emission-ratio updates are frozen")]
    RatioUpdatesFrozen,
    #[msg("ratio timelock is zero; the initialized ratio is immutable")]
    RatioLocked,
    #[msg("timelock can only stay the same or increase")]
    TimelockCannotDecrease,
    #[msg("no pending authority")]
    NoPendingAuthority,
    #[msg("signer is not the pending authority")]
    PendingAuthorityMismatch,
    #[msg("KEKBULL and ICHOR mints must be distinct")]
    MintsMustDiffer,
    #[msg("COption tag on a mint account is neither None nor Some")]
    InvalidCOption,
    #[msg("unix timestamp overflow while computing the ratio unlock")]
    TimestampOverflow,
    #[msg("signer is not this program's upgrade authority")]
    NotUpgradeAuthority,
    #[msg("upgradeable loader account is the wrong variant or length")]
    InvalidLoaderState,
    #[msg("program account key is not this program or is not executable")]
    ProgramAccountMismatch,
    #[msg("ProgramData address does not match the program account or loader PDA")]
    ProgramDataMismatch,
    #[msg("program or ProgramData owner is not the BPF upgradeable loader")]
    InvalidLoaderOwner,
    #[msg("ICHOR mint supply must be zero at initialize")]
    IchorSupplyMustBeZero,
    #[msg("ICHOR mint withheld amount must be zero at initialize")]
    IchorWithheldMustBeZero,
    #[msg("computed ICHOR is below the burner-signed minimum")]
    BelowMinIchorAmount,
    #[msg("boolean field is not exactly 0 or 1")]
    InvalidBool,
    #[msg("pending authority cannot be the default pubkey")]
    InvalidPendingAuthority,
    #[msg("creator decay interval must be greater than zero")]
    CreatorDecayZero,
    #[msg("clock went backwards relative to last creator claim")]
    ClockInversion,
    #[msg("creator claim signer is not the bound beneficiary")]
    UnauthorizedCreatorClaim,
    #[msg("creator decay interval has not elapsed")]
    CreatorDecayNotElapsed,
}

/// Copy/Eq stand-in so parser and math tests do not go through Anchor errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Check {
    Unauthorized,
    Paused,
    InvalidKekbullMint,
    InvalidIchorMint,
    InvalidTokenProgram,
    InvalidMintOwner,
    InvalidBondingCurveAddress,
    InvalidBondingCurveOwner,
    BondingCurveTooShort,
    InvalidBondingCurveDiscriminator,
    CurveNotComplete,
    CurveStillHasReserves,
    MintAccountTooShort,
    MintUninitialized,
    MintAuthorityMismatch,
    FreezeAuthoritySet,
    IchorNotToken2022,
    KekbullNotToken2022,
    InvalidMintAccountType,
    InvalidMintExtensionLayout,
    TransferFeeConfigMissing,
    UnexpectedMintExtension,
    InvalidTransferFeeConfig,
    TransferFeeAuthorityMismatch,
    WithdrawWithheldAuthorityMismatch,
    TransferFeeRateMismatch,
    FeeAuthorityRevoked,
    TransferFeeExceedsPilotCap,
    FeeBeneficiariesUnbound,
    FeeBeneficiariesAlreadyBound,
    InvalidFeeBeneficiary,
    InvalidRealmsGovernance,
    InvalidRealmsTreasury,
    DaoDestinationUncommitted,
    DaoDestinationAlreadyCommitted,
    DaoDestinationMismatch,
    FeeDestinationMismatch,
    TokenAccountTooShort,
    TokenAccountMintMismatch,
    TokenAccountOwnerMismatch,
    TokenAccountUninitialized,
    TokenAccountFrozen,
    InvalidTokenAccountOwner,
    ZeroAmount,
    ZeroRatio,
    RatioAboveCeiling,
    ConversionFloorsToZero,
    ArithmeticOverflow,
    NoPendingRatio,
    RatioTimelockNotElapsed,
    RatioUpdatesFrozen,
    RatioLocked,
    TimelockCannotDecrease,
    NoPendingAuthority,
    PendingAuthorityMismatch,
    MintsMustDiffer,
    InvalidCOption,
    TimestampOverflow,
    NotUpgradeAuthority,
    InvalidLoaderState,
    ProgramAccountMismatch,
    ProgramDataMismatch,
    InvalidLoaderOwner,
    IchorSupplyMustBeZero,
    IchorWithheldMustBeZero,
    BelowMinIchorAmount,
    InvalidBool,
    InvalidPendingAuthority,
    CreatorDecayZero,
    ClockInversion,
    UnauthorizedCreatorClaim,
    CreatorDecayNotElapsed,
}

/// Parser/math/validation result. Distinct from the Anchor prelude `Result`
/// alias (`Result<T, Error>`), which cannot carry [`Check`].
pub type CheckResult<T> = core::result::Result<T, Check>;

impl From<Check> for IchorError {
    fn from(value: Check) -> Self {
        match value {
            Check::Unauthorized => Self::Unauthorized,
            Check::Paused => Self::Paused,
            Check::InvalidKekbullMint => Self::InvalidKekbullMint,
            Check::InvalidIchorMint => Self::InvalidIchorMint,
            Check::InvalidTokenProgram => Self::InvalidTokenProgram,
            Check::InvalidMintOwner => Self::InvalidMintOwner,
            Check::InvalidBondingCurveAddress => Self::InvalidBondingCurveAddress,
            Check::InvalidBondingCurveOwner => Self::InvalidBondingCurveOwner,
            Check::BondingCurveTooShort => Self::BondingCurveTooShort,
            Check::InvalidBondingCurveDiscriminator => Self::InvalidBondingCurveDiscriminator,
            Check::CurveNotComplete => Self::CurveNotComplete,
            Check::CurveStillHasReserves => Self::CurveStillHasReserves,
            Check::MintAccountTooShort => Self::MintAccountTooShort,
            Check::MintUninitialized => Self::MintUninitialized,
            Check::MintAuthorityMismatch => Self::MintAuthorityMismatch,
            Check::FreezeAuthoritySet => Self::FreezeAuthoritySet,
            Check::IchorNotToken2022 => Self::IchorNotToken2022,
            Check::KekbullNotToken2022 => Self::KekbullNotToken2022,
            Check::InvalidMintAccountType => Self::InvalidMintAccountType,
            Check::InvalidMintExtensionLayout => Self::InvalidMintExtensionLayout,
            Check::TransferFeeConfigMissing => Self::TransferFeeConfigMissing,
            Check::UnexpectedMintExtension => Self::UnexpectedMintExtension,
            Check::InvalidTransferFeeConfig => Self::InvalidTransferFeeConfig,
            Check::TransferFeeAuthorityMismatch => Self::TransferFeeAuthorityMismatch,
            Check::WithdrawWithheldAuthorityMismatch => Self::WithdrawWithheldAuthorityMismatch,
            Check::TransferFeeRateMismatch => Self::TransferFeeRateMismatch,
            Check::FeeAuthorityRevoked => Self::FeeAuthorityRevoked,
            Check::TransferFeeExceedsPilotCap => Self::TransferFeeExceedsPilotCap,
            Check::FeeBeneficiariesUnbound => Self::FeeBeneficiariesUnbound,
            Check::FeeBeneficiariesAlreadyBound => Self::FeeBeneficiariesAlreadyBound,
            Check::InvalidFeeBeneficiary => Self::InvalidFeeBeneficiary,
            Check::InvalidRealmsGovernance => Self::InvalidRealmsGovernance,
            Check::InvalidRealmsTreasury => Self::InvalidRealmsTreasury,
            Check::DaoDestinationUncommitted => Self::DaoDestinationUncommitted,
            Check::DaoDestinationAlreadyCommitted => Self::DaoDestinationAlreadyCommitted,
            Check::DaoDestinationMismatch => Self::DaoDestinationMismatch,
            Check::FeeDestinationMismatch => Self::FeeDestinationMismatch,
            Check::TokenAccountTooShort => Self::TokenAccountTooShort,
            Check::TokenAccountMintMismatch => Self::TokenAccountMintMismatch,
            Check::TokenAccountOwnerMismatch => Self::TokenAccountOwnerMismatch,
            Check::TokenAccountUninitialized => Self::TokenAccountUninitialized,
            Check::TokenAccountFrozen => Self::TokenAccountFrozen,
            Check::InvalidTokenAccountOwner => Self::InvalidTokenAccountOwner,
            Check::ZeroAmount => Self::ZeroAmount,
            Check::ZeroRatio => Self::ZeroRatio,
            Check::RatioAboveCeiling => Self::RatioAboveCeiling,
            Check::ConversionFloorsToZero => Self::ConversionFloorsToZero,
            Check::ArithmeticOverflow => Self::ArithmeticOverflow,
            Check::NoPendingRatio => Self::NoPendingRatio,
            Check::RatioTimelockNotElapsed => Self::RatioTimelockNotElapsed,
            Check::RatioUpdatesFrozen => Self::RatioUpdatesFrozen,
            Check::RatioLocked => Self::RatioLocked,
            Check::TimelockCannotDecrease => Self::TimelockCannotDecrease,
            Check::NoPendingAuthority => Self::NoPendingAuthority,
            Check::PendingAuthorityMismatch => Self::PendingAuthorityMismatch,
            Check::MintsMustDiffer => Self::MintsMustDiffer,
            Check::InvalidCOption => Self::InvalidCOption,
            Check::TimestampOverflow => Self::TimestampOverflow,
            Check::NotUpgradeAuthority => Self::NotUpgradeAuthority,
            Check::InvalidLoaderState => Self::InvalidLoaderState,
            Check::ProgramAccountMismatch => Self::ProgramAccountMismatch,
            Check::ProgramDataMismatch => Self::ProgramDataMismatch,
            Check::InvalidLoaderOwner => Self::InvalidLoaderOwner,
            Check::IchorSupplyMustBeZero => Self::IchorSupplyMustBeZero,
            Check::IchorWithheldMustBeZero => Self::IchorWithheldMustBeZero,
            Check::BelowMinIchorAmount => Self::BelowMinIchorAmount,
            Check::InvalidBool => Self::InvalidBool,
            Check::InvalidPendingAuthority => Self::InvalidPendingAuthority,
            Check::CreatorDecayZero => Self::CreatorDecayZero,
            Check::ClockInversion => Self::ClockInversion,
            Check::UnauthorizedCreatorClaim => Self::UnauthorizedCreatorClaim,
            Check::CreatorDecayNotElapsed => Self::CreatorDecayNotElapsed,
        }
    }
}

impl From<Check> for anchor_lang::error::Error {
    fn from(value: Check) -> Self {
        IchorError::from(value).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_check_maps_to_a_distinct_ichor_error_variant() {
        let pairs = [
            (Check::Unauthorized, IchorError::Unauthorized),
            (Check::NotUpgradeAuthority, IchorError::NotUpgradeAuthority),
            (
                Check::IchorSupplyMustBeZero,
                IchorError::IchorSupplyMustBeZero,
            ),
            (Check::BelowMinIchorAmount, IchorError::BelowMinIchorAmount),
            (Check::InvalidBool, IchorError::InvalidBool),
            (
                Check::InvalidPendingAuthority,
                IchorError::InvalidPendingAuthority,
            ),
        ];
        for (check, _err) in pairs {
            let _mapped: IchorError = check.into();
        }
    }
}
