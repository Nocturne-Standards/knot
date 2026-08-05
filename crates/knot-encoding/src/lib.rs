// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Nocturne Standards

//! Canonical multisig proposal encoding, plus shared layer-E call types.
//!
//! Three surfaces (do not conflate):
//! - **§4a signing preimage** — malleability-free byte concatenation → Keccak256 digest.
//!   This is what members sign. Never use rkyv for these bytes.
//! - **§4b ProposalBlob** — in-memory shape for intent + partials (never itself signed).
//!   JSON+hex transport for blobs lives in `knot-tool` / `knot-collector`;
//!   this crate does not serialize §4b with rkyv.
//! - **Layer E (`call-types` feature)** — rkyv ABI structs shared by
//!   `knot-registry` and `knot-proposals` (and host tools after Wave 7).
//!   Includes quorum types plus registry/proposals call args (`CreateAccountArgs`,
//!   `ProposeArgs`, …). Off by default so §4a consumers never inherit
//!   `dusk-core` / `rkyv`. Spec 26.
//!
//! Spec: `docs/multisig/multisig-suite-and-atlas-implementation-plan.md` §4;
//! call types: `docs/superpowers/specs/2026-07-31-shared-code/26-multisig-shared-call-types.md`.

#![cfg_attr(not(test), no_std)]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use tiny_keccak::{Hasher, Keccak};

pub mod fingerprint;
pub use fingerprint::{digest_hex, digest_mnemonic, digest_safety_number};

#[cfg(feature = "call-types")]
pub mod call_types;

// Goldens are a `pub mod`, not `#[cfg(test)]` — consumers assert against these
// consts rather than hand-copying hex (spec 26 Task 7 / Wave 4 pattern).
#[cfg(feature = "call-types")]
pub mod layout_goldens;

/// Encoding failure for §4a proposal preimage construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncodingError {
    FieldTooLarge { field: &'static str, len: usize },
    CapacityOverflow,
}

impl core::fmt::Display for EncodingError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            EncodingError::FieldTooLarge { field, len } => {
                write!(f, "{field} length {len} exceeds u32::MAX")
            }
            EncodingError::CapacityOverflow => {
                write!(f, "preimage capacity overflow")
            }
        }
    }
}

impl core::error::Error for EncodingError {}

/// Digest-gate failure — distinguish mismatch from encoding limits (L14).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateError {
    DigestMismatch,
    Encoding(EncodingError),
}

impl core::fmt::Display for GateError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            GateError::DigestMismatch => {
                write!(f, "signed_digest does not match recomputed digest")
            }
            GateError::Encoding(e) => write!(f, "{e}"),
        }
    }
}

impl core::error::Error for GateError {}

/// Reject lengths that cannot be represented as u32 length-prefix fields.
pub fn checked_u32_len(field: &'static str, len: usize) -> Result<u32, EncodingError> {
    u32::try_from(len).map_err(|_| EncodingError::FieldTooLarge { field, len })
}

/// Versioned domain tag for the proposal signing preimage.
pub const DOMAIN_PROPOSAL_V2: &[u8] = b"nocturne.knot.multisig.proposal.v2";

/// Domain tag for registry `change_account` quorum message.
pub const DOMAIN_CHANGE_ACCOUNT_V2: &[u8] = b"nocturne.knot.multisig-registry.change_account.v2";

/// v3 proposal signing domain — binds `chain_id`, `self_id`, and `epoch`
/// (IMPLEMENTATION.md §2.12).
pub const DOMAIN_PROPOSAL_V3: &[u8] = b"nocturne.knot.multisig.proposal.v3";

/// v3 registry `change_account` quorum domain — binds `chain_id`, `self_id`,
/// and explicit `member_count` (IMPLEMENTATION.md §2.12).
pub const DOMAIN_CHANGE_ACCOUNT_V3: &[u8] = b"nocturne.knot.multisig-registry.change_account.v3";

