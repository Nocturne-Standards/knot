//! Tests for `knot-proposals` v3: epoch, caller nonce, digest consumed flag,
//! prune, rich events path, CEI finalize.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use dusk_bytes::Serializable;
use dusk_core::abi::{ContractId, Metadata};
use dusk_core::signatures::bls::{PublicKey as BlsPublicKey, SecretKey as BlsSecretKey};
use dusk_vm::{CallReceipt, ContractData, Session, VM};
use knot_encoding::events::{
    AuthorizedAccountSet, DataSet, ProposalApproved, ProposalCreated, ProposalFinalized, Pruned,
    ProposalTtlSet, RegistrySet, TombstoneSet,
};
use knot_encoding::{
    cancel_proposal_message_v1, change_account_message_v3, proposal_digest_v3,
    set_timelock_message_v1,
};
use rand::SeedableRng;
use rand::rngs::StdRng;
use rkyv::Serialize;
use rkyv::ser::Serializer;
use rkyv::ser::serializers::AllocSerializer;

#[path = "../src/call_types.rs"]
mod call_types;
use call_types::{
    ApproveArgs, CancelProposalArgs, DigestView, ProposalStatus, ProposalView, ProposeArgs,
};

#[path = "../../knot-registry/src/call_types.rs"]
mod registry_call_types;
use registry_call_types::{ChangeAccountArgs, CreateAccountArgs, SetTimelockArgs, SignatureEntry};

const PROPOSALS_BYTECODE: &[u8] =
    include_bytes!("../../../target/contract/wasm32-unknown-unknown/release/knot_proposals.wasm");
const PROPOSALS_DATA_BYTECODE: &[u8] = include_bytes!(
    "../../../target/contract/wasm32-unknown-unknown/release/knot_proposals_data.wasm"
);
const REGISTRY_BYTECODE: &[u8] =
    include_bytes!("../../../target/contract/wasm32-unknown-unknown/release/knot_registry.wasm");
const REGISTRY_DATA_BYTECODE: &[u8] = include_bytes!(
    "../../../target/contract/wasm32-unknown-unknown/release/knot_registry_data.wasm"
);
const ATLAS_BYTECODE: &[u8] =
    include_bytes!("../../../target/contract/wasm32-unknown-unknown/release/knot_mock_atlas.wasm");
const TARGET_BYTECODE: &[u8] = include_bytes!(
    "../test-target/target/contract/wasm32-unknown-unknown/release/proposals_test_target.wasm"
);

const PROPOSALS_ID: ContractId = ContractId::from_bytes([0xb2; 32]);
const PROPOSALS_ID_B: ContractId = ContractId::from_bytes([0xb4; 32]);
const PROPOSALS_DATA_ID: ContractId = ContractId::from_bytes([0xb5; 32]);
const REGISTRY_ID: ContractId = ContractId::from_bytes([0xa1; 32]);
const REGISTRY_DATA_ID: ContractId = ContractId::from_bytes([0xa2; 32]);
const ATLAS_ID: ContractId = ContractId::from_bytes([0xc1; 32]);
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

fn deploy_stack(owner_pk: &BlsPublicKey) -> Session {
    let vm = VM::ephemeral().expect("Creating ephemeral VM should work");
    let mut session = vm.genesis_session(CHAIN_ID);
    let owner = owner_pk.to_bytes().to_vec();

    for (bytecode, id) in [
        (ATLAS_BYTECODE, ATLAS_ID),
        (REGISTRY_DATA_BYTECODE, REGISTRY_DATA_ID),
        (REGISTRY_BYTECODE, REGISTRY_ID),
        (PROPOSALS_DATA_BYTECODE, PROPOSALS_DATA_ID),
        (PROPOSALS_BYTECODE, PROPOSALS_ID),
        (TARGET_BYTECODE, TARGET_ID),
    ] {
        session
            .deploy(
                bytecode,
                ContractData::builder().owner(owner.clone()).contract_id(id),
                POINT_LIMIT,
            )
            .expect("deploy");
    }

    session
        .call::<(String, ContractId), ()>(
            ATLAS_ID,
            "set_service",
            &(String::from("knot-registry"), REGISTRY_ID),
            POINT_LIMIT,
        )
        .expect("set_service registry");
    session
        .call::<(String, ContractId), ()>(
            ATLAS_ID,
            "set_service",
            &(String::from("knot-proposals"), PROPOSALS_ID),
            POINT_LIMIT,
        )
        .expect("set_service proposals");
    session
}

