// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Nocturne Standards

//! Warden host-side tests. Acceptance list in `docs/warden.md`.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use dusk_bytes::Serializable;
use dusk_core::abi::{ContractId, Event, Metadata};
use dusk_core::signatures::bls::{PublicKey as BlsPublicKey, SecretKey as BlsSecretKey};
use dusk_core::transfer::TRANSFER_CONTRACT;
use dusk_vm::{CallReceipt, ContractData, Session, VM};
use rand::SeedableRng;
use rand::rngs::StdRng;
use rkyv::Deserialize;

use atlas_encoding::call_types::{Account, InitArgs, PendingChange, PendingView};
use knot_warden_encoding::call_types::{
    InitWardenArgs, PendingAdmin, PendingAdminView, PendingServiceView, SetServiceArgs,
};
use knot_warden_encoding::events::{
    AtlasCancelForwarded, AtlasSet, DelaySet, GuardianForwarded, SchedulerSet, ServiceExecuted,
    ServiceScheduled, TimelockForwarded,
};

const ATLAS_BYTECODE: &[u8] =
    include_bytes!("../../../../atlas/target/contract/wasm32-unknown-unknown/release/atlas.wasm");
const WARDEN_BYTECODE: &[u8] =
    include_bytes!("../target/contract/wasm32-unknown-unknown/release/knot_warden.wasm");
const PROXY_BYTECODE: &[u8] = include_bytes!(
    "../test-proxy/target/contract/wasm32-unknown-unknown/release/knot_warden_test_proxy.wasm"
);

const ATLAS_ID: ContractId = ContractId::from_bytes([0xa7; 32]);
const WARDEN_ID: ContractId = ContractId::from_bytes([0xb1; 32]);
const PROXY_ID: ContractId = ContractId::from_bytes([0xb2; 32]);
const SERVICE_ID: ContractId = ContractId::from_bytes([0x11; 32]);
const SERVICE_ID_2: ContractId = ContractId::from_bytes([0x22; 32]);
const CHAIN_ID: u8 = 0xCA;
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

fn set_height(session: &mut Session, h: u64) {
    session
        .set_meta(Metadata::BLOCK_HEIGHT, h)
        .expect("setting block_height metadata should succeed");
}

fn deploy(owner_pk: &BlsPublicKey) -> Session {
    let vm = VM::ephemeral().expect("ephemeral VM");
    let mut session = vm.genesis_session(CHAIN_ID);
    for (bytecode, id) in [
        (ATLAS_BYTECODE, ATLAS_ID),
        (WARDEN_BYTECODE, WARDEN_ID),
        (PROXY_BYTECODE, PROXY_ID),
    ] {
        session
            .deploy(
                bytecode,
                ContractData::builder()
                    .owner(owner_pk.to_bytes().to_vec())
                    .contract_id(id),
                POINT_LIMIT,
            )
            .expect("deploy");
    }
    set_height(&mut session, 0);
    session
}

fn assert_panics(err: impl core::fmt::Debug, needle: &str) {
    let text = format!("{err:?}");
    assert!(
        text.contains(needle),
        "expected panic containing {needle:?}, got {text}"
    );
}

fn init_atlas(session: &mut Session, owner: &BlsPublicKey, timelock_blocks: u64) {
    set_sender(session, Some(owner));
    session
        .call::<InitArgs, ()>(
            ATLAS_ID,
            "init_guardian",
            &InitArgs {
                guardian: Account::Contract(WARDEN_ID),
                timelock_blocks,
            },
            POINT_LIMIT,
        )
        .expect("init_guardian");
}

fn init_warden(session: &mut Session, owner: &BlsPublicKey, scheduler: Account, delay_blocks: u64) {
    set_sender(session, Some(owner));
    session
        .call::<InitWardenArgs, ()>(
            WARDEN_ID,
            "init_warden",
            &InitWardenArgs {
                atlas: ATLAS_ID,
                scheduler,
                delay_blocks,
            },
            POINT_LIMIT,
        )
        .expect("init_warden");
}

