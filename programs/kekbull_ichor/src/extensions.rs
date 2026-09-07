//! Token-2022 mint TLV parser for the approved ICHOR extension set.
//!
//! The only accepted mint extension is `TransferFeeConfig`. Any other
//! initialized type - including MetadataPointer, TransferHook, and
//! MintCloseAuthority - is rejected. Trailing `Uninitialized` TLV slots
//! (type 0) are padding and stop the walk.
//!
//! Layout is the Token-2022 mint used by `spl-token-2022` 8.0.1:
//! 82-byte base, 83 zero bytes, 1-byte `AccountType::Mint`, then TLV
//! (`u16` type, `u16` length, value). `TransferFee` / `TransferFeeConfig`
//! are Pod with 1-byte alignment (108 bytes).

use anchor_lang::prelude::*;

use crate::constants::{
    EXTENSION_TYPE_TRANSFER_FEE_CONFIG, EXTENSION_TYPE_UNINITIALIZED, MINT_BASE_LEN,
    TOKEN_2022_ACCOUNT_TYPE_MINT, TOKEN_2022_ACCOUNT_TYPE_OFFSET, TOKEN_2022_TLV_OFFSET,
    TRANSFER_FEE_BASIS_POINTS, TRANSFER_FEE_CONFIG_LEN, TRANSFER_FEE_CONFIG_OFF_AUTHORITY,
    TRANSFER_FEE_CONFIG_OFF_NEWER, TRANSFER_FEE_CONFIG_OFF_OLDER, TRANSFER_FEE_CONFIG_OFF_WITHDRAW,
    TRANSFER_FEE_CONFIG_OFF_WITHHELD, TRANSFER_FEE_LEN, TRANSFER_FEE_MAXIMUM_FEE,
    TRANSFER_FEE_OFF_BASIS_POINTS, TRANSFER_FEE_OFF_MAXIMUM_FEE,
};
use crate::error::{Check, CheckResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferFeeView {
    pub epoch: u64,
    pub maximum_fee: u64,
    pub transfer_fee_basis_points: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferFeeConfigView {
    pub transfer_fee_config_authority: Option<Pubkey>,
    pub withdraw_withheld_authority: Option<Pubkey>,
    pub withheld_amount: u64,
    pub older_transfer_fee: TransferFeeView,
    pub newer_transfer_fee: TransferFeeView,
}

fn optional_nonzero_pubkey(bytes: &[u8]) -> CheckResult<Option<Pubkey>> {
    if bytes.len() != 32 {
        return Err(Check::InvalidTransferFeeConfig);
    }
    let key = Pubkey::try_from(bytes).map_err(|_| Check::InvalidTransferFeeConfig)?;
    if key == Pubkey::default() {
        Ok(None)
    } else {
        Ok(Some(key))
    }
}

fn parse_transfer_fee(data: &[u8]) -> CheckResult<TransferFeeView> {
    if data.len() < TRANSFER_FEE_LEN {
        return Err(Check::InvalidTransferFeeConfig);
    }
    Ok(TransferFeeView {
        epoch: u64::from_le_bytes(
            data[0..8]
                .try_into()
                .map_err(|_| Check::InvalidTransferFeeConfig)?,
        ),
        maximum_fee: u64::from_le_bytes(
            data[TRANSFER_FEE_OFF_MAXIMUM_FEE..TRANSFER_FEE_OFF_MAXIMUM_FEE + 8]
                .try_into()
                .map_err(|_| Check::InvalidTransferFeeConfig)?,
        ),
        transfer_fee_basis_points: u16::from_le_bytes(
            data[TRANSFER_FEE_OFF_BASIS_POINTS..TRANSFER_FEE_OFF_BASIS_POINTS + 2]
                .try_into()
                .map_err(|_| Check::InvalidTransferFeeConfig)?,
        ),
    })
}

pub fn parse_transfer_fee_config_value(data: &[u8]) -> CheckResult<TransferFeeConfigView> {
    if data.len() != TRANSFER_FEE_CONFIG_LEN {
        return Err(Check::InvalidTransferFeeConfig);
    }
    Ok(TransferFeeConfigView {
        transfer_fee_config_authority: optional_nonzero_pubkey(
            &data[TRANSFER_FEE_CONFIG_OFF_AUTHORITY..TRANSFER_FEE_CONFIG_OFF_AUTHORITY + 32],
        )?,
        withdraw_withheld_authority: optional_nonzero_pubkey(
            &data[TRANSFER_FEE_CONFIG_OFF_WITHDRAW..TRANSFER_FEE_CONFIG_OFF_WITHDRAW + 32],
        )?,
        withheld_amount: u64::from_le_bytes(
            data[TRANSFER_FEE_CONFIG_OFF_WITHHELD..TRANSFER_FEE_CONFIG_OFF_WITHHELD + 8]
                .try_into()
                .map_err(|_| Check::InvalidTransferFeeConfig)?,
        ),
        older_transfer_fee: parse_transfer_fee(
            &data[TRANSFER_FEE_CONFIG_OFF_OLDER..TRANSFER_FEE_CONFIG_OFF_OLDER + TRANSFER_FEE_LEN],
        )?,
        newer_transfer_fee: parse_transfer_fee(
            &data[TRANSFER_FEE_CONFIG_OFF_NEWER..TRANSFER_FEE_CONFIG_OFF_NEWER + TRANSFER_FEE_LEN],
        )?,
    })
}

/// Walk mint TLV. Returns the sole `TransferFeeConfig` or a check error.
pub fn parse_ichor_transfer_fee_config(data: &[u8]) -> CheckResult<TransferFeeConfigView> {
    if data.len() < TOKEN_2022_TLV_OFFSET {
        return Err(Check::TransferFeeConfigMissing);
    }
    if data.len() < TOKEN_2022_ACCOUNT_TYPE_OFFSET {
        return Err(Check::InvalidMintExtensionLayout);
    }
    if data[MINT_BASE_LEN..TOKEN_2022_ACCOUNT_TYPE_OFFSET]
        .iter()
        .any(|b| *b != 0)
    {
        return Err(Check::InvalidMintExtensionLayout);
    }
    if data[TOKEN_2022_ACCOUNT_TYPE_OFFSET] != TOKEN_2022_ACCOUNT_TYPE_MINT {
        return Err(Check::InvalidMintAccountType);
    }

    let tlv = &data[TOKEN_2022_TLV_OFFSET..];
    let mut offset = 0usize;
    let mut found = None;
    while offset < tlv.len() {
        if tlv.len().saturating_sub(offset) < 4 {
            if tlv[offset..].iter().all(|b| *b == 0) {
                break;
            }
            return Err(Check::InvalidMintExtensionLayout);
        }
        let ext_type = u16::from_le_bytes(tlv[offset..offset + 2].try_into().unwrap());
        let ext_len = u16::from_le_bytes(tlv[offset + 2..offset + 4].try_into().unwrap()) as usize;
        if ext_type == EXTENSION_TYPE_UNINITIALIZED {
            break;
        }
        let value_start = offset.saturating_add(4);
        let value_end = value_start.saturating_add(ext_len);
        if value_end > tlv.len() {
            return Err(Check::InvalidMintExtensionLayout);
        }
        if ext_type != EXTENSION_TYPE_TRANSFER_FEE_CONFIG {
            return Err(Check::UnexpectedMintExtension);
        }
        if found.is_some() {
            return Err(Check::UnexpectedMintExtension);
        }
        found = Some(parse_transfer_fee_config_value(
            &tlv[value_start..value_end],
        )?);
        offset = value_end;
    }
    found.ok_or(Check::TransferFeeConfigMissing)
}

pub fn require_approved_transfer_fee(fee: &TransferFeeView) -> CheckResult<()> {
    if fee.transfer_fee_basis_points != TRANSFER_FEE_BASIS_POINTS
        || fee.maximum_fee != TRANSFER_FEE_MAXIMUM_FEE
    {
        return Err(Check::TransferFeeRateMismatch);
    }
    Ok(())
}

pub fn require_ichor_transfer_fee_config(
    config: &TransferFeeConfigView,
    fee_authority: &Pubkey,
    withdraw_authority: &Pubkey,
) -> CheckResult<()> {
    match config.transfer_fee_config_authority {
        Some(auth) if auth == *fee_authority => {}
        _ => return Err(Check::TransferFeeAuthorityMismatch),
    }
    match config.withdraw_withheld_authority {
        Some(auth) if auth == *withdraw_authority => {}
        _ => return Err(Check::WithdrawWithheldAuthorityMismatch),
    }
    require_approved_transfer_fee(&config.older_transfer_fee)?;
    require_approved_transfer_fee(&config.newer_transfer_fee)?;
    if config.withheld_amount != 0 {
        return Err(Check::IchorWithheldMustBeZero);
    }
    Ok(())
}

pub fn require_operational_transfer_fee_config(
    config: &TransferFeeConfigView,
    fee_authority: &Pubkey,
    withdraw_authority: &Pubkey,
    authority_revoked: bool,
) -> CheckResult<()> {
    match (authority_revoked, config.transfer_fee_config_authority) {
        (false, Some(auth)) if auth == *fee_authority => {}
        (true, None) => {}
        _ => return Err(Check::TransferFeeAuthorityMismatch),
    }
    match config.withdraw_withheld_authority {
        Some(auth) if auth == *withdraw_authority => {}
        _ => return Err(Check::WithdrawWithheldAuthorityMismatch),
    }
    require_approved_transfer_fee(&config.older_transfer_fee)?;
    require_approved_transfer_fee(&config.newer_transfer_fee)?;
    Ok(())
}

pub fn require_pilot_transfer_fee(basis_points: u16, maximum_fee: u64) -> CheckResult<()> {
    if basis_points != TRANSFER_FEE_BASIS_POINTS || maximum_fee != TRANSFER_FEE_MAXIMUM_FEE {
        return Err(Check::TransferFeeExceedsPilotCap);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::{
        COPTION_NONE, COPTION_SOME, EXTENSION_TYPE_METADATA_POINTER,
        EXTENSION_TYPE_MINT_CLOSE_AUTHORITY, EXTENSION_TYPE_TRANSFER_HOOK, MINT_OFF_AUTHORITY,
        MINT_OFF_DECIMALS, MINT_OFF_FREEZE_AUTHORITY, MINT_OFF_IS_INITIALIZED,
    };

    fn pk(b: u8) -> Pubkey {
        Pubkey::new_from_array([b; 32])
    }

    fn base_mint(authority: Pubkey) -> Vec<u8> {
        let mut d = vec![0u8; TOKEN_2022_TLV_OFFSET];
        d[MINT_OFF_AUTHORITY..MINT_OFF_AUTHORITY + 4].copy_from_slice(&COPTION_SOME.to_le_bytes());
        d[MINT_OFF_AUTHORITY + 4..MINT_OFF_AUTHORITY + 36].copy_from_slice(authority.as_ref());
        d[MINT_OFF_DECIMALS] = 9;
        d[MINT_OFF_IS_INITIALIZED] = 1;
        d[MINT_OFF_FREEZE_AUTHORITY..MINT_OFF_FREEZE_AUTHORITY + 4]
            .copy_from_slice(&COPTION_NONE.to_le_bytes());
        d[TOKEN_2022_ACCOUNT_TYPE_OFFSET] = TOKEN_2022_ACCOUNT_TYPE_MINT;
        d
    }

    fn fee_value(
        fee_auth: Pubkey,
        withdraw: Pubkey,
        withheld: u64,
        older_bps: u16,
        older_max: u64,
        newer_bps: u16,
        newer_max: u64,
    ) -> Vec<u8> {
        let mut v = vec![0u8; TRANSFER_FEE_CONFIG_LEN];
        v[TRANSFER_FEE_CONFIG_OFF_AUTHORITY..TRANSFER_FEE_CONFIG_OFF_AUTHORITY + 32]
            .copy_from_slice(fee_auth.as_ref());
        v[TRANSFER_FEE_CONFIG_OFF_WITHDRAW..TRANSFER_FEE_CONFIG_OFF_WITHDRAW + 32]
            .copy_from_slice(withdraw.as_ref());
        v[TRANSFER_FEE_CONFIG_OFF_WITHHELD..TRANSFER_FEE_CONFIG_OFF_WITHHELD + 8]
            .copy_from_slice(&withheld.to_le_bytes());
        v[TRANSFER_FEE_CONFIG_OFF_OLDER + TRANSFER_FEE_OFF_MAXIMUM_FEE
            ..TRANSFER_FEE_CONFIG_OFF_OLDER + TRANSFER_FEE_OFF_MAXIMUM_FEE + 8]
            .copy_from_slice(&older_max.to_le_bytes());
        v[TRANSFER_FEE_CONFIG_OFF_OLDER + TRANSFER_FEE_OFF_BASIS_POINTS
            ..TRANSFER_FEE_CONFIG_OFF_OLDER + TRANSFER_FEE_OFF_BASIS_POINTS + 2]
            .copy_from_slice(&older_bps.to_le_bytes());
        v[TRANSFER_FEE_CONFIG_OFF_NEWER + TRANSFER_FEE_OFF_MAXIMUM_FEE
            ..TRANSFER_FEE_CONFIG_OFF_NEWER + TRANSFER_FEE_OFF_MAXIMUM_FEE + 8]
            .copy_from_slice(&newer_max.to_le_bytes());
        v[TRANSFER_FEE_CONFIG_OFF_NEWER + TRANSFER_FEE_OFF_BASIS_POINTS
            ..TRANSFER_FEE_CONFIG_OFF_NEWER + TRANSFER_FEE_OFF_BASIS_POINTS + 2]
            .copy_from_slice(&newer_bps.to_le_bytes());
        v
    }

    fn push_ext(buf: &mut Vec<u8>, ext_type: u16, value: &[u8]) {
        buf.extend_from_slice(&ext_type.to_le_bytes());
        buf.extend_from_slice(&(value.len() as u16).to_le_bytes());
        buf.extend_from_slice(value);
    }

    fn approved_mint(fee_auth: Pubkey, withdraw: Pubkey) -> Vec<u8> {
        let mut d = base_mint(fee_auth);
        push_ext(
            &mut d,
            EXTENSION_TYPE_TRANSFER_FEE_CONFIG,
            &fee_value(
                fee_auth,
                withdraw,
                0,
                TRANSFER_FEE_BASIS_POINTS,
                TRANSFER_FEE_MAXIMUM_FEE,
                TRANSFER_FEE_BASIS_POINTS,
                TRANSFER_FEE_MAXIMUM_FEE,
            ),
        );
        d
    }

    #[test]
    fn parser_reads_approved_transfer_fee_config() {
        let fee_auth = pk(9);
        let withdraw = pk(10);
        let parsed = parse_ichor_transfer_fee_config(&approved_mint(fee_auth, withdraw)).unwrap();
        assert_eq!(parsed.transfer_fee_config_authority, Some(fee_auth));
        assert_eq!(parsed.withdraw_withheld_authority, Some(withdraw));
        assert_eq!(parsed.withheld_amount, 0);
        assert_eq!(
            parsed.newer_transfer_fee.transfer_fee_basis_points,
            TRANSFER_FEE_BASIS_POINTS
        );
        assert_eq!(
            parsed.newer_transfer_fee.maximum_fee,
            TRANSFER_FEE_MAXIMUM_FEE
        );
        assert_eq!(
            require_ichor_transfer_fee_config(&parsed, &fee_auth, &withdraw),
            Ok(())
        );
    }

    #[test]
    fn trailing_uninitialized_padding_is_ignored() {
        let fee_auth = pk(9);
        let withdraw = pk(10);
        let mut d = approved_mint(fee_auth, withdraw);
        push_ext(&mut d, EXTENSION_TYPE_UNINITIALIZED, &[]);
        d.extend_from_slice(&[0u8; 16]);
        assert!(parse_ichor_transfer_fee_config(&d).is_ok());
    }

    #[test]
    fn extra_extension_is_rejected() {
        let fee_auth = pk(9);
        let withdraw = pk(10);
        for extra in [
            EXTENSION_TYPE_TRANSFER_HOOK,
            EXTENSION_TYPE_METADATA_POINTER,
            EXTENSION_TYPE_MINT_CLOSE_AUTHORITY,
        ] {
            let mut d = approved_mint(fee_auth, withdraw);
            push_ext(&mut d, extra, &[0u8; 32]);
            assert_eq!(
                parse_ichor_transfer_fee_config(&d),
                Err(Check::UnexpectedMintExtension)
            );
        }
    }

    #[test]
    fn only_hostile_or_unrelated_extension_without_fee_is_rejected() {
        let mut d = base_mint(pk(9));
        push_ext(&mut d, EXTENSION_TYPE_TRANSFER_HOOK, &[0u8; 32]);
        assert_eq!(
            parse_ichor_transfer_fee_config(&d),
            Err(Check::UnexpectedMintExtension)
        );
    }

    #[test]
    fn missing_fee_config_and_short_legacy_mint_are_rejected() {
        assert_eq!(
            parse_ichor_transfer_fee_config(&vec![0u8; MINT_BASE_LEN]),
            Err(Check::TransferFeeConfigMissing)
        );
        let bare = base_mint(pk(9));
        assert_eq!(
            parse_ichor_transfer_fee_config(&bare),
            Err(Check::TransferFeeConfigMissing)
        );
    }

    #[test]
    fn wrong_account_type_and_nonzero_padding_are_rejected() {
        let fee_auth = pk(9);
        let withdraw = pk(10);
        let mut d = approved_mint(fee_auth, withdraw);
        d[TOKEN_2022_ACCOUNT_TYPE_OFFSET] = 2;
        assert_eq!(
            parse_ichor_transfer_fee_config(&d),
            Err(Check::InvalidMintAccountType)
        );
        let mut padded = approved_mint(fee_auth, withdraw);
        padded[MINT_BASE_LEN] = 1;
        assert_eq!(
            parse_ichor_transfer_fee_config(&padded),
            Err(Check::InvalidMintExtensionLayout)
        );
    }

    #[test]
    fn truncated_tlv_is_rejected() {
        let mut d = base_mint(pk(9));
        d.extend_from_slice(&EXTENSION_TYPE_TRANSFER_FEE_CONFIG.to_le_bytes());
        d.extend_from_slice(&108u16.to_le_bytes());
        d.extend_from_slice(&[0u8; 10]);
        assert_eq!(
            parse_ichor_transfer_fee_config(&d),
            Err(Check::InvalidMintExtensionLayout)
        );
    }

    #[test]
    fn authority_and_rate_validation() {
        let fee_auth = pk(9);
        let withdraw = pk(10);
        let ok = parse_transfer_fee_config_value(&fee_value(
            fee_auth,
            withdraw,
            0,
            TRANSFER_FEE_BASIS_POINTS,
            TRANSFER_FEE_MAXIMUM_FEE,
            TRANSFER_FEE_BASIS_POINTS,
            TRANSFER_FEE_MAXIMUM_FEE,
        ))
        .unwrap();
        assert_eq!(
            require_ichor_transfer_fee_config(&ok, &fee_auth, &withdraw),
            Ok(())
        );
        assert_eq!(
            require_ichor_transfer_fee_config(&ok, &pk(1), &withdraw),
            Err(Check::TransferFeeAuthorityMismatch)
        );
        assert_eq!(
            require_ichor_transfer_fee_config(&ok, &fee_auth, &pk(2)),
            Err(Check::WithdrawWithheldAuthorityMismatch)
        );

        let none = parse_transfer_fee_config_value(&fee_value(
            Pubkey::default(),
            withdraw,
            0,
            TRANSFER_FEE_BASIS_POINTS,
            TRANSFER_FEE_MAXIMUM_FEE,
            TRANSFER_FEE_BASIS_POINTS,
            TRANSFER_FEE_MAXIMUM_FEE,
        ))
        .unwrap();
        assert_eq!(none.transfer_fee_config_authority, None);
        assert_eq!(
            require_ichor_transfer_fee_config(&none, &fee_auth, &withdraw),
            Err(Check::TransferFeeAuthorityMismatch)
        );

        let wrong_rate = parse_transfer_fee_config_value(&fee_value(
            fee_auth,
            withdraw,
            0,
            26,
            TRANSFER_FEE_MAXIMUM_FEE,
            26,
            TRANSFER_FEE_MAXIMUM_FEE,
        ))
        .unwrap();
        assert_eq!(
            require_ichor_transfer_fee_config(&wrong_rate, &fee_auth, &withdraw),
            Err(Check::TransferFeeRateMismatch)
        );

        let wrong_max = parse_transfer_fee_config_value(&fee_value(
            fee_auth,
            withdraw,
            0,
            TRANSFER_FEE_BASIS_POINTS,
            1,
            TRANSFER_FEE_BASIS_POINTS,
            1,
        ))
        .unwrap();
        assert_eq!(
            require_ichor_transfer_fee_config(&wrong_max, &fee_auth, &withdraw),
            Err(Check::TransferFeeRateMismatch)
        );

        let withheld = parse_transfer_fee_config_value(&fee_value(
            fee_auth,
            withdraw,
            1,
            TRANSFER_FEE_BASIS_POINTS,
            TRANSFER_FEE_MAXIMUM_FEE,
            TRANSFER_FEE_BASIS_POINTS,
            TRANSFER_FEE_MAXIMUM_FEE,
        ))
        .unwrap();
        assert_eq!(
            require_ichor_transfer_fee_config(&withheld, &fee_auth, &withdraw),
            Err(Check::IchorWithheldMustBeZero)
        );
    }

    #[test]
    fn newer_only_rate_mismatch_is_rejected() {
        let fee_auth = pk(9);
        let withdraw = pk(10);
        let newer_wrong = parse_transfer_fee_config_value(&fee_value(
            fee_auth,
            withdraw,
            0,
            TRANSFER_FEE_BASIS_POINTS,
            TRANSFER_FEE_MAXIMUM_FEE,
            0,
            TRANSFER_FEE_MAXIMUM_FEE,
        ))
        .unwrap();
        assert_eq!(
            require_ichor_transfer_fee_config(&newer_wrong, &fee_auth, &withdraw),
            Err(Check::TransferFeeRateMismatch)
        );
    }

    #[test]
    fn wrong_value_length_is_rejected() {
        assert_eq!(
            parse_transfer_fee_config_value(&[0u8; 107]),
            Err(Check::InvalidTransferFeeConfig)
        );
        assert_eq!(
            parse_transfer_fee_config_value(&[0u8; 109]),
            Err(Check::InvalidTransferFeeConfig)
        );
    }

    #[test]
    fn transfer_fee_is_fixed_not_merely_capped() {
        assert_eq!(
            require_pilot_transfer_fee(TRANSFER_FEE_BASIS_POINTS, TRANSFER_FEE_MAXIMUM_FEE),
            Ok(())
        );
        assert_eq!(
            require_pilot_transfer_fee(0, 0),
            Err(Check::TransferFeeExceedsPilotCap)
        );
        assert_eq!(
            require_pilot_transfer_fee(24, 1),
            Err(Check::TransferFeeExceedsPilotCap)
        );
        assert_eq!(
            require_pilot_transfer_fee(26, TRANSFER_FEE_MAXIMUM_FEE),
            Err(Check::TransferFeeExceedsPilotCap)
        );
    }

    #[test]
    fn operational_config_accepts_only_live_or_revoked_fixed_authority() {
        let fee_auth = pk(9);
        let withdraw = pk(10);
        let mut config =
            parse_ichor_transfer_fee_config(&approved_mint(fee_auth, withdraw)).unwrap();
        assert_eq!(
            require_operational_transfer_fee_config(&config, &fee_auth, &withdraw, false),
            Ok(())
        );
        config.withheld_amount = 99;
        assert_eq!(
            require_operational_transfer_fee_config(&config, &fee_auth, &withdraw, false),
            Ok(())
        );
        config.transfer_fee_config_authority = None;
        assert_eq!(
            require_operational_transfer_fee_config(&config, &fee_auth, &withdraw, true),
            Ok(())
        );
        assert_eq!(
            require_operational_transfer_fee_config(&config, &fee_auth, &withdraw, false),
            Err(Check::TransferFeeAuthorityMismatch)
        );
    }

    #[test]
    fn mutation_of_type_or_rate_bytes_is_detected() {
        let fee_auth = pk(9);
        let withdraw = pk(10);
        let mut d = approved_mint(fee_auth, withdraw);
        let tlv = TOKEN_2022_TLV_OFFSET;
        d[tlv] = EXTENSION_TYPE_TRANSFER_HOOK as u8;
        assert_eq!(
            parse_ichor_transfer_fee_config(&d),
            Err(Check::UnexpectedMintExtension)
        );

        let mut rate = approved_mint(fee_auth, withdraw);
        let newer_bps = TOKEN_2022_TLV_OFFSET
            + 4
            + TRANSFER_FEE_CONFIG_OFF_NEWER
            + TRANSFER_FEE_OFF_BASIS_POINTS;
        rate[newer_bps..newer_bps + 2].copy_from_slice(&100u16.to_le_bytes());
        let parsed = parse_ichor_transfer_fee_config(&rate).unwrap();
        assert_eq!(
            require_ichor_transfer_fee_config(&parsed, &fee_auth, &withdraw),
            Err(Check::TransferFeeRateMismatch)
        );
    }
}
