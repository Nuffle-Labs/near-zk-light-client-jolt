use crate::error::Error;
use crate::prelude::*;
use alloc::boxed::Box;
use borsh::BorshSerialize;
use ed25519_dalek::Verifier;
use types::*;

type Result<T> = core::result::Result<T, Error>;

// TODO: remove
pub const NUM_BLOCK_PRODUCER_SEATS: usize = 50;

#[derive(Debug)]
pub struct Synced {
    pub new_head: Header,
    pub next_bps: Option<(EpochId, Vec<ValidatorStake>)>,
}

pub struct Protocol;

impl Protocol {
    pub fn sync(
        head: &Header,
        epoch_bps: &[ValidatorStake],
        next_block: LightClientBlockView,
    ) -> Result<Synced> {
        Self::ensure_not_already_verified(head, &next_block.inner_lite.height)?;
        Self::ensure_epoch_is_current_or_next(head, &next_block.inner_lite.epoch_id)?;
        Self::ensure_if_next_epoch_contains_next_bps(
            head,
            &next_block.inner_lite.epoch_id,
            &next_block.next_bps,
        )?;

        let new_head = Header {
            prev_block_hash: next_block.prev_block_hash,
            inner_rest_hash: next_block.inner_rest_hash,
            inner_lite: next_block.inner_lite.clone(),
        };

        let approval_message = Self::reconstruct_approval_message(&next_block).unwrap();

        let StakeInfo { total, approved } = Self::validate_signatures(
            &next_block.approvals_after_next,
            epoch_bps,
            &approval_message,
        );

        Self::ensure_stake_is_sufficient(&total, &approved)?;

        Ok(Synced {
            new_head,
            next_bps: Self::ensure_next_bps_is_valid(
                &next_block.inner_lite.next_bp_hash,
                next_block.next_bps,
            )?
            .map(|next_bps| next_bps.into_iter().map(Into::into).collect())
            .map(|next_bps| (head.inner_lite.next_epoch_id, next_bps)),
        })
        .map(|synced| synced)
    }
    pub fn inclusion_proof_verify(proof: LcProof) -> Result<bool> {
        match proof {
            LcProof::Basic {
                head_block_root,
                proof,
            } => {
                let block_hash = proof.block_header_lite.hash();
                let block_hash_matches = block_hash == proof.outcome_proof.block_hash;

                let outcome_hash = hash_borsh(
                    proof
                        .outcome_proof
                        .outcome
                        .to_hashes(proof.outcome_proof.id),
                );

                let outcome_verified = Self::verify_outcome(
                    &outcome_hash,
                    proof.outcome_proof.proof.iter(),
                    proof.outcome_root_proof.iter(),
                    &proof.block_header_lite.inner_lite.outcome_root,
                );

                let block_verified =
                    Self::verify_block(&head_block_root, proof.block_proof.iter(), &block_hash);

                Ok(block_hash_matches && outcome_verified && block_verified)
                    .map(|verified| verified)
            }
        }
    }