fn bootstrap(
    session: &mut Session,
    owner: &BlsPublicKey,
    scheduler: Account,
    warden_delay: u64,
    atlas_timelock: u64,
) {
    init_atlas(session, owner, atlas_timelock);
    init_warden(session, owner, scheduler, warden_delay);
}

fn service(name: &str, id: ContractId) -> SetServiceArgs {
    SetServiceArgs {
        name: String::from(name),
        id,
    }
}

fn schedule(session: &mut Session, sender: &BlsPublicKey, name: &str, id: ContractId) {
    set_sender(session, Some(sender));
    session
        .call::<SetServiceArgs, ()>(
            WARDEN_ID,
            "schedule_service",
            &service(name, id),
            POINT_LIMIT,
        )
        .expect("schedule_service");
}

fn warden_topics<T>(receipt: &CallReceipt<T>) -> Vec<&str> {
    receipt
        .events
        .iter()
        .filter(|e| e.source == WARDEN_ID)
        .map(|e| e.topic.as_str())
        .collect()
}

fn event<'a, T>(receipt: &'a CallReceipt<T>, source: ContractId, topic: &str) -> &'a Event {
    let hits: Vec<_> = receipt
        .events
        .iter()
        .filter(|e| e.source == source && e.topic == topic)
        .collect();
    assert_eq!(hits.len(), 1, "expected one {topic}");
    hits[0]
}

fn decode_event<T>(ev: &Event) -> T
where
    T: rkyv::Archive,
    T::Archived: Deserialize<T, rkyv::Infallible>
        + for<'b> bytecheck::CheckBytes<rkyv::validation::validators::DefaultValidator<'b>>,
{
    let archived = rkyv::check_archived_root::<T>(&ev.data)
        .unwrap_or_else(|e| panic!("archive {}: {e}", ev.topic));
    archived
        .deserialize(&mut rkyv::Infallible)
        .unwrap_or_else(|_| panic!("deserialize {}", ev.topic))
}

fn delay_blocks(session: &mut Session) -> u64 {
    session
        .call::<(), u64>(WARDEN_ID, "delay_blocks", &(), POINT_LIMIT)
        .unwrap()
        .data
}

fn pending_service(session: &mut Session, name: &str) -> Option<PendingServiceView> {
    session
        .call::<String, Option<PendingServiceView>>(
            WARDEN_ID,
            "pending_service",
            &String::from(name),
            POINT_LIMIT,
        )
        .unwrap()
        .data
}

fn pending_admin(session: &mut Session) -> Option<PendingAdminView> {
    session
        .call::<(), Option<PendingAdminView>>(WARDEN_ID, "pending_admin", &(), POINT_LIMIT)
        .unwrap()
        .data
}

fn resolve(session: &mut Session, name: &str) -> Option<ContractId> {
    session
        .call::<String, Option<ContractId>>(ATLAS_ID, "resolve", &String::from(name), POINT_LIMIT)
        .unwrap()
        .data
}

