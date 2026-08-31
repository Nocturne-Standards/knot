// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 Nocturne Standards

//! Public BLS signature verification for partials and party roster signup.
//!
//! The collector never holds secret keys or signs — it only verifies incoming
//! signatures so the relay cannot be used as an unauthenticated griefing path.

use dusk_bytes::Serializable;
use dusk_core::signatures::bls::{
    BlsVersion, MultisigPublicKey, MultisigSignature, PublicKey as BlsPublicKey,
    Signature as BlsSignature, aggregate as aggregate_multisig_pk,
    verify_multisig as dusk_verify_multisig,
};

pub use knot_encoding::{DOMAIN_PARTY_V1, party_signup_preimage};

/// Verifies a BLS multisig partial over `msg` (typically the 32-byte proposal digest).
fn sig_bytes_to_array(sig: &[u8]) -> Option<[u8; 48]> {
    if sig.len() != 48 {
        return None;
    }
    let mut out = [0u8; 48];
    out.copy_from_slice(sig);
    Some(out)
}

pub fn verify_bls_partial(pk_bytes: &[u8; 96], msg: &[u8], sig_bytes: &[u8]) -> bool {
    let pk = match BlsPublicKey::from_bytes(pk_bytes) {
        Ok(pk) => pk,
        Err(_) => return false,
    };
    let sig_arr = match sig_bytes_to_array(sig_bytes) {
        Some(s) => s,
        None => return false,
    };
    let sig = match MultisigSignature::from_bytes(&sig_arr) {
        Ok(sig) => sig,
        Err(_) => return false,
    };
    verify_multisig(&pk, msg, &sig)
}

/// Verifies a standard BLS signature over `msg` (party roster signup).
pub fn verify_bls_standard(pk_bytes: &[u8; 96], msg: &[u8], sig_bytes: &[u8]) -> bool {
    let pk = match BlsPublicKey::from_bytes(pk_bytes) {
        Ok(pk) => pk,
        Err(_) => return false,
    };
    let sig_arr = match sig_bytes_to_array(sig_bytes) {
        Some(s) => s,
        None => return false,
    };
    let sig = match BlsSignature::from_bytes(&sig_arr) {
        Ok(sig) => sig,
        Err(_) => return false,
    };
    pk.verify(&sig, msg).is_ok()
}

fn verify_multisig(pk: &BlsPublicKey, msg: &[u8], sig: &MultisigSignature) -> bool {
    for version in [BlsVersion::V2, BlsVersion::V1] {
        let apk = match version {
            BlsVersion::V2 => aggregate_multisig_pk(core::slice::from_ref(pk), version).ok(),
            // PreforkHostQuery: VM::ephemeral PreFork — dusk-vm-issue-1; live clients use sign()/sign_multisig() (F-001)
            BlsVersion::V1 => MultisigPublicKey::aggregate_insecure(core::slice::from_ref(pk)).ok(),
        };
        let Some(apk) = apk else {
            continue;
        };
        if dusk_verify_multisig(&apk, sig, msg, version).is_ok() {
            return true;
        }
    }
    false
}
