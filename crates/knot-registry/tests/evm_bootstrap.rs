//! Tests for the EVM root bootstrap sender (D9): owner-only messenger /
//! receiver config, and `publish_bootstrap_root` sending the account's
//! current member set to `KnotEvmRoot.bootstrap` through the L1 messenger
//! only when the account's quorum signed the bootstrap message.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use dusk_bytes::Serializable;
use dusk_core::abi::{ContractId, Metadata};
use dusk_core::signatures::bls::{PublicKey as BlsPublicKey, SecretKey as BlsSecretKey};
use dusk_vm::{ContractData, Session, VM};
use knot_encoding::{bootstrap_min_gas, encode_bootstrap_calldata, evm_bootstrap_message_v1};
use rand::SeedableRng;
use rand::rngs::StdRng;

#[path = "../src/call_types.rs"]
mod call_types;
use call_types::{ChangeAccountArgs, CreateAccountArgs, PublishBootstrapRootArgs, SignatureEntry};

const REGISTRY_BYTECODE: &[u8] =
    include_bytes!("../../../target/contract/wasm32-unknown-unknown/release/knot_registry.wasm");
const REGISTRY_DATA_BYTECODE: &[u8] = include_bytes!(
    "../../../target/contract/wasm32-unknown-unknown/release/knot_registry_data.wasm"
);
const ATLAS_BYTECODE: &[u8] =
    include_bytes!("../../../target/contract/wasm32-unknown-unknown/release/knot_mock_atlas.wasm");
/// `sendMessage` recorder standing in for the L1 messenger.
const MESSENGER_BYTECODE: &[u8] = include_bytes!(
    "../../knot-proposals/test-target/target/contract/wasm32-unknown-unknown/release/proposals_test_target.wasm"
);

const REGISTRY_ID: ContractId = ContractId::from_bytes([0xa1; 32]);
const REGISTRY_DATA_ID: ContractId = ContractId::from_bytes([0xa2; 32]);
const ATLAS_ID: ContractId = ContractId::from_bytes([0xc1; 32]);
const MESSENGER_ID: ContractId = ContractId::from_bytes([0x63; 32]);
const RECEIVER: [u8; 20] = [0x86; 20];
const CHAIN_ID: u8 = 0xCA;
const POINT_LIMIT: u64 = 0x10000000;

type Sent = (ContractId, [u8; 20], Vec<u8>, u32);

struct Fixture {
    session: Session,
    owner: BlsPublicKey,
    sks: Vec<BlsSecretKey>,
    pks: Vec<BlsPublicKey>,
    account_id: u64,
}

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