#[test]
fn init_is_owner_once_and_deploy_does_not_init() {
    let rng = &mut StdRng::seed_from_u64(1);
    let (_, owner) = keypair(rng);
    let (_, op) = keypair(rng);
    let (_, other) = keypair(rng);
    let session = &mut deploy(&owner);

    assert!(
        session
            .call::<(), Option<Account>>(WARDEN_ID, "scheduler", &(), POINT_LIMIT)
            .unwrap()
            .data
            .is_none(),
        "deploy must not call init_warden"
    );

    set_sender(session, Some(&other));
    let denied = session.call::<InitWardenArgs, ()>(
        WARDEN_ID,
        "init_warden",
        &InitWardenArgs {
            atlas: ATLAS_ID,
            scheduler: Account::External(op),
            delay_blocks: 0,
        },
        POINT_LIMIT,
    );
    assert_panics(
        denied.expect_err("non-owner"),
        "Only the deploy owner may init_warden",
    );

    set_sender(session, Some(&owner));
    let via_proxy = session.call::<(ContractId, InitWardenArgs), ()>(
        PROXY_ID,
        "call_init_warden",
        &(
            WARDEN_ID,
            InitWardenArgs {
                atlas: ATLAS_ID,
                scheduler: Account::External(op),
                delay_blocks: 0,
            },
        ),
        POINT_LIMIT,
    );
    assert_panics(
        via_proxy.expect_err("inter-contract init"),
        "Only the deploy owner may init_warden",
    );

    init_warden(session, &owner, Account::External(op), 4);
    let receipt = session
        .call::<(), Option<Account>>(WARDEN_ID, "scheduler", &(), POINT_LIMIT)
        .unwrap();
    assert_eq!(receipt.data, Some(Account::External(op)));
    assert_eq!(delay_blocks(session), 4);
    assert_eq!(
        session
            .call::<(), Option<ContractId>>(WARDEN_ID, "atlas", &(), POINT_LIMIT)
            .unwrap()
            .data,
        Some(ATLAS_ID)
    );

    set_sender(session, Some(&owner));
    let second = session.call::<InitWardenArgs, ()>(
        WARDEN_ID,
        "init_warden",
        &InitWardenArgs {
            atlas: ATLAS_ID,
            scheduler: Account::External(other),
            delay_blocks: 0,
        },
        POINT_LIMIT,
    );
    assert_panics(
        second.expect_err("second init"),
        "warden already initialized",
    );
}

#[test]
fn init_emits_scheduler_and_delay() {
    let rng = &mut StdRng::seed_from_u64(2);
    let (_, owner) = keypair(rng);
    let (_, op) = keypair(rng);
    let session = &mut deploy(&owner);
    set_sender(session, Some(&owner));
    let receipt = session
        .call::<InitWardenArgs, ()>(
            WARDEN_ID,
            "init_warden",
            &InitWardenArgs {
                atlas: ATLAS_ID,
                scheduler: Account::External(op),
                delay_blocks: 9,
            },
            POINT_LIMIT,
        )
        .unwrap();
    assert_eq!(
        warden_topics(&receipt),
        ["atlas_set", "scheduler_set", "delay_set"]
    );
    let atlas: AtlasSet = decode_event(event(&receipt, WARDEN_ID, "atlas_set"));
    assert_eq!(atlas.atlas, ATLAS_ID);
    let scheduler: SchedulerSet = decode_event(event(&receipt, WARDEN_ID, "scheduler_set"));
    assert_eq!(scheduler.scheduler, Account::External(op));
    let delay: DelaySet = decode_event(event(&receipt, WARDEN_ID, "delay_set"));
    assert_eq!(delay.blocks, 9);
}

#[test]
fn uninitialized_writes_panic_and_there_is_no_init_export() {
    let rng = &mut StdRng::seed_from_u64(3);
    let (_, owner) = keypair(rng);
    let (_, op) = keypair(rng);
    let session = &mut deploy(&owner);
    set_sender(session, Some(&op));
    let write = session.call::<SetServiceArgs, ()>(
        WARDEN_ID,
        "schedule_service",
        &service("treasury", SERVICE_ID),
        POINT_LIMIT,
    );
    assert_panics(
        write.expect_err("uninit"),
        "warden not initialized: call init_warden first",
    );
    assert!(pending_service(session, "treasury").is_none());
    assert_eq!(delay_blocks(session), 0);

    set_sender(session, Some(&owner));
    let init_export = session.call::<(), ()>(WARDEN_ID, "init", &(), POINT_LIMIT);
    assert!(init_export.is_err(), "no init export");
}

