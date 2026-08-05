//! Tests for `knot-proposals` v3: epoch, caller nonce, digest consumed flag,
//! prune, rich events path, CEI finalize.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use dusk_bytes::Serializable;
use dusk_core::abi::{ContractId, Metadata};
use dusk_core::signatures::bls::{PublicKey as BlsPublicKey, SecretKey as BlsSecretKey};
use dusk_vm::{ContractData, Session, VM};
use knot_encoding::proposal_digest_v3;
use rand::SeedableRng;
use rand::rngs::StdRng;
use rkyv::Serialize;
use rkyv::ser::Serializer;
use rkyv::ser::serializers::AllocSerializer;

#[path = "../src/call_types.rs"]
mod call_types;
use call_types::{ApproveArgs, ProposalStatus, ProposalView, ProposeArgs};

#[path = "../../knot-registry/src/call_types.rs"]
mod registry_call_types;
use registry_call_types::CreateAccountArgs;

const PROPOSALS_BYTECODE: &[u8] =
    include_bytes!("../../../target/contract/wasm32-unknown-unknown/release/knot_proposals.wasm");
const REGISTRY_BYTECODE: &[u8] =
    include_bytes!("../../../target/contract/wasm32-unknown-unknown/release/knot_registry.wasm");
const TARGET_BYTECODE: &[u8] = include_bytes!(
    "../test-target/target/contract/wasm32-unknown-unknown/release/proposals_test_target.wasm"
);

const PROPOSALS_ID: ContractId = ContractId::from_bytes([0xb2; 32]);
const PROPOSALS_ID_B: ContractId = ContractId::from_bytes([0xb4; 32]);
const REGISTRY_ID: ContractId = ContractId::from_bytes([0xa1; 32]);
const TARGET_ID: ContractId = ContractId::from_bytes([0xb3; 32]);
const CHAIN_ID: u8 = 0xCA;
const POINT_LIMIT: u64 = 0x10000000;
const DEFAULT_TTL: u64 = 1000;

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

fn set_block_height(session: &mut Session, height: u64) {
    session
        .set_meta(Metadata::BLOCK_HEIGHT, Some(height))
        .expect("setting block_height metadata should succeed");
}

fn rkyv_bytes<T>(value: &T) -> Vec<u8>
where
    T: Serialize<AllocSerializer<256>>,
{
    let mut ser = AllocSerializer::<256>::default();
    ser.serialize_value(value).expect("rkyv serialize");
    ser.into_serializer().into_inner().to_vec()
}

fn deadline_at_height(height: u64) -> u64 {
    height + DEFAULT_TTL
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
        .expect("Deploying knot-registry should succeed");

    session
        .deploy(
            PROPOSALS_BYTECODE,
            ContractData::builder()
                .owner(owner_pk.to_bytes().to_vec())
                .contract_id(PROPOSALS_ID),
            POINT_LIMIT,
        )
        .expect("Deploying knot-proposals should succeed");

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
        .call::<bool, ()>(PROPOSALS_ID, "set_tombstone", &false, POINT_LIMIT)
        .expect("set_tombstone");
    set_sender(session, None);
}