/// M12 party roster signup domain — collector relay proof-of-possession.
pub const DOMAIN_PARTY_V1: &[u8] = b"nocturne.knot.collector.party.v1";

/// Length-prefixed party signup message (M12): `DOMAIN || name_len:u32_le || name || pk[96]`.
pub fn party_signup_preimage(name: &str, pk_bytes: &[u8; 96]) -> Result<Vec<u8>, EncodingError> {
    let name_bytes = name.as_bytes();
    let name_len = checked_u32_len("name", name_bytes.len())?;
    let mut out = Vec::with_capacity(DOMAIN_PARTY_V1.len() + 4 + name_bytes.len() + 96);
    out.extend_from_slice(DOMAIN_PARTY_V1);
    out.extend_from_slice(&name_len.to_le_bytes());
    out.extend_from_slice(name_bytes);
    out.extend_from_slice(pk_bytes);
    Ok(out)
}

/// Full 32-byte digest members must sign to authorize a committee change.
///
/// `member_pks` are compressed BLS pk bytes (96 each), in the same order
/// as `new_members` on-chain. Layout (no length prefix on member count):
/// `DOMAIN || account_id_le || nonce_le || pk₀…pkₙ || threshold_le_u32`,
/// then Keccak-256 of the full preimage.
pub fn change_account_digest(
    account_id: u64,
    nonce: u64,
    member_pks: &[[u8; 96]],
    new_threshold: u32,
) -> [u8; 32] {
    let mut hasher = Keccak::v256();
    hasher.update(DOMAIN_CHANGE_ACCOUNT_V2);
    hasher.update(&account_id.to_le_bytes());
    hasher.update(&nonce.to_le_bytes());
    for pk in member_pks {
        hasher.update(pk);
    }
    hasher.update(&new_threshold.to_le_bytes());
    let mut out = [0u8; 32];
    hasher.finalize(&mut out);
    out
}

/// Same digest as [`change_account_digest`], returned as a `Vec` for
/// host/`abi::verify_bls` message buffers.
pub fn change_account_message(
    account_id: u64,
    nonce: u64,
    member_pks: &[[u8; 96]],
    new_threshold: u32,
) -> Vec<u8> {
    change_account_digest(account_id, nonce, member_pks, new_threshold).to_vec()
}

/// §2.12 v3 `change_account` preimage — binds registry instance and chain.
pub fn change_account_preimage_v3(
    chain_id: u64,
    self_id: &[u8; 32],
    account_id: u64,
    nonce: u64,
    member_pks: &[[u8; 96]],
    new_threshold: u32,
) -> Result<Vec<u8>, EncodingError> {
    let member_count = checked_u32_len("member_pks", member_pks.len())?;
    let pk_bytes = member_pks
        .len()
        .checked_mul(96)
        .ok_or(EncodingError::CapacityOverflow)?;
    let capacity = DOMAIN_CHANGE_ACCOUNT_V3
        .len()
        .checked_add(8)
        .and_then(|n| n.checked_add(32))
        .and_then(|n| n.checked_add(8))
        .and_then(|n| n.checked_add(8))
        .and_then(|n| n.checked_add(4))
        .and_then(|n| n.checked_add(pk_bytes))
        .and_then(|n| n.checked_add(4))
        .ok_or(EncodingError::CapacityOverflow)?;
    let mut out = Vec::with_capacity(capacity);
    out.extend_from_slice(DOMAIN_CHANGE_ACCOUNT_V3);
    out.extend_from_slice(&chain_id.to_le_bytes());
    out.extend_from_slice(self_id);
    out.extend_from_slice(&account_id.to_le_bytes());
    out.extend_from_slice(&nonce.to_le_bytes());
    out.extend_from_slice(&member_count.to_le_bytes());
    for pk in member_pks {
        out.extend_from_slice(pk);
    }
    out.extend_from_slice(&new_threshold.to_le_bytes());
    Ok(out)
}

