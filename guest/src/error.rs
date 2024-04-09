#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    BlockAlreadyVerified,
    BlockNotCurrentOrNextEpoch,
    SignatureInvalid,
    NotEnoughApprovedStake,
    NextBpsInvalid,
    ValidatorNotSigned,
}