#[test]
fn external_scheduler_rejects_inter_contract_call() {
    let rng = &mut StdRng::seed_from_u64(4);
    let (_, owner) = keypair(rng);
    let (_, op) = keypair(rng);
    let session = &mut deploy(&owner);
    bootstrap(session, &owner, Account::External(op), 0, 0);

    set_sender(session, Some(&op));
    let err = session.call::<(ContractId, String, ContractId), ()>(
        PROXY_ID,
        "call_schedule_service",
        &(WARDEN_ID, String::from("treasury"), SERVICE_ID),
        POINT_LIMIT,
    );
    assert_panics(
        err.expect_err("caller is proxy"),
        "External scheduler: direct call required",
    );
    assert!(resolve(session, "treasury").is_none());
}

#[test]
fn external_scheduler_accepts_transfer_contract_caller() {
    let rng = &mut StdRng::seed_from_u64(41);
    let (_, owner) = keypair(rng);
    let (_, op) = keypair(rng);
    let session = &mut deploy(&owner);
    session
        .deploy(
            PROXY_BYTECODE,
            ContractData::builder()
                .owner(owner.to_bytes().to_vec())
                .contract_id(TRANSFER_CONTRACT),
            POINT_LIMIT,
        )
        .expect("deploy proxy at the transfer contract id");

    set_sender(session, Some(&owner));
    session
        .call::<(ContractId, InitWardenArgs), ()>(
            TRANSFER_CONTRACT,
            "call_init_warden",
            &(
                WARDEN_ID,
                InitWardenArgs {
                    atlas: ATLAS_ID,
                    scheduler: Account::External(op),
                    delay_blocks: 0,
                },
            ),
            POINT_LIMIT,
        )
        .expect("init_warden via transfer contract");
    init_atlas(session, &owner, 0);

    set_sender(session, Some(&op));
    session
        .call::<(ContractId, String, ContractId), ()>(
            TRANSFER_CONTRACT,
            "call_schedule_service",
            &(WARDEN_ID, String::from("treasury"), SERVICE_ID),
            POINT_LIMIT,
        )
        .expect("schedule_service via transfer contract");
    assert_eq!(resolve(session, "treasury"), Some(SERVICE_ID));
}

#[test]
fn contract_scheduler_only_when_caller_is_that_id() {
    let rng = &mut StdRng::seed_from_u64(5);
    let (_, owner) = keypair(rng);
    let (_, op) = keypair(rng);
    let session = &mut deploy(&owner);
    bootstrap(session, &owner, Account::Contract(PROXY_ID), 0, 0);

    set_sender(session, Some(&op));
    let direct = session.call::<SetServiceArgs, ()>(
        WARDEN_ID,
        "schedule_service",
        &service("treasury", SERVICE_ID),
        POINT_LIMIT,
    );
    assert_panics(
        direct.expect_err("direct call"),
        "Contract scheduler: caller must be the scheduler contract",
    );

    set_sender(session, Some(&op));
    session
        .call::<(ContractId, String, ContractId), ()>(
            PROXY_ID,
            "call_schedule_service",
            &(WARDEN_ID, String::from("treasury"), SERVICE_ID),
            POINT_LIMIT,
        )
        .expect("proxy is the scheduler");
    assert_eq!(resolve(session, "treasury"), Some(SERVICE_ID));
}