/// Keccak256 of the §2.12 v3 `change_account` preimage.
pub fn change_account_digest_v3(
    chain_id: u64,
    self_id: &[u8; 32],
    account_id: u64,
    nonce: u64,
    member_pks: &[[u8; 96]],
    new_threshold: u32,
) -> Result<[u8; 32], EncodingError> {
    let preimage = change_account_preimage_v3(
        chain_id,
        self_id,
        account_id,
        nonce,
        member_pks,
        new_threshold,
    )?;
    let mut hasher = Keccak::v256();
    hasher.update(&preimage);
    let mut out = [0u8; 32];
    hasher.finalize(&mut out);
    Ok(out)
}

/// Same digest as [`change_account_digest_v3`], as a `Vec` for host buffers.
pub fn change_account_message_v3(
    chain_id: u64,
    self_id: &[u8; 32],
    account_id: u64,
    nonce: u64,
    member_pks: &[[u8; 96]],
    new_threshold: u32,
) -> Result<Vec<u8>, EncodingError> {
    Ok(change_account_digest_v3(
        chain_id,
        self_id,
        account_id,
        nonce,
        member_pks,
        new_threshold,
    )?
    .to_vec())
}

/// Fields that fully determine the §4a preimage / digest (v2 layout).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProposalIntent {
    pub chain_id: u64,
    pub committee_id: u64,
    pub nonce: u64,
    pub target_contract_id: [u8; 32],
    pub function_name: String,
    pub call_args: Vec<u8>,
    pub deadline: u64,
}

/// Fields that fully determine the §2.12 v3 proposal preimage / digest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProposalIntentV3 {
    pub chain_id: u64,
    pub self_id: [u8; 32],
    pub epoch: u64,
    pub committee_id: u64,
    pub nonce: u64,
    pub target_contract_id: [u8; 32],
    pub function_name: String,
    pub call_args: Vec<u8>,
    pub deadline: u64,
}

/// Optional human hint for UI only — never the trust root for display.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedIntent {
    pub intent: ProposalIntent,
    pub human_summary: Option<String>,
}

/// One member's partial BLS multisig signature over `signed_digest`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PartialSig {
    pub signer_pk: [u8; 96],
    pub sig: Vec<u8>,
}

/// §4b transport blob (PSBT-analogue). Never signed as a whole.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProposalBlob {
    pub version: u16,
    pub intent: DecodedIntent,
    /// Creator-supplied; each signer must recompute and assert equality.
    pub signed_digest: [u8; 32],
    pub threshold: u32,
    pub partials: Vec<PartialSig>,
}

impl ProposalIntentV3 {
    /// Stream the §2.12 v3 preimage into Keccak-256 and return the full digest.
    pub fn digest(&self) -> Result<[u8; 32], EncodingError> {
        proposal_digest_v3(
            self.chain_id,
            &self.self_id,
            self.epoch,
            self.committee_id,
            self.nonce,
            &self.target_contract_id,
            self.function_name.as_bytes(),
            &self.call_args,
            self.deadline,
        )
    }

    /// Build the raw §2.12 v3 preimage bytes (without hashing).
    pub fn preimage_bytes(&self) -> Result<Vec<u8>, EncodingError> {
        proposal_preimage_v3(
            self.chain_id,
            &self.self_id,
            self.epoch,
            self.committee_id,
            self.nonce,
            &self.target_contract_id,
            self.function_name.as_bytes(),
            &self.call_args,
            self.deadline,
        )
    }
}

impl ProposalIntent {
    /// Stream the §4a preimage into Keccak-256 and return the full 32-byte digest.
    pub fn digest(&self) -> Result<[u8; 32], EncodingError> {
        proposal_digest(
            self.chain_id,
            self.committee_id,
            self.nonce,
            &self.target_contract_id,
            self.function_name.as_bytes(),
            &self.call_args,
            self.deadline,
        )
    }

