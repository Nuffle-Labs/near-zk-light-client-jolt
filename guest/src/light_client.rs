use crate::error::Error;
pub use crate::merkle::*;
use borsh::BorshSerialize;
pub use near_crypto::{ED25519PublicKey, PublicKey, Signature};
use serde::{Deserialize, Serialize};
use types::*;

pub mod types;

pub type Header = LightClientBlockLiteView;
pub type BasicProof = RpcLightClientExecutionProofResponse;
type Result<T> = core::result::Result<T, Error>;

pub const NUM_BLOCK_PRODUCER_SEATS: usize = 50;

#[derive(Debug)]
pub struct Synced {
    pub new_head: Header,
    pub next_bps: Option<(EpochId, Vec<ValidatorStake>)>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Proof {
    Basic {
        head_block_root: Hash,
        proof: Box<BasicProof>,
    },
}

impl From<(Hash, BasicProof)> for Proof {
    fn from((head_block_root, proof): (Hash, BasicProof)) -> Self {
        Self::Basic {
            head_block_root,
            proof: Box::new(proof),
        }
    }
}

impl Proof {
    pub fn block_merkle_root(&self) -> &Hash {
        match self {
            Self::Basic {
                head_block_root, ..
            } => head_block_root,
        }
    }
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
    pub fn inclusion_proof_verify(proof: Proof) -> Result<bool> {
        match proof {
            Proof::Basic {
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
            //temp_vec.extend_from_slice(&(endorsement.try_to_vec().ok()?[..]));
            temp_vec.extend_from_slice(&((block_view.inner_lite.height + 2).to_le_bytes()[..]));
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
            Some(signature) if signature.verify(msg, pk) => Ok(()),
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StakeInfo {
    pub total: u128,
    pub approved: u128,
}

impl From<(u128, u128)> for StakeInfo {
    fn from((total, approved): (u128, u128)) -> Self {
        Self { total, approved }
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

#[cfg(test)]
mod tests {
    use itertools::Itertools;
    use near_jsonrpc_primitives::types::light_client::RpcLightClientExecutionProofResponse;
    use serde_json::{self};
    use test_utils::*;

    use super::*;

    #[test]
    fn test_sync_across_epoch_boundaries() {
        let (mut head, mut next_bps, next_block) = testnet_state();
        println!("head: {:#?}", head.inner_lite);
        let mut next_epoch_id = EpochId(head.inner_lite.next_epoch_id);

        let mut sync_and_update = |next_block: LightClientBlockView| {
            let sync_next = Protocol::sync(&head, &next_bps[..], next_block.clone()).unwrap();
            // Assert we matched the epoch id for the new BPS
            assert_eq!(
                head.inner_lite.next_epoch_id,
                sync_next.next_bps.as_ref().unwrap().0 .0
            );

            println!("new head: {:#?}", sync_next.new_head.inner_lite);

            head = sync_next.new_head;
            next_bps = sync_next.next_bps.unwrap().1;

            // Assert new head is the new block
            assert_eq!(head.inner_lite, next_block.inner_lite);
            // Assert new BPS is from the next block producers because we're
            // in an epoch boundary
            assert_eq!(
                &next_bps,
                &next_block
                    .next_bps
                    .unwrap()
                    .into_iter()
                    .map(Into::into)
                    .collect_vec()
            );
            next_epoch_id.0 = head.inner_lite.next_epoch_id;
        };

        // Do first sync
        sync_and_update(next_block.clone());

        // Get next header, do next sync
        let next_block = test_last();
        sync_and_update(next_block.body);
    }

    #[test]
    fn test_validate_already_verified() {
        let (head, _, _) = test_state();
        assert_eq!(
            Protocol::ensure_not_already_verified(&head, &BlockHeight::MIN),
            Err(Error::BlockAlreadyVerified)
        );
    }

    #[test]
    fn test_validate_bad_epoch() {
        let (head, _, _) = test_state();
        assert_eq!(
            Protocol::ensure_epoch_is_current_or_next(&head, &Hash::hash_bytes(b"bogus hash")),
            Err(Error::BlockNotCurrentOrNextEpoch)
        );
    }

    #[test]
    fn test_next_epoch_bps_invalid() {
        let (head, _, mut next_block) = test_state();
        next_block.next_bps = None;

        assert_eq!(
            Protocol::ensure_if_next_epoch_contains_next_bps(
                &head,
                &next_block.inner_lite.epoch_id,
                &next_block.next_bps
            ),
            Err(Error::NextBpsInvalid)
        );
    }

    #[test]
    fn test_next_invalid_signature() {
        let (_, next_bps, next_block) = test_state();
        assert_eq!(
            Protocol::validate_signature(
                &b"bogus approval message"[..],
                &next_block.approvals_after_next[0],
                next_bps[0].public_key(),
            ),
            Err(Error::SignatureInvalid)
        );
    }

    #[test]
    fn test_next_invalid_signatures_no_approved_stake() {
        let (_, next_bps, mut next_block) = test_state();

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
            &next_bps.clone(),
            &approval_message.unwrap(),
        );

        assert_eq!((total, approved), (440511369730158962073902098744970, 0));
    }

    #[test]
    fn test_next_invalid_signatures_stake_isnt_sufficient() {
        let (_, next_bps, next_block) = test_state();

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

        assert_eq!(https://github.com/sp1-patches/curve25519-dalek.git
            Protocol::ensure_stake_is_sufficient(&total, &(min_approval_amount - 1)),
            Err(Error::NotEnoughApprovedStake)
        );
    }

    #[test]
    fn test_next_bps_invalid_hash() {
        let (_, _, next_block) = test_state();

        assert_eq!(
            Protocol::ensure_next_bps_is_valid(&Hash::hash_borsh(b"invalid"), next_block.next_bps),
            Err(Error::NextBpsInvalid)
        );
    }

    #[test]
    fn test_next_bps() {
        let (_, _, next_block) = test_state();

        assert_eq!(
            Protocol::ensure_next_bps_is_valid(
                &next_block.inner_lite.next_bp_hash,
                next_block.next_bps.clone()
            )
            .unwrap(),
            next_block.next_bps
        );
    }

    #[test]
    fn test_next_bps_noop_on_empty() {
        let (_, _, next_block) = test_state();
        assert_eq!(
            Protocol::ensure_next_bps_is_valid(&next_block.inner_lite.next_bp_hash, None).unwrap(),
            None
        );
    }

    #[test]
    fn test_outcome_root() {
        let req = r#"{"outcome_proof":{"proof":[],"block_hash":"5CY72FinjVV2Hd5zRikYYMaKh67pftXJsw8vwRXAUAQF","id":"9UhBumQ3eEmPH5ALc3NwiDCQfDrFakteRD7rHE9CfZ32","outcome":{"logs":[],"receipt_ids":["2mrt6jXKwWzkGrhucAtSc8R3mjrhkwCjnqVckPdCMEDo"],"gas_burnt":2434069818500,"tokens_burnt":"243406981850000000000","executor_id":"datayalla.testnet","status":{"SuccessReceiptId":"2mrt6jXKwWzkGrhucAtSc8R3mjrhkwCjnqVckPdCMEDo"},"metadata":{"version":1,"gas_profile":null}}},"outcome_root_proof":[{"hash":"9f7YjLvzvSspJMMJ3DDTrFaEyPQ5qFqQDNoWzAbSTjTy","direction":"Right"},{"hash":"67ZxFmzWXbWJSyi7Wp9FTSbbJx2nMr7wSuW3EP1cJm4K","direction":"Left"}],"block_header_lite":{"prev_block_hash":"AEnTyGRrk2roQkYSWoqYhzkbp5SWWJtCd71ZYyj1P26i","inner_rest_hash":"G25j8jSWRyrXV317cPC3qYA4SyJWXsBfErjhBYQkxw5A","inner_lite":{"height":134481525,"epoch_id":"4tBzDozzGED3QiCRURfViVuyJy5ikaN9dVH7m2MYkTyw","next_epoch_id":"9gYJSiT3TQbKbwui5bdbzBA9PCMSSfiffWhBdMtcasm2","prev_state_root":"EwkRecSP8GRvaxL7ynCEoHhsL1ksU6FsHVLCevcccF5q","outcome_root":"8Eu5qpDUMpW5nbmTrTKmDH2VYqFEHTKPETSTpPoyGoGc","timestamp":1691615068679535000,"timestamp_nanosec":"1691615068679535094","next_bp_hash":"8LCFsP6LeueT4X3PEni9CMvH7maDYpBtfApWZdXmagss","block_merkle_root":"583vb6csYnczHyt5z6Msm4LzzGkceTZHdvXjC8vcWeGK"}},"block_proof":[{"hash":"AEnTyGRrk2roQkYSWoqYhzkbp5SWWJtCd71ZYyj1P26i","direction":"Left"},{"hash":"HgZaHXpb5zs4rxUQTeW69XBNLBJoo4sz2YEDh7aFnMpC","direction":"Left"},{"hash":"EYNXYsnESQkXo7B27a9xu6YgbDSyynNcByW5Q2SqAaKH","direction":"Right"},{"hash":"AbKbsD7snoSnmzAtwNqXLBT5sm7bZr48GCCLSdksFuzi","direction":"Left"},{"hash":"7KKmS7n3MtCfv7UqciidJ24Abqsk8m85jVQTh94KTjYS","direction":"Left"},{"hash":"5nKA1HCZMJbdCccZ16abZGEng4sMoZhKez74rcCFjnhL","direction":"Left"},{"hash":"BupagAycSLD7v42ksgMKJFiuCzCdZ6ksrGLwukw7Vfe3","direction":"Right"},{"hash":"D6v37P4kcVJh8N9bV417eqJoyMeQbuZ743oNsbKxsU7z","direction":"Right"},{"hash":"8sWxxbe1rdquP5VdYfQbw1UvtcXDRansJYJV5ySzyow4","direction":"Right"},{"hash":"CmKVKWRqEqi4UaeKKYXpPSesYqdQYwHQM3E4xLKEUAj8","direction":"Left"},{"hash":"3TvjFzVyPBvPpph5zL6VCASLCxdNeiKV6foPwUpAGqRv","direction":"Left"},{"hash":"AnzSG9f91ePS6L6ii3eAkocp4iKjp6wjzSwWsDYWLnMX","direction":"Right"},{"hash":"FYVJDL4T6c87An3pdeBvntB68NzpcPtpvLP6ifjxxNkr","direction":"Left"},{"hash":"2YMF6KE8XTz7Axj3uyAoFbZisWej9Xo8mxgVtauWCZaV","direction":"Left"},{"hash":"4BHtLcxqNfWSneBdW76qsd8om8Gjg58Qw5BX8PHz93hf","direction":"Left"},{"hash":"7G3QUT7NQSHyXNQyzm8dsaYrFk5LGhYaG7aVafKAekyG","direction":"Left"},{"hash":"3XaMNnvnX69gGqBJX43Na1bSTJ4VUe7z6h5ZYJsaSZZR","direction":"Left"},{"hash":"FKu7GtfviPioyAGXGZLBVTJeG7KY5BxGwuL447oAZxiL","direction":"Right"},{"hash":"BePd7DPKUQnGtnSds5fMJGBUwHGxSNBpaNLwceJGUcJX","direction":"Left"},{"hash":"2BVKWMd9pXZTEyE9D3KL52hAWAyMrXj1NqutamyurrY1","direction":"Left"},{"hash":"EWavHKhwQiT8ApnXvybvc9bFY6aJYJWqBhcrZpubKXtA","direction":"Left"},{"hash":"83Fsd3sdx5tsJkb6maBE1yViKiqbWCCNfJ4XZRsKnRZD","direction":"Left"},{"hash":"AaT9jQmUvVpgDHdFkLR2XctaUVdTti49enmtbT5hsoyL","direction":"Left"}]}"#;
        let p: RpcLightClientExecutionProofResponse = serde_json::from_str(req).unwrap();

        let outcome_hash = hash_borsh(p.outcome_proof.to_hashes());

        let root_matches = Protocol::verify_outcome(
            &outcome_hash,
            p.outcome_proof.proof.iter(),
            p.outcome_root_proof.iter(),
            &p.block_header_lite.inner_lite.outcome_root,
        );
        assert!(root_matches);
    }
}
