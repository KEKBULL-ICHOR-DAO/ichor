//! SPL / Token-2022 mint and token-account parsers.
//!
//! Base layouts are identical across the two token programs. Decimals and
//! authorities are read from bytes, never assumed.

use anchor_lang::prelude::*;

use crate::constants::{
    COPTION_NONE, COPTION_SOME, MINT_BASE_LEN, MINT_OFF_AUTHORITY, MINT_OFF_DECIMALS,
    MINT_OFF_FREEZE_AUTHORITY, MINT_OFF_IS_INITIALIZED, MINT_OFF_SUPPLY, TOKEN_ACCOUNT_BASE_LEN,
    TOKEN_ACCOUNT_OFF_AMOUNT, TOKEN_ACCOUNT_OFF_MINT, TOKEN_ACCOUNT_OFF_OWNER,
    TOKEN_ACCOUNT_OFF_STATE, TOKEN_ACCOUNT_STATE_FROZEN, TOKEN_ACCOUNT_STATE_INITIALIZED,
    TOKEN_ACCOUNT_STATE_UNINITIALIZED,
};
use crate::error::{Check, CheckResult};

pub fn parse_bool(byte: u8) -> CheckResult<bool> {
    match byte {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(Check::InvalidBool),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MintView {
    pub mint_authority: Option<Pubkey>,
    pub supply: u64,
    pub decimals: u8,
    pub is_initialized: bool,
    pub freeze_authority: Option<Pubkey>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenAccountView {
    pub mint: Pubkey,
    pub owner: Pubkey,
    pub amount: u64,
    pub state: u8,
}

fn read_coption(data: &[u8], off: usize) -> CheckResult<Option<Pubkey>> {
    if data.len() < off + 36 {
        return Err(Check::MintAccountTooShort);
    }
    let tag = u32::from_le_bytes(data[off..off + 4].try_into().unwrap());
    let key = Pubkey::try_from(&data[off + 4..off + 36]).map_err(|_| Check::MintAccountTooShort)?;
    match tag {
        COPTION_NONE => Ok(None),
        COPTION_SOME => Ok(Some(key)),
        _ => Err(Check::InvalidCOption),
    }
}

pub fn parse_mint(data: &[u8]) -> CheckResult<MintView> {
    if data.len() < MINT_BASE_LEN {
        return Err(Check::MintAccountTooShort);
    }
    Ok(MintView {
        mint_authority: read_coption(data, MINT_OFF_AUTHORITY)?,
        supply: u64::from_le_bytes(
            data[MINT_OFF_SUPPLY..MINT_OFF_SUPPLY + 8]
                .try_into()
                .unwrap(),
        ),
        decimals: data[MINT_OFF_DECIMALS],
        is_initialized: parse_bool(data[MINT_OFF_IS_INITIALIZED])?,
        freeze_authority: read_coption(data, MINT_OFF_FREEZE_AUTHORITY)?,
    })
}

pub fn parse_token_account(data: &[u8]) -> CheckResult<TokenAccountView> {
    if data.len() < TOKEN_ACCOUNT_BASE_LEN {
        return Err(Check::TokenAccountTooShort);
    }
    Ok(TokenAccountView {
        mint: Pubkey::try_from(&data[TOKEN_ACCOUNT_OFF_MINT..TOKEN_ACCOUNT_OFF_MINT + 32])
            .map_err(|_| Check::TokenAccountTooShort)?,
        owner: Pubkey::try_from(&data[TOKEN_ACCOUNT_OFF_OWNER..TOKEN_ACCOUNT_OFF_OWNER + 32])
            .map_err(|_| Check::TokenAccountTooShort)?,
        amount: u64::from_le_bytes(
            data[TOKEN_ACCOUNT_OFF_AMOUNT..TOKEN_ACCOUNT_OFF_AMOUNT + 8]
                .try_into()
                .unwrap(),
        ),
        state: data[TOKEN_ACCOUNT_OFF_STATE],
    })
}

pub fn require_initialized_mint(mint: &MintView) -> CheckResult<()> {
    if !mint.is_initialized {
        return Err(Check::MintUninitialized);
    }
    Ok(())
}

pub fn require_ichor_authorities(mint: &MintView, config_pda: &Pubkey) -> CheckResult<()> {
    require_initialized_mint(mint)?;
    match mint.mint_authority {
        Some(auth) if auth == *config_pda => {}
        _ => return Err(Check::MintAuthorityMismatch),
    }
    if mint.freeze_authority.is_some() {
        return Err(Check::FreezeAuthoritySet);
    }
    Ok(())
}

pub fn require_token_account(
    account: &TokenAccountView,
    expected_mint: &Pubkey,
    expected_owner: Option<&Pubkey>,
) -> CheckResult<()> {
    if account.mint != *expected_mint {
        return Err(Check::TokenAccountMintMismatch);
    }
    if let Some(owner) = expected_owner {
        if account.owner != *owner {
            return Err(Check::TokenAccountOwnerMismatch);
        }
    }
    match account.state {
        TOKEN_ACCOUNT_STATE_INITIALIZED => Ok(()),
        TOKEN_ACCOUNT_STATE_UNINITIALIZED => Err(Check::TokenAccountUninitialized),
        TOKEN_ACCOUNT_STATE_FROZEN => Err(Check::TokenAccountFrozen),
        _ => Err(Check::TokenAccountUninitialized),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn coption(tag: u32, key: Pubkey) -> [u8; 36] {
        let mut out = [0u8; 36];
        out[0..4].copy_from_slice(&tag.to_le_bytes());
        out[4..36].copy_from_slice(key.as_ref());
        out
    }

    fn mint_bytes(
        authority: Option<Pubkey>,
        supply: u64,
        decimals: u8,
        initialized: bool,
        freeze: Option<Pubkey>,
    ) -> Vec<u8> {
        let mut d = vec![0u8; MINT_BASE_LEN];
        let auth = coption(
            if authority.is_some() {
                COPTION_SOME
            } else {
                COPTION_NONE
            },
            authority.unwrap_or_default(),
        );
        d[0..36].copy_from_slice(&auth);
        d[MINT_OFF_SUPPLY..MINT_OFF_SUPPLY + 8].copy_from_slice(&supply.to_le_bytes());
        d[MINT_OFF_DECIMALS] = decimals;
        d[MINT_OFF_IS_INITIALIZED] = u8::from(initialized);
        let fr = coption(
            if freeze.is_some() {
                COPTION_SOME
            } else {
                COPTION_NONE
            },
            freeze.unwrap_or_default(),
        );
        d[MINT_OFF_FREEZE_AUTHORITY..MINT_OFF_FREEZE_AUTHORITY + 36].copy_from_slice(&fr);
        d
    }

    fn token_bytes(mint: Pubkey, owner: Pubkey, amount: u64, state: u8) -> Vec<u8> {
        let mut d = vec![0u8; TOKEN_ACCOUNT_BASE_LEN];
        d[0..32].copy_from_slice(mint.as_ref());
        d[32..64].copy_from_slice(owner.as_ref());
        d[64..72].copy_from_slice(&amount.to_le_bytes());
        d[TOKEN_ACCOUNT_OFF_STATE] = state;
        d
    }

    #[test]
    fn parse_mint_reads_decimals_and_authorities() {
        let pda = Pubkey::new_from_array([11u8; 32]);
        let d = mint_bytes(Some(pda), 99, 9, true, None);
        let m = parse_mint(&d).unwrap();
        assert_eq!(m.decimals, 9);
        assert_eq!(m.supply, 99);
        assert_eq!(m.mint_authority, Some(pda));
        assert_eq!(m.freeze_authority, None);
        assert!(m.is_initialized);
        assert_eq!(require_ichor_authorities(&m, &pda), Ok(()));
    }

    #[test]
    fn token_2022_extension_bytes_do_not_change_base_fields() {
        let pda = Pubkey::new_from_array([11u8; 32]);
        let mut d = mint_bytes(Some(pda), 0, 6, true, None);
        d.extend_from_slice(&[0u8; 84]); // padding + account type + dummy TLV
        let m = parse_mint(&d).unwrap();
        assert_eq!(m.decimals, 6);
    }

    #[test]
    fn wrong_or_missing_mint_authority_is_rejected() {
        let pda = Pubkey::new_from_array([11u8; 32]);
        let other = Pubkey::new_from_array([12u8; 32]);
        let none = parse_mint(&mint_bytes(None, 0, 9, true, None)).unwrap();
        assert_eq!(
            require_ichor_authorities(&none, &pda),
            Err(Check::MintAuthorityMismatch)
        );
        let wrong = parse_mint(&mint_bytes(Some(other), 0, 9, true, None)).unwrap();
        assert_eq!(
            require_ichor_authorities(&wrong, &pda),
            Err(Check::MintAuthorityMismatch)
        );
        let frozen = parse_mint(&mint_bytes(Some(pda), 0, 9, true, Some(other))).unwrap();
        assert_eq!(
            require_ichor_authorities(&frozen, &pda),
            Err(Check::FreezeAuthoritySet)
        );
        let uninit = parse_mint(&mint_bytes(Some(pda), 0, 9, false, None)).unwrap();
        assert_eq!(
            require_ichor_authorities(&uninit, &pda),
            Err(Check::MintUninitialized)
        );
    }

    #[test]
    fn short_and_bad_coption_are_rejected() {
        assert_eq!(parse_mint(&[0u8; 81]), Err(Check::MintAccountTooShort));
        let mut bad = mint_bytes(Some(Pubkey::default()), 0, 6, true, None);
        bad[0..4].copy_from_slice(&2u32.to_le_bytes());
        assert_eq!(parse_mint(&bad), Err(Check::InvalidCOption));
    }

    #[test]
    fn parse_token_account_and_state_gates() {
        let mint = Pubkey::new_from_array([3u8; 32]);
        let owner = Pubkey::new_from_array([4u8; 32]);
        let ok = parse_token_account(&token_bytes(
            mint,
            owner,
            50,
            TOKEN_ACCOUNT_STATE_INITIALIZED,
        ))
        .unwrap();
        assert_eq!(ok.amount, 50);
        assert_eq!(require_token_account(&ok, &mint, Some(&owner)), Ok(()));
        assert_eq!(
            require_token_account(&ok, &Pubkey::new_from_array([9u8; 32]), Some(&owner)),
            Err(Check::TokenAccountMintMismatch)
        );
        assert_eq!(
            require_token_account(&ok, &mint, Some(&Pubkey::new_from_array([9u8; 32]))),
            Err(Check::TokenAccountOwnerMismatch)
        );

        let frozen =
            parse_token_account(&token_bytes(mint, owner, 1, TOKEN_ACCOUNT_STATE_FROZEN)).unwrap();
        assert_eq!(
            require_token_account(&frozen, &mint, None),
            Err(Check::TokenAccountFrozen)
        );
        let uninit = parse_token_account(&token_bytes(
            mint,
            owner,
            1,
            TOKEN_ACCOUNT_STATE_UNINITIALIZED,
        ))
        .unwrap();
        assert_eq!(
            require_token_account(&uninit, &mint, None),
            Err(Check::TokenAccountUninitialized)
        );
        assert_eq!(
            parse_token_account(&[0u8; 164]),
            Err(Check::TokenAccountTooShort)
        );
    }

    #[test]
    fn mint_initialized_flag_must_be_exactly_0_or_1() {
        assert_eq!(parse_bool(0), Ok(false));
        assert_eq!(parse_bool(1), Ok(true));
        assert_eq!(parse_bool(2), Err(Check::InvalidBool));
        let mut d = mint_bytes(Some(Pubkey::new_from_array([1u8; 32])), 0, 9, true, None);
        d[MINT_OFF_IS_INITIALIZED] = 2;
        assert_eq!(parse_mint(&d), Err(Check::InvalidBool));
        d[MINT_OFF_IS_INITIALIZED] = 0;
        let uninit = parse_mint(&d).unwrap();
        assert!(!uninit.is_initialized);
        assert_eq!(
            require_initialized_mint(&uninit),
            Err(Check::MintUninitialized)
        );
    }

    #[test]
    fn token_2022_account_extensions_do_not_shift_base_fields() {
        let mint = Pubkey::new_from_array([3u8; 32]);
        let owner = Pubkey::new_from_array([4u8; 32]);
        let mut d = token_bytes(mint, owner, 7, TOKEN_ACCOUNT_STATE_INITIALIZED);
        d.extend_from_slice(&[1u8; 40]);
        let parsed = parse_token_account(&d).unwrap();
        assert_eq!(parsed.mint, mint);
        assert_eq!(parsed.owner, owner);
        assert_eq!(parsed.amount, 7);
    }
}
