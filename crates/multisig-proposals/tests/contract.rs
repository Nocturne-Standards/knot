//! Tests for `multisig-proposals` v0.3: structured propose, digest signing,
//! CEI finalize via `call_raw`, per-committee nonce, caps, tombstone bool.

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
    // Default tombstone=false → Executed; keep explicit for lab clarity.
    session
        .call::<bool, ()>(PROPOSALS_ID, "set_tombstone", &false, POINT_LIMIT)
        .expect("set_tombstone");
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
    propose_fn(session, account_id, "set_value", value)
}

fn propose_fn(
    session: &mut Session,
    account_id: u64,
    function_name: &str,
    value: u64,
) -> (u64, [u8; 32]) {
    let args = ProposeArgs {
        registry_account_id: account_id,
        target: TARGET_ID,
        function_name: String::from(function_name),
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
fn propose_fails_before_init_chain_id() {
    let rng = &mut StdRng::seed_from_u64(16);
    let (_owner_sk, owner_pk) = keypair(rng);
    let mut session = initialize(&owner_pk);

    set_sender(&mut session, Some(&owner_pk));
    session
        .call::<ContractId, ()>(PROPOSALS_ID, "init_registry", &REGISTRY_ID, POINT_LIMIT)
        .expect("init_registry");
    set_sender(&mut session, None);

    let args = ProposeArgs {
        registry_account_id: 0,
        target: TARGET_ID,
        function_name: String::from("set_value"),
        call_args: rkyv_bytes(&1u64),
        deadline: 0,
    };
    let result = session.call::<ProposeArgs, u64>(PROPOSALS_ID, "propose", &args, POINT_LIMIT);
    assert!(result.is_err(), "propose before init_chain_id must fail");
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

#[test]
fn finalize_reentrancy_runs_target_once() {
    let rng = &mut StdRng::seed_from_u64(9);
    let (_owner_sk, owner_pk) = keypair(rng);
    let (sk1, pk1) = keypair(rng);
    let (sk2, pk2) = keypair(rng);

    let mut session = initialize(&owner_pk);
    init_proposals(&mut session, &owner_pk);
    let account_id = create_account(&mut session, alloc::vec![pk1, pk2], 2);

    let (proposal_id, digest) =
        propose_fn(&mut session, account_id, "set_value_reenter_finalize", 77);

    // Wire target to call back into finalize(same id) during execute.
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

    let view = session
        .call::<u64, Option<ProposalView>>(PROPOSALS_ID, "proposal", &proposal_id, POINT_LIMIT)
        .expect("proposal query")
        .data
        .expect("exists");
    assert_eq!(view.status, ProposalStatus::Executed);

    let hits = session
        .call::<(), u64>(TARGET_ID, "hit_count", &(), POINT_LIMIT)
        .expect("hit_count")
        .data;
    assert_eq!(hits, 1, "reentrant finalize must not double-execute target");

    let target_value = session
        .call::<(), u64>(TARGET_ID, "value", &(), POINT_LIMIT)
        .expect("value")
        .data;
    assert_eq!(target_value, 77);

    let nonce = session
        .call::<u64, u64>(PROPOSALS_ID, "committee_nonce", &account_id, POINT_LIMIT)
        .expect("nonce")
        .data;
    assert_eq!(nonce, 1, "committee nonce must bump exactly once");
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

    let (proposal_id, digest) = propose_fn(&mut session, account_id, "fail_set", 1);
    approve(&mut session, proposal_id, &sk1, &pk1, &digest);
    approve(&mut session, proposal_id, &sk2, &pk2, &digest);

    assert!(
        session
            .call::<u64, ()>(PROPOSALS_ID, "finalize", &proposal_id, POINT_LIMIT)
            .is_err(),
        "finalize must fail when call_raw target panics"
    );

    let status = session
        .call::<u64, Option<ProposalStatus>>(PROPOSALS_ID, "status", &proposal_id, POINT_LIMIT)
        .expect("status")
        .data
        .expect("exists");
    assert_eq!(status, ProposalStatus::Open);

    let nonce = session
        .call::<u64, u64>(PROPOSALS_ID, "committee_nonce", &account_id, POINT_LIMIT)
        .expect("nonce")
        .data;
    assert_eq!(nonce, 0, "failed finalize must not bump nonce");
}

#[test]
fn set_tombstone_true_marks_tombstoned() {
    let rng = &mut StdRng::seed_from_u64(11);
    let (_owner_sk, owner_pk) = keypair(rng);
    let (sk1, pk1) = keypair(rng);

    let mut session = initialize(&owner_pk);
    init_proposals(&mut session, &owner_pk);
    set_sender(&mut session, Some(&owner_pk));
    session
        .call::<bool, ()>(PROPOSALS_ID, "set_tombstone", &true, POINT_LIMIT)
        .expect("set_tombstone(true)");
    set_sender(&mut session, None);

    let account_id = create_account(&mut session, alloc::vec![pk1], 1);
    let (proposal_id, digest) = propose_set_value(&mut session, account_id, 5);
    approve(&mut session, proposal_id, &sk1, &pk1, &digest);
    session
        .call::<u64, ()>(PROPOSALS_ID, "finalize", &proposal_id, POINT_LIMIT)
        .expect("finalize");

    let status = session
        .call::<u64, Option<ProposalStatus>>(PROPOSALS_ID, "status", &proposal_id, POINT_LIMIT)
        .expect("status")
        .data
        .expect("exists");
    assert_eq!(status, ProposalStatus::Tombstoned);
}

#[test]
fn propose_rejects_oversized_function_name() {
    let rng = &mut StdRng::seed_from_u64(12);
    let (_owner_sk, owner_pk) = keypair(rng);
    let (_sk1, pk1) = keypair(rng);

    let mut session = initialize(&owner_pk);
    init_proposals(&mut session, &owner_pk);
    let account_id = create_account(&mut session, alloc::vec![pk1], 1);

    let name: String = (0..65).map(|_| 'a').collect();
    let args = ProposeArgs {
        registry_account_id: account_id,
        target: TARGET_ID,
        function_name: name,
        call_args: rkyv_bytes(&1u64),
        deadline: 0,
    };
    assert!(
        session
            .call::<ProposeArgs, u64>(PROPOSALS_ID, "propose", &args, POINT_LIMIT)
            .is_err(),
        "function_name len 65 must fail"
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
        deadline: 100, // == block_height → reject
    };
    assert!(
        session
            .call::<ProposeArgs, u64>(PROPOSALS_ID, "propose", &args, POINT_LIMIT)
            .is_err(),
        "deadline == block_height must fail at propose"
    );
}

#[test]
fn init_chain_id_wipes_open_proposals() {
    let rng = &mut StdRng::seed_from_u64(14);
    let (_owner_sk, owner_pk) = keypair(rng);
    let (_sk1, pk1) = keypair(rng);

    let mut session = initialize(&owner_pk);
    init_proposals(&mut session, &owner_pk);
    let account_id = create_account(&mut session, alloc::vec![pk1], 1);
    let (proposal_id, _) = propose_set_value(&mut session, account_id, 3);

    set_sender(&mut session, Some(&owner_pk));
    session
        .call::<u64, ()>(PROPOSALS_ID, "init_chain_id", &99u64, POINT_LIMIT)
        .expect("init_chain_id wipe");
    set_sender(&mut session, None);

    let status = session
        .call::<u64, Option<ProposalStatus>>(PROPOSALS_ID, "status", &proposal_id, POINT_LIMIT)
        .expect("status")
        .data
        .expect("exists");
    assert_eq!(status, ProposalStatus::Tombstoned);

    // Same payload can be re-proposed under the new chain_id (digest domain changed;
    // wiped entry removed from by_digest).
    let (id2, _) = propose_set_value(&mut session, account_id, 3);
    assert_ne!(id2, proposal_id);
    let status2 = session
        .call::<u64, Option<ProposalStatus>>(PROPOSALS_ID, "status", &id2, POINT_LIMIT)
        .expect("status")
        .data
        .expect("exists");
    assert_eq!(status2, ProposalStatus::Open);
}

#[test]
fn propose_rejects_oversized_call_args() {
    let rng = &mut StdRng::seed_from_u64(15);
    let (_owner_sk, owner_pk) = keypair(rng);
    let (_sk1, pk1) = keypair(rng);

    let mut session = initialize(&owner_pk);
    init_proposals(&mut session, &owner_pk);
    let account_id = create_account(&mut session, alloc::vec![pk1], 1);

    let args = ProposeArgs {
        registry_account_id: account_id,
        target: TARGET_ID,
        function_name: String::from("set_value"),
        call_args: alloc::vec![0u8; 4097],
        deadline: 0,
    };
    assert!(
        session
            .call::<ProposeArgs, u64>(PROPOSALS_ID, "propose", &args, POINT_LIMIT)
            .is_err(),
        "call_args len 4097 must fail"
    );
}
