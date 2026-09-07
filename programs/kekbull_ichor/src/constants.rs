//! Cluster-identical program IDs and the measured pump.fun bonding-curve core.
//!
//! These are protocol constants, not mint parameters. Mint decimals, supply,
//! and authorities are read from accounts. Do not add a hardcoded KEKBULL or
//! ICHOR mint here - those are written into Config at initialize.

use anchor_lang::prelude::*;

/// Official pump.fun program. Byte-identical on devnet and mainnet
/// (`docs/TESTING.md`). Do not substitute a third-party fork.
pub const PUMP_PROGRAM: Pubkey = pubkey!("6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P");

/// Published SPL Governance / Realms instances
/// (`https://docs.realms.today/developer-resources/sdk/sdk-dao-creation`).
/// DAO destination commitment accepts `REALMS_PROGRAM` (GovER5) **or**
/// `REALMS_KEKBULL_PROGRAM` (TransferFee mint fork). `REALMS_TEST_PROGRAM`
/// (GTesT) is a published test instance for rehearsal / network naming -
/// never a commitment target or GovER5 / mainnet proof.
/// A caller-supplied fake program that owns a 65-byte lookalike Governance
/// account is rejected.
pub const REALMS_PROGRAM: Pubkey = pubkey!("GovER5Lthms3bLBqWub97yVrMmEogzX7xNjdXpPPCVZw");
/// Published test instance. Same bytecode family; used by the official-devnet
/// harness `realmsInstance=test`. Not an arbitrary program id. Not accepted
/// by write-once DAO destination commitment.
pub const REALMS_TEST_PROGRAM: Pubkey = pubkey!("GTesTBiEWE32WHXXE2S4XbZvA5CrEc4xs6ZgRe895dP");
/// KEKBULL-operated SPL Governance 3.1.2 fork (TransferFeeConfig mint assert).
/// Official-devnet program id - keep in sync with
/// `ichor/client/src/network.ts` `REALMS_INSTANCES.kekbull.id` and
/// `ichor/kekbull_governance/PROGRAM_ID.txt`.
pub const REALMS_KEKBULL_PROGRAM: Pubkey =
    pubkey!("2uNHeSLiNn6dLLtiGrpCd8UZKBfV36kap57eg9kV39Fj");

/// `GovernanceAccountType::RealmV1` / `RealmV2` (Borsh u8). Counted from
/// `spl-governance` `state/enums.rs` with `GovernanceV2 = 18` as the
/// already-pinned account-governance discriminant.
pub const REALM_ACCOUNT_TYPE_V1: u8 = 1;
pub const REALM_ACCOUNT_TYPE_V2: u8 = 16;

/// Legacy SPL Token program. Rejected for ICHOR; both mints are Token-2022.
pub const TOKEN_PROGRAM_ID: Pubkey = pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");

/// Token-2022 program - KEKBULL (`create_v2`) and ICHOR owner.
pub const TOKEN_2022_PROGRAM_ID: Pubkey = pubkey!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");

/// BPF upgradeable loader (v3). Owner of the Program and ProgramData accounts.
pub const BPF_LOADER_UPGRADEABLE: Pubkey = pubkey!("BPFLoaderUpgradeab1e11111111111111111111111");

/// PDA seed for the single Config account. Mint authority for ICHOR is this PDA.
pub const CONFIG_SEED: &[u8] = b"config";

/// PDA seed for the withdraw-withheld authority. Distinct from Config.
pub const WITHDRAW_WITHHELD_SEED: &[u8] = b"withdraw-withheld";

/// PDA seed for the creator-fee escrow authority. Distinct from Config
/// and from withdraw-withheld. Owns the canonical Token-2022 ICHOR ATA
/// that accrues the 25% creator share until claim or sweep.
pub const CREATOR_ESCROW_SEED: &[u8] = b"creator-escrow";

