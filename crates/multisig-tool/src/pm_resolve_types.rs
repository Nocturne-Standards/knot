//! Thin local duplicates of prediction-market `CouncilSigEntry` / `ResolveArgs`
//! for rkyv submit — same convention as GateArgs duplication (no path-include
//! of the full PM call_types, which pulls unrelated contract deps).
//!
//! Field order must stay in lockstep with
//! `prediction-market/crates/prediction-market/src/call_types.rs`.

use dusk_core::signatures::bls::{PublicKey as BlsPublicKey, Signature as BlsSignature};
use rkyv::{Archive, Deserialize, Serialize};
use bytecheck::CheckBytes;

/// Mirrors PM `CouncilSigEntry` / registry `SignatureEntry` (signer + sig).
#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
pub struct CouncilSigEntry {
    pub signer: BlsPublicKey,
    pub signature: BlsSignature,
}

/// Mirrors PM `ResolveArgs` for `prediction-market.resolve`.
#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
pub struct ResolveArgs {
    pub market_id: u64,
    pub winning_outcome: u8,
    pub quorum_sigs: Vec<CouncilSigEntry>,
}