    /// Build the raw §4a preimage bytes (without hashing). Useful for tests.
    pub fn preimage_bytes(&self) -> Result<Vec<u8>, EncodingError> {
        proposal_preimage(
            self.chain_id,
            self.committee_id,
            self.nonce,
            &self.target_contract_id,
            self.function_name.as_bytes(),
            &self.call_args,
            self.deadline,
        )
    }
}

/// Length-prefixed §4a preimage (domain + fixed fields + variable fields).
pub fn proposal_preimage(
    chain_id: u64,
    committee_id: u64,
    nonce: u64,
    target_contract_id: &[u8; 32],
    function_name: &[u8],
    call_args: &[u8],
    deadline: u64,
) -> Result<Vec<u8>, EncodingError> {
    let fn_len = checked_u32_len("function_name", function_name.len())?;
    let args_len = checked_u32_len("call_args", call_args.len())?;

    let capacity = DOMAIN_PROPOSAL_V2
        .len()
        .checked_add(8)
        .and_then(|n| n.checked_add(8))
        .and_then(|n| n.checked_add(8))
        .and_then(|n| n.checked_add(32))
        .and_then(|n| n.checked_add(4))
        .and_then(|n| n.checked_add(function_name.len()))
        .and_then(|n| n.checked_add(4))
        .and_then(|n| n.checked_add(call_args.len()))
        .and_then(|n| n.checked_add(8))
        .ok_or(EncodingError::CapacityOverflow)?;
    let mut out = Vec::with_capacity(capacity);
    out.extend_from_slice(DOMAIN_PROPOSAL_V2);
    out.extend_from_slice(&chain_id.to_le_bytes());
    out.extend_from_slice(&committee_id.to_le_bytes());
    out.extend_from_slice(&nonce.to_le_bytes());
    out.extend_from_slice(target_contract_id);
    out.extend_from_slice(&fn_len.to_le_bytes());
    out.extend_from_slice(function_name);
    out.extend_from_slice(&args_len.to_le_bytes());
    out.extend_from_slice(call_args);
    out.extend_from_slice(&deadline.to_le_bytes());
    Ok(out)
}

/// Length-prefixed v3 proposal preimage.
#[allow(clippy::too_many_arguments)]
pub fn proposal_preimage_v3(
    chain_id: u64,
    self_id: &[u8; 32],
    epoch: u64,
    committee_id: u64,
    nonce: u64,
    target_contract_id: &[u8; 32],
    function_name: &[u8],
    call_args: &[u8],
    deadline: u64,
) -> Result<Vec<u8>, EncodingError> {
    let fn_len = checked_u32_len("function_name", function_name.len())?;
    let args_len = checked_u32_len("call_args", call_args.len())?;

    let capacity = DOMAIN_PROPOSAL_V3
        .len()
        .checked_add(8)
        .and_then(|n| n.checked_add(32))
        .and_then(|n| n.checked_add(8))
        .and_then(|n| n.checked_add(8))
        .and_then(|n| n.checked_add(8))
        .and_then(|n| n.checked_add(32))
        .and_then(|n| n.checked_add(4))
        .and_then(|n| n.checked_add(function_name.len()))
        .and_then(|n| n.checked_add(4))
        .and_then(|n| n.checked_add(call_args.len()))
        .and_then(|n| n.checked_add(8))
        .ok_or(EncodingError::CapacityOverflow)?;
    let mut out = Vec::with_capacity(capacity);
    out.extend_from_slice(DOMAIN_PROPOSAL_V3);
    out.extend_from_slice(&chain_id.to_le_bytes());
    out.extend_from_slice(self_id);
    out.extend_from_slice(&epoch.to_le_bytes());
    out.extend_from_slice(&committee_id.to_le_bytes());
    out.extend_from_slice(&nonce.to_le_bytes());
    out.extend_from_slice(target_contract_id);
    out.extend_from_slice(&fn_len.to_le_bytes());
    out.extend_from_slice(function_name);
    out.extend_from_slice(&args_len.to_le_bytes());
    out.extend_from_slice(call_args);
    out.extend_from_slice(&deadline.to_le_bytes());
    Ok(out)
}