#[test]
fn schedule_rejects_empty_unchanged_and_duplicate_name() {
    let rng = &mut StdRng::seed_from_u64(6);
    let (_, owner) = keypair(rng);
    let (_, op) = keypair(rng);
    let session = &mut deploy(&owner);
    bootstrap(session, &owner, Account::External(op), 0, 0);

    set_sender(session, Some(&op));
    let empty = session.call::<SetServiceArgs, ()>(
        WARDEN_ID,
        "schedule_service",
        &service("", SERVICE_ID),
        POINT_LIMIT,
    );
    assert_panics(empty.expect_err("empty"), "empty service name");
    assert!(pending_service(session, "").is_none());

    schedule(session, &op, "treasury", SERVICE_ID);
    assert_eq!(resolve(session, "treasury"), Some(SERVICE_ID));
    assert!(pending_service(session, "treasury").is_none());

    set_sender(session, Some(&op));
    let same = session.call::<SetServiceArgs, ()>(
        WARDEN_ID,
        "schedule_service",
        &service("treasury", SERVICE_ID),
        POINT_LIMIT,
    );
    assert_panics(same.expect_err("unchanged"), "service unchanged");
    assert!(pending_service(session, "treasury").is_none());
    assert_eq!(resolve(session, "treasury"), Some(SERVICE_ID));

    // Non-zero delay so a second schedule can observe the occupied name.
    let session = &mut deploy(&owner);
    bootstrap(session, &owner, Account::External(op), 5, 0);
    schedule(session, &op, "treasury", SERVICE_ID);
    set_sender(session, Some(&op));
    let dup = session.call::<SetServiceArgs, ()>(
        WARDEN_ID,
        "schedule_service",
        &service("treasury", SERVICE_ID_2),
        POINT_LIMIT,
    );
    assert_panics(
        dup.expect_err("duplicate"),
        "a pending service change already exists for this name; cancel or execute it first",
    );
    assert_eq!(pending_service(session, "treasury").unwrap().id, SERVICE_ID);
}

#[test]
fn two_names_can_be_pending() {
    let rng = &mut StdRng::seed_from_u64(7);
    let (_, owner) = keypair(rng);
    let (_, op) = keypair(rng);
    let session = &mut deploy(&owner);
    bootstrap(session, &owner, Account::External(op), 5, 0);
    schedule(session, &op, "treasury", SERVICE_ID);
    schedule(session, &op, "policy", SERVICE_ID_2);
    assert_eq!(pending_service(session, "treasury").unwrap().execute_at, 5);
    assert_eq!(pending_service(session, "policy").unwrap().id, SERVICE_ID_2);
}

#[test]
fn execute_waits_then_permissionless_apply() {
    let rng = &mut StdRng::seed_from_u64(8);
    let (_, owner) = keypair(rng);
    let (_, op) = keypair(rng);
    let (_, other) = keypair(rng);
    let session = &mut deploy(&owner);
    bootstrap(session, &owner, Account::External(op), 5, 0);
    schedule(session, &op, "treasury", SERVICE_ID);

    set_height(session, 4);
    set_sender(session, Some(&other));
    let early = session.call::<String, ()>(
        WARDEN_ID,
        "execute_service",
        &String::from("treasury"),
        POINT_LIMIT,
    );
    assert_panics(early.expect_err("early"), "delay not elapsed");
    assert!(resolve(session, "treasury").is_none());

    set_height(session, 5);
    set_sender(session, Some(&other));
    let receipt = session
        .call::<String, ()>(
            WARDEN_ID,
            "execute_service",
            &String::from("treasury"),
            POINT_LIMIT,
        )
        .expect("permissionless execute");
    assert_eq!(warden_topics(&receipt), ["service_executed"]);
    let executed: ServiceExecuted = decode_event(event(&receipt, WARDEN_ID, "service_executed"));
    assert_eq!(executed.id, SERVICE_ID);
    assert_eq!(resolve(session, "treasury"), Some(SERVICE_ID));
    assert!(pending_service(session, "treasury").is_none());
}

