//! Layer-E multisig ABI types shared across `multisig-registry` and
//! `multisig-proposals` (spec 26). Layout-neutral move — do not add
//! `repr(C)` here (spec 23b owns that). Gated behind the `call-types`
//! feature so §4a digest consumers do not pull in `dusk-core`.

use alloc::vec::Vec;

use bytecheck::CheckBytes;
use dusk_core::signatures::bls::{PublicKey as BlsPublicKey, Signature as BlsSignature};
use rkyv::{Archive, Deserialize, Serialize};

/// One member's signature over the message being authorized. `signer` must
/// be one of the account's `members` and must not repeat across entries in
/// the same call — see `quorum_met`'s dedupe check.
#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[cfg_attr(feature = "data-driver", derive(serde::Serialize, serde::Deserialize))]
pub struct SignatureEntry {
    pub signer: BlsPublicKey,
    pub signature: BlsSignature,
}

/// Pure quorum check: does `sigs` carry >= `threshold` valid, distinct-member
/// signatures over `msg` for account `account_id`? Callers (this crate's own
/// `change_account`, or another contract via `abi::call`) choose `msg`'s
/// content themselves — this registry does not impose or track a message
/// format, only verifies signatures against the account's current member
/// set. Replay protection is therefore the *caller's* responsibility (e.g.
/// fold a nonce the caller owns into `msg`), except for `change_account`
/// itself, which folds this account's own `nonce` in automatically.
#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[cfg_attr(feature = "data-driver", derive(serde::Serialize, serde::Deserialize))]
pub struct VerifyQuorumArgs {
    pub account_id: u64,
    pub msg: Vec<u8>,
    pub sigs: Vec<SignatureEntry>,
}

/// Read-only view of an account, returned by `account`.
#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[cfg_attr(feature = "data-driver", derive(serde::Serialize, serde::Deserialize))]
pub struct MultisigAccountView {
    pub members: Vec<BlsPublicKey>,
    pub threshold: u32,
    pub nonce: u64,
}
