//! Tests for `knot-registry`: account creation/threshold validation,
//! quorum verification (enough/not-enough/duplicate/non-member sigs), and
//! `change_account`'s quorum-gated member-set replacement + nonce replay
//! protection.

extern crate alloc;

use alloc::vec::Vec;

use dusk_bytes::Serializable;
use dusk_core::abi::ContractId;
use dusk_core::signatures::bls::{
    MultisigSignature, PublicKey as BlsPublicKey, SecretKey as BlsSecretKey,
};
use dusk_vm::{ContractData, Session, VM};
use knot_encoding::change_account_message_v3;
use rand::rngs::StdRng;
use rand::SeedableRng;

#[path = "../src/call_types.rs"]
mod call_types;
use call_types::{
    ChangeAccountArgs, CreateAccountArgs, MultisigAccountView,
    SignatureEntry, VerifyQuorumAggregateArgs, VerifyQuorumArgs,
};

const REGISTRY_BYTECODE: &[u8] = include_bytes!(
    "../../../target/contract/wasm32-unknown-unknown/release/knot_registry.wasm"
);

const REGISTRY_ID: ContractId = ContractId::from_bytes([0xa1; 32]);
const CHAIN_ID: u8 = 0xCA;
const POINT_LIMIT: u64 = 0x10000000;

fn initialize() -> Session {
    let vm = VM::ephemeral().expect("Creating ephemeral VM should work");
    let mut session = vm.genesis_session(CHAIN_ID);

    session
        .deploy(
            REGISTRY_BYTECODE,
            ContractData::builder()
                .owner([0; 32])
                .contract_id(REGISTRY_ID),
            POINT_LIMIT,
        )
        .expect("Deploying knot-registry should succeed");

    session
}

fn keypair(rng: &mut StdRng) -> (BlsSecretKey, BlsPublicKey) {
    let sk = BlsSecretKey::random(rng);
    let pk = BlsPublicKey::from(&sk);
    (sk, pk)
}

fn change_message(
    account_id: u64,
    nonce: u64,
    new_members: &[BlsPublicKey],
    new_threshold: u32,
) -> Vec<u8> {
    let member_pks: Vec<[u8; 96]> = new_members.iter().map(|pk| pk.to_bytes()).collect();
    change_account_message_v3(
        u64::from(CHAIN_ID),
        &REGISTRY_ID.to_bytes(),
        account_id,
        nonce,
        &member_pks,
        new_threshold,
    )
    .expect("test committee within encoding caps")
}

fn sign_all(msg: &[u8], sks: &[(&BlsSecretKey, &BlsPublicKey)]) -> Vec<SignatureEntry> {
    sks.iter()
        .map(|(sk, pk)| SignatureEntry {
            signer: **pk,
            signature: sk.sign_insecure(msg),
        })
        .collect()
}

#[test]
fn create_account_rejects_out_of_range_threshold() {
    let rng = &mut StdRng::seed_from_u64(1);
    let mut session = initialize();
    let (_sk1, pk1) = keypair(rng);
    let (_sk2, pk2) = keypair(rng);

    let args = CreateAccountArgs {
        members: alloc::vec![pk1, pk2],
        threshold: 0,
    };
    let result =
        session.call::<CreateAccountArgs, u64>(REGISTRY_ID, "create_account", &args, POINT_LIMIT);
    assert!(result.is_err(), "threshold 0 should be rejected");

    let args = CreateAccountArgs {
        members: alloc::vec![pk1, pk2],
        threshold: 3,
    };
    let result =
        session.call::<CreateAccountArgs, u64>(REGISTRY_ID, "create_account", &args, POINT_LIMIT);
    assert!(result.is_err(), "threshold above member count should be rejected");
}

#[test]
fn create_account_rejects_duplicate_members() {
    let rng = &mut StdRng::seed_from_u64(2);
    let mut session = initialize();
    let (_sk1, pk1) = keypair(rng);

    let args = CreateAccountArgs {
        members: alloc::vec![pk1, pk1],
        threshold: 1,
    };
    let result =
        session.call::<CreateAccountArgs, u64>(REGISTRY_ID, "create_account", &args, POINT_LIMIT);
    assert!(result.is_err(), "duplicate members should be rejected");
}

