//! Tests for `multisig-proposals` v0.2 (M1): structured propose, digest
//! signing, execute via `call_raw`, per-committee nonce, merge/tombstone.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use dusk_bytes::Serializable;
use dusk_core::abi::{ContractId, Metadata};
use dusk_core::signatures::bls::{PublicKey as BlsPublicKey, SecretKey as BlsSecretKey};
use dusk_vm::{ContractData, Session, VM};
use rand::rngs::StdRng;
use rand::SeedableRng;
use rkyv::ser::serializers::AllocSerializer;
use rkyv::ser::Serializer;
use rkyv::Serialize;

#[path = "../src/call_types.rs"]
mod call_types;
use call_types::{ApproveArgs, ProposalStatus, ProposalView, ProposeArgs};

#[path = "../../multisig-registry/src/call_types.rs"]
mod registry_call_types;
use registry_call_types::CreateAccountArgs;

const PROPOSALS_BYTECODE: &[u8] = include_bytes!(
    "../../../target/contract/wasm32-unknown-unknown/release/multisig_proposals.wasm"
);
const REGISTRY_BYTECODE: &[u8] = include_bytes!(
    "../../../target/contract/wasm32-unknown-unknown/release/multisig_registry.wasm"
);
const TARGET_BYTECODE: &[u8] = include_bytes!(
    "../test-target/target/contract/wasm32-unknown-unknown/release/proposals_test_target.wasm"
);

const PROPOSALS_ID: ContractId = ContractId::from_bytes([0xb2; 32]);
const REGISTRY_ID: ContractId = ContractId::from_bytes([0xa1; 32]);
const TARGET_ID: ContractId = ContractId::from_bytes([0xb3; 32]);
const CHAIN_ID: u8 = 0xCA;
const DIGEST_CHAIN_ID: u64 = 0xCA;
const POINT_LIMIT: u64 = 0x10000000;

fn keypair(rng: &mut StdRng) -> (BlsSecretKey, BlsPublicKey) {
    let sk = BlsSecretKey::random(rng);
    let pk = BlsPublicKey::from(&sk);
    (sk, pk)
}

fn set_sender(session: &mut Session, sender: Option<&BlsPublicKey>) {
    session
        .set_meta(Metadata::PUBLIC_SENDER, sender.copied())
        .expect("setting public_sender metadata should succeed");
}

fn rkyv_bytes<T>(value: &T) -> Vec<u8>
where
    T: Serialize<AllocSerializer<256>>,
{
    let mut ser = AllocSerializer::<256>::default();
    ser.serialize_value(value).expect("rkyv serialize");
    ser.into_serializer().into_inner().to_vec()
}

fn initialize(owner_pk: &BlsPublicKey) -> Session {
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
        .expect("Deploying multisig-registry should succeed");

    session
        .deploy(
            PROPOSALS_BYTECODE,
            ContractData::builder()
                .owner(owner_pk.to_bytes().to_vec())
                .contract_id(PROPOSALS_ID),
            POINT_LIMIT,
        )
        .expect("Deploying multisig-proposals should succeed");

    session
        .deploy(
            TARGET_BYTECODE,
            ContractData::builder()
                .owner([0; 32])
                .contract_id(TARGET_ID),
            POINT_LIMIT,
        )
        .expect("Deploying test target should succeed");

    session
}

fn init_proposals(session: &mut Session, owner_pk: &BlsPublicKey) {
    set_sender(session, Some(owner_pk));
    session
        .call::<ContractId, ()>(PROPOSALS_ID, "init_registry", &REGISTRY_ID, POINT_LIMIT)
        .expect("init_registry");
    session
        .call::<u64, ()>(PROPOSALS_ID, "init_chain_id", &DIGEST_CHAIN_ID, POINT_LIMIT)
        .expect("init_chain_id");
    // Lab: skip tombstone delay so status is Executed after finalize.
    session
        .call::<u64, ()>(PROPOSALS_ID, "set_tombstone_ttl", &0u64, POINT_LIMIT)
        .expect("set_tombstone_ttl");
    set_sender(session, None);
}

fn create_account(
    session: &mut Session,
    members: Vec<BlsPublicKey>,
    threshold: u32,
) -> u64 {
    session
        .call::<CreateAccountArgs, u64>(
            REGISTRY_ID,
            "create_account",
            &CreateAccountArgs { members, threshold },
            POINT_LIMIT,
        )
        .expect("create_account should succeed")
        .data
}

fn propose_set_value(session: &mut Session, account_id: u64, value: u64) -> (u64, [u8; 32]) {
    let args = ProposeArgs {
        registry_account_id: account_id,
        target: TARGET_ID,
        function_name: String::from("set_value"),
        call_args: rkyv_bytes(&value),
        deadline: 0,
    };
    let proposal_id = session
        .call::<ProposeArgs, u64>(PROPOSALS_ID, "propose", &args, POINT_LIMIT)
        .expect("propose should succeed")
        .data;
    let view = session
        .call::<u64, Option<ProposalView>>(PROPOSALS_ID, "proposal", &proposal_id, POINT_LIMIT)
        .expect("proposal query")
        .data
        .expect("proposal exists");
    (proposal_id, view.signed_digest)
}

