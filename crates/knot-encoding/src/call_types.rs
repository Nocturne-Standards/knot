//! Layer-E multisig ABI types shared across `knot-registry` and
//! `knot-proposals`, carved out into a dedicated host-side feature. Pins
//! `#[archive_attr(repr(C))]` on struct Archive types (not fieldless
//! `#[repr(u8)]` enums — see `ProposalStatus`). Gated behind the
//! `call-types` feature so §4a digest consumers do not pull in `dusk-core`.

use alloc::string::String;
use alloc::vec::Vec;

use bytecheck::CheckBytes;
use dusk_core::abi::ContractId;
use dusk_core::signatures::bls::{
    MultisigSignature, PublicKey as BlsPublicKey, Signature as BlsSignature,
};
use rkyv::{Archive, Deserialize, Serialize};

/// One member's signature over the message being authorized. `signer` must
/// be one of the account's `members` and must not repeat across entries in
/// the same call — see `quorum_met`'s dedupe check.
#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[archive_attr(repr(C))]
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
#[archive_attr(repr(C))]
#[cfg_attr(feature = "data-driver", derive(serde::Serialize, serde::Deserialize))]
pub struct VerifyQuorumArgs {
    pub account_id: u64,
    pub msg: Vec<u8>,
    pub sigs: Vec<SignatureEntry>,
}

/// Read-only view of an account, returned by `account`.
#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[archive_attr(repr(C))]
#[cfg_attr(feature = "data-driver", derive(serde::Serialize, serde::Deserialize))]
pub struct MultisigAccountView {
    pub members: Vec<BlsPublicKey>,
    pub threshold: u32,
    pub nonce: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[archive_attr(repr(C))]
#[cfg_attr(feature = "data-driver", derive(serde::Serialize, serde::Deserialize))]
pub struct CreateAccountArgs {
    pub members: Vec<BlsPublicKey>,
    pub threshold: u32,
}

/// Replaces an account's member set / threshold. Authorized by a quorum of
/// the account's *current* members signing over
/// [`crate::change_account_digest`] of
/// `(account_id, current_nonce, new_members, new_threshold)`.
///
/// There is **no `nonce` field** on this args struct — the contract folds
/// the account's on-chain `nonce` into the digest itself. Signers must
/// read `account(id).nonce` (or `account_meta`) before signing; a captured
/// quorum for an older nonce fails once the account has changed.
#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[archive_attr(repr(C))]
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
#[archive_attr(repr(C))]
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
#[archive_attr(repr(C))]
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
#[archive_attr(repr(C))]
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

/// Proposal lifecycle.
///
/// Fieldless `#[repr(u8)]` enum: rkyv's archived form is also a unit enum with
/// an integer discriminant. `#[archive_attr(repr(C))]` is rejected by rustc
/// ("enums may only be repr(i*) or repr(u*)") when applied here, so the pin
/// is the existing `repr(u8)` plus the layout golden — not `archive_attr`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[cfg_attr(feature = "data-driver", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum ProposalStatus {
    Open = 0,
    /// Threshold met and `call_raw` succeeded (`tombstone` config false).
    Executed = 1,
    /// Consumed / blocked from immediate re-propose (`tombstone` true, or wiped).
    Tombstoned = 2,
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[archive_attr(repr(C))]
#[cfg_attr(feature = "data-driver", derive(serde::Serialize, serde::Deserialize))]
pub struct ApproveArgs {
    pub proposal_id: u64,
    pub signer: BlsPublicKey,
    pub signature: BlsSignature,
}

/// Structured propose input — v3 fields. Digest is recomputed on-chain.
#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[archive_attr(repr(C))]
#[cfg_attr(feature = "data-driver", derive(serde::Serialize, serde::Deserialize))]
pub struct ProposeArgs {
    pub registry_account_id: u64,
    pub target: ContractId,
    pub function_name: String,
    pub call_args: Vec<u8>,
    /// Caller-supplied uniquifier (not the registry account nonce).
    pub nonce: u64,
    /// Block height deadline; must be in `(block_height(), block_height() + ttl]`.
    pub deadline: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[archive_attr(repr(C))]
#[cfg_attr(feature = "data-driver", derive(serde::Serialize, serde::Deserialize))]
pub struct ProposalView {
    pub registry_account_id: u64,
    pub epoch: u64,
    pub nonce: u64,
    pub target: ContractId,
    pub function_name: String,
    pub call_args: Vec<u8>,
    pub deadline: u64,
    /// Full 32-byte §4a digest — what members must sign.
    pub signed_digest: [u8; 32],
    pub approvals: Vec<BlsPublicKey>,
    pub approval_sigs: Vec<BlsSignature>,
    pub status: ProposalStatus,
}