#[test]
fn create_account_then_query_roundtrips() {
    let rng = &mut StdRng::seed_from_u64(3);
    let mut session = initialize();
    let (_sk1, pk1) = keypair(rng);
    let (_sk2, pk2) = keypair(rng);
    let (_sk3, pk3) = keypair(rng);

    let args = CreateAccountArgs {
        members: alloc::vec![pk1, pk2, pk3],
        threshold: 2,
    };
    let id = session
        .call::<CreateAccountArgs, u64>(REGISTRY_ID, "create_account", &args, POINT_LIMIT)
        .expect("create_account should succeed")
        .data;

    let view = session
        .call::<u64, Option<MultisigAccountView>>(REGISTRY_ID, "account", &id, POINT_LIMIT)
        .expect("account query should succeed")
        .data
        .expect("account should exist");
    assert_eq!(view.members, alloc::vec![pk1, pk2, pk3]);
    assert_eq!(view.threshold, 2);
    assert_eq!(view.nonce, 0);
}

#[test]
fn verify_quorum_true_only_with_enough_distinct_valid_signers() {
    let rng = &mut StdRng::seed_from_u64(4);
    let mut session = initialize();
    let (sk1, pk1) = keypair(rng);
    let (sk2, pk2) = keypair(rng);
    let (_sk3, pk3) = keypair(rng);
    let (outsider_sk, outsider_pk) = keypair(rng);

    let create_args = CreateAccountArgs {
        members: alloc::vec![pk1, pk2, pk3],
        threshold: 2,
    };
    let id = session
        .call::<CreateAccountArgs, u64>(REGISTRY_ID, "create_account", &create_args, POINT_LIMIT)
        .expect("create_account should succeed")
        .data;

    let msg = b"authorize-something".to_vec();

    // Only one valid signer: below threshold.
    let args = VerifyQuorumArgs {
        account_id: id,
        msg: msg.clone(),
        sigs: sign_all(&msg, &[(&sk1, &pk1)]),
    };
    let passed = session
        .call::<VerifyQuorumArgs, bool>(REGISTRY_ID, "verify_quorum", &args, POINT_LIMIT)
        .expect("verify_quorum call should succeed")
        .data;
    assert!(!passed, "one signer should not meet a threshold of 2");

    // Two distinct valid signers: meets threshold.
    let args = VerifyQuorumArgs {
        account_id: id,
        msg: msg.clone(),
        sigs: sign_all(&msg, &[(&sk1, &pk1), (&sk2, &pk2)]),
    };
    let passed = session
        .call::<VerifyQuorumArgs, bool>(REGISTRY_ID, "verify_quorum", &args, POINT_LIMIT)
        .expect("verify_quorum call should succeed")
        .data;
    assert!(passed, "two distinct member signers should meet threshold 2");

    // Same signer counted twice: still only counts once, below threshold.
    let mut sigs = sign_all(&msg, &[(&sk1, &pk1)]);
    sigs.push(sigs[0].clone());
    let args = VerifyQuorumArgs {
        account_id: id,
        msg: msg.clone(),
        sigs,
    };
    let passed = session
        .call::<VerifyQuorumArgs, bool>(REGISTRY_ID, "verify_quorum", &args, POINT_LIMIT)
        .expect("verify_quorum call should succeed")
        .data;
    assert!(!passed, "a repeated signer must not be double-counted");

    // Non-member signature: ignored, below threshold even with a second sig.
    let args = VerifyQuorumArgs {
        account_id: id,
        msg: msg.clone(),
        sigs: sign_all(&msg, &[(&sk1, &pk1), (&outsider_sk, &outsider_pk)]),
    };
    let passed = session
        .call::<VerifyQuorumArgs, bool>(REGISTRY_ID, "verify_quorum", &args, POINT_LIMIT)
        .expect("verify_quorum call should succeed")
        .data;
    assert!(!passed, "a non-member signature must not count toward quorum");
}