fn approve(
    session: &mut Session,
    proposal_id: u64,
    sk: &BlsSecretKey,
    pk: &BlsPublicKey,
    digest: &[u8; 32],
) {
    let args = ApproveArgs {
        proposal_id,
        signer: *pk,
        signature: sk.sign_insecure(digest),
    };
    session
        .call::<ApproveArgs, ()>(PROPOSALS_ID, "approve", &args, POINT_LIMIT)
        .expect("approve should succeed");
}

#[test]
fn init_registry_rejects_non_owner() {
    let rng = &mut StdRng::seed_from_u64(1);
    let (_owner_sk, owner_pk) = keypair(rng);
    let (_attacker_sk, attacker_pk) = keypair(rng);
    let mut session = initialize(&owner_pk);

    set_sender(&mut session, Some(&attacker_pk));
    let result =
        session.call::<ContractId, ()>(PROPOSALS_ID, "init_registry", &REGISTRY_ID, POINT_LIMIT);
    assert!(result.is_err(), "init_registry should reject a non-owner");
}

#[test]
fn propose_approve_finalize_executes_target() {
    let rng = &mut StdRng::seed_from_u64(2);
    let (_owner_sk, owner_pk) = keypair(rng);
    let (sk1, pk1) = keypair(rng);
    let (sk2, pk2) = keypair(rng);
    let (_sk3, pk3) = keypair(rng);

    let mut session = initialize(&owner_pk);
    init_proposals(&mut session, &owner_pk);

    let account_id = create_account(&mut session, alloc::vec![pk1, pk2, pk3], 2);
    let (proposal_id, digest) = propose_set_value(&mut session, account_id, 42);

    let status = session
        .call::<u64, Option<ProposalStatus>>(PROPOSALS_ID, "status", &proposal_id, POINT_LIMIT)
        .expect("status")
        .data
        .expect("exists");
    assert_eq!(status, ProposalStatus::Open);

    approve(&mut session, proposal_id, &sk1, &pk1, &digest);
    approve(&mut session, proposal_id, &sk2, &pk2, &digest);

    session
        .call::<u64, ()>(PROPOSALS_ID, "finalize", &proposal_id, POINT_LIMIT)
        .expect("finalize should succeed once threshold is met");

    let view = session
        .call::<u64, Option<ProposalView>>(PROPOSALS_ID, "proposal", &proposal_id, POINT_LIMIT)
        .expect("proposal query")
        .data
        .expect("exists");
    assert_eq!(view.status, ProposalStatus::Executed);
    assert_eq!(view.approvals.len(), 2);

    let target_value = session
        .call::<(), u64>(TARGET_ID, "value", &(), POINT_LIMIT)
        .expect("target value")
        .data;
    assert_eq!(target_value, 42);

    let nonce = session
        .call::<u64, u64>(PROPOSALS_ID, "committee_nonce", &account_id, POINT_LIMIT)
        .expect("nonce")
        .data;
    assert_eq!(nonce, 1);
}

#[test]
fn approve_rejects_non_member_and_bad_signature() {
    let rng = &mut StdRng::seed_from_u64(3);
    let (_owner_sk, owner_pk) = keypair(rng);
    let (sk1, pk1) = keypair(rng);
    let (_sk2, pk2) = keypair(rng);
    let (outsider_sk, outsider_pk) = keypair(rng);

    let mut session = initialize(&owner_pk);
    init_proposals(&mut session, &owner_pk);
    let account_id = create_account(&mut session, alloc::vec![pk1, pk2], 2);
    let (proposal_id, digest) = propose_set_value(&mut session, account_id, 7);

    let bad = ApproveArgs {
        proposal_id,
        signer: outsider_pk,
        signature: outsider_sk.sign_insecure(&digest),
    };
    assert!(
        session
            .call::<ApproveArgs, ()>(PROPOSALS_ID, "approve", &bad, POINT_LIMIT)
            .is_err(),
        "non-member approve must fail"
    );

    let wrong = [0u8; 32];
    let bad = ApproveArgs {
        proposal_id,
        signer: pk1,
        signature: sk1.sign_insecure(&wrong),
    };
    assert!(
        session
            .call::<ApproveArgs, ()>(PROPOSALS_ID, "approve", &bad, POINT_LIMIT)
            .is_err(),
        "wrong-digest signature must fail"
    );
}

