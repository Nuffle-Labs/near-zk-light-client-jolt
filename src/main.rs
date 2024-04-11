use guest::{
    hash, hash_borsh, BasicProof, Error, Hash, Header, LcProof, LightClientBlockView, Protocol,
    StakeInfo, ValidatorStake,
};
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::{self};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
pub struct LightClientFixture<T> {
    pub last_block_hash: near_primitives::hash::CryptoHash,
    pub body: T,
}

pub fn workspace_dir() -> PathBuf {
    let output = std::process::Command::new(env!("CARGO"))
        .arg("locate-project")
        .arg("--workspace")
        .arg("--message-format=plain")
        .output()
        .unwrap()
        .stdout;
    let cargo_path = Path::new(std::str::from_utf8(&output).unwrap().trim());
    cargo_path.parent().unwrap().to_path_buf()
}

pub fn fixture<T: DeserializeOwned>(file: &str) -> T {
    serde_json::from_reader(
        std::fs::File::open(format!("{}/fixtures/{}", workspace_dir().display(), file)).unwrap(),
    )
    .unwrap()
}
pub fn test_last() -> LightClientFixture<near_primitives::views::LightClientBlockView> {
    fixture("test_2.json")
}

pub fn test_next() -> LightClientFixture<near_primitives::views::LightClientBlockView> {
    fixture("test_1.json")
}

pub fn test_first() -> LightClientFixture<near_primitives::views::LightClientBlockView> {
    fixture("test_0.json")
}

pub fn test_state() -> (Header, Vec<ValidatorStake>, LightClientBlockView) {
    let first = test_first().body;
    let head = view_to_lite_view(first.clone());
    let bps = first
        .next_bps
        .unwrap()
        .into_iter()
        .map(Into::into)
        .collect();
    let next = test_next();

    (head.into(), bps, next.body.into())
}

pub fn view_to_lite_view(
    h: near_primitives::views::LightClientBlockView,
) -> near_primitives::views::LightClientBlockLiteView {
    near_primitives::views::LightClientBlockLiteView {
        prev_block_hash: h.prev_block_hash,
        inner_rest_hash: h.inner_rest_hash,
        inner_lite: h.inner_lite,
    }
}

pub fn sync() {
    let (head, bps, next_block) = test_state();
    let (prove, verify) = guest::build_sync();
    let (output, proof) = prove(head, bps, next_block);
    let is_valid = verify(proof);
    println!("output: {}", output);
    println!("valid: {}", is_valid);
    // fails on signatures
    // fails on state checks
    // fails on input len
}

pub fn main() {
    let (head, bps, next_block) = test_state();

    validate_already_verified(head.clone());
    validate_bad_epoch(head.clone());
    // next_bps_invalid_hash(next_block.clone());
    // next_epoch_bps_invalid(head, next_block.clone());
    // next_invalid_signature(next_block.clone(), bps.clone());
    // next_invalid_signatures_no_approved_stake(next_block, bps.clone());
}

pub fn validate_already_verified(head: Header) {
    let (prove, verify) = guest::build_validate_already_verified();
    let (output, proof) = prove(head);
    let is_valid = verify(proof);
    // println!("output: {}", output);
    println!("valid: {}", is_valid);
}

pub fn validate_bad_epoch(head: Header) {
    let (prove, verify) = guest::build_validate_bad_epoch();
    let (output, proof) = prove(head);
    let is_valid = verify(proof);
    // println!("output: {}", output);
    println!("valid: {}", is_valid);
}

pub fn next_epoch_bps_invalid(head: Header, next_block: LightClientBlockView) {
    let (prove, verify) = guest::build_next_epoch_bps_invalid();
    let (output, proof) = prove(head, next_block);
    let is_valid = verify(proof);
    // println!("output: {}", output);
    println!("valid: {}", is_valid);
}

pub fn next_invalid_signature(next_block: LightClientBlockView, next_bps: Vec<ValidatorStake>) {
    let (prove, verify) = guest::build_next_invalid_signature();
    let (output, proof) = prove(next_block, next_bps);
    let is_valid = verify(proof);
    // println!("output: {}", output);
    println!("valid: {}", is_valid);
}

pub fn next_invalid_signatures_no_approved_stake(
    next_block: LightClientBlockView,
    next_bps: Vec<ValidatorStake>,
) {
    let (prove, verify) = guest::build_next_invalid_signatures_no_approved_stake();
    let (output, proof) = prove(next_block, next_bps);
    let is_valid = verify(proof);
    // println!("output: {}", output);
    println!("valid: {}", is_valid);
}