/// Keccak256 of the v3 proposal preimage — full 32 bytes, never truncated.
#[allow(clippy::too_many_arguments)]
pub fn proposal_digest_v3(
    chain_id: u64,
    self_id: &[u8; 32],
    epoch: u64,
    committee_id: u64,
    nonce: u64,
    target_contract_id: &[u8; 32],
    function_name: &[u8],
    call_args: &[u8],
    deadline: u64,
) -> Result<[u8; 32], EncodingError> {
    let preimage = proposal_preimage_v3(
        chain_id,
        self_id,
        epoch,
        committee_id,
        nonce,
        target_contract_id,
        function_name,
        call_args,
        deadline,
    )?;
    let mut hasher = Keccak::v256();
    hasher.update(&preimage);
    let mut out = [0u8; 32];
    hasher.finalize(&mut out);
    Ok(out)
}

/// Keccak256 of the §4a preimage — full 32 bytes, never truncated.
pub fn proposal_digest(
    chain_id: u64,
    committee_id: u64,
    nonce: u64,
    target_contract_id: &[u8; 32],
    function_name: &[u8],
    call_args: &[u8],
    deadline: u64,
) -> Result<[u8; 32], EncodingError> {
    let preimage = proposal_preimage(
        chain_id,
        committee_id,
        nonce,
        target_contract_id,
        function_name,
        call_args,
        deadline,
    )?;
    let mut hasher = Keccak::v256();
    hasher.update(&preimage);
    let mut out = [0u8; 32];
    hasher.finalize(&mut out);
    Ok(out)
}

/// Recompute digest from intent fields and assert it matches `claimed`.
/// Returns `Ok(digest)` or [`GateError`] if encoding fails or digest disagrees.
pub fn recompute_and_verify(
    intent: &ProposalIntent,
    claimed: &[u8; 32],
) -> Result<[u8; 32], GateError> {
    let got = intent.digest().map_err(GateError::Encoding)?;
    if &got == claimed {
        Ok(got)
    } else {
        Err(GateError::DigestMismatch)
    }
}

/// Recompute §2.12 v3 digest from intent fields and assert it matches `claimed`.
pub fn recompute_and_verify_v3(
    intent: &ProposalIntentV3,
    claimed: &[u8; 32],
) -> Result<[u8; 32], GateError> {
    let got = intent.digest().map_err(GateError::Encoding)?;
    if &got == claimed {
        Ok(got)
    } else {
        Err(GateError::DigestMismatch)
    }
}

