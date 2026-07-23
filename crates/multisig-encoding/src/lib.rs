// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Leon Frenzel

//! Canonical multisig proposal encoding.
//!
//! Two artifacts (do not conflate):
//! - **§4a signing preimage** — malleability-free byte concatenation → Keccak256 digest.
//!   This is what members sign. Never use rkyv for these bytes.
//! - **§4b ProposalBlob** — rkyv transport container (intent + partials). Never itself signed.
//!
//! Spec: `docs/multisig/multisig-suite-and-atlas-implementation-plan.md` §4.

#![cfg_attr(not(test), no_std)]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use tiny_keccak::{Hasher, Keccak};

pub mod fingerprint;
pub use fingerprint::{digest_hex, digest_mnemonic, digest_safety_number};

/// Versioned domain tag for the proposal signing preimage.
pub const DOMAIN_PROPOSAL_V1: &[u8] = b"sme-platform.multisig.proposal.v1";

/// Fields that fully determine the §4a preimage / digest.
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

impl ProposalIntent {
    /// Stream the §4a preimage into Keccak-256 and return the full 32-byte digest.
    pub fn digest(&self) -> [u8; 32] {
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
    pub fn preimage_bytes(&self) -> Vec<u8> {
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
) -> Vec<u8> {
    let fn_len = u32::try_from(function_name.len()).expect("function_name length fits u32");
    let args_len = u32::try_from(call_args.len()).expect("call_args length fits u32");

    let mut out = Vec::with_capacity(
        DOMAIN_PROPOSAL_V1.len()
            + 8
            + 8
            + 8
            + 32
            + 4
            + function_name.len()
            + 4
            + call_args.len()
            + 8,
    );
    out.extend_from_slice(DOMAIN_PROPOSAL_V1);
    out.extend_from_slice(&chain_id.to_le_bytes());
    out.extend_from_slice(&committee_id.to_le_bytes());
    out.extend_from_slice(&nonce.to_le_bytes());
    out.extend_from_slice(target_contract_id);
    out.extend_from_slice(&fn_len.to_le_bytes());
    out.extend_from_slice(function_name);
    out.extend_from_slice(&args_len.to_le_bytes());
    out.extend_from_slice(call_args);
    out.extend_from_slice(&deadline.to_le_bytes());
    out
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
) -> [u8; 32] {
    let preimage = proposal_preimage(
        chain_id,
        committee_id,
        nonce,
        target_contract_id,
        function_name,
        call_args,
        deadline,
    );
    let mut hasher = Keccak::v256();
    hasher.update(&preimage);
    let mut out = [0u8; 32];
    hasher.finalize(&mut out);
    out
}

/// Recompute digest from intent fields and assert it matches `claimed`.
/// Returns `Ok(digest)` or `Err` if the blob's claimed digest disagrees.
pub fn recompute_and_verify(intent: &ProposalIntent, claimed: &[u8; 32]) -> Result<[u8; 32], ()> {
    let got = intent.digest();
    if &got == claimed {
        Ok(got)
    } else {
        Err(())
    }
}

/// Signer-side anti-blind-signing gate for a §4b blob.
///
/// Always recomputes from **canonical** `intent` fields. `human_summary` is
/// never trusted (phishing surface) — callers must display canonical fields
/// and only then sign. Returns `Err(())` when `signed_digest` ≠ recomputed.
pub fn gate_blob_for_signing(blob: &ProposalBlob) -> Result<[u8; 32], ()> {
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
        let a = sample_intent().digest();
        let b = sample_intent().digest();
        assert_eq!(a, b);
        assert_eq!(a.len(), 32);
    }

    #[test]
    fn two_processes_byte_identical_preimage() {
        let i = sample_intent();
        assert_eq!(i.preimage_bytes(), sample_intent().preimage_bytes());
    }

    #[test]
    fn recompute_rejects_mismatched_digest() {
        let i = sample_intent();
        let good = i.digest();
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
        let honest = intent.digest();
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
        let digest = intent.digest();
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
        assert_ne!(a.digest(), b.digest());

        b = sample_intent();
        b.function_name = "set_constant".to_string();
        assert_ne!(a.digest(), b.digest());

        b = sample_intent();
        b.call_args = b"other".to_vec();
        assert_ne!(a.digest(), b.digest());

        // Field-shifting: move bytes from fn name into args with adjusted lengths
        // must not collide with the honest encoding of a different split.
        a.function_name = "ab".to_string();
        a.call_args = b"cd".to_vec();
        b.function_name = "abc".to_string();
        b.call_args = b"d".to_vec();
        assert_ne!(a.digest(), b.digest());
        assert_ne!(a.preimage_bytes(), b.preimage_bytes());
    }

    #[test]
    fn length_prefix_rejects_field_shifting() {
        // Concatenation without length prefixes would make "ab"||"cd" == "abc"||"d".
        // With u32 LE length prefixes, preimages differ.
        let left = proposal_preimage(1, 1, 0, &[0u8; 32], b"ab", b"cd", 0);
        let right = proposal_preimage(1, 1, 0, &[0u8; 32], b"abc", b"d", 0);
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
            let d1 = intent.digest();
            let d2 = proposal_digest(
                intent.chain_id,
                intent.committee_id,
                intent.nonce,
                &intent.target_contract_id,
                intent.function_name.as_bytes(),
                &intent.call_args,
                intent.deadline,
            );
            assert_eq!(d1, d2);
            assert!(recompute_and_verify(&intent, &d1).is_ok());
        }
    }
}