/// Official SPL Governance native-treasury PDA seed
/// (`getNativeTreasuryAddress(program, governance)`).
pub const NATIVE_TREASURY_SEED: &[u8] = b"native-treasury";

/// Pilot / initialize transfer-fee rate. 25 bps = 0.25%.
pub const TRANSFER_FEE_BASIS_POINTS: u16 = 25;
/// Pilot / initialize maximum transfer fee. No per-transfer cap.
pub const TRANSFER_FEE_MAXIMUM_FEE: u64 = u64::MAX;

/// Finding 2.5 ceiling: human ICHOR per human KEKBULL cannot exceed 1
/// (`numerator <= denominator`). PLAN §10's burn-trace claim is otherwise
/// vacuously true at `u64::MAX : 1`.
pub const MAX_EMISSION_RATIO_NUMERATOR_PER_DENOMINATOR: u64 = 1;

/// Harvested-fee split: 25% creator / 75% Realms. Remainder after
/// `floor(amount * CREATOR_BPS / 10_000)` is paid to Realms.
pub const FEE_SPLIT_CREATOR_BPS: u16 = 2_500;
pub const FEE_SPLIT_REALMS_BPS: u16 = 7_500;
pub const FEE_SPLIT_BPS_DENOMINATOR: u16 = 10_000;

/// Token-2022 mint: 82-byte base, zero padding to `Account::LEN` (165), then
/// `AccountType`. Numeric values match `TOKEN_ACCOUNT_BASE_LEN` below.
pub const TOKEN_2022_ACCOUNT_TYPE_OFFSET: usize = 165;
pub const TOKEN_2022_TLV_OFFSET: usize = 166;
pub const TOKEN_2022_ACCOUNT_TYPE_UNINITIALIZED: u8 = 0;
pub const TOKEN_2022_ACCOUNT_TYPE_MINT: u8 = 1;
pub const TOKEN_2022_ACCOUNT_TYPE_ACCOUNT: u8 = 2;

/// `spl_token_2022::extension::ExtensionType` discriminants we parse.
pub const EXTENSION_TYPE_UNINITIALIZED: u16 = 0;
pub const EXTENSION_TYPE_TRANSFER_FEE_CONFIG: u16 = 1;
pub const EXTENSION_TYPE_TRANSFER_FEE_AMOUNT: u16 = 2;
pub const EXTENSION_TYPE_MINT_CLOSE_AUTHORITY: u16 = 3;
pub const EXTENSION_TYPE_TRANSFER_HOOK: u16 = 14;
pub const EXTENSION_TYPE_METADATA_POINTER: u16 = 18;

/// Packed `TransferFee` / `TransferFeeConfig` (Pod, align-1).
pub const TRANSFER_FEE_LEN: usize = 18;
pub const TRANSFER_FEE_CONFIG_LEN: usize = 108;
pub const TRANSFER_FEE_OFF_EPOCH: usize = 0;
pub const TRANSFER_FEE_OFF_MAXIMUM_FEE: usize = 8;
pub const TRANSFER_FEE_OFF_BASIS_POINTS: usize = 16;
pub const TRANSFER_FEE_CONFIG_OFF_AUTHORITY: usize = 0;
pub const TRANSFER_FEE_CONFIG_OFF_WITHDRAW: usize = 32;
pub const TRANSFER_FEE_CONFIG_OFF_WITHHELD: usize = 64;
pub const TRANSFER_FEE_CONFIG_OFF_OLDER: usize = 72;
pub const TRANSFER_FEE_CONFIG_OFF_NEWER: usize = 90;

/// Pump bonding-curve PDA seed. Matches `src/pumpfun/program.rs` and the
/// official IDL (`bonding-curve` + mint, program `PUMP_PROGRAM`).
pub const BONDING_CURVE_SEED: &[u8] = b"bonding-curve";

/// Anchor account discriminator for pump.fun `BondingCurve`, from
/// `docs/idl/pump.json`.
pub const BONDING_CURVE_DISCRIMINATOR: [u8; 8] = [23, 183, 248, 55, 96, 216, 172, 96];