pub fn next_invalid_signatures_stake_isnt_sufficient(
    next_block: LightClientBlockView,
    next_bps: Vec<ValidatorStake>,
) {
    let (prove, verify) = guest::build_next_invalid_signatures_stake_isnt_sufficient();
    let (output, proof) = prove(next_block, next_bps);
    let is_valid = verify(proof);
    // println!("output: {}", output);
    println!("valid: {}", is_valid);
}

pub fn next_bps_invalid_hash(next_block: LightClientBlockView) {
    let (prove, verify) = guest::build_next_bps_invalid_hash();
    let (output, proof) = prove(next_block);
    let is_valid = verify(proof);
    // println!("output: {}", output);
    println!("valid: {}", is_valid);
}

pub fn next_bps(next_block: LightClientBlockView) {
    let (prove, verify) = guest::build_next_bps();
    let (output, proof) = prove(next_block);
    let is_valid = verify(proof);
    // println!("output: {}", output);
    println!("valid: {}", is_valid);
}

pub fn next_bps_noop_on_empty(next_block: LightClientBlockView) {
    let (prove, verify) = guest::build_next_bps_noop_on_empty();
    let (output, proof) = prove(next_block);
    let is_valid = verify(proof);
    // println!("output: {}", output);
    println!("valid: {}", is_valid);
}

pub fn outcome_root(p: BasicProof) {
    let (prove, verify) = guest::build_outcome_root();
    let (output, proof) = prove(p);
    let is_valid = verify(proof);
    // println!("output: {}", output);
    println!("valid: {}", is_valid);
}

#[cfg(test)]
mod tests {
    use super::*;

    // fn test_sync_across_epoch_boundaries() {
    //     let (mut head, mut next_bps, next_block) = test_state();
    //     println!("head: {:#?}", head.inner_lite);
    //     let mut next_epoch_id = head.inner_lite.next_epoch_id;
    //
    //     let mut sync_and_update = |next_block: LightClientBlockView| {
    //         let sync_next = Protocol::sync(&head, &next_bps[..], next_block.clone()).unwrap();
    //         // Assert we matched the epoch id for the new BPS
    //         assert_eq!(
    //             head.inner_lite.next_epoch_id,
    //             sync_next.next_bps.as_ref().unwrap().0
    //         );
    //
    //         println!("new head: {:#?}", sync_next.new_head.inner_lite);
    //
    //         head = sync_next.new_head;
    //         next_bps = sync_next.next_bps.unwrap().1;
    //
    //         // Assert new head is the new block
    //         assert_eq!(head.inner_lite, next_block.inner_lite);
    //         // Assert new BPS is from the next block producers because we're
    //         // in an epoch boundary
    //         assert_eq!(
    //             &next_bps,
    //             &next_block
    //                 .next_bps
    //                 .unwrap()
    //                 .into_iter()
    //                 .map(Into::into)
    //                 .collect::<Vec<_>>()
    //         );
    //         next_epoch_id = head.inner_lite.next_epoch_id;
    //     };
    //
    //     // Do first sync
    //     sync_and_update(next_block.clone());
    //
    //     // Get next header, do next sync
    //     let next_block = test_last();
    //     sync_and_update(next_block.body);
    // }

    #[test]
    fn test_validate_already_verified() {
        let (head, _, _) = test_state();

        validate_already_verified(&head);
    }

