//! Thin wrappers over `dusk_core::signatures::bls` for signing and
//! aggregating — same primitives `knot-registry`'s own tests
//! (`knot-registry/tests/contract.rs`) and `rusk-experiments/
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
use knot_encoding::call_types::ProposalView;
use knot_encoding::{ProposalIntentV3, recompute_and_verify_v3};

/// Chain id for v3 digests on testnet (`init_chain_id=2` in deploy-history).
pub const DIGEST_CHAIN_ID: u64 = 2;

pub fn digest_chain_id() -> u64 {
    std::env::var("KNOT_CHAIN_ID")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DIGEST_CHAIN_ID)
}

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

/// Thin wrap of [`knot_encoding::change_account_message_v3`] — the fixed
/// digest a quorum of an account's *current* members must sign to authorize
/// `change_account`. Accepts `BlsPublicKey`s and maps them to the byte API.
pub fn change_account_message(
    registry_self_id: &[u8; 32],
    account_id: u64,
    nonce: u64,
    new_members: &[BlsPublicKey],
    new_threshold: u32,
) -> Vec<u8> {
    use dusk_bytes::Serializable;
    let member_pks: Vec<[u8; 96]> = new_members.iter().map(|pk| pk.to_bytes()).collect();
    knot_encoding::change_account_message_v3(
        digest_chain_id(),
        registry_self_id,
        account_id,
        nonce,
        &member_pks,
        new_threshold,
    )
    .expect("committee within encoding caps")
}

/// Build §2.12 v3 intent from an on-chain [`ProposalView`].
pub fn proposal_intent_v3_from_view(
    view: &ProposalView,
    chain_id: u64,
    proposals_self_id: &[u8; 32],
) -> ProposalIntentV3 {
    ProposalIntentV3 {
        chain_id,
        self_id: *proposals_self_id,
        epoch: view.epoch,
        committee_id: view.registry_account_id,
        nonce: view.nonce,
        target_contract_id: view.target.to_bytes(),
        function_name: view.function_name.clone(),
        call_args: view.call_args.clone(),
        deadline: view.deadline,
    }
}

/// Recompute and verify a proposal digest against on-chain `signed_digest`.
pub fn verify_proposal_view_digest(
    view: &ProposalView,
    chain_id: u64,
    proposals_self_id: &[u8; 32],
) -> Result<[u8; 32], ()> {
    let intent = proposal_intent_v3_from_view(view, chain_id, proposals_self_id);
    recompute_and_verify_v3(&intent, &view.signed_digest)
}

/// 32-byte fingerprint for out-of-band compare before BLS signing.
///
/// Canonical 32-byte messages (e.g. `change_account_message`) use the bytes
/// directly; arbitrary quorum UTF-8/hex payloads are hashed under a lab domain.
pub fn signing_message_fingerprint(msg: &[u8]) -> [u8; 32] {
    if msg.len() == 32 {
        let mut digest = [0u8; 32];
        digest.copy_from_slice(msg);
        digest
    } else {
        use tiny_keccak::{Hasher, Keccak};
        let mut hasher = Keccak::v256();
        hasher.update(b"nocturne.knot.lab.quorum-message-fingerprint.v1");
        hasher.update(msg);
        let mut out = [0u8; 32];
        hasher.finalize(&mut out);
        out
    }
}

/// Hex + BIP39 mnemonic + safety-number for a signing message buffer.
pub fn message_fingerprint_display(msg: &[u8]) -> (String, String, String) {
    let digest = signing_message_fingerprint(msg);
    (
        knot_encoding::digest_hex(&digest),
        knot_encoding::digest_mnemonic(&digest),
        knot_encoding::digest_safety_number(&digest),
    )
}