/// 8-byte discriminator + five u64 reserves/supply + `complete` + `creator`.
/// Trailing fields (`is_mayhem`, `is_cashback`, `quote_mint`, …) exist on
/// current 151-byte accounts and are ignored. Graduation is decided from this
/// core only.
pub const BONDING_CURVE_CORE_LEN: usize = 81;

pub const CURVE_OFF_DISCRIMINATOR: usize = 0;
pub const CURVE_OFF_VIRTUAL_TOKEN_RESERVES: usize = 8;
pub const CURVE_OFF_VIRTUAL_QUOTE_RESERVES: usize = 16;
pub const CURVE_OFF_REAL_TOKEN_RESERVES: usize = 24;
pub const CURVE_OFF_REAL_QUOTE_RESERVES: usize = 32;
pub const CURVE_OFF_TOKEN_TOTAL_SUPPLY: usize = 40;
pub const CURVE_OFF_COMPLETE: usize = 48;
pub const CURVE_OFF_CREATOR: usize = 49;

/// SPL / Token-2022 mint base layout (82 bytes). Extensions, if any, follow.
pub const MINT_BASE_LEN: usize = 82;
pub const MINT_OFF_AUTHORITY: usize = 0;
pub const MINT_OFF_SUPPLY: usize = 36;
pub const MINT_OFF_DECIMALS: usize = 44;
pub const MINT_OFF_IS_INITIALIZED: usize = 45;
pub const MINT_OFF_FREEZE_AUTHORITY: usize = 46;

/// SPL / Token-2022 token-account base layout (165 bytes).
pub const TOKEN_ACCOUNT_BASE_LEN: usize = 165;
pub const TOKEN_ACCOUNT_OFF_MINT: usize = 0;
pub const TOKEN_ACCOUNT_OFF_OWNER: usize = 32;
pub const TOKEN_ACCOUNT_OFF_AMOUNT: usize = 64;
pub const TOKEN_ACCOUNT_OFF_STATE: usize = 108;

pub const TOKEN_ACCOUNT_STATE_UNINITIALIZED: u8 = 0;
pub const TOKEN_ACCOUNT_STATE_INITIALIZED: u8 = 1;
pub const TOKEN_ACCOUNT_STATE_FROZEN: u8 = 2;

/// COption tag: 0 = None, 1 = Some. SPL packs this as 4 little-endian bytes.
pub const COPTION_NONE: u32 = 0;
pub const COPTION_SOME: u32 = 1;

pub fn bonding_curve_pda(mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[BONDING_CURVE_SEED, mint.as_ref()], &PUMP_PROGRAM)
}

pub fn config_pda(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[CONFIG_SEED], program_id)
}

pub fn withdraw_withheld_pda(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[WITHDRAW_WITHHELD_SEED], program_id)
}

pub fn creator_escrow_pda(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[CREATOR_ESCROW_SEED], program_id)
}

/// Realms native treasury: `["native-treasury", governance]` on a **published**
/// Realms program (`REALMS_PROGRAM` or `REALMS_TEST_PROGRAM`). PDA math is
/// instance-specific; commitment still requires GovER5.
pub fn native_treasury_pda(realms_program: &Pubkey, governance: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[NATIVE_TREASURY_SEED, governance.as_ref()], realms_program)
}

/// Named published instances. Does not authorize DAO commitment.
pub fn is_published_realms_program(id: &Pubkey) -> bool {
    *id == REALMS_PROGRAM || *id == REALMS_TEST_PROGRAM
}