#[test]
fn cancel_service_scheduler_only_including_after_execute_at() {
    let rng = &mut StdRng::seed_from_u64(9);
    let (_, owner) = keypair(rng);
    let (_, op) = keypair(rng);
    let (_, other) = keypair(rng);
    let session = &mut deploy(&owner);
    bootstrap(session, &owner, Account::External(op), 5, 0);
    schedule(session, &op, "treasury", SERVICE_ID);

    set_sender(session, Some(&other));
    let denied = session.call::<String, ()>(
        WARDEN_ID,
        "cancel_service",
        &String::from("treasury"),
        POINT_LIMIT,
    );
    assert_panics(
        denied.expect_err("non-scheduler"),
        "Only the warden scheduler may perform this action",
    );

    set_height(session, 9);
    set_sender(session, Some(&op));
    session
        .call::<String, ()>(
            WARDEN_ID,
            "cancel_service",
            &String::from("treasury"),
            POINT_LIMIT,
        )
        .expect("cancel after execute_at");
    assert!(pending_service(session, "treasury").is_none());
    assert!(resolve(session, "treasury").is_none());
}

#[test]
fn zero_delay_emits_both_and_updates_atlas() {
    let rng = &mut StdRng::seed_from_u64(10);
    let (_, owner) = keypair(rng);
    let (_, op) = keypair(rng);
    let session = &mut deploy(&owner);
    bootstrap(session, &owner, Account::External(op), 0, 0);
    set_sender(session, Some(&op));
    let receipt = session
        .call::<SetServiceArgs, ()>(
            WARDEN_ID,
            "schedule_service",
            &service("treasury", SERVICE_ID),
            POINT_LIMIT,
        )
        .unwrap();
    assert_eq!(
        warden_topics(&receipt),
        ["service_scheduled", "service_executed"]
    );
    let scheduled: ServiceScheduled = decode_event(event(&receipt, WARDEN_ID, "service_scheduled"));
    assert_eq!(scheduled.execute_at, 0);
    assert_eq!(scheduled.id, SERVICE_ID);
    assert!(
        receipt
            .events
            .iter()
            .any(|e| e.source == ATLAS_ID && e.topic == "service_updated")
    );
    assert_eq!(resolve(session, "treasury"), Some(SERVICE_ID));
}

#[test]
fn set_delay_unchanged_does_not_take_slot_and_real_change_waits() {
    let rng = &mut StdRng::seed_from_u64(11);
    let (_, owner) = keypair(rng);
    let (_, op) = keypair(rng);
    let session = &mut deploy(&owner);
    bootstrap(session, &owner, Account::External(op), 10, 0);

    set_sender(session, Some(&op));
    let same = session.call::<u64, ()>(WARDEN_ID, "set_delay", &10, POINT_LIMIT);
    assert_panics(same.expect_err("unchanged"), "delay unchanged");
    assert!(pending_admin(session).is_none());

    set_sender(session, Some(&op));
    session
        .call::<u64, ()>(WARDEN_ID, "set_delay", &20, POINT_LIMIT)
        .expect("set_delay");
    let pending = pending_admin(session).unwrap();
    assert_eq!(pending.change, PendingAdmin::Delay(20));
    assert_eq!(pending.execute_at, 10);
    assert_eq!(delay_blocks(session), 10);

    schedule(session, &op, "treasury", SERVICE_ID);
    assert_eq!(
        pending_service(session, "treasury").unwrap().execute_at,
        10,
        "service execute_at is fixed from the delay at schedule time"
    );
}

#[test]
fn set_delay_zero_does_not_apply_early() {
    let rng = &mut StdRng::seed_from_u64(12);
    let (_, owner) = keypair(rng);
    let (_, op) = keypair(rng);
    let (_, other) = keypair(rng);
    let session = &mut deploy(&owner);
    bootstrap(session, &owner, Account::External(op), 10, 0);
    set_sender(session, Some(&op));
    session
        .call::<u64, ()>(WARDEN_ID, "set_delay", &0, POINT_LIMIT)
        .unwrap();

    set_height(session, 9);
    set_sender(session, Some(&other));
    let early = session.call::<(), ()>(WARDEN_ID, "execute_admin", &(), POINT_LIMIT);
    assert_panics(early.expect_err("early"), "delay not elapsed");
    assert_eq!(delay_blocks(session), 10);
    assert_eq!(
        pending_admin(session).unwrap().change,
        PendingAdmin::Delay(0)
    );

    set_height(session, 10);
    set_sender(session, Some(&other));
    session
        .call::<(), ()>(WARDEN_ID, "execute_admin", &(), POINT_LIMIT)
        .expect("execute_admin");
    assert_eq!(delay_blocks(session), 0);
    assert!(pending_admin(session).is_none());
}

