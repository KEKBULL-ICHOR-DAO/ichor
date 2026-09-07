use anchor_lang::prelude::*;

#[event]
pub struct Initialized {
    pub authority: Pubkey,
    pub kekbull_mint: Pubkey,
    pub ichor_mint: Pubkey,
    pub bonding_curve: Pubkey,
    pub emission_numerator: u64,
    pub emission_denominator: u64,
    pub ratio_timelock_secs: u64,
    pub creator_decay_secs: u64,
}

#[event]
pub struct PauseSet {
    pub authority: Pubkey,
    pub paused: bool,
}

#[event]
pub struct EmissionRatioProposed {
    pub authority: Pubkey,
    pub numerator: u64,
    pub denominator: u64,
    pub unlock_ts: i64,
}

#[event]
pub struct EmissionRatioApplied {
    pub numerator: u64,
    pub denominator: u64,
}

#[event]
pub struct EmissionRatioCancelled {
    pub authority: Pubkey,
}

#[event]
pub struct RatioUpdatesFrozenEvent {
    pub authority: Pubkey,
}

#[event]
pub struct RatioTimelockIncreased {
    pub authority: Pubkey,
    pub ratio_timelock_secs: u64,
}

#[event]
pub struct PendingAuthoritySet {
    pub authority: Pubkey,
    pub pending_authority: Option<Pubkey>,
}

#[event]
pub struct AuthorityAccepted {
    pub authority: Pubkey,
}

#[event]
pub struct DaoDestinationCommitted {
    pub authority: Pubkey,
    pub realms_program: Pubkey,
    pub realms_realm: Pubkey,
    pub realms_governance: Pubkey,
    pub realms_native_treasury: Pubkey,
}

#[event]
pub struct FeeDistributionBound {
    pub authority: Pubkey,
    pub creator_beneficiary: Pubkey,
    pub realms_program: Pubkey,
    pub realms_governance: Pubkey,
    pub realms_native_treasury: Pubkey,
}

/// Measured harvest. `creator_amount` / `realms_amount` are the transfer
/// instruction sizes (25/75 of the reloaded vault). Remainder after
/// `floor(withdrawn * 2500 / 10_000)` is paid to Realms
/// (`remainder_recipient`). `creator_received` / `realms_received` are
/// destination-balance deltas after Token-2022 may withhold a second fee.
#[event]
pub struct TransferFeesDistributed {
    pub caller: Pubkey,
    pub withdrawn: u64,
    pub vault_distributed: u64,
    pub creator_amount: u64,
    pub realms_amount: u64,
    pub creator_received: u64,
    pub realms_received: u64,
    pub remainder_recipient: Pubkey,
    pub creator_destination: Pubkey,
    pub realms_destination: Pubkey,
}

#[event]
pub struct TransferFeeUpdated {
    pub authority: Pubkey,
    pub transfer_fee_basis_points: u16,
    pub maximum_fee: u64,
}

#[event]
pub struct TransferFeeAuthorityRevoked {
    pub authority: Pubkey,
}

#[event]
pub struct CreatorFeesClaimed {
    pub beneficiary: Pubkey,
    pub amount: u64,
    pub received: u64,
    pub destination: Pubkey,
    pub last_creator_claim_ts: i64,
}

#[event]
pub struct UnclaimedCreatorFeesSwept {
    pub caller: Pubkey,
    pub amount: u64,
    pub received: u64,
    pub realms_destination: Pubkey,
    pub last_creator_claim_ts: i64,
}

#[event]
pub struct CreatorBeneficiarySet {
    pub previous_beneficiary: Pubkey,
    pub new_beneficiary: Pubkey,
    pub last_creator_claim_ts: i64,
}

#[event]
pub struct Converted {
    pub burner: Pubkey,
    pub recipient: Pubkey,
    pub kekbull_mint: Pubkey,
    pub ichor_mint: Pubkey,
    pub kekbull_burned: u64,
    pub ichor_minted: u64,
    pub min_ichor_amount: u64,
    pub ratio_numerator: u64,
    pub ratio_denominator: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converted_event_records_recipient_output_and_signed_min() {
        let ev = Converted {
            burner: Pubkey::new_from_array([1u8; 32]),
            recipient: Pubkey::new_from_array([2u8; 32]),
            kekbull_mint: Pubkey::new_from_array([3u8; 32]),
            ichor_mint: Pubkey::new_from_array([4u8; 32]),
            kekbull_burned: 1_000_000,
            ichor_minted: 1_000_000_000,
            min_ichor_amount: 500_000_000,
            ratio_numerator: 1,
            ratio_denominator: 1,
        };
        assert_ne!(ev.burner, ev.recipient);
        assert!(ev.ichor_minted >= ev.min_ichor_amount);
        assert_eq!(ev.kekbull_burned, 1_000_000);
    }

    #[test]
    fn distributed_event_names_realms_as_remainder_recipient() {
        let realms = Pubkey::new_from_array([7u8; 32]);
        let ev = TransferFeesDistributed {
            caller: Pubkey::new_from_array([1u8; 32]),
            withdrawn: 101,
            vault_distributed: 101,
            creator_amount: 25,
            realms_amount: 76,
            creator_received: 25,
            realms_received: 76,
            remainder_recipient: realms,
            creator_destination: Pubkey::new_from_array([8u8; 32]),
            realms_destination: Pubkey::new_from_array([9u8; 32]),
        };
        assert_eq!(ev.creator_amount + ev.realms_amount, ev.vault_distributed);
        assert_eq!(ev.remainder_recipient, realms);
        assert_eq!(ev.realms_amount, 76);
    }
}