/// Registry owned by `owner`, messenger stand-in deployed, one 2-of-3 account.
fn fixture(seed: u64) -> Fixture {
    let rng = &mut StdRng::seed_from_u64(seed);
    let (_osk, owner) = keypair(rng);
    let mut sks = Vec::new();
    let mut pks = Vec::new();
    for _ in 0..3 {
        let (sk, pk) = keypair(rng);
        sks.push(sk);
        pks.push(pk);
    }

    let vm = VM::ephemeral().expect("Creating ephemeral VM should work");
    let mut session = vm.genesis_session(CHAIN_ID);
    for (bytecode, id) in [
        (ATLAS_BYTECODE, ATLAS_ID),
        (REGISTRY_DATA_BYTECODE, REGISTRY_DATA_ID),
        (REGISTRY_BYTECODE, REGISTRY_ID),
        (MESSENGER_BYTECODE, MESSENGER_ID),
    ] {
        session
            .deploy(
                bytecode,
                ContractData::builder()
                    .owner(owner.to_bytes().to_vec())
                    .contract_id(id),
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
        .expect("set_service");
    set_sender(&mut session, Some(&owner));
    session
        .call::<ContractId, ()>(REGISTRY_ID, "init_data", &REGISTRY_DATA_ID, POINT_LIMIT)
        .expect("init_data");
    set_sender(&mut session, None);

    let account_id = session
        .call::<CreateAccountArgs, u64>(
            REGISTRY_ID,
            "create_account",
            &CreateAccountArgs {
                members: pks.clone(),
                threshold: 2,
            },
            POINT_LIMIT,
        )
        .expect("create_account")
        .data;

    Fixture {
        session,
        owner,
        sks,
        pks,
        account_id,
    }
}

fn configure(f: &mut Fixture) {
    let owner = f.owner;
    set_sender(&mut f.session, Some(&owner));
    f.session
        .call::<ContractId, ()>(
            REGISTRY_ID,
            "init_l1_messenger_contract",
            &MESSENGER_ID,
            POINT_LIMIT,
        )
        .expect("owner sets messenger");
    f.session
        .call::<[u8; 20], ()>(
            REGISTRY_ID,
            "init_evm_root_receiver",
            &RECEIVER,
            POINT_LIMIT,
        )
        .expect("owner sets receiver");
    set_sender(&mut f.session, None);
}

fn member_bytes(pks: &[BlsPublicKey]) -> Vec<[u8; 96]> {
    pks.iter().map(|pk| pk.to_bytes()).collect()
}

fn bootstrap_msg(f: &Fixture, nonce: u64, receiver: &[u8; 20]) -> Vec<u8> {
    evm_bootstrap_message_v1(
        u64::from(CHAIN_ID),
        &REGISTRY_ID.to_bytes(),
        f.account_id,
        nonce,
        receiver,
        &member_bytes(&f.pks),
        2,
    )
    .expect("within caps")
}

fn sign(msg: &[u8], signers: &[(&BlsSecretKey, &BlsPublicKey)]) -> Vec<SignatureEntry> {
    signers
        .iter()
        .map(|(sk, pk)| SignatureEntry {
            signer: **pk,
            // PreforkHostQuery: VM::ephemeral PreFork — dusk-vm-issue-1; live clients use sign()/sign_multisig() (F-001)
            signature: sk.sign_insecure(msg),
        })
        .collect()
}

fn publish(f: &mut Fixture, sigs: Vec<SignatureEntry>) -> Result<(), dusk_vm::Error> {
    let args = PublishBootstrapRootArgs {
        account_id: f.account_id,
        sigs,
    };
    f.session
        .call::<PublishBootstrapRootArgs, ()>(
            REGISTRY_ID,
            "publish_bootstrap_root",
            &args,
            POINT_LIMIT,
        )
        .map(|_| ())
}

fn sent(f: &mut Fixture) -> Vec<Sent> {
    f.session
        .call::<(), Vec<Sent>>(MESSENGER_ID, "messages", &(), POINT_LIMIT)
        .expect("messages view")
        .data
}

#[test]
fn init_rejects_non_owner() {
    let mut f = fixture(0xb001);
    let rng = &mut StdRng::seed_from_u64(0xbad);
    let (_ask, attacker) = keypair(rng);
    for sender in [Some(&attacker), None] {
        set_sender(&mut f.session, sender);
        assert!(
            f.session
                .call::<ContractId, ()>(
                    REGISTRY_ID,
                    "init_l1_messenger_contract",
                    &MESSENGER_ID,
                    POINT_LIMIT
                )
                .is_err(),
            "init_l1_messenger_contract must be owner-only"
        );
        assert!(
            f.session
                .call::<[u8; 20], ()>(
                    REGISTRY_ID,
                    "init_evm_root_receiver",
                    &RECEIVER,
                    POINT_LIMIT
                )
                .is_err(),
            "init_evm_root_receiver must be owner-only"
        );
    }
}

#[test]
fn init_evm_root_receiver_rejects_zero() {
    let mut f = fixture(0xb002);
    let owner = f.owner;
    set_sender(&mut f.session, Some(&owner));
    assert!(
        f.session
            .call::<[u8; 20], ()>(
                REGISTRY_ID,
                "init_evm_root_receiver",
                &[0u8; 20],
                POINT_LIMIT
            )
            .is_err()
    );
}

#[test]
fn publish_requires_configuration() {
    let mut f = fixture(0xb003);
    let msg = bootstrap_msg(&f, 0, &RECEIVER);
    let sigs = sign(&msg, &[(&f.sks[0], &f.pks[0]), (&f.sks[1], &f.pks[1])]);
    assert!(
        publish(&mut f, sigs).is_err(),
        "unconfigured publish must fail"
    );
    assert!(sent(&mut f).is_empty());
}

#[test]
fn publish_with_quorum_sends_bootstrap_calldata() {
    let mut f = fixture(0xb004);
    configure(&mut f);
    let msg = bootstrap_msg(&f, 0, &RECEIVER);
    let sigs = sign(&msg, &[(&f.sks[0], &f.pks[0]), (&f.sks[2], &f.pks[2])]);
    publish(&mut f, sigs).expect("quorum publish succeeds");

    let msgs = sent(&mut f);
    assert_eq!(msgs.len(), 1);
    let (caller, target, payload, min_gas) = &msgs[0];
    assert_eq!(
        *caller, REGISTRY_ID,
        "messenger sees knot-registry as sender"
    );
    assert_eq!(*target, RECEIVER);
    assert_eq!(
        *payload,
        encode_bootstrap_calldata(f.account_id, u64::from(CHAIN_ID), &member_bytes(&f.pks), 2)
    );
    assert_eq!(*min_gas, bootstrap_min_gas(3));
}

#[test]
fn publish_below_threshold_refused() {
    let mut f = fixture(0xb005);
    configure(&mut f);
    let msg = bootstrap_msg(&f, 0, &RECEIVER);
    let one = sign(&msg, &[(&f.sks[0], &f.pks[0])]);
    assert!(publish(&mut f, one).is_err());
    // one member twice still counts once
    let twice = sign(&msg, &[(&f.sks[0], &f.pks[0]), (&f.sks[0], &f.pks[0])]);
    assert!(publish(&mut f, twice).is_err());
    assert!(sent(&mut f).is_empty());
}

#[test]
fn publish_refuses_signatures_over_other_messages() {
    let mut f = fixture(0xb006);
    configure(&mut f);
    for msg in [
        bootstrap_msg(&f, 1, &RECEIVER),   // wrong account nonce
        bootstrap_msg(&f, 0, &[0x87; 20]), // other receiver
        alloc::vec![0x42u8; 32],           // unrelated message
    ] {
        let sigs = sign(&msg, &[(&f.sks[0], &f.pks[0]), (&f.sks[1], &f.pks[1])]);
        assert!(publish(&mut f, sigs).is_err());
    }
    assert!(sent(&mut f).is_empty());
}

#[test]
fn publish_refuses_non_member_signers() {
    let mut f = fixture(0xb007);
    configure(&mut f);
    let rng = &mut StdRng::seed_from_u64(0x07);
    let (osk, opk) = keypair(rng);
    let msg = bootstrap_msg(&f, 0, &RECEIVER);
    let sigs = sign(&msg, &[(&f.sks[0], &f.pks[0]), (&osk, &opk)]);
    assert!(publish(&mut f, sigs).is_err());
}

#[test]
fn publish_unknown_account_refused() {
    let mut f = fixture(0xb008);
    configure(&mut f);
    let msg = bootstrap_msg(&f, 0, &RECEIVER);
    let sigs = sign(&msg, &[(&f.sks[0], &f.pks[0]), (&f.sks[1], &f.pks[1])]);
    let args = PublishBootstrapRootArgs {
        account_id: f.account_id + 1,
        sigs,
    };
    assert!(
        f.session
            .call::<PublishBootstrapRootArgs, ()>(
                REGISTRY_ID,
                "publish_bootstrap_root",
                &args,
                POINT_LIMIT
            )
            .is_err()
    );
}

#[test]
fn publish_after_change_account_sends_new_set() {
    let mut f = fixture(0xb009);
    configure(&mut f);
    // rotate on DuskDS first: members [pk0, pk1], threshold 1, nonce 0 -> 1
    let new_members = alloc::vec![f.pks[0], f.pks[1]];
    let change_msg = knot_encoding::change_account_message_v3(
        u64::from(CHAIN_ID),
        &REGISTRY_ID.to_bytes(),
        f.account_id,
        0,
        &member_bytes(&new_members),
        1,
    )
    .unwrap();
    let sigs = sign(
        &change_msg,
        &[(&f.sks[0], &f.pks[0]), (&f.sks[1], &f.pks[1])],
    );
    f.session
        .call::<ChangeAccountArgs, ()>(
            REGISTRY_ID,
            "change_account",
            &ChangeAccountArgs {
                account_id: f.account_id,
                new_members: new_members.clone(),
                new_threshold: 1,
                sigs,
            },
            POINT_LIMIT,
        )
        .expect("change_account");

    // a quorum gathered for the old nonce no longer works
    let stale = bootstrap_msg(&f, 0, &RECEIVER);
    let sigs = sign(&stale, &[(&f.sks[0], &f.pks[0]), (&f.sks[1], &f.pks[1])]);
    assert!(publish(&mut f, sigs).is_err());

    let msg = evm_bootstrap_message_v1(
        u64::from(CHAIN_ID),
        &REGISTRY_ID.to_bytes(),
        f.account_id,
        1,
        &RECEIVER,
        &member_bytes(&new_members),
        1,
    )
    .unwrap();
    let sigs = sign(&msg, &[(&f.sks[1], &f.pks[1])]);
    publish(&mut f, sigs).expect("new quorum publishes");
    let msgs = sent(&mut f);
    assert_eq!(msgs.len(), 1);
    assert_eq!(
        msgs[0].2,
        encode_bootstrap_calldata(
            f.account_id,
            u64::from(CHAIN_ID),
            &member_bytes(&new_members),
            1
        )
    );
}

#[test]
fn init_is_once() {
    let mut f = fixture(0xb00a);
    configure(&mut f);
    let owner = f.owner;
    set_sender(&mut f.session, Some(&owner));
    assert!(
        f.session
            .call::<ContractId, ()>(
                REGISTRY_ID,
                "init_l1_messenger_contract",
                &MESSENGER_ID,
                POINT_LIMIT
            )
            .is_err(),
        "a second messenger init must fail"
    );
    assert!(
        f.session
            .call::<[u8; 20], ()>(
                REGISTRY_ID,
                "init_evm_root_receiver",
                &RECEIVER,
                POINT_LIMIT
            )
            .is_err(),
        "a second receiver init must fail"
    );
}

#[test]
fn publish_same_nonce_refused() {
    let mut f = fixture(0xb00b);
    configure(&mut f);
    let msg = bootstrap_msg(&f, 0, &RECEIVER);
    let sigs = sign(&msg, &[(&f.sks[0], &f.pks[0]), (&f.sks[1], &f.pks[1])]);
    publish(&mut f, sigs.clone()).expect("first publish");
    assert!(
        publish(&mut f, sigs).is_err(),
        "the same nonce must not send a second bootstrap"
    );
    assert_eq!(sent(&mut f).len(), 1);
}