fn create_account(session: &mut Session, members: Vec<BlsPublicKey>, threshold: u32) -> u64 {
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

fn propose_set_value(
    session: &mut Session,
    account_id: u64,
    value: u64,
    nonce: u64,
) -> (u64, [u8; 32]) {
    propose_fn(
        session,
        account_id,
        "set_value",
        value,
        nonce,
        deadline_at_height(0),
    )
}

fn propose_fn(
    session: &mut Session,
    account_id: u64,
    function_name: &str,
    value: u64,
    nonce: u64,
    deadline: u64,
) -> (u64, [u8; 32]) {
    let args = ProposeArgs {
        registry_account_id: account_id,
        target: TARGET_ID,
        function_name: String::from(function_name),
        call_args: rkyv_bytes(&value),
        nonce,
        deadline,
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
fn abi_chain_id_available_under_ephemeral_vm() {
    let rng = &mut StdRng::seed_from_u64(0xCAFE);
    let (_owner_sk, owner_pk) = keypair(rng);
    let mut session = initialize(&owner_pk);

    let chain_id: u8 = session
        .call::<(), u8>(TARGET_ID, "chain_id", &(), POINT_LIMIT)
        .expect("abi::chain_id probe call should succeed")
        .data;

    assert_eq!(chain_id, CHAIN_ID);
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
    let (proposal_id, digest) = propose_set_value(&mut session, account_id, 42, 1);

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

    let target_value = session
        .call::<(), u64>(TARGET_ID, "value", &(), POINT_LIMIT)
        .expect("target value")
        .data;
    assert_eq!(target_value, 42);
}

#[test]
fn three_parallel_proposals_one_finalizes_others_still_finalizable() {
    let rng = &mut StdRng::seed_from_u64(20);
    let (_owner_sk, owner_pk) = keypair(rng);
    let (sk1, pk1) = keypair(rng);
    let (sk2, pk2) = keypair(rng);

    let mut session = initialize(&owner_pk);
    init_proposals(&mut session, &owner_pk);
    let account_id = create_account(&mut session, alloc::vec![pk1, pk2], 2);

    let (p1, d1) = propose_set_value(&mut session, account_id, 1, 1);
    let (p2, d2) = propose_set_value(&mut session, account_id, 2, 2);
    let (p3, d3) = propose_set_value(&mut session, account_id, 3, 3);

    approve(&mut session, p2, &sk1, &pk1, &d2);
    approve(&mut session, p2, &sk2, &pk2, &d2);
    session
        .call::<u64, ()>(PROPOSALS_ID, "finalize", &p2, POINT_LIMIT)
        .expect("finalize p2");

    approve(&mut session, p1, &sk1, &pk1, &d1);
    approve(&mut session, p1, &sk2, &pk2, &d1);
    session
        .call::<u64, ()>(PROPOSALS_ID, "finalize", &p1, POINT_LIMIT)
        .expect("finalize p1 after p2 landed");

    approve(&mut session, p3, &sk1, &pk1, &d3);
    approve(&mut session, p3, &sk2, &pk2, &d3);
    session
        .call::<u64, ()>(PROPOSALS_ID, "finalize", &p3, POINT_LIMIT)
        .expect("finalize p3");
}

#[test]
fn re_propose_executed_digest_panics() {
    let rng = &mut StdRng::seed_from_u64(21);
    let (_owner_sk, owner_pk) = keypair(rng);
    let (sk1, pk1) = keypair(rng);

    let mut session = initialize(&owner_pk);
    init_proposals(&mut session, &owner_pk);
    let account_id = create_account(&mut session, alloc::vec![pk1], 1);

    let (proposal_id, digest) = propose_set_value(&mut session, account_id, 9, 7);
    approve(&mut session, proposal_id, &sk1, &pk1, &digest);
    session
        .call::<u64, ()>(PROPOSALS_ID, "finalize", &proposal_id, POINT_LIMIT)
        .expect("finalize");

    let args = ProposeArgs {
        registry_account_id: account_id,
        target: TARGET_ID,
        function_name: String::from("set_value"),
        call_args: rkyv_bytes(&9u64),
        nonce: 7,
        deadline: deadline_at_height(0),
    };
    assert!(
        session
            .call::<ProposeArgs, u64>(PROPOSALS_ID, "propose", &args, POINT_LIMIT)
            .is_err(),
        "re-propose executed digest must panic"
    );
}

#[test]
fn deadline_eq_block_height_accepted() {
    let rng = &mut StdRng::seed_from_u64(22);
    let (_owner_sk, owner_pk) = keypair(rng);
    let (_sk1, pk1) = keypair(rng);

    let mut session = initialize(&owner_pk);
    init_proposals(&mut session, &owner_pk);
    let account_id = create_account(&mut session, alloc::vec![pk1], 1);

    set_block_height(&mut session, 100);
    let args = ProposeArgs {
        registry_account_id: account_id,
        target: TARGET_ID,
        function_name: String::from("set_value"),
        call_args: rkyv_bytes(&1u64),
        nonce: 1,
        deadline: 100,
    };
    session
        .call::<ProposeArgs, u64>(PROPOSALS_ID, "propose", &args, POINT_LIMIT)
        .expect("deadline == block_height must be accepted at propose");
}

#[test]
fn propose_rejects_deadline_exceeds_ttl() {
    let rng = &mut StdRng::seed_from_u64(23);
    let (_owner_sk, owner_pk) = keypair(rng);
    let (_sk1, pk1) = keypair(rng);

    let mut session = initialize(&owner_pk);
    init_proposals(&mut session, &owner_pk);
    let account_id = create_account(&mut session, alloc::vec![pk1], 1);

    let args = ProposeArgs {
        registry_account_id: account_id,
        target: TARGET_ID,
        function_name: String::from("set_value"),
        call_args: rkyv_bytes(&1u64),
        nonce: 1,
        deadline: DEFAULT_TTL + 1,
    };
    assert!(
        session
            .call::<ProposeArgs, u64>(PROPOSALS_ID, "propose", &args, POINT_LIMIT)
            .is_err(),
        "deadline > now + ttl must fail"
    );
}

#[test]
fn propose_rejects_zero_deadline() {
    let rng = &mut StdRng::seed_from_u64(24);
    let (_owner_sk, owner_pk) = keypair(rng);
    let (_sk1, pk1) = keypair(rng);

    let mut session = initialize(&owner_pk);
    init_proposals(&mut session, &owner_pk);
    let account_id = create_account(&mut session, alloc::vec![pk1], 1);

    let args = ProposeArgs {
        registry_account_id: account_id,
        target: TARGET_ID,
        function_name: String::from("set_value"),
        call_args: rkyv_bytes(&1u64),
        nonce: 1,
        deadline: 0,
    };
    assert!(
        session
            .call::<ProposeArgs, u64>(PROPOSALS_ID, "propose", &args, POINT_LIMIT)
            .is_err(),
        "deadline 0 must fail"
    );
}

#[test]
fn set_proposal_ttl_rejects_zero_and_over_max() {
    let rng = &mut StdRng::seed_from_u64(25);
    let (_owner_sk, owner_pk) = keypair(rng);
    let mut session = initialize(&owner_pk);

    set_sender(&mut session, Some(&owner_pk));
    session
        .call::<ContractId, ()>(PROPOSALS_ID, "init_registry", &REGISTRY_ID, POINT_LIMIT)
        .expect("init_registry");

    assert!(
        session
            .call::<u64, ()>(PROPOSALS_ID, "set_proposal_ttl", &0u64, POINT_LIMIT)
            .is_err()
    );
    assert!(
        session
            .call::<u64, ()>(PROPOSALS_ID, "set_proposal_ttl", &100_001u64, POINT_LIMIT)
            .is_err()
    );
}

#[test]
fn epoch_bump_invalidates_old_proposals() {
    let rng = &mut StdRng::seed_from_u64(26);
    let (_owner_sk, owner_pk) = keypair(rng);
    let (sk1, pk1) = keypair(rng);

    let mut session = initialize(&owner_pk);
    init_proposals(&mut session, &owner_pk);
    let account_id = create_account(&mut session, alloc::vec![pk1], 1);
    let (proposal_id, digest) = propose_set_value(&mut session, account_id, 1, 1);

    set_sender(&mut session, Some(&owner_pk));
    session
        .call::<ContractId, ()>(PROPOSALS_ID, "init_registry", &REGISTRY_ID, POINT_LIMIT)
        .expect("re-init registry bumps epoch");
    set_sender(&mut session, None);

    let approve_args = ApproveArgs {
        proposal_id,
        signer: pk1,
        signature: sk1.sign_insecure(&digest),
    };
    assert!(
        session
            .call::<ApproveArgs, ()>(PROPOSALS_ID, "approve", &approve_args, POINT_LIMIT)
            .is_err(),
        "old-epoch proposal must not be approvable"
    );
}

#[test]
fn prune_retains_consumed_digest_before_deadline() {
    let rng = &mut StdRng::seed_from_u64(27);
    let (_owner_sk, owner_pk) = keypair(rng);
    let (sk1, pk1) = keypair(rng);

    let mut session = initialize(&owner_pk);
    init_proposals(&mut session, &owner_pk);
    let account_id = create_account(&mut session, alloc::vec![pk1], 1);
    let (proposal_id, digest) = propose_set_value(&mut session, account_id, 5, 11);
    approve(&mut session, proposal_id, &sk1, &pk1, &digest);
    session
        .call::<u64, ()>(PROPOSALS_ID, "finalize", &proposal_id, POINT_LIMIT)
        .expect("finalize");

    let pruned: u32 = session
        .call::<u32, u32>(PROPOSALS_ID, "prune", &128u32, POINT_LIMIT)
        .expect("prune")
        .data;
    assert!(pruned >= 1);

    let args = ProposeArgs {
        registry_account_id: account_id,
        target: TARGET_ID,
        function_name: String::from("set_value"),
        call_args: rkyv_bytes(&5u64),
        nonce: 11,
        deadline: deadline_at_height(0),
    };
    assert!(
        session
            .call::<ProposeArgs, u64>(PROPOSALS_ID, "propose", &args, POINT_LIMIT)
            .is_err(),
        "consumed digest must still block re-propose before deadline expiry"
    );
}

#[test]
fn finalize_targeting_self_panics() {
    let rng = &mut StdRng::seed_from_u64(28);
    let (_owner_sk, owner_pk) = keypair(rng);
    let (sk1, pk1) = keypair(rng);

    let mut session = initialize(&owner_pk);
    init_proposals(&mut session, &owner_pk);
    let account_id = create_account(&mut session, alloc::vec![pk1], 1);

    let args = ProposeArgs {
        registry_account_id: account_id,
        target: PROPOSALS_ID,
        function_name: String::from("epoch"),
        call_args: rkyv_bytes(&()),
        nonce: 99,
        deadline: deadline_at_height(0),
    };
    let proposal_id = session
        .call::<ProposeArgs, u64>(PROPOSALS_ID, "propose", &args, POINT_LIMIT)
        .expect("propose self-target")
        .data;
    let view = session
        .call::<u64, Option<ProposalView>>(PROPOSALS_ID, "proposal", &proposal_id, POINT_LIMIT)
        .expect("proposal")
        .data
        .expect("exists");
    approve(&mut session, proposal_id, &sk1, &pk1, &view.signed_digest);

    assert!(
        session
            .call::<u64, ()>(PROPOSALS_ID, "finalize", &proposal_id, POINT_LIMIT)
            .is_err(),
        "finalize targeting self must panic"
    );
}

#[test]
fn init_registry_after_many_proposals_succeeds() {
    let rng = &mut StdRng::seed_from_u64(29);
    let (_owner_sk, owner_pk) = keypair(rng);
    let (_sk1, pk1) = keypair(rng);

    let mut session = initialize(&owner_pk);
    init_proposals(&mut session, &owner_pk);
    let account_id = create_account(&mut session, alloc::vec![pk1], 1);

    for n in 0..200u64 {
        let _ = propose_set_value(&mut session, account_id, n, n + 1);
    }

    set_sender(&mut session, Some(&owner_pk));
    session
        .call::<ContractId, ()>(PROPOSALS_ID, "init_registry", &REGISTRY_ID, POINT_LIMIT)
        .expect("init_registry after many proposals must be O(1)");
    set_sender(&mut session, None);
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

    let (id1, _) = propose_set_value(&mut session, account_id, 3, 5);
    let (id2, _) = propose_set_value(&mut session, account_id, 3, 5);
    assert_eq!(id1, id2, "identical open digests must merge");
}

#[test]
fn h1_same_intent_two_proposals_contracts_differ() {
    let rng = &mut StdRng::seed_from_u64(30);
    let (_owner_sk, owner_pk) = keypair(rng);
    let vm = VM::ephemeral().expect("vm");
    let mut session = vm.genesis_session(CHAIN_ID);

    session
        .deploy(
            PROPOSALS_BYTECODE,
            ContractData::builder()
                .owner(owner_pk.to_bytes().to_vec())
                .contract_id(PROPOSALS_ID),
            POINT_LIMIT,
        )
        .expect("deploy A");
    session
        .deploy(
            PROPOSALS_BYTECODE,
            ContractData::builder()
                .owner(owner_pk.to_bytes().to_vec())
                .contract_id(PROPOSALS_ID_B),
            POINT_LIMIT,
        )
        .expect("deploy B");

    let epoch = 1u64;
    let digest_a = proposal_digest_v3(
        u64::from(CHAIN_ID),
        &PROPOSALS_ID.to_bytes(),
        epoch,
        1,
        42,
        &TARGET_ID.to_bytes(),
        b"set_value",
        &rkyv_bytes(&1u64),
        DEFAULT_TTL,
    )
    .unwrap();
    let digest_b = proposal_digest_v3(
        u64::from(CHAIN_ID),
        &PROPOSALS_ID_B.to_bytes(),
        epoch,
        1,
        42,
        &TARGET_ID.to_bytes(),
        b"set_value",
        &rkyv_bytes(&1u64),
        DEFAULT_TTL,
    )
    .unwrap();
    assert_ne!(digest_a, digest_b);
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
    let (proposal_id, digest) = propose_set_value(&mut session, account_id, 7, 1);

    let bad = ApproveArgs {
        proposal_id,
        signer: outsider_pk,
        signature: outsider_sk.sign_insecure(&digest),
    };
    assert!(
        session
            .call::<ApproveArgs, ()>(PROPOSALS_ID, "approve", &bad, POINT_LIMIT)
            .is_err()
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
            .is_err()
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
        nonce: 1,
        deadline: deadline_at_height(0),
    };
    assert!(
        session
            .call::<ProposeArgs, u64>(PROPOSALS_ID, "propose", &args, POINT_LIMIT)
            .is_err()
    );
}

#[test]
fn propose_rejects_past_deadline() {
    let rng = &mut StdRng::seed_from_u64(13);
    let (_owner_sk, owner_pk) = keypair(rng);
    let (_sk1, pk1) = keypair(rng);

    let mut session = initialize(&owner_pk);
    init_proposals(&mut session, &owner_pk);
    let account_id = create_account(&mut session, alloc::vec![pk1], 1);

    set_block_height(&mut session, 100);
    let args = ProposeArgs {
        registry_account_id: account_id,
        target: TARGET_ID,
        function_name: String::from("set_value"),
        call_args: rkyv_bytes(&1u64),
        nonce: 1,
        deadline: 99,
    };
    assert!(
        session
            .call::<ProposeArgs, u64>(PROPOSALS_ID, "propose", &args, POINT_LIMIT)
            .is_err()
    );
}

#[test]
fn finalize_reentrancy_runs_target_once() {
    let rng = &mut StdRng::seed_from_u64(9);
    let (_owner_sk, owner_pk) = keypair(rng);
    let (sk1, pk1) = keypair(rng);
    let (sk2, pk2) = keypair(rng);

    let mut session = initialize(&owner_pk);
    init_proposals(&mut session, &owner_pk);
    let account_id = create_account(&mut session, alloc::vec![pk1, pk2], 2);

    let (proposal_id, digest) = propose_fn(
        &mut session,
        account_id,
        "set_value_reenter_finalize",
        77,
        1,
        deadline_at_height(0),
    );

    session
        .call::<(ContractId, u64), ()>(
            TARGET_ID,
            "configure_reenter",
            &(PROPOSALS_ID, proposal_id),
            POINT_LIMIT,
        )
        .expect("configure_reenter");

    approve(&mut session, proposal_id, &sk1, &pk1, &digest);
    approve(&mut session, proposal_id, &sk2, &pk2, &digest);

    session
        .call::<u64, ()>(PROPOSALS_ID, "finalize", &proposal_id, POINT_LIMIT)
        .expect("finalize with reentrant target should succeed under CEI");

    let hits = session
        .call::<(), u64>(TARGET_ID, "hit_count", &(), POINT_LIMIT)
        .expect("hit_count")
        .data;
    assert_eq!(hits, 1);
}

#[test]
fn finalize_failed_call_raw_leaves_proposal_open() {
    let rng = &mut StdRng::seed_from_u64(10);
    let (_owner_sk, owner_pk) = keypair(rng);
    let (sk1, pk1) = keypair(rng);
    let (sk2, pk2) = keypair(rng);

    let mut session = initialize(&owner_pk);
    init_proposals(&mut session, &owner_pk);
    let account_id = create_account(&mut session, alloc::vec![pk1, pk2], 2);

    let (proposal_id, digest) = propose_fn(
        &mut session,
        account_id,
        "fail_set",
        1,
        1,
        deadline_at_height(0),
    );
    approve(&mut session, proposal_id, &sk1, &pk1, &digest);
    approve(&mut session, proposal_id, &sk2, &pk2, &digest);

    assert!(
        session
            .call::<u64, ()>(PROPOSALS_ID, "finalize", &proposal_id, POINT_LIMIT)
            .is_err()
    );

    let status = session
        .call::<u64, Option<ProposalStatus>>(PROPOSALS_ID, "status", &proposal_id, POINT_LIMIT)
        .expect("status")
        .data
        .expect("exists");
    assert_eq!(status, ProposalStatus::Open);
}