    pub(crate) fn verify_outcome<'a>(
        outcome_hash: &Hash,
        outcome_proof: impl Iterator<Item = &'a MerklePathItem>,
        outcome_root_proof: impl Iterator<Item = &'a MerklePathItem>,
        expected_outcome_root: &Hash,
    ) -> bool {
        let outcome_root = compute_root_from_path(outcome_proof, *outcome_hash);
        #[cfg(test)]
        println!("outcome_root: {:?}", hex::encode(outcome_root));

        let leaf = hash_borsh(outcome_root);
        #[cfg(test)]
        println!("leaf: {:?}", hex::encode(leaf));

        let outcome_root = compute_root_from_path(outcome_root_proof, leaf);
        #[cfg(test)]
        println!("outcome_root: {:?}", hex::encode(outcome_root));

        &outcome_root == expected_outcome_root
    }

    pub(crate) fn verify_block<'a>(
        block_merkle_root: &Hash,
        block_proof: impl Iterator<Item = &'a MerklePathItem>,
        block_hash: &Hash,
    ) -> bool {
        verify_hash(*block_merkle_root, block_proof, *block_hash)
    }

    pub fn reconstruct_approval_message(block_view: &LightClientBlockView) -> Option<Vec<u8>> {
        let new_head = Header {
            prev_block_hash: block_view.prev_block_hash,
            inner_rest_hash: block_view.inner_rest_hash,
            inner_lite: block_view.inner_lite.clone(),
        };

        let next_block_hash = combine_hash(&block_view.next_block_inner_hash, &new_head.hash());

        let endorsement = ApprovalInner::Endorsement(next_block_hash);

        let approval_message = {
            let mut temp_vec = Vec::new();
            BorshSerialize::serialize(&endorsement, &mut temp_vec).ok()?;
            temp_vec.extend_from_slice(&((block_view.inner_lite.height + 2).to_le_bytes()[..]));
            #[cfg(test)]
            println!("temp_vec len: {:?}", temp_vec.len());
            temp_vec
        };

        Option::Some(approval_message)
    }

    pub fn ensure_not_already_verified(head: &Header, block_height: &BlockHeight) -> Result<()> {
        if block_height <= &head.inner_lite.height {
            Err(Error::BlockAlreadyVerified)
        } else {
            Ok(())
        }
    }

    pub fn ensure_epoch_is_current_or_next(head: &Header, epoch_id: &Hash) -> Result<()> {
        if ![head.inner_lite.epoch_id, head.inner_lite.next_epoch_id].contains(epoch_id) {
            Err(Error::BlockNotCurrentOrNextEpoch)
        } else {
            Ok(())
        }
    }

    pub fn ensure_if_next_epoch_contains_next_bps(
        head: &Header,
        epoch_id: &Hash,
        next_bps: &Option<Vec<ValidatorStakeView>>,
    ) -> Result<()> {
        if &head.inner_lite.next_epoch_id == epoch_id && next_bps.is_none() {
            Err(Error::NextBpsInvalid)
        } else {
            Ok(())
        }
    }

    pub fn validate_signatures(
        signatures: &[Option<Box<Signature>>],
        epoch_bps: &[ValidatorStake],
        approval_message: &[u8],
    ) -> StakeInfo {
        signatures
            .iter()
            .zip(epoch_bps.iter())
            .take(NUM_BLOCK_PRODUCER_SEATS)
            .fold((0, 0), |(total_stake, approved_stake), (sig, vs)| {
                let pk = vs.public_key.clone();
                let stake = vs.stake;
                let total_stake = total_stake + stake;

                let approved_stake = match Self::validate_signature(approval_message, sig, &pk) {
                    Ok(_) => approved_stake + stake,
                    Err(Error::SignatureInvalid) => approved_stake,
                    Err(Error::ValidatorNotSigned) => approved_stake,
                    Err(_) => approved_stake,
                };

                (total_stake, approved_stake)
            })
            .into()
    }

    pub fn validate_signature(
        msg: &[u8],
        sig: &Option<Box<Signature>>,
        pk: &PublicKey,
    ) -> Result<()> {
        match sig {
            Some(signature) => match ed25519_dalek::VerifyingKey::from_bytes(pk) {
                Err(_) => Err(Error::SignatureInvalid),
                Ok(public_key) => Ok(public_key.verify(msg, &signature.0).unwrap()),
            },
            Some(signature) => Err(Error::SignatureInvalid),
            _ => Err(Error::ValidatorNotSigned),
        }
    }

    pub fn ensure_stake_is_sufficient(total_stake: &u128, approved_stake: &u128) -> Result<()> {
        let threshold = total_stake / 3 * 2;

        if approved_stake <= &threshold {
            Err(Error::NotEnoughApprovedStake)
        } else {
            Ok(())
        }
    }

    pub fn ensure_next_bps_is_valid(
        expected_hash: &Hash,
        next_bps: Option<Vec<ValidatorStakeView>>,
    ) -> Result<Option<Vec<ValidatorStakeView>>> {
        if let Some(next_bps) = next_bps {
            let next_bps_hash = hash_borsh(next_bps.clone());

            if &next_bps_hash == expected_hash {
                Ok(Some(next_bps))
            } else {
                Err(Error::NextBpsInvalid)
            }
        } else {
            Ok(None)
        }
    }
}

#[macro_export]
macro_rules! cvec {
	($($x:expr),*) => {
		{
			let mut temp_vec = Vec::new();
			$(
				temp_vec.extend_from_slice(&$x);
			)*
			temp_vec
		}
	};
}
