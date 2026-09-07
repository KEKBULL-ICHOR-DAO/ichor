//! BPF upgradeable loader (v3) Program / ProgramData parser.
//!
//! Layout re-reviewed against current loader-v3 `UpgradeableLoaderState`
//! (same bytes TasteMaker reads; SIMD-0433 does not move the metadata header):
//!
//! ```text
//! Program (owner = BPFLoaderUpgradeab1e, executable):
//!   @0  u32 LE discriminant = 2
//!   @4  programdata_address  Pubkey
//!   length >= 36
//!
//! ProgramData (owner = BPFLoaderUpgradeab1e):
//!   @0  u32 LE discriminant = 3
//!   @4  slot                 u64
//!   @12 Option tag           u8  (0 = None, 1 = Some; anything else is corrupt)
//!   @13 upgrade_authority    Pubkey if tag == 1
//!   Some => length >= 45; None => length >= 13
//! ```
//!
//! The ProgramData address must equal both the address embedded in the Program
//! account and the canonical loader PDA `["<program id>"]` on the upgradeable
//! loader. No hardcoded initializer key.

use anchor_lang::prelude::*;

use crate::constants::BPF_LOADER_UPGRADEABLE;
use crate::error::{Check, CheckResult};

/// `UpgradeableLoaderState::Program`.
pub const LOADER_STATE_PROGRAM: u32 = 2;
/// `UpgradeableLoaderState::ProgramData`.
pub const LOADER_STATE_PROGRAM_DATA: u32 = 3;
pub const MIN_PROGRAM_ACCOUNT_LEN: usize = 36;
pub const MIN_PROGRAMDATA_NONE_LEN: usize = 13;
pub const MIN_PROGRAMDATA_SOME_LEN: usize = 45;
pub const OPTION_NONE: u8 = 0;
pub const OPTION_SOME: u8 = 1;

pub fn program_data_pda(program_id: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[program_id.as_ref()], &BPF_LOADER_UPGRADEABLE).0
}