#[test]
fn verify_quorum_returns_false_for_unknown_account() {
    let mut session = initialize();
    let args = VerifyQuorumArgs {
        account_id: 999,
        msg: b"anything".to_vec(),
        sigs: Vec::new(),
    };
    let passed = session
        .call::<VerifyQuorumArgs, bool>(REGISTRY_ID, "verify_quorum", &args, POINT_LIMIT)
        .expect("verify_quorum call should succeed even for an unknown account")
        .data;
    assert!(!passed);
}

#[test]
fn change_account_requires_quorum_and_bumps_nonce_preventing_replay() {
    let rng = &mut StdRng::seed_from_u64(5);
    let mut session = initialize();
    let (sk1, pk1) = keypair(rng);
    let (sk2, pk2) = keypair(rng);
    let (_sk3, pk3) = keypair(rng);
    let (_new_sk, new_pk) = keypair(rng);

    let create_args = CreateAccountArgs {
        members: alloc::vec![pk1, pk2, pk3],
        threshold: 2,
    };
    let id = session
        .call::<CreateAccountArgs, u64>(REGISTRY_ID, "create_account", &create_args, POINT_LIMIT)
        .expect("create_account should succeed")
        .data;

    let new_members = alloc::vec![pk1, new_pk];
    let new_threshold = 2u32;
    let msg = change_message(id, 0, &new_members, new_threshold);

    // Insufficient quorum (only one current member signs): rejected.
    let bad_args = ChangeAccountArgs {
        account_id: id,
        new_members: new_members.clone(),
        new_threshold,
        sigs: sign_all(&msg, &[(&sk1, &pk1)]),
    };
    let result = session.call::<ChangeAccountArgs, ()>(
        REGISTRY_ID,
        "change_account",
        &bad_args,
        POINT_LIMIT,
    );
    assert!(result.is_err(), "change_account should reject an under-quorum request");

    // Valid quorum: succeeds, bumps nonce.
    let good_args = ChangeAccountArgs {
        account_id: id,
        new_members: new_members.clone(),
        new_threshold,
        sigs: sign_all(&msg, &[(&sk1, &pk1), (&sk2, &pk2)]),
    };
    session
        .call::<ChangeAccountArgs, ()>(REGISTRY_ID, "change_account", &good_args, POINT_LIMIT)
        .expect("change_account should succeed with a valid quorum");

    let view = session
        .call::<u64, Option<MultisigAccountView>>(REGISTRY_ID, "account", &id, POINT_LIMIT)
        .expect("account query should succeed")
        .data
        .expect("account should exist");
    assert_eq!(view.members, new_members);
    assert_eq!(view.threshold, new_threshold);
    assert_eq!(view.nonce, 1);

    // Replaying the same (now-stale nonce-0) quorum signature again fails —
    // change_message's nonce no longer matches the account's current nonce.
    let replay_args = ChangeAccountArgs {
        account_id: id,
        new_members: new_members.clone(),
        new_threshold,
        sigs: sign_all(&msg, &[(&sk1, &pk1), (&sk2, &pk2)]),
    };
    let result = session.call::<ChangeAccountArgs, ()>(
        REGISTRY_ID,
        "change_account",
        &replay_args,
        POINT_LIMIT,
    );
    assert!(result.is_err(), "a replayed stale-nonce quorum signature must be rejected");
}

/// Aggregates a multisignature over `msg` from exactly `signers`, the way a
/// real coordinator would off-chain: each signs, then the signatures are
/// summed. Same pattern as `rusk-experiments/multisig-approval`'s tests —
/// see that crate's README for why `sign_multisig_insecure` (not the
/// default secure `sign_multisig`) is what `VM::ephemeral()`'s
/// `verify_bls_multisig` host query actually checks against (documented in
/// `references/dusk-native/dusk-vm-issue-1-ephemeral-hardfork-policy-unreachable.md`).
fn aggregate_signature(
    signers: &[(BlsSecretKey, BlsPublicKey)],
    msg: &[u8],
) -> MultisigSignature {
    let sigs: Vec<MultisigSignature> = signers
        .iter()
        .map(|(sk, pk)| sk.sign_multisig_insecure(pk, msg))
        .collect();
    let (first, rest) = sigs.split_first().expect("at least one signer");
    first.aggregate(rest)
}