/// Signer-side anti-blind-signing gate for a §4b blob.
///
/// Always recomputes from **canonical** `intent` fields. `human_summary` is
/// never trusted (phishing surface) — callers must display canonical fields
/// and only then sign. Returns [`GateError`] when `signed_digest` ≠ recomputed.
pub fn gate_blob_for_signing(blob: &ProposalBlob) -> Result<[u8; 32], GateError> {
    let _ = blob.intent.human_summary.as_ref(); // intentionally unused for trust
    recompute_and_verify(&blob.intent.intent, &blob.signed_digest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;
    use proptest::prelude::*;

    fn sample_intent() -> ProposalIntent {
        ProposalIntent {
            chain_id: 1,
            committee_id: 7,
            nonce: 3,
            target_contract_id: [0xab; 32],
            function_name: "set_service".to_string(),
            call_args: b"\x00\x01\x02".to_vec(),
            deadline: 1000,
        }
    }

    #[test]
    fn digest_is_deterministic() {
        let a = sample_intent().digest().unwrap();
        let b = sample_intent().digest().unwrap();
        assert_eq!(a, b);
        assert_eq!(a.len(), 32);
    }

    /// Golden digest for [`sample_intent`] — locked before Result API change (2026-07-28).
    #[test]
    fn sample_intent_digest_golden() {
        let digest = sample_intent().digest().unwrap();
        let expected =
            hex_decode("8426c1fa5895fe6b2e3a3fe0e3588eaff4b123fde07b075352264f41dfd9c9dd");
        assert_eq!(digest, expected);
    }

    #[test]
    fn proposal_preimage_rejects_len_over_u32() {
        assert!(matches!(
            checked_u32_len("function_name", (u32::MAX as usize) + 1),
            Err(EncodingError::FieldTooLarge {
                field: "function_name",
                ..
            })
        ));
    }

    #[test]
    fn change_account_digest_known_vector() {
        let digest = change_account_digest(1, 0, &[[0u8; 96], [1u8; 96]], 2);
        let msg = change_account_message(1, 0, &[[0u8; 96], [1u8; 96]], 2);
        assert_eq!(digest.as_slice(), msg.as_slice());
        // Hex locked 2026-08-04 after nocturne.knot domain bump (v2 tags).
        let expected =
            hex_decode("ab2fc0f6d9b490a645b0b5768bcfbfabfce53392251f28bc776e10b6ad22c457");
        assert_eq!(digest, expected);
    }

    fn hex_decode(s: &str) -> [u8; 32] {
        let mut out = [0u8; 32];
        assert_eq!(s.len(), 64);
        for i in 0..32 {
            out[i] = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap();
        }
        out
    }

    #[test]
    fn two_processes_byte_identical_preimage() {
        let i = sample_intent();
        assert_eq!(
            i.preimage_bytes().unwrap(),
            sample_intent().preimage_bytes().unwrap()
        );
    }

    #[test]
    fn recompute_rejects_mismatched_digest() {
        let i = sample_intent();
        let good = i.digest().unwrap();
        assert!(recompute_and_verify(&i, &good).is_ok());
        let mut bad = good;
        bad[0] ^= 0xff;
        assert!(recompute_and_verify(&i, &bad).is_err());
    }

    /// Adversarial: blob display (canonical intent) and claimed digest disagree
    /// → signer must refuse. Proves shown-X cannot induce a signature over Y.
    #[test]
    fn adversarial_blob_digest_mismatch_refuses_signing() {
        let intent = sample_intent();
        let honest = intent.digest().unwrap();
        let mut evil_digest = honest;
        evil_digest[0] ^= 0xaa;

        let blob = ProposalBlob {
            version: 1,
            intent: DecodedIntent {
                intent: intent.clone(),
                // Lying summary must not matter — gate still refuses on digest.
                human_summary: Some("harmless UI hint".to_string()),
            },
            signed_digest: evil_digest,
            threshold: 2,
            partials: Vec::new(),
        };
        assert!(
            gate_blob_for_signing(&blob).is_err(),
            "signer must refuse when claimed digest ≠ recomputed from intent"
        );
        // Honest digest still gates cleanly.
        let mut good = blob;
        good.signed_digest = honest;
        assert!(gate_blob_for_signing(&good).is_ok());
    }

    /// Adversarial: `human_summary` disagrees with args — still verify from
    /// canonical fields only; summary never participates in the digest.
    #[test]
    fn adversarial_lying_human_summary_ignored_for_digest() {
        let intent = sample_intent();
        let digest = intent.digest().unwrap();
        let blob = ProposalBlob {
            version: 1,
            intent: DecodedIntent {
                intent: intent.clone(),
                human_summary: Some(
                    "TRANSFER ALL FUNDS TO ATTACKER (do not trust this string)".to_string(),
                ),
            },
            signed_digest: digest,
            threshold: 2,
            partials: Vec::new(),
        };
        assert_eq!(
            gate_blob_for_signing(&blob).expect("canonical digest must still match"),
            digest
        );
        // Display trust root is still the canonical fields, not the summary.
        assert_eq!(blob.intent.intent.function_name, "set_service");
        assert_eq!(blob.intent.intent.call_args, b"\x00\x01\x02");
    }

    #[test]
    fn distinct_intents_do_not_collide() {
        let mut a = sample_intent();
        let mut b = sample_intent();
        b.nonce = a.nonce + 1;
        assert_ne!(a.digest().unwrap(), b.digest().unwrap());

        b = sample_intent();
        b.function_name = "set_constant".to_string();
        assert_ne!(a.digest().unwrap(), b.digest().unwrap());

        b = sample_intent();
        b.call_args = b"other".to_vec();
        assert_ne!(a.digest().unwrap(), b.digest().unwrap());

        // Field-shifting: move bytes from fn name into args with adjusted lengths
        // must not collide with the honest encoding of a different split.
        a.function_name = "ab".to_string();
        a.call_args = b"cd".to_vec();
        b.function_name = "abc".to_string();
        b.call_args = b"d".to_vec();
        assert_ne!(a.digest().unwrap(), b.digest().unwrap());
        assert_ne!(a.preimage_bytes().unwrap(), b.preimage_bytes().unwrap());
    }

    #[test]
    fn length_prefix_rejects_field_shifting() {
        // Concatenation without length prefixes would make "ab"||"cd" == "abc"||"d".
        // With u32 LE length prefixes, preimages differ.
        let left = proposal_preimage(1, 1, 0, &[0u8; 32], b"ab", b"cd", 0).unwrap();
        let right = proposal_preimage(1, 1, 0, &[0u8; 32], b"abc", b"d", 0).unwrap();
        assert_ne!(left, right);
    }

    proptest! {
        #[test]
        fn digest_stable_under_rebuild(
            chain_id in any::<u64>(),
            committee_id in any::<u64>(),
            nonce in any::<u64>(),
            target in any::<[u8; 32]>(),
            fn_name in "\\PC{0,64}",
            args in prop::collection::vec(any::<u8>(), 0..128),
            deadline in any::<u64>(),
        ) {
            let intent = ProposalIntent {
                chain_id,
                committee_id,
                nonce,
                target_contract_id: target,
                function_name: fn_name,
                call_args: args,
                deadline,
            };
            let d1 = intent.digest().unwrap();
            let d2 = proposal_digest(
                intent.chain_id,
                intent.committee_id,
                intent.nonce,
                &intent.target_contract_id,
                intent.function_name.as_bytes(),
                &intent.call_args,
                intent.deadline,
            )
            .unwrap();
            assert_eq!(d1, d2);
            assert!(recompute_and_verify(&intent, &d1).is_ok());
        }
    }

    fn sample_intent_v3() -> ProposalIntentV3 {
        ProposalIntentV3 {
            chain_id: 0xCA,
            self_id: [0x11; 32],
            epoch: 2,
            committee_id: 7,
            nonce: 3,
            target_contract_id: [0xab; 32],
            function_name: "set_service".to_string(),
            call_args: b"\x00\x01\x02".to_vec(),
            deadline: 1000,
        }
    }

    #[test]
    fn v3_digest_is_deterministic() {
        let a = sample_intent_v3().digest().unwrap();
        let b = sample_intent_v3().digest().unwrap();
        assert_eq!(a, b);
        assert_eq!(a.len(), 32);
    }

    /// Golden digest for [`sample_intent_v3`] — locked 2026-08-05 (v3 layout).
    #[test]
    fn sample_intent_v3_digest_golden() {
        let digest = sample_intent_v3().digest().unwrap();
        let expected =
            hex_decode("2b2243ab796615051a9c4478dfd63f21acd2ebf0857ca6962447bf6f74606e80");
        assert_eq!(digest, expected);
    }

    #[test]
    fn change_account_digest_v3_known_vector() {
        let self_id = [0x22; 32];
        let member_pks = [[0u8; 96], [1u8; 96]];
        let digest = change_account_digest_v3(0xCA, &self_id, 1, 0, &member_pks, 2).unwrap();
        let msg = change_account_message_v3(0xCA, &self_id, 1, 0, &member_pks, 2).unwrap();
        assert_eq!(digest.as_slice(), msg.as_slice());
        let expected =
            hex_decode("0d9f0b3d1d74c4805365e4c495bf6d4e40c8bc7a6732c734fb32e64048dcbf8d");
        assert_eq!(digest, expected);
    }

    /// H1: same intent on two proposals contracts (different `self_id`) → different digests.
    #[test]
    fn h1_same_intent_two_contracts_differ() {
        let a = sample_intent_v3();
        let mut b = sample_intent_v3();
        b.self_id = [0x99; 32];
        assert_ne!(a.digest().unwrap(), b.digest().unwrap());
    }

    /// H1: same intent on two chain ids → different digests.
    #[test]
    fn h1_same_intent_two_chain_ids_differ() {
        let a = sample_intent_v3();
        let mut b = sample_intent_v3();
        b.chain_id = 0xBB;
        assert_ne!(a.digest().unwrap(), b.digest().unwrap());
    }

    /// H2: `change_account` binds chain and registry instance.
    #[test]
    fn h2_change_account_chain_and_registry_binding() {
        let self_a = [0x11; 32];
        let self_b = [0x22; 32];
        let pks = [[0u8; 96], [1u8; 96]];
        let base = change_account_digest_v3(0xCA, &self_a, 1, 0, &pks, 2).unwrap();
        let other_chain = change_account_digest_v3(0xBB, &self_a, 1, 0, &pks, 2).unwrap();
        let other_registry = change_account_digest_v3(0xCA, &self_b, 1, 0, &pks, 2).unwrap();
        assert_ne!(base, other_chain);
        assert_ne!(base, other_registry);
        assert_ne!(other_chain, other_registry);
    }

    #[test]
    fn v3_distinct_intents_do_not_collide() {
        let a = sample_intent_v3();
        let mut b = sample_intent_v3();
        b.epoch = a.epoch + 1;
        assert_ne!(a.digest().unwrap(), b.digest().unwrap());

        b = sample_intent_v3();
        b.nonce = a.nonce + 1;
        assert_ne!(a.digest().unwrap(), b.digest().unwrap());

        b = sample_intent_v3();
        b.function_name = "set_constant".to_string();
        assert_ne!(a.digest().unwrap(), b.digest().unwrap());

        b = sample_intent_v3();
        b.call_args = b"other".to_vec();
        assert_ne!(a.digest().unwrap(), b.digest().unwrap());
    }

    #[test]
    fn v3_differs_from_v2_for_same_logical_fields() {
        let v3 = sample_intent_v3();
        let v2 = ProposalIntent {
            chain_id: v3.chain_id,
            committee_id: v3.committee_id,
            nonce: v3.nonce,
            target_contract_id: v3.target_contract_id,
            function_name: v3.function_name.clone(),
            call_args: v3.call_args.clone(),
            deadline: v3.deadline,
        };
        assert_ne!(v3.digest().unwrap(), v2.digest().unwrap());
    }

    #[test]
    fn change_account_v3_member_count_prefix_binds_field_shifting() {
        let self_id = [0x33; 32];
        let one_pk = [[0u8; 96]];
        let two_pks = [[0u8; 96], [1u8; 96]];
        let d_one = change_account_digest_v3(1, &self_id, 0, 0, &one_pk, 1).unwrap();
        let d_two = change_account_digest_v3(1, &self_id, 0, 0, &two_pks, 1).unwrap();
        assert_ne!(d_one, d_two);
    }
}
