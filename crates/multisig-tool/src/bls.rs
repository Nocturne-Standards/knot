//! Thin wrappers over `dusk_core::signatures::bls` for signing and
//! aggregating — same primitives `multisig-registry`'s own tests
//! (`multisig-registry/tests/contract.rs`) and `rusk-experiments/
//! multisig-approval` already exercise against the real host query.
//!
//! **Testnet uses the post-hardfork secure scheme** (`sign` /
//! `sign_multisig`). `VM::ephemeral()` unit tests are stuck on
//! pre-Aegis/`HardFork::PreFork` and must keep using `sign_insecure` /
//! `sign_multisig_insecure` — see
//! `references/dusk-native/dusk-vm-issue-1-ephemeral-hardfork-policy-unreachable.md`
//! and `rfq-settlement/README.md` ("Real testnet enforces `sign()`, not
//! `sign_insecure()`"). Do not "match the tests" here: this tool talks to
//! the real node.

use dusk_core::signatures::bls::{
    MultisigSignature, PublicKey as BlsPublicKey, SecretKey as BlsSecretKey, Signature as BlsSignature,
};
use tiny_keccak::{Hasher, Keccak};

pub fn sign(sk: &BlsSecretKey, msg: &[u8]) -> BlsSignature {
    sk.sign(msg)
}

pub fn sign_multisig(sk: &BlsSecretKey, pk: &BlsPublicKey, msg: &[u8]) -> MultisigSignature {
    sk.sign_multisig(pk, msg)
}

/// Combines individual `sign_multisig` outputs into one aggregate — order
/// doesn't matter, `MultisigSignature::aggregate` just sums points.
pub fn aggregate(sigs: &[MultisigSignature]) -> MultisigSignature {
    let (first, rest) = sigs.split_first().expect("at least one signature to aggregate");
    first.aggregate(rest)
}

const DOMAIN_CHANGE_ACCOUNT: &[u8] = b"sme-platform.multisig-registry.change_account.v1";

/// Reproduces `multisig-registry`'s private `change_message` encoding
/// exactly (`multisig-registry/src/state.rs`) — the fixed digest a quorum of
/// an account's *current* members must sign to authorize `change_account`.
/// Kept in sync by hand since the contract's version is intentionally
/// private (not something external callers should construct differently) —
/// if the contract's encoding ever changes, this must change with it.
pub fn change_account_message(
    account_id: u64,
    nonce: u64,
    new_members: &[BlsPublicKey],
    new_threshold: u32,
) -> Vec<u8> {
    use dusk_bytes::Serializable;
    let mut hasher = Keccak::v256();
    hasher.update(DOMAIN_CHANGE_ACCOUNT);
    hasher.update(&account_id.to_le_bytes());
    hasher.update(&nonce.to_le_bytes());
    for member in new_members {
        hasher.update(&member.to_bytes());
    }
    hasher.update(&new_threshold.to_le_bytes());
    let mut out = [0u8; 32];
    hasher.finalize(&mut out);
    out.to_vec()
}