fn signer_keys(signers: &[(BlsSecretKey, BlsPublicKey)]) -> Vec<BlsPublicKey> {
    signers.iter().map(|(_, pk)| *pk).collect()
}

#[test]
fn verify_quorum_aggregate_true_for_valid_threshold_subset() {
    let rng = &mut StdRng::seed_from_u64(6);
    let mut session = initialize();
    let sk1 = BlsSecretKey::random(rng);
    let pk1 = BlsPublicKey::from(&sk1);
    let sk2 = BlsSecretKey::random(rng);
    let pk2 = BlsPublicKey::from(&sk2);
    let sk3 = BlsSecretKey::random(rng);
    let pk3 = BlsPublicKey::from(&sk3);
    let members = [(sk1, pk1), (sk2, pk2), (sk3, pk3)];

    let create_args = CreateAccountArgs {
        members: alloc::vec![pk1, pk2, pk3],
        threshold: 2,
    };
    let id = session
        .call::<CreateAccountArgs, u64>(REGISTRY_ID, "create_account", &create_args, POINT_LIMIT)
        .expect("create_account should succeed")
        .data;

    let msg = b"aggregate-authorize-something".to_vec();
    let two_of_three = &members[0..2];
    let sig = aggregate_signature(two_of_three, &msg);

    let args = VerifyQuorumAggregateArgs {
        account_id: id,
        msg: msg.clone(),
        signer_keys: signer_keys(two_of_three),
        aggregate_sig: sig,
    };
    let passed = session
        .call::<VerifyQuorumAggregateArgs, bool>(
            REGISTRY_ID,
            "verify_quorum_aggregate",
            &args,
            POINT_LIMIT,
        )
        .expect("verify_quorum_aggregate call should succeed")
        .data;
    assert!(passed, "a valid 2-of-3 aggregate should meet threshold 2");
}

#[test]
fn verify_quorum_aggregate_false_below_threshold() {
    let rng = &mut StdRng::seed_from_u64(7);
    let mut session = initialize();
    let sk1 = BlsSecretKey::random(rng);
    let pk1 = BlsPublicKey::from(&sk1);
    let sk2 = BlsSecretKey::random(rng);
    let pk2 = BlsPublicKey::from(&sk2);
    let sk3 = BlsSecretKey::random(rng);
    let pk3 = BlsPublicKey::from(&sk3);
    let members = [(sk1, pk1), (sk2, pk2), (sk3, pk3)];

    let create_args = CreateAccountArgs {
        members: alloc::vec![pk1, pk2, pk3],
        threshold: 2,
    };
    let id = session
        .call::<CreateAccountArgs, u64>(REGISTRY_ID, "create_account", &create_args, POINT_LIMIT)
        .expect("create_account should succeed")
        .data;

    let msg = b"aggregate-authorize-something".to_vec();
    let one_signer = &members[0..1];
    let sig = aggregate_signature(one_signer, &msg);

    let args = VerifyQuorumAggregateArgs {
        account_id: id,
        msg,
        signer_keys: signer_keys(one_signer),
        aggregate_sig: sig,
    };
    let passed = session
        .call::<VerifyQuorumAggregateArgs, bool>(
            REGISTRY_ID,
            "verify_quorum_aggregate",
            &args,
            POINT_LIMIT,
        )
        .expect("verify_quorum_aggregate call should succeed")
        .data;
    assert!(!passed, "one signer must not meet a threshold of 2");
}