#[test]
fn set_scheduler_unchanged_and_waits() {
    let rng = &mut StdRng::seed_from_u64(13);
    let (_, owner) = keypair(rng);
    let (_, op) = keypair(rng);
    let (_, next) = keypair(rng);
    let session = &mut deploy(&owner);
    bootstrap(session, &owner, Account::External(op), 8, 0);

    set_sender(session, Some(&op));
    let same = session.call::<Account, ()>(
        WARDEN_ID,
        "set_scheduler",
        &Account::External(op),
        POINT_LIMIT,
    );
    assert_panics(same.expect_err("unchanged"), "scheduler unchanged");
    assert!(pending_admin(session).is_none());

    set_sender(session, Some(&op));
    session
        .call::<Account, ()>(
            WARDEN_ID,
            "set_scheduler",
            &Account::External(next),
            POINT_LIMIT,
        )
        .unwrap();
    assert_eq!(
        pending_admin(session).unwrap().change,
        PendingAdmin::Scheduler(Account::External(next))
    );
    assert_eq!(
        session
            .call::<(), Option<Account>>(WARDEN_ID, "scheduler", &(), POINT_LIMIT)
            .unwrap()
            .data,
        Some(Account::External(op))
    );

    set_height(session, 7);
    set_sender(session, Some(&op));
    let early = session.call::<(), ()>(WARDEN_ID, "execute_admin", &(), POINT_LIMIT);
    assert_panics(early.expect_err("early"), "delay not elapsed");

    set_height(session, 8);
    set_sender(session, Some(&owner));
    session
        .call::<(), ()>(WARDEN_ID, "execute_admin", &(), POINT_LIMIT)
        .unwrap();
    assert_eq!(
        session
            .call::<(), Option<Account>>(WARDEN_ID, "scheduler", &(), POINT_LIMIT)
            .unwrap()
            .data,
        Some(Account::External(next))
    );

    set_sender(session, Some(&op));
    let old = session.call::<SetServiceArgs, ()>(
        WARDEN_ID,
        "schedule_service",
        &service("treasury", SERVICE_ID),
        POINT_LIMIT,
    );
    assert!(old.is_err(), "old scheduler is no longer authority");
}

#[test]
fn zero_delay_set_scheduler_applies() {
    let rng = &mut StdRng::seed_from_u64(14);
    let (_, owner) = keypair(rng);
    let (_, op) = keypair(rng);
    let (_, next) = keypair(rng);
    let session = &mut deploy(&owner);
    bootstrap(session, &owner, Account::External(op), 0, 0);
    set_sender(session, Some(&op));
    let receipt = session
        .call::<Account, ()>(
            WARDEN_ID,
            "set_scheduler",
            &Account::External(next),
            POINT_LIMIT,
        )
        .unwrap();
    assert_eq!(
        warden_topics(&receipt),
        ["admin_scheduled", "scheduler_set"]
    );
    assert_eq!(
        session
            .call::<(), Option<Account>>(WARDEN_ID, "scheduler", &(), POINT_LIMIT)
            .unwrap()
            .data,
        Some(Account::External(next))
    );
    assert!(pending_admin(session).is_none());
}

