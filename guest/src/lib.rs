#![cfg_attr(feature = "guest", no_std)]
#![no_main]

pub use crate::light_client::Protocol;
pub use error::Error;
pub use types::{
    combine_hash, hash, hash_borsh, BasicProof, Hash, Header, LcProof, LightClientBlockView,
    StakeInfo, ValidatorStake,
};

#[cfg(feature = "std")]
pub use near_primitives;

mod error;
mod light_client;

pub mod prelude {
    pub extern crate alloc;
    pub use alloc::*;
    pub use vec::Vec;
}

#[jolt::provable]
fn sync(head: Header, epoch_bps: Vec<ValidatorStake>, next_block: LightClientBlockView) -> bool {
    Protocol::sync(&head, &epoch_bps, next_block).is_ok()
}

#[jolt::provable]
fn fib(n: u32) -> u128 {
    let mut a: u128 = 0;
    let mut b: u128 = 1;
    let mut sum: u128;
    for _ in 1..n {
        sum = a + b;
        a = b;
        b = sum;
    }

    b
}

#[jolt::provable]
fn validate_already_verified(head: Header) {
    assert_eq!(
        Protocol::ensure_not_already_verified(&head, &1),
        Err(Error::BlockAlreadyVerified)
    );
}

#[jolt::provable]
fn validate_bad_epoch(head: Header) {
    assert_eq!(
        Protocol::ensure_epoch_is_current_or_next(&head, &hash(b"bogus hash")),
        Err(Error::BlockNotCurrentOrNextEpoch)
    );
}

#[jolt::provable]
fn next_epoch_bps_invalid(head: Header, next_block: LightClientBlockView) {
    assert_eq!(
        Protocol::ensure_if_next_epoch_contains_next_bps(
            &head,
            &next_block.inner_lite.epoch_id,
            &next_block.next_bps
        ),
        Err(Error::NextBpsInvalid)
    );
}

#[jolt::provable]
fn next_invalid_signature(next_block: LightClientBlockView, next_bps: Vec<ValidatorStake>) {
    assert_eq!(
        Protocol::validate_signature(
            &b"bogus approval message"[..],
            &next_block.approvals_after_next[0],
            &next_bps[0].public_key,
        ),
        Err(Error::SignatureInvalid)
    );
}

#[jolt::provable]
fn next_invalid_signatures_no_approved_stake(
    next_block: LightClientBlockView,
    next_bps: Vec<ValidatorStake>,
) {
    let mut next_block = next_block.clone();
    let approval_message = Protocol::reconstruct_approval_message(&next_block);
    // Nobody signed anything
    next_block.approvals_after_next = next_block
        .approvals_after_next
        .iter()
        .cloned()
        .map(|_| None)
        .collect();

    let StakeInfo { total, approved } = Protocol::validate_signatures(
        &next_block.approvals_after_next,
        &next_bps[..],
        &approval_message.unwrap(),
    );

    assert_eq!((total, approved), (440511369730158962073902098744970, 0));
}

#[jolt::provable]
fn next_invalid_signatures_stake_isnt_sufficient(
    next_block: LightClientBlockView,
    next_bps: Vec<ValidatorStake>,
) {
    let approval_message = Protocol::reconstruct_approval_message(&next_block);

    let StakeInfo { total, approved } = Protocol::validate_signatures(
        &next_block.approvals_after_next,
        &next_bps[..],
        &approval_message.unwrap(),
    );

    assert_eq!(
        (total, approved),
        (
            440511369730158962073902098744970,
            296239000750863364078617965755968
        )
    );

    assert!(Protocol::ensure_stake_is_sufficient(&total, &approved).is_ok());

    let min_approval_amount = (total / 3) * 2;

    assert_eq!(
        Protocol::ensure_stake_is_sufficient(&total, &(min_approval_amount - 1)),
        Err(Error::NotEnoughApprovedStake)
    );
}

#[jolt::provable]
fn next_bps_invalid_hash(next_block: LightClientBlockView) {
    assert_eq!(
        Protocol::ensure_next_bps_is_valid(&hash_borsh(b"invalid"), next_block.next_bps.clone()),
        Err(Error::NextBpsInvalid)
    );
}

#[jolt::provable]
fn next_bps(next_block: LightClientBlockView) {
    assert_eq!(
        Protocol::ensure_next_bps_is_valid(
            &next_block.inner_lite.next_bp_hash,
            next_block.next_bps.clone()
        )
        .unwrap(),
        next_block.next_bps
    );
}

#[jolt::provable]
fn next_bps_noop_on_empty(next_block: LightClientBlockView) {
    assert_eq!(
        Protocol::ensure_next_bps_is_valid(&next_block.inner_lite.next_bp_hash, None).unwrap(),
        None
    );
}

#[jolt::provable]
fn outcome_root(p: BasicProof) {
    let outcome_hash = hash_borsh(p.outcome_proof.outcome.to_hashes(p.outcome_proof.id));

    let root_matches = Protocol::verify_outcome(
        &outcome_hash,
        p.outcome_proof.proof.iter(),
        p.outcome_root_proof.iter(),
        &p.block_header_lite.inner_lite.outcome_root,
    );
    assert!(root_matches);
}