#[test]
fn verify_quorum_aggregate_false_for_non_member_signer() {
    let rng = &mut StdRng::seed_from_u64(8);
    let mut session = initialize();
    let sk1 = BlsSecretKey::random(rng);
    let pk1 = BlsPublicKey::from(&sk1);
    let sk2 = BlsSecretKey::random(rng);
    let pk2 = BlsPublicKey::from(&sk2);
    let outsider_sk = BlsSecretKey::random(rng);
    let outsider_pk = BlsPublicKey::from(&outsider_sk);

    let create_args = CreateAccountArgs {
        members: alloc::vec![pk1, pk2],
        threshold: 2,
    };
    let id = session
        .call::<CreateAccountArgs, u64>(REGISTRY_ID, "create_account", &create_args, POINT_LIMIT)
        .expect("create_account should succeed")
        .data;

    // One real member plus one outsider — same count as the threshold, but
    // the outsider isn't a registered member.
    let msg = b"aggregate-authorize-something".to_vec();
    let mixed = [(sk1, pk1), (outsider_sk, outsider_pk)];
    let sig = aggregate_signature(&mixed, &msg);

    let args = VerifyQuorumAggregateArgs {
        account_id: id,
        msg,
        signer_keys: signer_keys(&mixed),
        aggregate_sig: sig,
    };
    let passed = session
        .call::<VerifyQuorumAggregateArgs, bool>(
            REGISTRY_ID,
            "verify_quorum_aggregate",
            &args,
            POINT_LIMIT,
        )
        .expect("verify_quorum_aggregate call should succeed")
        .data;
    assert!(!passed, "an outsider signer must be rejected even at the right count");
}

#[test]
fn verify_quorum_aggregate_false_for_wrong_message() {
    let rng = &mut StdRng::seed_from_u64(9);
    let mut session = initialize();
    let sk1 = BlsSecretKey::random(rng);
    let pk1 = BlsPublicKey::from(&sk1);
    let sk2 = BlsSecretKey::random(rng);
    let pk2 = BlsPublicKey::from(&sk2);
    let members = [(sk1, pk1), (sk2, pk2)];

    let create_args = CreateAccountArgs {
        members: alloc::vec![pk1, pk2],
        threshold: 2,
    };
    let id = session
        .call::<CreateAccountArgs, u64>(REGISTRY_ID, "create_account", &create_args, POINT_LIMIT)
        .expect("create_account should succeed")
        .data;

    // Signed over one message, submitted against a different one.
    let signed_msg = b"the-real-message".to_vec();
    let submitted_msg = b"a-different-message".to_vec();
    let sig = aggregate_signature(&members, &signed_msg);

    let args = VerifyQuorumAggregateArgs {
        account_id: id,
        msg: submitted_msg,
        signer_keys: signer_keys(&members),
        aggregate_sig: sig,
    };
    let passed = session
        .call::<VerifyQuorumAggregateArgs, bool>(
            REGISTRY_ID,
            "verify_quorum_aggregate",
            &args,
            POINT_LIMIT,
        )
        .expect("verify_quorum_aggregate call should succeed")
        .data;
    assert!(!passed, "an aggregate signed over a different message must not verify");
}

#[test]
fn verify_quorum_aggregate_false_for_unknown_account() {
    let mut session = initialize();
    let rng = &mut StdRng::seed_from_u64(10);
    let sk1 = BlsSecretKey::random(rng);
    let pk1 = BlsPublicKey::from(&sk1);
    let msg = b"anything".to_vec();
    let sig = aggregate_signature(&[(sk1, pk1)], &msg);

    let args = VerifyQuorumAggregateArgs {
        account_id: 999,
        msg,
        signer_keys: alloc::vec![pk1],
        aggregate_sig: sig,
    };
    let passed = session
        .call::<VerifyQuorumAggregateArgs, bool>(
            REGISTRY_ID,
            "verify_quorum_aggregate",
            &args,
            POINT_LIMIT,
        )
        .expect("verify_quorum_aggregate call should succeed even for an unknown account")
        .data;
    assert!(!passed);
}

