//! Call argument / return types for `multisig-proposals` (v0.3).
//!
//! `SignatureEntry`, `VerifyQuorumArgs`, and `MultisigAccountView` live in
//! `multisig-encoding` (spec 26); re-exported here so existing paths keep
//! working.

use alloc::string::String;
use alloc::vec::Vec;

use bytecheck::CheckBytes;
use dusk_core::abi::ContractId;
use dusk_core::signatures::bls::{PublicKey as BlsPublicKey, Signature as BlsSignature};
use rkyv::{Archive, Deserialize, Serialize};

pub use multisig_encoding::call_types::{MultisigAccountView, SignatureEntry, VerifyQuorumArgs};

/// Proposal lifecycle.
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
#[cfg_attr(feature = "data-driver", derive(serde::Serialize, serde::Deserialize))]
pub struct ApproveArgs {
    pub proposal_id: u64,
    pub signer: BlsPublicKey,
    pub signature: BlsSignature,
}

/// Structured propose input — §4a fields. Digest is recomputed on-chain.
#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[cfg_attr(feature = "data-driver", derive(serde::Serialize, serde::Deserialize))]
pub struct ProposeArgs {
    pub registry_account_id: u64,
    pub target: ContractId,
    pub function_name: String,
    pub call_args: Vec<u8>,
    /// Block height TTL; 0 = none (discouraged).
    pub deadline: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[cfg_attr(feature = "data-driver", derive(serde::Serialize, serde::Deserialize))]
pub struct ProposalView {
    pub registry_account_id: u64,
    pub chain_id: u64,
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
