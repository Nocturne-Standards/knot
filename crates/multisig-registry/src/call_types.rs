//! Call argument / return types for `multisig-registry`.
//!
//! Kept outside the `#[dusk_forge::contract]` module so host-side tests can
//! construct the same layouts the WASM contract deserializes (rkyv) — same
//! convention as `prediction-market/src/call_types.rs`.
//!
//! `SignatureEntry`, `VerifyQuorumArgs`, and `MultisigAccountView` live in
//! `multisig-encoding` (spec 26); re-exported here so existing paths keep
//! working.

use alloc::vec::Vec;

use bytecheck::CheckBytes;
use dusk_core::signatures::bls::{MultisigSignature, PublicKey as BlsPublicKey};
use rkyv::{Archive, Deserialize, Serialize};

pub use multisig_encoding::call_types::{MultisigAccountView, SignatureEntry, VerifyQuorumArgs};

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[cfg_attr(feature = "data-driver", derive(serde::Serialize, serde::Deserialize))]
pub struct CreateAccountArgs {
    pub members: Vec<BlsPublicKey>,
    pub threshold: u32,
}

/// Replaces an account's member set / threshold. Authorized by a quorum of
/// the account's *current* members signing over
/// [`multisig_encoding::change_account_digest`] of
/// `(account_id, current_nonce, new_members, new_threshold)`.
///
/// There is **no `nonce` field** on this args struct — the contract folds
/// the account's on-chain `nonce` into the digest itself. Signers must
/// read `account(id).nonce` (or `account_meta`) before signing; a captured
/// quorum for an older nonce fails once the account has changed.
#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[cfg_attr(feature = "data-driver", derive(serde::Serialize, serde::Deserialize))]
pub struct ChangeAccountArgs {
    pub account_id: u64,
    pub new_members: Vec<BlsPublicKey>,
    pub new_threshold: u32,
    pub sigs: Vec<SignatureEntry>,
}

/// Aggregate-signature quorum check — same question as `VerifyQuorumArgs`
/// ("did enough members authorize `msg`?"), verified with a single native
/// pairing check instead of one `abi::verify_bls` per signer.
///
/// `signer_keys` is the *subset* of the account's members who actually
/// signed (order must match how `aggregate_sig` was built — see below);
/// `aggregate_sig` is the point-sum of each of their individual
/// `sign_multisig(sk, pk, msg)` outputs (`MultisigSignature::aggregate`).
/// Unlike per-signature verification, a single wrong or missing signer in
/// `signer_keys` invalidates the whole aggregate — there is no way to
/// verify "these 2 of these 3 keys signed" from one aggregate the way
/// `VerifyQuorumArgs` can check each signature independently; the
/// aggregate is only valid for exactly the key set it was built over. The
/// coordinator assembling `aggregate_sig` off-chain must already know
/// which subset actually signed before combining.
#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[cfg_attr(feature = "data-driver", derive(serde::Serialize, serde::Deserialize))]
pub struct VerifyQuorumAggregateArgs {
    pub account_id: u64,
    pub msg: Vec<u8>,
    pub signer_keys: Vec<BlsPublicKey>,
    pub aggregate_sig: MultisigSignature,
}

/// Lightweight account summary without `BlsPublicKey` values — used to
/// isolate whether free-read failures are about state visibility or about
/// returning BLS keys over RUES.
#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[cfg_attr(feature = "data-driver", derive(serde::Serialize, serde::Deserialize))]
pub struct AccountMeta {
    pub threshold: u32,
    pub nonce: u64,
    pub members_len: u32,
}

/// Per-signature breakdown from `diagnose_quorum` — no host verify is
/// skipped: `member_match` is pure `contains`, `sig_ok` is `abi::verify_bls`.
#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[cfg_attr(feature = "data-driver", derive(serde::Serialize, serde::Deserialize))]
pub struct DiagnoseQuorumResult {
    pub exists: bool,
    pub threshold: u32,
    pub members_len: u32,
    pub member_matches: u32,
    pub sigs_ok: u32,
    /// Raw 96-byte compressed forms of the account's current members (empty
    /// when `exists` is false). Each inner `Vec` is length 96 — stored as
    /// `Vec<Vec<u8>>` rather than `Vec<[u8; 96]>` so the data-driver serde
    /// path builds (fixed arrays aren't in serde's default impl set here).
    pub member_pk_bytes: Vec<Vec<u8>>,
}