    // fn test_validate_bad_epoch() {
    //     let (head, _, _) = test_state();
    //     assert_eq!(
    //         Protocol::ensure_epoch_is_current_or_next(&head, &hash(b"bogus hash")),
    //         Err(Error::BlockNotCurrentOrNextEpoch)
    //     );
    // }
    //
    // fn test_next_epoch_bps_invalid() {
    //     let (head, _, mut next_block) = test_state();
    //     next_block.next_bps = None;
    //
    //     assert_eq!(
    //         Protocol::ensure_if_next_epoch_contains_next_bps(
    //             &head,
    //             &next_block.inner_lite.epoch_id,
    //             &next_block.next_bps
    //         ),
    //         Err(Error::NextBpsInvalid)
    //     );
    // }
    //
    // fn test_next_invalid_signature() {
    //     let (_, next_bps, next_block) = test_state();
    //     assert_eq!(
    //         Protocol::validate_signature(
    //             &b"bogus approval message"[..],
    //             &next_block.approvals_after_next[0],
    //             &next_bps[0].public_key,
    //         ),
    //         Err(Error::SignatureInvalid)
    //     );
    // }
    //
    // fn test_next_invalid_signatures_no_approved_stake() {
    //     let (_, next_bps, mut next_block) = test_state();
    //
    //     let approval_message = Protocol::reconstruct_approval_message(&next_block);
    //     // Nobody signed anything
    //     next_block.approvals_after_next = next_block
    //         .approvals_after_next
    //         .iter()
    //         .cloned()
    //         .map(|_| None)
    //         .collect();
    //
    //     let StakeInfo { total, approved } = Protocol::validate_signatures(
    //         &next_block.approvals_after_next,
    //         &next_bps.clone(),
    //         &approval_message.unwrap(),
    //     );
    //
    //     assert_eq!((total, approved), (440511369730158962073902098744970, 0));
    // }
    //
    // fn test_next_invalid_signatures_stake_isnt_sufficient() {
    //     let (_, next_bps, next_block) = test_state();
    //
    //     let approval_message = Protocol::reconstruct_approval_message(&next_block);
    //
    //     let StakeInfo { total, approved } = Protocol::validate_signatures(
    //         &next_block.approvals_after_next,
    //         &next_bps[..],
    //         &approval_message.unwrap(),
    //     );
    //
    //     assert_eq!(
    //         (total, approved),
    //         (
    //             440511369730158962073902098744970,
    //             296239000750863364078617965755968
    //         )
    //     );
    //
    //     assert!(Protocol::ensure_stake_is_sufficient(&total, &approved).is_ok());
    //
    //     let min_approval_amount = (total / 3) * 2;
    //
    //     assert_eq!(
    //         Protocol::ensure_stake_is_sufficient(&total, &(min_approval_amount - 1)),
    //         Err(Error::NotEnoughApprovedStake)
    //     );
    // }
    //
    // fn test_next_bps_invalid_hash() {
    //     let (_, _, next_block) = test_state();
    //
    //     assert_eq!(
    //         Protocol::ensure_next_bps_is_valid(&hash_borsh(b"invalid"), next_block.next_bps),
    //         Err(Error::NextBpsInvalid)
    //     );
    // }
    //
    // fn test_next_bps() {
    //     let (_, _, next_block) = test_state();
    //
    //     assert_eq!(
    //         Protocol::ensure_next_bps_is_valid(
    //             &next_block.inner_lite.next_bp_hash,
    //             next_block.next_bps.clone()
    //         )
    //         .unwrap(),
    //         next_block.next_bps
    //     );
    // }
    //
    // fn test_next_bps_noop_on_empty() {
    //     let (_, _, next_block) = test_state();
    //     assert_eq!(
    //         Protocol::ensure_next_bps_is_valid(&next_block.inner_lite.next_bp_hash, None).unwrap(),
    //         None
    //     );
    // }
    //
    // fn test_outcome_root() {
    //     let req = r#"{"outcome_proof":{"proof":[],"block_hash":"5CY72FinjVV2Hd5zRikYYMaKh67pftXJsw8vwRXAUAQF","id":"9UhBumQ3eEmPH5ALc3NwiDCQfDrFakteRD7rHE9CfZ32","outcome":{"logs":[],"receipt_ids":["2mrt6jXKwWzkGrhucAtSc8R3mjrhkwCjnqVckPdCMEDo"],"gas_burnt":2434069818500,"tokens_burnt":"243406981850000000000","executor_id":"datayalla.testnet","status":{"SuccessReceiptId":"2mrt6jXKwWzkGrhucAtSc8R3mjrhkwCjnqVckPdCMEDo"},"metadata":{"version":1,"gas_profile":null}}},"outcome_root_proof":[{"hash":"9f7YjLvzvSspJMMJ3DDTrFaEyPQ5qFqQDNoWzAbSTjTy","direction":"Right"},{"hash":"67ZxFmzWXbWJSyi7Wp9FTSbbJx2nMr7wSuW3EP1cJm4K","direction":"Left"}],"block_header_lite":{"prev_block_hash":"AEnTyGRrk2roQkYSWoqYhzkbp5SWWJtCd71ZYyj1P26i","inner_rest_hash":"G25j8jSWRyrXV317cPC3qYA4SyJWXsBfErjhBYQkxw5A","inner_lite":{"height":134481525,"epoch_id":"4tBzDozzGED3QiCRURfViVuyJy5ikaN9dVH7m2MYkTyw","next_epoch_id":"9gYJSiT3TQbKbwui5bdbzBA9PCMSSfiffWhBdMtcasm2","prev_state_root":"EwkRecSP8GRvaxL7ynCEoHhsL1ksU6FsHVLCevcccF5q","outcome_root":"8Eu5qpDUMpW5nbmTrTKmDH2VYqFEHTKPETSTpPoyGoGc","timestamp":1691615068679535000,"timestamp_nanosec":"1691615068679535094","next_bp_hash":"8LCFsP6LeueT4X3PEni9CMvH7maDYpBtfApWZdXmagss","block_merkle_root":"583vb6csYnczHyt5z6Msm4LzzGkceTZHdvXjC8vcWeGK"}},"block_proof":[{"hash":"AEnTyGRrk2roQkYSWoqYhzkbp5SWWJtCd71ZYyj1P26i","direction":"Left"},{"hash":"HgZaHXpb5zs4rxUQTeW69XBNLBJoo4sz2YEDh7aFnMpC","direction":"Left"},{"hash":"EYNXYsnESQkXo7B27a9xu6YgbDSyynNcByW5Q2SqAaKH","direction":"Right"},{"hash":"AbKbsD7snoSnmzAtwNqXLBT5sm7bZr48GCCLSdksFuzi","direction":"Left"},{"hash":"7KKmS7n3MtCfv7UqciidJ24Abqsk8m85jVQTh94KTjYS","direction":"Left"},{"hash":"5nKA1HCZMJbdCccZ16abZGEng4sMoZhKez74rcCFjnhL","direction":"Left"},{"hash":"BupagAycSLD7v42ksgMKJFiuCzCdZ6ksrGLwukw7Vfe3","direction":"Right"},{"hash":"D6v37P4kcVJh8N9bV417eqJoyMeQbuZ743oNsbKxsU7z","direction":"Right"},{"hash":"8sWxxbe1rdquP5VdYfQbw1UvtcXDRansJYJV5ySzyow4","direction":"Right"},{"hash":"CmKVKWRqEqi4UaeKKYXpPSesYqdQYwHQM3E4xLKEUAj8","direction":"Left"},{"hash":"3TvjFzVyPBvPpph5zL6VCASLCxdNeiKV6foPwUpAGqRv","direction":"Left"},{"hash":"AnzSG9f91ePS6L6ii3eAkocp4iKjp6wjzSwWsDYWLnMX","direction":"Right"},{"hash":"FYVJDL4T6c87An3pdeBvntB68NzpcPtpvLP6ifjxxNkr","direction":"Left"},{"hash":"2YMF6KE8XTz7Axj3uyAoFbZisWej9Xo8mxgVtauWCZaV","direction":"Left"},{"hash":"4BHtLcxqNfWSneBdW76qsd8om8Gjg58Qw5BX8PHz93hf","direction":"Left"},{"hash":"7G3QUT7NQSHyXNQyzm8dsaYrFk5LGhYaG7aVafKAekyG","direction":"Left"},{"hash":"3XaMNnvnX69gGqBJX43Na1bSTJ4VUe7z6h5ZYJsaSZZR","direction":"Left"},{"hash":"FKu7GtfviPioyAGXGZLBVTJeG7KY5BxGwuL447oAZxiL","direction":"Right"},{"hash":"BePd7DPKUQnGtnSds5fMJGBUwHGxSNBpaNLwceJGUcJX","direction":"Left"},{"hash":"2BVKWMd9pXZTEyE9D3KL52hAWAyMrXj1NqutamyurrY1","direction":"Left"},{"hash":"EWavHKhwQiT8ApnXvybvc9bFY6aJYJWqBhcrZpubKXtA","direction":"Left"},{"hash":"83Fsd3sdx5tsJkb6maBE1yViKiqbWCCNfJ4XZRsKnRZD","direction":"Left"},{"hash":"AaT9jQmUvVpgDHdFkLR2XctaUVdTti49enmtbT5hsoyL","direction":"Left"}]}"#;
    //     let p: BasicProof = serde_json::from_str(req).unwrap();
    //
    //     let outcome_hash = hash_borsh(p.outcome_proof.outcome.to_hashes(p.outcome_proof.id));
    //
    //     // let root_matches = Protocol::verify_outcome(
    //     //     &outcome_hash,
    //     //     p.outcome_proof.proof.iter(),
    //     //     p.outcome_root_proof.iter(),
    //     //     &p.block_header_lite.inner_lite.outcome_root,
    //     // );
    //     // assert!(root_matches);
    // }
}