#[test]
fn approve_rejects_duplicate_signer() {
    let rng = &mut StdRng::seed_from_u64(4);
    let (_owner_sk, owner_pk) = keypair(rng);
    let (sk1, pk1) = keypair(rng);
    let (_sk2, pk2) = keypair(rng);

    let mut session = initialize(&owner_pk);
    init_proposals(&mut session, &owner_pk);
    let account_id = create_account(&mut session, alloc::vec![pk1, pk2], 2);
    let (proposal_id, digest) = propose_set_value(&mut session, account_id, 1);

    approve(&mut session, proposal_id, &sk1, &pk1, &digest);

    let dup = ApproveArgs {
        proposal_id,
        signer: pk1,
        signature: sk1.sign_insecure(&digest),
    };
    assert!(
        session
            .call::<ApproveArgs, ()>(PROPOSALS_ID, "approve", &dup, POINT_LIMIT)
            .is_err(),
        "duplicate approve must fail"
    );
}

#[test]
fn finalize_rejects_under_threshold_then_succeeds() {
    let rng = &mut StdRng::seed_from_u64(5);
    let (_owner_sk, owner_pk) = keypair(rng);
    let (sk1, pk1) = keypair(rng);
    let (sk2, pk2) = keypair(rng);
    let (_sk3, pk3) = keypair(rng);

    let mut session = initialize(&owner_pk);
    init_proposals(&mut session, &owner_pk);
    let account_id = create_account(&mut session, alloc::vec![pk1, pk2, pk3], 2);
    let (proposal_id, digest) = propose_set_value(&mut session, account_id, 9);

    approve(&mut session, proposal_id, &sk1, &pk1, &digest);

    assert!(
        session
            .call::<u64, ()>(PROPOSALS_ID, "finalize", &proposal_id, POINT_LIMIT)
            .is_err(),
        "finalize with 1 of 2 must fail"
    );

    approve(&mut session, proposal_id, &sk2, &pk2, &digest);
    session
        .call::<u64, ()>(PROPOSALS_ID, "finalize", &proposal_id, POINT_LIMIT)
        .expect("finalize with 2 of 2 should succeed");

    assert!(
        session
            .call::<u64, ()>(PROPOSALS_ID, "finalize", &proposal_id, POINT_LIMIT)
            .is_err(),
        "finalize on an already-executed proposal must fail"
    );
}

#[test]
fn propose_fails_before_init_registry() {
    let rng = &mut StdRng::seed_from_u64(6);
    let (_owner_sk, owner_pk) = keypair(rng);
    let mut session = initialize(&owner_pk);

    let args = ProposeArgs {
        registry_account_id: 0,
        target: TARGET_ID,
        function_name: String::from("set_value"),
        call_args: rkyv_bytes(&1u64),
        deadline: 0,
    };
    let result = session.call::<ProposeArgs, u64>(PROPOSALS_ID, "propose", &args, POINT_LIMIT);
    assert!(result.is_err(), "propose before init_registry must fail");
}

#[test]
fn identical_open_digest_merges() {
    let rng = &mut StdRng::seed_from_u64(7);
    let (_owner_sk, owner_pk) = keypair(rng);
    let (_sk1, pk1) = keypair(rng);
    let (_sk2, pk2) = keypair(rng);

    let mut session = initialize(&owner_pk);
    init_proposals(&mut session, &owner_pk);
    let account_id = create_account(&mut session, alloc::vec![pk1, pk2], 2);

    let (id1, _) = propose_set_value(&mut session, account_id, 3);
    let (id2, _) = propose_set_value(&mut session, account_id, 3);
    assert_eq!(id1, id2, "identical open digests must merge");
}

#[test]
fn free_reads_roundtrip() {
    let rng = &mut StdRng::seed_from_u64(8);
    let (_owner_sk, owner_pk) = keypair(rng);
    let (sk1, pk1) = keypair(rng);
    let (_sk2, pk2) = keypair(rng);

    let mut session = initialize(&owner_pk);
    init_proposals(&mut session, &owner_pk);
    let account_id = create_account(&mut session, alloc::vec![pk1, pk2], 1);

    assert_eq!(
        session
            .call::<(), u64>(PROPOSALS_ID, "next_proposal_id", &(), POINT_LIMIT)
            .unwrap()
            .data,
        0
    );

    let (proposal_id, digest) = propose_set_value(&mut session, account_id, 5);
    assert_eq!(
        session
            .call::<(), u64>(PROPOSALS_ID, "next_proposal_id", &(), POINT_LIMIT)
            .unwrap()
            .data,
        1
    );

    approve(&mut session, proposal_id, &sk1, &pk1, &digest);

    let view = session
        .call::<u64, Option<ProposalView>>(PROPOSALS_ID, "proposal", &proposal_id, POINT_LIMIT)
        .unwrap()
        .data
        .expect("view");
    assert_eq!(view.approvals, alloc::vec![pk1]);
    assert_eq!(view.status, ProposalStatus::Open);
    assert_eq!(view.signed_digest, digest);
}