pub fn require_upgrade_authority(
    program_id: &Pubkey,
    program_key: &Pubkey,
    program_owner: &Pubkey,
    program_executable: bool,
    program_data: &[u8],
    programdata_key: &Pubkey,
    programdata_owner: &Pubkey,
    programdata_data: &[u8],
    signer: &Pubkey,
) -> CheckResult<()> {
    if program_key != program_id || !program_executable {
        return Err(Check::ProgramAccountMismatch);
    }
    if *program_owner != BPF_LOADER_UPGRADEABLE || *programdata_owner != BPF_LOADER_UPGRADEABLE {
        return Err(Check::InvalidLoaderOwner);
    }
    if program_data.len() < MIN_PROGRAM_ACCOUNT_LEN {
        return Err(Check::InvalidLoaderState);
    }
    let program_disc = u32::from_le_bytes(program_data[0..4].try_into().unwrap());
    if program_disc != LOADER_STATE_PROGRAM {
        return Err(Check::InvalidLoaderState);
    }
    let embedded = Pubkey::try_from(&program_data[4..36]).map_err(|_| Check::InvalidLoaderState)?;
    let canonical = program_data_pda(program_id);
    if *programdata_key != embedded || *programdata_key != canonical {
        return Err(Check::ProgramDataMismatch);
    }
    if programdata_data.len() < MIN_PROGRAMDATA_NONE_LEN {
        return Err(Check::InvalidLoaderState);
    }
    let data_disc = u32::from_le_bytes(programdata_data[0..4].try_into().unwrap());
    if data_disc != LOADER_STATE_PROGRAM_DATA {
        return Err(Check::InvalidLoaderState);
    }
    let option_tag = programdata_data[12];
    match option_tag {
        OPTION_NONE => Err(Check::NotUpgradeAuthority),
        OPTION_SOME => {
            if programdata_data.len() < MIN_PROGRAMDATA_SOME_LEN {
                return Err(Check::InvalidLoaderState);
            }
            let authority = Pubkey::try_from(&programdata_data[13..45])
                .map_err(|_| Check::InvalidLoaderState)?;
            if authority != *signer {
                return Err(Check::NotUpgradeAuthority);
            }
            Ok(())
        }
        _ => Err(Check::InvalidLoaderState),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pk(b: u8) -> Pubkey {
        Pubkey::new_from_array([b; 32])
    }

    fn program_bytes(programdata: &Pubkey) -> Vec<u8> {
        let mut d = vec![0u8; MIN_PROGRAM_ACCOUNT_LEN];
        d[0..4].copy_from_slice(&LOADER_STATE_PROGRAM.to_le_bytes());
        d[4..36].copy_from_slice(programdata.as_ref());
        d
    }

    fn programdata_some(authority: &Pubkey) -> Vec<u8> {
        let mut d = vec![0u8; MIN_PROGRAMDATA_SOME_LEN];
        d[0..4].copy_from_slice(&LOADER_STATE_PROGRAM_DATA.to_le_bytes());
        d[12] = OPTION_SOME;
        d[13..45].copy_from_slice(authority.as_ref());
        d
    }

    fn programdata_none() -> Vec<u8> {
        let mut d = vec![0u8; MIN_PROGRAMDATA_NONE_LEN];
        d[0..4].copy_from_slice(&LOADER_STATE_PROGRAM_DATA.to_le_bytes());
        d[12] = OPTION_NONE;
        d
    }

    fn valid_ids() -> (Pubkey, Pubkey, Pubkey) {
        let program_id = pk(7);
        let pda = program_data_pda(&program_id);
        (program_id, pda, pk(3))
    }

    #[test]
    fn upgrade_authority_can_initialize() {
        let (program_id, pda, auth) = valid_ids();
        assert_eq!(
            require_upgrade_authority(
                &program_id,
                &program_id,
                &BPF_LOADER_UPGRADEABLE,
                true,
                &program_bytes(&pda),
                &pda,
                &BPF_LOADER_UPGRADEABLE,
                &programdata_some(&auth),
                &auth,
            ),
            Ok(())
        );
    }

    #[test]
    fn front_run_by_non_upgrade_authority_is_rejected() {
        let (program_id, pda, auth) = valid_ids();
        let interloper = pk(9);
        assert_eq!(
            require_upgrade_authority(
                &program_id,
                &program_id,
                &BPF_LOADER_UPGRADEABLE,
                true,
                &program_bytes(&pda),
                &pda,
                &BPF_LOADER_UPGRADEABLE,
                &programdata_some(&auth),
                &interloper,
            ),
            Err(Check::NotUpgradeAuthority)
        );
    }

    #[test]
    fn revoked_or_missing_authority_cannot_initialize() {
        let (program_id, pda, auth) = valid_ids();
        assert_eq!(
            require_upgrade_authority(
                &program_id,
                &program_id,
                &BPF_LOADER_UPGRADEABLE,
                true,
                &program_bytes(&pda),
                &pda,
                &BPF_LOADER_UPGRADEABLE,
                &programdata_none(),
                &auth,
            ),
            Err(Check::NotUpgradeAuthority)
        );
    }

    #[test]
    fn wrong_program_key_or_non_executable_rejected() {
        let (program_id, pda, auth) = valid_ids();
        let bytes = program_bytes(&pda);
        let data = programdata_some(&auth);
        assert_eq!(
            require_upgrade_authority(
                &program_id,
                &pk(1),
                &BPF_LOADER_UPGRADEABLE,
                true,
                &bytes,
                &pda,
                &BPF_LOADER_UPGRADEABLE,
                &data,
                &auth,
            ),
            Err(Check::ProgramAccountMismatch)
        );
        assert_eq!(
            require_upgrade_authority(
                &program_id,
                &program_id,
                &BPF_LOADER_UPGRADEABLE,
                false,
                &bytes,
                &pda,
                &BPF_LOADER_UPGRADEABLE,
                &data,
                &auth,
            ),
            Err(Check::ProgramAccountMismatch)
        );
    }

    #[test]
    fn loader_owner_and_state_are_strict() {
        let (program_id, pda, auth) = valid_ids();
        let bytes = program_bytes(&pda);
        let data = programdata_some(&auth);
        assert_eq!(
            require_upgrade_authority(
                &program_id,
                &program_id,
                &crate::constants::TOKEN_PROGRAM_ID,
                true,
                &bytes,
                &pda,
                &BPF_LOADER_UPGRADEABLE,
                &data,
                &auth,
            ),
            Err(Check::InvalidLoaderOwner)
        );
        let mut bad_disc = bytes.clone();
        bad_disc[0..4].copy_from_slice(&1u32.to_le_bytes());
        assert_eq!(
            require_upgrade_authority(
                &program_id,
                &program_id,
                &BPF_LOADER_UPGRADEABLE,
                true,
                &bad_disc,
                &pda,
                &BPF_LOADER_UPGRADEABLE,
                &data,
                &auth,
            ),
            Err(Check::InvalidLoaderState)
        );
        let mut bad_opt = data.clone();
        bad_opt[12] = 2;
        assert_eq!(
            require_upgrade_authority(
                &program_id,
                &program_id,
                &BPF_LOADER_UPGRADEABLE,
                true,
                &bytes,
                &pda,
                &BPF_LOADER_UPGRADEABLE,
                &bad_opt,
                &auth,
            ),
            Err(Check::InvalidLoaderState)
        );
        assert_eq!(
            require_upgrade_authority(
                &program_id,
                &program_id,
                &BPF_LOADER_UPGRADEABLE,
                true,
                &bytes,
                &pk(4),
                &BPF_LOADER_UPGRADEABLE,
                &data,
                &auth,
            ),
            Err(Check::ProgramDataMismatch)
        );
    }

    #[test]
    fn programdata_pda_is_seeded_by_program_id_on_the_loader() {
        let program_id = pk(7);
        let expected =
            Pubkey::find_program_address(&[program_id.as_ref()], &BPF_LOADER_UPGRADEABLE).0;
        assert_eq!(program_data_pda(&program_id), expected);
        assert_ne!(program_data_pda(&program_id), program_data_pda(&pk(8)));
    }
}