fn initialize(owner_pk: &BlsPublicKey) -> Session {
    let mut session = deploy_stack(owner_pk);
    set_sender(&mut session, Some(owner_pk));
    session
        .call::<ContractId, ()>(REGISTRY_ID, "init_data", &REGISTRY_DATA_ID, POINT_LIMIT)
        .expect("registry init_data");
    session
        .call::<ContractId, ()>(PROPOSALS_ID, "init_data", &PROPOSALS_DATA_ID, POINT_LIMIT)
        .expect("proposals init_data");
    set_sender(&mut session, None);
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

fn create_account(
    session: &mut Session,
    owner_pk: &BlsPublicKey,
    members: Vec<BlsPublicKey>,
    threshold: u32,
) -> u64 {
    let id = session
        .call::<CreateAccountArgs, u64>(
            REGISTRY_ID,
            "create_account",
            &CreateAccountArgs { members, threshold },
            POINT_LIMIT,
        )
        .expect("create_account should succeed")
        .data;
    set_sender(session, Some(owner_pk));
    session
        .call::<u64, ()>(PROPOSALS_ID, "set_authorized_account", &id, POINT_LIMIT)
        .expect("set_authorized_account");
    set_sender(session, None);
    id
}

fn create_unbound_account(
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
        // PreforkHostQuery: VM::ephemeral PreFork — dusk-vm-issue-1; live clients use sign()/sign_multisig() (F-001)
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

    let account_id = create_account(&mut session, &owner_pk, alloc::vec![pk1, pk2, pk3], 2);
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
    let account_id = create_account(&mut session, &owner_pk, alloc::vec![pk1, pk2], 2);

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
    let account_id = create_account(&mut session, &owner_pk, alloc::vec![pk1], 1);

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
    let account_id = create_account(&mut session, &owner_pk, alloc::vec![pk1], 1);

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
    let account_id = create_account(&mut session, &owner_pk, alloc::vec![pk1], 1);

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
    let account_id = create_account(&mut session, &owner_pk, alloc::vec![pk1], 1);

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
    let account_id = create_account(&mut session, &owner_pk, alloc::vec![pk1], 1);
    let (proposal_id, digest) = propose_set_value(&mut session, account_id, 1, 1);

    set_sender(&mut session, Some(&owner_pk));
    session
        .call::<ContractId, ()>(PROPOSALS_ID, "init_registry", &REGISTRY_ID, POINT_LIMIT)
        .expect("re-init registry bumps epoch");
    set_sender(&mut session, None);

    let approve_args = ApproveArgs {
        proposal_id,
        signer: pk1,
        // PreforkHostQuery: VM::ephemeral PreFork — dusk-vm-issue-1; live clients use sign()/sign_multisig() (F-001)
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
    let account_id = create_account(&mut session, &owner_pk, alloc::vec![pk1], 1);
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
    let account_id = create_account(&mut session, &owner_pk, alloc::vec![pk1], 1);

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
    let account_id = create_account(&mut session, &owner_pk, alloc::vec![pk1], 1);

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
    let account_id = create_account(&mut session, &owner_pk, alloc::vec![pk1, pk2], 2);

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
    let account_id = create_account(&mut session, &owner_pk, alloc::vec![pk1, pk2], 2);
    let (proposal_id, digest) = propose_set_value(&mut session, account_id, 7, 1);

    let bad = ApproveArgs {
        proposal_id,
        signer: outsider_pk,
        // PreforkHostQuery: VM::ephemeral PreFork — dusk-vm-issue-1; live clients use sign()/sign_multisig() (F-001)
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
        // PreforkHostQuery: VM::ephemeral PreFork — dusk-vm-issue-1; live clients use sign()/sign_multisig() (F-001)
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
    let account_id = create_account(&mut session, &owner_pk, alloc::vec![pk1], 1);

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
    let account_id = create_account(&mut session, &owner_pk, alloc::vec![pk1, pk2], 2);

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
    let account_id = create_account(&mut session, &owner_pk, alloc::vec![pk1, pk2], 2);

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

fn sign_all(msg: &[u8], sks: &[(&BlsSecretKey, &BlsPublicKey)]) -> Vec<SignatureEntry> {
    sks.iter()
        .map(|(sk, pk)| SignatureEntry {
            signer: **pk,
            // PreforkHostQuery: VM::ephemeral PreFork — dusk-vm-issue-1; live clients use sign()/sign_multisig() (F-001)
            signature: sk.sign_insecure(msg),
        })
        .collect()
}

fn raise_delay(
    session: &mut Session,
    account_id: u64,
    nonce: u64,
    blocks: u64,
    sks: &[(&BlsSecretKey, &BlsPublicKey)],
) {
    let msg = set_timelock_message_v1(
        u64::from(CHAIN_ID),
        &REGISTRY_ID.to_bytes(),
        account_id,
        nonce,
        blocks,
    )
    .unwrap();
    session
        .call::<SetTimelockArgs, ()>(
            REGISTRY_ID,
            "set_timelock",
            &SetTimelockArgs {
                account_id,
                blocks,
                sigs: sign_all(&msg, sks),
            },
            POINT_LIMIT,
        )
        .expect("set_timelock");
}

#[test]
fn delay_zero_finalize_still_call_raw() {
    let rng = &mut StdRng::seed_from_u64(40);
    let (_owner_sk, owner_pk) = keypair(rng);
    let (sk1, pk1) = keypair(rng);
    let (sk2, pk2) = keypair(rng);
    let mut session = initialize(&owner_pk);
    init_proposals(&mut session, &owner_pk);
    let account_id = create_account(&mut session, &owner_pk, alloc::vec![pk1, pk2], 2);
    let (proposal_id, digest) = propose_set_value(&mut session, account_id, 7, 1);
    approve(&mut session, proposal_id, &sk1, &pk1, &digest);
    approve(&mut session, proposal_id, &sk2, &pk2, &digest);
    session
        .call::<u64, ()>(PROPOSALS_ID, "finalize", &proposal_id, POINT_LIMIT)
        .expect("delay 0 finalize");
    let value = session
        .call::<(), u64>(TARGET_ID, "value", &(), POINT_LIMIT)
        .unwrap()
        .data;
    assert_eq!(value, 7);
}

#[test]
fn delay_queues_then_execute_after_eta() {
    let rng = &mut StdRng::seed_from_u64(41);
    let (_owner_sk, owner_pk) = keypair(rng);
    let (sk1, pk1) = keypair(rng);
    let (sk2, pk2) = keypair(rng);
    let mut session = initialize(&owner_pk);
    init_proposals(&mut session, &owner_pk);
    let account_id = create_account(&mut session, &owner_pk, alloc::vec![pk1, pk2], 2);
    raise_delay(
        &mut session,
        account_id,
        0,
        5,
        &[(&sk1, &pk1), (&sk2, &pk2)],
    );

    let (proposal_id, digest) = propose_set_value(&mut session, account_id, 99, 1);
    approve(&mut session, proposal_id, &sk1, &pk1, &digest);
    approve(&mut session, proposal_id, &sk2, &pk2, &digest);
    session
        .call::<u64, ()>(PROPOSALS_ID, "finalize", &proposal_id, POINT_LIMIT)
        .expect("finalize queues");
    let view = session
        .call::<u64, Option<ProposalView>>(PROPOSALS_ID, "proposal", &proposal_id, POINT_LIMIT)
        .unwrap()
        .data
        .unwrap();
    assert_eq!(view.status, ProposalStatus::Queued);
    assert_eq!(view.execute_at, 5);
    let value = session
        .call::<(), u64>(TARGET_ID, "value", &(), POINT_LIMIT)
        .unwrap()
        .data;
    assert_eq!(value, 0, "call_raw not until execute");

    assert!(
        session
            .call::<u64, ()>(PROPOSALS_ID, "execute", &proposal_id, POINT_LIMIT)
            .is_err(),
        "execute before eta must fail"
    );

    set_block_height(&mut session, 5);
    session
        .call::<u64, ()>(PROPOSALS_ID, "execute", &proposal_id, POINT_LIMIT)
        .expect("execute after eta");
    let value = session
        .call::<(), u64>(TARGET_ID, "value", &(), POINT_LIMIT)
        .unwrap()
        .data;
    assert_eq!(value, 99);
}

#[test]
fn finalize_panics_if_delay_exceeds_deadline() {
    let rng = &mut StdRng::seed_from_u64(42);
    let (_owner_sk, owner_pk) = keypair(rng);
    let (sk1, pk1) = keypair(rng);
    let (sk2, pk2) = keypair(rng);
    let mut session = initialize(&owner_pk);
    init_proposals(&mut session, &owner_pk);
    let account_id = create_account(&mut session, &owner_pk, alloc::vec![pk1, pk2], 2);
    raise_delay(
        &mut session,
        account_id,
        0,
        500,
        &[(&sk1, &pk1), (&sk2, &pk2)],
    );

    let (proposal_id, digest) = propose_fn(&mut session, account_id, "set_value", 1, 1, 10);
    approve(&mut session, proposal_id, &sk1, &pk1, &digest);
    approve(&mut session, proposal_id, &sk2, &pk2, &digest);
    assert!(
        session
            .call::<u64, ()>(PROPOSALS_ID, "finalize", &proposal_id, POINT_LIMIT)
            .is_err(),
        "now+delay > deadline must fail"
    );
}

#[test]
fn cancel_queued_is_immediate_and_digest_stays_consumed() {
    let rng = &mut StdRng::seed_from_u64(43);
    let (_owner_sk, owner_pk) = keypair(rng);
    let (sk1, pk1) = keypair(rng);
    let (sk2, pk2) = keypair(rng);
    let mut session = initialize(&owner_pk);
    init_proposals(&mut session, &owner_pk);
    let account_id = create_account(&mut session, &owner_pk, alloc::vec![pk1, pk2], 2);
    raise_delay(
        &mut session,
        account_id,
        0,
        5,
        &[(&sk1, &pk1), (&sk2, &pk2)],
    );
    let (proposal_id, digest) = propose_set_value(&mut session, account_id, 3, 1);
    approve(&mut session, proposal_id, &sk1, &pk1, &digest);
    approve(&mut session, proposal_id, &sk2, &pk2, &digest);
    session
        .call::<u64, ()>(PROPOSALS_ID, "finalize", &proposal_id, POINT_LIMIT)
        .unwrap();

    let cancel_msg = cancel_proposal_message_v1(
        u64::from(CHAIN_ID),
        &PROPOSALS_ID.to_bytes(),
        proposal_id,
        &digest,
    )
    .unwrap();
    session
        .call::<CancelProposalArgs, ()>(
            PROPOSALS_ID,
            "cancel",
            &CancelProposalArgs {
                proposal_id,
                sigs: sign_all(&cancel_msg, &[(&sk1, &pk1), (&sk2, &pk2)]),
            },
            POINT_LIMIT,
        )
        .expect("cancel immediate");
    let status = session
        .call::<u64, Option<ProposalStatus>>(PROPOSALS_ID, "status", &proposal_id, POINT_LIMIT)
        .unwrap()
        .data
        .unwrap();
    assert_eq!(status, ProposalStatus::Cancelled);

    let args = ProposeArgs {
        registry_account_id: account_id,
        target: TARGET_ID,
        function_name: String::from("set_value"),
        call_args: rkyv_bytes(&3u64),
        nonce: 1,
        deadline: deadline_at_height(0),
    };
    assert!(
        session
            .call::<ProposeArgs, u64>(PROPOSALS_ID, "propose", &args, POINT_LIMIT)
            .is_err(),
        "cancelled digest stays consumed until deadline"
    );
}

#[test]
fn prune_keeps_queued_until_deadline() {
    let rng = &mut StdRng::seed_from_u64(44);
    let (_owner_sk, owner_pk) = keypair(rng);
    let (sk1, pk1) = keypair(rng);
    let (sk2, pk2) = keypair(rng);
    let mut session = initialize(&owner_pk);
    init_proposals(&mut session, &owner_pk);
    let account_id = create_account(&mut session, &owner_pk, alloc::vec![pk1, pk2], 2);
    raise_delay(
        &mut session,
        account_id,
        0,
        5,
        &[(&sk1, &pk1), (&sk2, &pk2)],
    );
    let (proposal_id, digest) = propose_set_value(&mut session, account_id, 3, 1);
    approve(&mut session, proposal_id, &sk1, &pk1, &digest);
    approve(&mut session, proposal_id, &sk2, &pk2, &digest);
    session
        .call::<u64, ()>(PROPOSALS_ID, "finalize", &proposal_id, POINT_LIMIT)
        .unwrap();
    let pruned = session
        .call::<u32, u32>(PROPOSALS_ID, "prune", &8u32, POINT_LIMIT)
        .unwrap()
        .data;
    assert_eq!(pruned, 0, "queued must survive prune before deadline");
    let view = session
        .call::<u64, Option<ProposalView>>(PROPOSALS_ID, "proposal", &proposal_id, POINT_LIMIT)
        .unwrap()
        .data
        .unwrap();
    assert_eq!(view.status, ProposalStatus::Queued);
}

#[test]
fn owner_config_rejects_contract_forwarding() {
    let rng = &mut StdRng::seed_from_u64(50);
    let (_owner_sk, owner_pk) = keypair(rng);
    let mut session = initialize(&owner_pk);
    init_proposals(&mut session, &owner_pk);

    set_sender(&mut session, Some(&owner_pk));
    let forwarded = session.call::<(ContractId, u64), ()>(
        TARGET_ID,
        "forward_set_ttl",
        &(PROPOSALS_ID, 50u64),
        POINT_LIMIT,
    );
    assert!(forwarded.is_err(), "foreign contract must not configure");
    let ttl = session
        .call::<(), u64>(PROPOSALS_ID, "proposal_ttl", &(), POINT_LIMIT)
        .unwrap()
        .data;
    assert_eq!(ttl, DEFAULT_TTL);

    session
        .call::<u64, ()>(PROPOSALS_ID, "set_proposal_ttl", &50u64, POINT_LIMIT)
        .expect("direct owner set_proposal_ttl");
    let ttl = session
        .call::<(), u64>(PROPOSALS_ID, "proposal_ttl", &(), POINT_LIMIT)
        .unwrap()
        .data;
    assert_eq!(ttl, 50);
}

#[test]
fn unrelated_committee_cannot_drive_executor() {
    let rng = &mut StdRng::seed_from_u64(51);
    let (_owner_sk, owner_pk) = keypair(rng);
    let (sk_a, pk_a) = keypair(rng);
    let (_sk_b, pk_b) = keypair(rng);
    let mut session = initialize(&owner_pk);
    init_proposals(&mut session, &owner_pk);
    let intended = create_account(&mut session, &owner_pk, alloc::vec![pk_a], 1);
    let other = create_unbound_account(&mut session, alloc::vec![pk_b], 1);

    session
        .call::<(ContractId, u64), ()>(
            TARGET_ID,
            "configure_reenter",
            &(PROPOSALS_ID, 0),
            POINT_LIMIT,
        )
        .expect("gate target");

    let rejected = session.call::<ProposeArgs, u64>(
        PROPOSALS_ID,
        "propose",
        &ProposeArgs {
            registry_account_id: other,
            target: TARGET_ID,
            function_name: String::from("gated_set"),
            call_args: rkyv_bytes(&9u64),
            nonce: 1,
            deadline: deadline_at_height(0),
        },
        POINT_LIMIT,
    );
    assert!(rejected.is_err(), "unrelated committee must not propose");

    let (proposal_id, digest) = propose_fn(
        &mut session,
        intended,
        "gated_set",
        9,
        1,
        deadline_at_height(0),
    );
    approve(&mut session, proposal_id, &sk_a, &pk_a, &digest);
    session
        .call::<u64, ()>(PROPOSALS_ID, "finalize", &proposal_id, POINT_LIMIT)
        .expect("intended committee finalizes");
    let value = session
        .call::<(), u64>(TARGET_ID, "value", &(), POINT_LIMIT)
        .unwrap()
        .data;
    assert_eq!(value, 9);
}

#[test]
fn rebind_blocks_open_proposal_from_previous_account() {
    let rng = &mut StdRng::seed_from_u64(52);
    let (_owner_sk, owner_pk) = keypair(rng);
    let (sk_a, pk_a) = keypair(rng);
    let (_sk_b, pk_b) = keypair(rng);
    let mut session = initialize(&owner_pk);
    init_proposals(&mut session, &owner_pk);
    let intended = create_account(&mut session, &owner_pk, alloc::vec![pk_a], 1);
    let other = create_unbound_account(&mut session, alloc::vec![pk_b], 1);
    let (proposal_id, digest) = propose_set_value(&mut session, intended, 4, 1);
    approve(&mut session, proposal_id, &sk_a, &pk_a, &digest);

    set_sender(&mut session, Some(&owner_pk));
    session
        .call::<u64, ()>(PROPOSALS_ID, "set_authorized_account", &other, POINT_LIMIT)
        .expect("rebind");
    set_sender(&mut session, None);

    let finalized = session.call::<u64, ()>(PROPOSALS_ID, "finalize", &proposal_id, POINT_LIMIT);
    assert!(
        finalized.is_err(),
        "open proposal from the old account must not run"
    );
}

#[test]
fn finalize_drops_retired_member_approvals() {
    let rng = &mut StdRng::seed_from_u64(53);
    let (_owner_sk, owner_pk) = keypair(rng);
    let (sk1, pk1) = keypair(rng);
    let (sk2, pk2) = keypair(rng);
    let (sk3, pk3) = keypair(rng);
    let mut session = initialize(&owner_pk);
    init_proposals(&mut session, &owner_pk);
    let account_id = create_account(&mut session, &owner_pk, alloc::vec![pk1, pk2, pk3], 2);
    let (proposal_id, digest) = propose_set_value(&mut session, account_id, 6, 1);
    approve(&mut session, proposal_id, &sk1, &pk1, &digest);
    approve(&mut session, proposal_id, &sk2, &pk2, &digest);
    approve(&mut session, proposal_id, &sk3, &pk3, &digest);

    let new_members = alloc::vec![pk1, pk2];
    let msg = change_account_message_v3(
        u64::from(CHAIN_ID),
        &REGISTRY_ID.to_bytes(),
        account_id,
        0,
        &new_members
            .iter()
            .map(|pk| pk.to_bytes())
            .collect::<Vec<_>>(),
        2,
    )
    .unwrap();
    session
        .call::<ChangeAccountArgs, ()>(
            REGISTRY_ID,
            "change_account",
            &ChangeAccountArgs {
                account_id,
                new_members,
                new_threshold: 2,
                sigs: sign_all(&msg, &[(&sk1, &pk1), (&sk2, &pk2)]),
            },
            POINT_LIMIT,
        )
        .expect("shrink committee");

    session
        .call::<u64, ()>(PROPOSALS_ID, "finalize", &proposal_id, POINT_LIMIT)
        .expect("surviving quorum finalizes");
    let value = session
        .call::<(), u64>(TARGET_ID, "value", &(), POINT_LIMIT)
        .unwrap()
        .data;
    assert_eq!(value, 6);
}

fn read_proposal(session: &mut Session, id: u64) -> Option<ProposalView> {
    session
        .call::<u64, Option<ProposalView>>(PROPOSALS_ID, "proposal", &id, POINT_LIMIT)
        .expect("proposal query")
        .data
}

fn read_digest(session: &mut Session, key: [u8; 32]) -> Option<DigestView> {
    session
        .call::<[u8; 32], Option<DigestView>>(PROPOSALS_DATA_ID, "digest", &key, POINT_LIMIT)
        .expect("digest query")
        .data
}

fn signed_digest(account: u64, nonce: u64, value: u64, deadline: u64) -> [u8; 32] {
    proposal_digest_v3(
        u64::from(CHAIN_ID),
        &PROPOSALS_ID.to_bytes(),
        1,
        account,
        nonce,
        &TARGET_ID.to_bytes(),
        b"set_value",
        &rkyv_bytes(&value),
        deadline,
    )
    .unwrap()
}

/// Smaller digest key stays live (`deadline` 1000). Larger key expires at height 11.
fn live_before_expired_digests(account: u64) -> ((u64, u64, u64), (u64, u64, u64)) {
    for n in 1..80 {
        let live = signed_digest(account, n, n, 1000);
        let expired = signed_digest(account, n + 80, n + 80, 10);
        if live < expired {
            return ((n, n, 1000), (n + 80, n + 80, 10));
        }
    }
    panic!("no digest pair with the live key first");
}

/// Smaller digest key expires at height 31. Larger key stays live.
fn short_before_long_digests(account: u64) -> ((u64, u64, u64), (u64, u64, u64)) {
    for n in 1..80 {
        let short = signed_digest(account, n, n, 30);
        let long = signed_digest(account, n + 80, n + 80, 1000);
        if short < long {
            return ((n, n, 30), (n + 80, n + 80, 1000));
        }
    }
    panic!("no digest pair with the short key first");
}

fn hand_over(session: &mut Session, owner: &BlsPublicKey) {
    session
        .deploy(
            PROPOSALS_BYTECODE,
            ContractData::builder()
                .owner(owner.to_bytes().to_vec())
                .contract_id(PROPOSALS_ID_B),
            POINT_LIMIT,
        )
        .expect("deploy replacement logic");
    session
        .call::<(String, ContractId), ()>(
            ATLAS_ID,
            "set_service",
            &(String::from("knot-proposals"), PROPOSALS_ID_B),
            POINT_LIMIT,
        )
        .expect("retarget proposals");
    set_sender(session, Some(owner));
    session
        .call::<ContractId, ()>(PROPOSALS_ID_B, "init_data", &PROPOSALS_DATA_ID, POINT_LIMIT)
        .expect("replacement init_data");
    set_sender(session, None);
}

#[test]
fn init_data_same_book_is_idempotent_and_other_book_panics() {
    let rng = &mut StdRng::seed_from_u64(57);
    let (_owner_sk, owner_pk) = keypair(rng);
    let mut session = initialize(&owner_pk);
    set_sender(&mut session, Some(&owner_pk));
    session
        .call::<ContractId, ()>(PROPOSALS_ID, "init_data", &PROPOSALS_DATA_ID, POINT_LIMIT)
        .expect("same book may be retried");
    let other = ContractId::from_bytes([0xff; 32]);
    let rejected = session.call::<ContractId, ()>(PROPOSALS_ID, "init_data", &other, POINT_LIMIT);
    assert!(rejected.is_err(), "a second book must panic");
}

#[test]
fn rebind_round_trip_does_not_revive_open_or_queued() {
    let rng = &mut StdRng::seed_from_u64(58);
    let (_owner_sk, owner_pk) = keypair(rng);
    let (sk_a, pk_a) = keypair(rng);
    let (_sk_b, pk_b) = keypair(rng);
    let mut session = initialize(&owner_pk);
    init_proposals(&mut session, &owner_pk);
    let account = create_account(&mut session, &owner_pk, alloc::vec![pk_a], 1);
    raise_delay(&mut session, account, 0, 5, &[(&sk_a, &pk_a)]);
    let other = create_unbound_account(&mut session, alloc::vec![pk_b], 1);

    let (open_id, open_digest) = propose_set_value(&mut session, account, 3, 1);
    approve(&mut session, open_id, &sk_a, &pk_a, &open_digest);
    let (queued_id, queued_digest) = propose_set_value(&mut session, account, 4, 2);
    approve(&mut session, queued_id, &sk_a, &pk_a, &queued_digest);
    session
        .call::<u64, ()>(PROPOSALS_ID, "finalize", &queued_id, POINT_LIMIT)
        .expect("queue");
    assert_eq!(
        read_proposal(&mut session, queued_id).unwrap().status,
        ProposalStatus::Queued
    );

    set_sender(&mut session, Some(&owner_pk));
    session
        .call::<u64, ()>(PROPOSALS_ID, "set_authorized_account", &other, POINT_LIMIT)
        .expect("rebind away");
    session
        .call::<u64, ()>(
            PROPOSALS_ID,
            "set_authorized_account",
            &account,
            POINT_LIMIT,
        )
        .expect("rebind back");
    set_sender(&mut session, None);

    assert!(
        session
            .call::<u64, ()>(PROPOSALS_ID, "finalize", &open_id, POINT_LIMIT)
            .is_err(),
        "open proposal stays dead after the round trip"
    );
    set_block_height(&mut session, 5);
    assert!(
        session
            .call::<u64, ()>(PROPOSALS_ID, "execute", &queued_id, POINT_LIMIT)
            .is_err(),
        "queued proposal stays dead after the round trip"
    );
    assert_eq!(
        read_proposal(&mut session, queued_id).unwrap().status,
        ProposalStatus::Queued
    );

    let (fresh_id, fresh_digest) = propose_set_value(&mut session, account, 8, 3);
    approve(&mut session, fresh_id, &sk_a, &pk_a, &fresh_digest);
    session
        .call::<u64, ()>(PROPOSALS_ID, "finalize", &fresh_id, POINT_LIMIT)
        .expect("a new proposal after the rebind can queue");
}

#[test]
fn replacement_logic_cannot_finalize_or_execute() {
    let rng = &mut StdRng::seed_from_u64(59);
    let (_owner_sk, owner_pk) = keypair(rng);
    let (sk_a, pk_a) = keypair(rng);
    let mut session = initialize(&owner_pk);
    init_proposals(&mut session, &owner_pk);
    let account = create_account(&mut session, &owner_pk, alloc::vec![pk_a], 1);

    let (open_id, open_digest) = propose_set_value(&mut session, account, 5, 1);
    approve(&mut session, open_id, &sk_a, &pk_a, &open_digest);

    raise_delay(&mut session, account, 0, 4, &[(&sk_a, &pk_a)]);
    let (queued_id, queued_digest) = propose_set_value(&mut session, account, 6, 2);
    approve(&mut session, queued_id, &sk_a, &pk_a, &queued_digest);
    session
        .call::<u64, ()>(PROPOSALS_ID, "finalize", &queued_id, POINT_LIMIT)
        .expect("queue before handover");

    hand_over(&mut session, &owner_pk);

    assert!(
        session
            .call::<u64, ()>(PROPOSALS_ID_B, "finalize", &open_id, POINT_LIMIT)
            .is_err(),
        "replacement logic must not finalize"
    );
    set_block_height(&mut session, 4);
    assert!(
        session
            .call::<u64, ()>(PROPOSALS_ID_B, "execute", &queued_id, POINT_LIMIT)
            .is_err(),
        "replacement logic must not execute"
    );
    let value = session
        .call::<(), u64>(TARGET_ID, "value", &(), POINT_LIMIT)
        .unwrap()
        .data;
    assert_eq!(value, 0);
    assert_eq!(
        read_proposal(&mut session, queued_id).unwrap().status,
        ProposalStatus::Queued
    );
    assert_eq!(
        read_proposal(&mut session, open_id).unwrap().status,
        ProposalStatus::Open
    );
}

#[test]
fn prune_zero_removes_nothing() {
    let rng = &mut StdRng::seed_from_u64(54);
    let (_owner_sk, owner_pk) = keypair(rng);
    let (sk1, pk1) = keypair(rng);
    let mut session = initialize(&owner_pk);
    init_proposals(&mut session, &owner_pk);
    let account_id = create_account(&mut session, &owner_pk, alloc::vec![pk1], 1);
    let (proposal_id, digest) = propose_set_value(&mut session, account_id, 1, 1);
    approve(&mut session, proposal_id, &sk1, &pk1, &digest);
    session
        .call::<u64, ()>(PROPOSALS_ID, "finalize", &proposal_id, POINT_LIMIT)
        .unwrap();

    set_block_height(&mut session, 1001);
    let pruned = session
        .call::<u32, u32>(PROPOSALS_ID, "prune", &0u32, POINT_LIMIT)
        .unwrap()
        .data;
    assert_eq!(pruned, 0);
    assert!(
        read_proposal(&mut session, proposal_id).is_some(),
        "expired proposal stays"
    );
    let rec = read_digest(&mut session, digest).expect("expired digest stays");
    assert!(rec.consumed);
}

#[test]
fn prune_examines_a_bounded_prefix() {
    let rng = &mut StdRng::seed_from_u64(55);
    let (_owner_sk, owner_pk) = keypair(rng);
    let (sk1, pk1) = keypair(rng);
    let mut session = initialize(&owner_pk);
    init_proposals(&mut session, &owner_pk);
    set_block_height(&mut session, 0);
    let account_id = create_account(&mut session, &owner_pk, alloc::vec![pk1], 1);
    let (live_id, _) = propose_fn(&mut session, account_id, "set_value", 1, 1, 1000);
    let (expired_id, _) = propose_fn(&mut session, account_id, "set_value", 2, 2, 10);
    set_block_height(&mut session, 11);

    let first = session
        .call::<u32, u32>(PROPOSALS_ID, "prune", &1u32, POINT_LIMIT)
        .unwrap()
        .data;
    assert_eq!(first, 0, "the live prefix is examined and kept");
    assert!(
        read_proposal(&mut session, expired_id).is_some(),
        "expired proposal is past the budget"
    );

    let second = session
        .call::<u32, u32>(PROPOSALS_ID, "prune", &1u32, POINT_LIMIT)
        .unwrap()
        .data;
    assert_eq!(second, 1, "the next call reaches the expired proposal");
    assert!(read_proposal(&mut session, expired_id).is_none());
    assert!(read_proposal(&mut session, live_id).is_some());
    let _ = sk1;
}

#[test]
fn prune_digest_examines_a_bounded_prefix() {
    let rng = &mut StdRng::seed_from_u64(60);
    let (_owner_sk, owner_pk) = keypair(rng);
    let (_sk1, pk1) = keypair(rng);
    let mut session = initialize(&owner_pk);
    init_proposals(&mut session, &owner_pk);
    set_block_height(&mut session, 0);
    let account_id = create_account(&mut session, &owner_pk, alloc::vec![pk1], 1);
    let ((n0, v0, d0), (n1, v1, d1)) = live_before_expired_digests(account_id);
    let (_, live_key) = propose_fn(&mut session, account_id, "set_value", v0, n0, d0);
    let (_, expired_key) = propose_fn(&mut session, account_id, "set_value", v1, n1, d1);
    assert!(live_key < expired_key);
    set_block_height(&mut session, 11);

    session
        .call::<u32, u32>(PROPOSALS_ID, "prune", &1u32, POINT_LIMIT)
        .unwrap();
    assert!(read_digest(&mut session, expired_key).is_some());
    assert!(read_digest(&mut session, live_key).is_some());

    session
        .call::<u32, u32>(PROPOSALS_ID, "prune", &1u32, POINT_LIMIT)
        .unwrap();
    assert!(read_digest(&mut session, expired_key).is_none());
    assert!(read_digest(&mut session, live_key).is_some());
}

#[test]
fn prune_cursor_wraps_on_both_maps() {
    let rng = &mut StdRng::seed_from_u64(61);
    let (_owner_sk, owner_pk) = keypair(rng);
    let (_sk1, pk1) = keypair(rng);
    let mut session = initialize(&owner_pk);
    init_proposals(&mut session, &owner_pk);
    set_block_height(&mut session, 0);
    let account_id = create_account(&mut session, &owner_pk, alloc::vec![pk1], 1);

    let (first_id, _) = propose_fn(&mut session, account_id, "set_value", 1, 1, 20);
    let (second_id, _) = propose_fn(&mut session, account_id, "set_value", 2, 2, 1000);
    session
        .call::<u32, u32>(PROPOSALS_ID, "prune", &1u32, POINT_LIMIT)
        .unwrap();
    session
        .call::<u32, u32>(PROPOSALS_ID, "prune", &1u32, POINT_LIMIT)
        .unwrap();
    set_block_height(&mut session, 21);
    let removed = session
        .call::<u32, u32>(PROPOSALS_ID, "prune", &1u32, POINT_LIMIT)
        .unwrap()
        .data;
    assert_eq!(removed, 1, "wrap examines the first proposal");
    assert!(read_proposal(&mut session, first_id).is_none());
    assert!(read_proposal(&mut session, second_id).is_some());

    let mut session = initialize(&owner_pk);
    init_proposals(&mut session, &owner_pk);
    set_block_height(&mut session, 0);
    let account_id = create_account(&mut session, &owner_pk, alloc::vec![pk1], 1);
    let ((n0, v0, d0), (n1, v1, d1)) = short_before_long_digests(account_id);
    let (_, short_key) = propose_fn(&mut session, account_id, "set_value", v0, n0, d0);
    let (_, long_key) = propose_fn(&mut session, account_id, "set_value", v1, n1, d1);
    assert!(short_key < long_key);
    session
        .call::<u32, u32>(PROPOSALS_ID, "prune", &1u32, POINT_LIMIT)
        .unwrap();
    session
        .call::<u32, u32>(PROPOSALS_ID, "prune", &1u32, POINT_LIMIT)
        .unwrap();
    set_block_height(&mut session, 31);
    session
        .call::<u32, u32>(PROPOSALS_ID, "prune", &1u32, POINT_LIMIT)
        .unwrap();
    assert!(
        read_digest(&mut session, short_key).is_none(),
        "wrap removes the expired first digest"
    );
    assert!(read_digest(&mut session, long_key).is_some());
}

#[test]
fn data_rejects_propose_when_atlas_points_elsewhere() {
    let rng = &mut StdRng::seed_from_u64(56);
    let (_owner_sk, owner_pk) = keypair(rng);
    let (_sk, pk) = keypair(rng);
    let mut session = initialize(&owner_pk);
    init_proposals(&mut session, &owner_pk);
    let account_id = create_account(&mut session, &owner_pk, alloc::vec![pk], 1);
    session
        .call::<(String, ContractId), ()>(
            ATLAS_ID,
            "set_service",
            &(String::from("knot-proposals"), PROPOSALS_DATA_ID),
            POINT_LIMIT,
        )
        .expect("retarget service");
    let rejected = session.call::<ProposeArgs, u64>(
        PROPOSALS_ID,
        "propose",
        &ProposeArgs {
            registry_account_id: account_id,
            target: TARGET_ID,
            function_name: String::from("set_value"),
            call_args: rkyv_bytes(&1u64),
            nonce: 1,
            deadline: deadline_at_height(0),
        },
        POINT_LIMIT,
    );
    assert!(
        rejected.is_err(),
        "data must reject a caller Atlas does not name"
    );
}

fn archived<T>(value: &T) -> Vec<u8>
where
    T: Serialize<AllocSerializer<4096>>,
{
    rkyv::to_bytes::<_, 4096>(value)
        .expect("archive")
        .into_vec()
}

fn proposals_event<'a, T>(receipt: &'a CallReceipt<T>, topic: &str) -> &'a [u8] {
    let hits: Vec<_> = receipt
        .events
        .iter()
        .filter(|event| event.source == PROPOSALS_ID && event.topic == topic)
        .collect();
    assert_eq!(hits.len(), 1, "{topic}");
    hits[0].data.as_slice()
}

#[test]
fn events_carry_the_written_value() {
    let rng = &mut StdRng::seed_from_u64(80);
    let (_owner_sk, owner_pk) = keypair(rng);
    let (sk1, pk1) = keypair(rng);
    let mut session = deploy_stack(&owner_pk);
    set_sender(&mut session, Some(&owner_pk));
    session
        .call::<ContractId, ()>(REGISTRY_ID, "init_data", &REGISTRY_DATA_ID, POINT_LIMIT)
        .expect("registry init_data");
    let init = session
        .call::<ContractId, ()>(PROPOSALS_ID, "init_data", &PROPOSALS_DATA_ID, POINT_LIMIT)
        .expect("proposals init_data");
    assert_eq!(
        proposals_event(&init, "data_set"),
        archived(&DataSet {
            data: PROPOSALS_DATA_ID,
        })
        .as_slice()
    );
    let same = session
        .call::<ContractId, ()>(PROPOSALS_ID, "init_data", &PROPOSALS_DATA_ID, POINT_LIMIT)
        .expect("same book");
    assert!(same.events.iter().all(|event| event.source != PROPOSALS_ID));

    let bound = session
        .call::<ContractId, ()>(PROPOSALS_ID, "init_registry", &REGISTRY_ID, POINT_LIMIT)
        .unwrap();
    assert_eq!(
        proposals_event(&bound, "registry_set"),
        archived(&RegistrySet {
            registry: REGISTRY_ID,
            epoch: 1,
        })
        .as_slice()
    );
    let tomb = session
        .call::<bool, ()>(PROPOSALS_ID, "set_tombstone", &true, POINT_LIMIT)
        .unwrap();
    assert_eq!(
        proposals_event(&tomb, "tombstone_set"),
        archived(&TombstoneSet { tombstone: true }).as_slice()
    );
    let ttl = session
        .call::<u64, ()>(PROPOSALS_ID, "set_proposal_ttl", &2000u64, POINT_LIMIT)
        .unwrap();
    assert_eq!(
        proposals_event(&ttl, "proposal_ttl_set"),
        archived(&ProposalTtlSet { blocks: 2000 }).as_slice()
    );

    let account_id = session
        .call::<CreateAccountArgs, u64>(
            REGISTRY_ID,
            "create_account",
            &CreateAccountArgs {
                members: alloc::vec![pk1],
                threshold: 1,
            },
            POINT_LIMIT,
        )
        .unwrap()
        .data;
    let authorized = session
        .call::<u64, ()>(
            PROPOSALS_ID,
            "set_authorized_account",
            &account_id,
            POINT_LIMIT,
        )
        .unwrap();
    assert_eq!(
        proposals_event(&authorized, "authorized_account_set"),
        archived(&AuthorizedAccountSet {
            account_id,
            auth_generation: 1,
        })
        .as_slice()
    );
    set_sender(&mut session, None);

    let value = 5u64;
    let call_args = rkyv_bytes(&value);
    let deadline = deadline_at_height(0);
    let args = ProposeArgs {
        registry_account_id: account_id,
        target: TARGET_ID,
        function_name: String::from("set_value"),
        call_args: call_args.clone(),
        nonce: 3,
        deadline,
    };
    let created = session
        .call::<ProposeArgs, u64>(PROPOSALS_ID, "propose", &args, POINT_LIMIT)
        .unwrap();
    let proposal_id = created.data;
    let digest = session
        .call::<u64, Option<ProposalView>>(PROPOSALS_ID, "proposal", &proposal_id, POINT_LIMIT)
        .unwrap()
        .data
        .unwrap()
        .signed_digest;
    assert_eq!(
        proposals_event(&created, "proposal_created"),
        archived(&ProposalCreated {
            proposal_id,
            signed_digest: digest,
            registry_account_id: account_id,
            deadline,
            epoch: 1,
            nonce: 3,
            auth_generation: 1,
            target: TARGET_ID,
            function_name: String::from("set_value"),
            call_args: call_args.clone(),
        })
        .as_slice()
    );

    // PreforkHostQuery: VM::ephemeral PreFork — dusk-vm-issue-1; live clients use sign()/sign_multisig() (F-001)
    let signature = sk1.sign_insecure(&digest);
    let approved = session
        .call::<ApproveArgs, ()>(
            PROPOSALS_ID,
            "approve",
            &ApproveArgs {
                proposal_id,
                signer: pk1,
                signature,
            },
            POINT_LIMIT,
        )
        .unwrap();
    assert_eq!(
        proposals_event(&approved, "proposal_approved"),
        archived(&ProposalApproved {
            proposal_id,
            signed_digest: digest,
            signer: pk1,
            signature,
        })
        .as_slice()
    );

    let finalized = session
        .call::<u64, ()>(PROPOSALS_ID, "finalize", &proposal_id, POINT_LIMIT)
        .unwrap();
    assert_eq!(
        proposals_event(&finalized, "proposal_finalized"),
        archived(&ProposalFinalized {
            proposal_id,
            signed_digest: digest,
            registry_account_id: account_id,
            target: TARGET_ID,
            function_name: String::from("set_value"),
            call_args,
        })
        .as_slice()
    );

    let pruned = session
        .call::<u32, u32>(PROPOSALS_ID, "prune", &128u32, POINT_LIMIT)
        .unwrap();
    assert_eq!(pruned.data, 1);
    assert_eq!(
        proposals_event(&pruned, "pruned"),
        archived(&Pruned {
            proposal_ids: alloc::vec![proposal_id],
            digest_keys: Vec::new(),
        })
        .as_slice()
    );

    set_block_height(&mut session, deadline + 1);
    let digests = session
        .call::<u32, u32>(PROPOSALS_ID, "prune", &128u32, POINT_LIMIT)
        .unwrap();
    assert_eq!(digests.data, 0);
    assert_eq!(
        proposals_event(&digests, "pruned"),
        archived(&Pruned {
            proposal_ids: Vec::new(),
            digest_keys: alloc::vec![digest],
        })
        .as_slice()
    );
}