/// Write-once DAO destination commitment accepts GovER5 or the kekbull fork.
pub fn is_commitment_realms_program(id: &Pubkey) -> bool {
    *id == REALMS_PROGRAM || *id == REALMS_KEKBULL_PROGRAM
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_ids_are_the_canonical_base58() {
        assert_eq!(
            PUMP_PROGRAM.to_string(),
            "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P"
        );
        assert_eq!(
            TOKEN_PROGRAM_ID.to_string(),
            "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
        );
        assert_eq!(
            TOKEN_2022_PROGRAM_ID.to_string(),
            "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
        );
        assert_eq!(
            BPF_LOADER_UPGRADEABLE.to_string(),
            "BPFLoaderUpgradeab1e11111111111111111111111"
        );
        assert_eq!(
            REALMS_PROGRAM.to_string(),
            "GovER5Lthms3bLBqWub97yVrMmEogzX7xNjdXpPPCVZw"
        );
        assert_eq!(
            REALMS_TEST_PROGRAM.to_string(),
            "GTesTBiEWE32WHXXE2S4XbZvA5CrEc4xs6ZgRe895dP"
        );
        assert_eq!(
            REALMS_KEKBULL_PROGRAM.to_string(),
            "2uNHeSLiNn6dLLtiGrpCd8UZKBfV36kap57eg9kV39Fj"
        );
        assert!(is_published_realms_program(&REALMS_PROGRAM));
        assert!(is_published_realms_program(&REALMS_TEST_PROGRAM));
        assert!(!is_published_realms_program(&PUMP_PROGRAM));
        assert!(!is_published_realms_program(&REALMS_KEKBULL_PROGRAM),
            "kekbull is commitment-eligible; published-for-naming stays GovER5|GTesT");
        assert!(is_commitment_realms_program(&REALMS_PROGRAM));
        assert!(
            is_commitment_realms_program(&REALMS_KEKBULL_PROGRAM),
            "kekbull fork is an allowed write-once DAO destination"
        );
        assert!(
            !is_commitment_realms_program(&REALMS_TEST_PROGRAM),
            "GTesT is published for rehearsal/network naming, not DAO commitment"
        );
        assert!(!is_commitment_realms_program(&PUMP_PROGRAM));
        assert_eq!(REALM_ACCOUNT_TYPE_V1, 1);
        assert_eq!(REALM_ACCOUNT_TYPE_V2, 16);
        assert_eq!(REALM_ACCOUNT_TYPE_V2 + 2, 18, "GovernanceV2 is 18");
    }

    #[test]
    fn bonding_curve_seeds_match_creatorhub() {
        // Same seeds as `src/pumpfun/program.rs`: ["bonding-curve", mint] on
        // PUMP_PROGRAM. A seed typo produces a plausible unused address.
        let mint = Pubkey::new_from_array([7u8; 32]);
        let (a, _) = bonding_curve_pda(&mint);
        let (b, _) =
            Pubkey::find_program_address(&[b"bonding-curve", mint.as_ref()], &PUMP_PROGRAM);
        assert_eq!(a, b);

        let (wrong_seed, _) =
            Pubkey::find_program_address(&[b"bonding_curve", mint.as_ref()], &PUMP_PROGRAM);
        assert_ne!(a, wrong_seed, "underscore vs hyphen must not collide");

        let (wrong_program, _) =
            Pubkey::find_program_address(&[BONDING_CURVE_SEED, mint.as_ref()], &TOKEN_PROGRAM_ID);
        assert_ne!(a, wrong_program);

        let other = Pubkey::new_from_array([8u8; 32]);
        assert_ne!(bonding_curve_pda(&mint).0, bonding_curve_pda(&other).0);
        assert_eq!(bonding_curve_pda(&mint).0, bonding_curve_pda(&mint).0);
    }

    #[test]
    fn config_pda_is_seeded_only_by_config_and_program() {
        let program_a = Pubkey::new_from_array([1u8; 32]);
        let program_b = Pubkey::new_from_array([2u8; 32]);
        assert_eq!(config_pda(&program_a).0, config_pda(&program_a).0);
        assert_ne!(config_pda(&program_a).0, config_pda(&program_b).0);
        let (wrong, _) = Pubkey::find_program_address(&[b"Config"], &program_a);
        assert_ne!(config_pda(&program_a).0, wrong);
    }

    #[test]
    fn withdraw_withheld_pda_is_not_the_config_pda() {
        let program = Pubkey::new_from_array([1u8; 32]);
        assert_ne!(withdraw_withheld_pda(&program).0, config_pda(&program).0);
        assert_eq!(
            withdraw_withheld_pda(&program).0,
            Pubkey::find_program_address(&[WITHDRAW_WITHHELD_SEED], &program).0
        );
        let (wrong, _) = Pubkey::find_program_address(&[b"withdraw_withheld"], &program);
        assert_ne!(withdraw_withheld_pda(&program).0, wrong);
    }

    #[test]
    fn creator_escrow_pda_is_not_config_or_withdraw_withheld() {
        let program = Pubkey::new_from_array([1u8; 32]);
        let escrow = creator_escrow_pda(&program).0;
        assert_ne!(escrow, config_pda(&program).0);
        assert_ne!(escrow, withdraw_withheld_pda(&program).0);
        assert_eq!(
            escrow,
            Pubkey::find_program_address(&[CREATOR_ESCROW_SEED], &program).0
        );
        let (wrong, _) = Pubkey::find_program_address(&[b"creator_escrow"], &program);
        assert_ne!(escrow, wrong);
    }

    #[test]
    fn native_treasury_pda_uses_official_realms_seed() {
        let realms = Pubkey::new_from_array([9u8; 32]);
        let governance = Pubkey::new_from_array([8u8; 32]);
        let (got, _) = native_treasury_pda(&realms, &governance);
        let (expected, _) =
            Pubkey::find_program_address(&[b"native-treasury", governance.as_ref()], &realms);
        assert_eq!(got, expected);
        let (wrong_seed, _) =
            Pubkey::find_program_address(&[b"treasury", governance.as_ref()], &realms);
        assert_ne!(got, wrong_seed);
        let other_gov = Pubkey::new_from_array([7u8; 32]);
        assert_ne!(
            native_treasury_pda(&realms, &governance).0,
            native_treasury_pda(&realms, &other_gov).0
        );
    }

    #[test]
    fn approved_fee_constants() {
        assert_eq!(TRANSFER_FEE_BASIS_POINTS, 25);
        assert_eq!(TRANSFER_FEE_MAXIMUM_FEE, u64::MAX);
        assert_eq!(FEE_SPLIT_CREATOR_BPS, 2_500);
        assert_eq!(FEE_SPLIT_REALMS_BPS, 7_500);
        assert_eq!(
            FEE_SPLIT_CREATOR_BPS + FEE_SPLIT_REALMS_BPS,
            FEE_SPLIT_BPS_DENOMINATOR
        );
        assert_eq!(TOKEN_2022_ACCOUNT_TYPE_OFFSET, TOKEN_ACCOUNT_BASE_LEN);
        assert_eq!(TOKEN_2022_TLV_OFFSET, TOKEN_ACCOUNT_BASE_LEN + 1);
        assert_eq!(
            TRANSFER_FEE_CONFIG_OFF_NEWER + TRANSFER_FEE_LEN,
            TRANSFER_FEE_CONFIG_LEN
        );
    }

    #[test]
    fn core_offsets_are_the_measured_81_byte_layout() {
        assert_eq!(BONDING_CURVE_CORE_LEN, 8 + 8 * 5 + 1 + 32);
        assert_eq!(CURVE_OFF_COMPLETE, 48);
        assert_eq!(CURVE_OFF_CREATOR, 49);
        assert_eq!(CURVE_OFF_REAL_TOKEN_RESERVES, 24);
        assert_eq!(CURVE_OFF_CREATOR + 32, BONDING_CURVE_CORE_LEN);
    }

    #[test]
    fn bonding_curve_discriminator_matches_published_idl() {
        assert_eq!(
            BONDING_CURVE_DISCRIMINATOR,
            [23, 183, 248, 55, 96, 216, 172, 96]
        );
    }
}