#[test]
fn set_guardian_forwards_without_touching_warden_maps() {
    let rng = &mut StdRng::seed_from_u64(15);
    let (_, owner) = keypair(rng);
    let (_, op) = keypair(rng);
    let (_, next) = keypair(rng);
    let (_, other) = keypair(rng);
    let session = &mut deploy(&owner);
    bootstrap(session, &owner, Account::External(op), 4, 10);

    set_sender(session, Some(&other));
    let denied = session.call::<Account, ()>(
        WARDEN_ID,
        "set_guardian",
        &Account::External(next),
        POINT_LIMIT,
    );
    assert_panics(
        denied.expect_err("non-scheduler"),
        "Only the warden scheduler may perform this action",
    );
    assert!(
        session
            .call::<(), Option<PendingView>>(ATLAS_ID, "pending", &(), POINT_LIMIT)
            .unwrap()
            .data
            .is_none()
    );

    set_sender(session, Some(&op));
    let receipt = session
        .call::<Account, ()>(
            WARDEN_ID,
            "set_guardian",
            &Account::External(next),
            POINT_LIMIT,
        )
        .expect("forward set_guardian");
    assert_eq!(warden_topics(&receipt), ["guardian_forwarded"]);
    let forwarded: GuardianForwarded =
        decode_event(event(&receipt, WARDEN_ID, "guardian_forwarded"));
    assert_eq!(forwarded.guardian, Account::External(next));
    assert!(pending_admin(session).is_none());
    assert!(pending_service(session, "treasury").is_none());
    let atlas_pending = session
        .call::<(), Option<PendingView>>(ATLAS_ID, "pending", &(), POINT_LIMIT)
        .unwrap()
        .data
        .expect("atlas pending");
    assert_eq!(
        atlas_pending.change,
        PendingChange::Guardian(Account::External(next))
    );
    assert_eq!(atlas_pending.execute_at, 10);

    set_sender(session, Some(&op));
    let blocked = session.call::<u64, ()>(WARDEN_ID, "set_timelock", &3, POINT_LIMIT);
    assert!(blocked.is_err(), "atlas slot is already taken");

    set_sender(session, Some(&op));
    let cancel = session
        .call::<(), ()>(WARDEN_ID, "cancel_atlas_pending", &(), POINT_LIMIT)
        .expect("cancel_atlas_pending");
    assert_eq!(warden_topics(&cancel), ["atlas_cancel_forwarded"]);
    let _: AtlasCancelForwarded = decode_event(event(&cancel, WARDEN_ID, "atlas_cancel_forwarded"));
    assert!(
        session
            .call::<(), Option<PendingView>>(ATLAS_ID, "pending", &(), POINT_LIMIT)
            .unwrap()
            .data
            .is_none()
    );

    set_sender(session, Some(&op));
    let timelock = session
        .call::<u64, ()>(WARDEN_ID, "set_timelock", &3, POINT_LIMIT)
        .expect("forward set_timelock");
    assert_eq!(warden_topics(&timelock), ["timelock_forwarded"]);
    let forwarded: TimelockForwarded =
        decode_event(event(&timelock, WARDEN_ID, "timelock_forwarded"));
    assert_eq!(forwarded.blocks, 3);
    let atlas_pending = session
        .call::<(), Option<PendingView>>(ATLAS_ID, "pending", &(), POINT_LIMIT)
        .unwrap()
        .data
        .unwrap();
    assert_eq!(atlas_pending.change, PendingChange::Timelock(3));
    assert!(pending_admin(session).is_none());
}

#[test]
fn delay_overflow_panics() {
    let rng = &mut StdRng::seed_from_u64(16);
    let (_, owner) = keypair(rng);
    let (_, op) = keypair(rng);
    let session = &mut deploy(&owner);
    bootstrap(session, &owner, Account::External(op), u64::MAX, 0);
    set_height(session, 1);
    set_sender(session, Some(&op));
    let err = session.call::<SetServiceArgs, ()>(
        WARDEN_ID,
        "schedule_service",
        &service("treasury", SERVICE_ID),
        POINT_LIMIT,
    );
    assert_panics(err.expect_err("overflow"), "delay overflow");
    assert!(pending_service(session, "treasury").is_none());
}