#[test]
fn wire_option_none_is_32_zero_bytes() {
    // Empirical: rkyv archives `Option<MultisigAccountView>::None` as 32
    // zero bytes (tagged union sized for the Some variant). Useful when
    // inspecting raw RUES bodies. The live "always None" false alarm was a
    // client hex-encoding bug (wrong u64 id), not this shape and not a
    // contract/query state failure — see knot-tool README.
    let none_bytes = rkyv::to_bytes::<_, 256>(&Option::<MultisigAccountView>::None).unwrap();
    assert_eq!(none_bytes.as_slice(), &[0u8; 32]);

    let mut rng = StdRng::seed_from_u64(1);
    let sk = BlsSecretKey::random(&mut rng);
    let pk = BlsPublicKey::from(&sk);
    let some = Some(MultisigAccountView {
        members: alloc::vec![pk],
        threshold: 1,
        nonce: 0,
    });
    let some_bytes = rkyv::to_bytes::<_, 256>(&some).unwrap();
    assert_ne!(some_bytes.as_slice(), &[0u8; 32]);
    assert!(some_bytes.len() > 32);
}

#[test]
fn public_key_rkyv_roundtrip_still_matches_for_contains() {
    let mut rng = StdRng::seed_from_u64(2);
    let sk = BlsSecretKey::random(&mut rng);
    let pk = BlsPublicKey::from(&sk);

    let members_bytes = rkyv::to_bytes::<_, 256>(&alloc::vec![pk]).unwrap();
    let mut aligned = rkyv::AlignedVec::with_capacity(members_bytes.len());
    aligned.extend_from_slice(&members_bytes);
    let archived = rkyv::check_archived_root::<Vec<BlsPublicKey>>(&aligned).unwrap();
    use rkyv::Deserialize;
    let members2: Vec<BlsPublicKey> = archived.deserialize(&mut rkyv::Infallible).unwrap();

    let signer_bytes = rkyv::to_bytes::<_, 256>(&pk).unwrap();
    let mut aligned2 = rkyv::AlignedVec::with_capacity(signer_bytes.len());
    aligned2.extend_from_slice(&signer_bytes);
    let archived_pk = rkyv::check_archived_root::<BlsPublicKey>(&aligned2).unwrap();
    let signer: BlsPublicKey = archived_pk.deserialize(&mut rkyv::Infallible).unwrap();

    assert!(members2.contains(&signer));
    assert_eq!(pk.to_bytes(), members2[0].to_bytes());
    assert_eq!(pk.to_bytes(), signer.to_bytes());
}

#[test]
fn next_account_id_and_account_roundtrip() {
    let rng = &mut StdRng::seed_from_u64(11);
    let mut session = initialize();
    let (_sk1, pk1) = keypair(rng);
    let (_sk2, pk2) = keypair(rng);

    assert_eq!(
        session
            .call::<(), u64>(REGISTRY_ID, "next_account_id", &(), POINT_LIMIT)
            .expect("next_account_id")
            .data,
        0
    );

    let id = session
        .call::<CreateAccountArgs, u64>(
            REGISTRY_ID,
            "create_account",
            &CreateAccountArgs {
                members: alloc::vec![pk1, pk2],
                threshold: 2,
            },
            POINT_LIMIT,
        )
        .expect("create")
        .data;
    assert_eq!(id, 0);
    assert_eq!(
        session
            .call::<(), u64>(REGISTRY_ID, "next_account_id", &(), POINT_LIMIT)
            .unwrap()
            .data,
        1
    );

    let view = session
        .call::<u64, Option<MultisigAccountView>>(REGISTRY_ID, "account", &id, POINT_LIMIT)
        .unwrap()
        .data
        .expect("account");
    assert_eq!(view.threshold, 2);
    assert_eq!(view.nonce, 0);
    assert_eq!(view.members.len(), 2);
    assert_eq!(view.members[0].to_bytes(), pk1.to_bytes());
    assert_eq!(view.members[1].to_bytes(), pk2.to_bytes());
}
