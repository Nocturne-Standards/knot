// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Nocturne Standards

//! Archived-layout goldens for warden call types and events.
//!
//! **rkyv camp:** this crate pins `rkyv = "=0.7.39"`.
//!
//! Struct Archive types carry `#[archive_attr(repr(C))]`. `PendingAdmin`
//! skips the attribute — rustc rejects it on archived enums. Pin is the golden.
//!
//! Fixed inputs: `StdRng::seed_from_u64(0xa11ce_u64)`; `ContractId` all `0x0d`;
//! name `"treasury-data"`; delay `86400`; `execute_at: 100`.
//! Same key as the Atlas `Account::External` golden.

extern crate alloc;

use alloc::format;
use alloc::string::String;

use dusk_core::abi::ContractId;
use dusk_core::signatures::bls::{PublicKey as BlsPublicKey, SecretKey as BlsSecretKey};
use rand::SeedableRng;
use rand::rngs::StdRng;
use rkyv::Serialize;
use rkyv::ser::serializers::AllocSerializer;

use knot_warden_encoding::call_types::{
    Account, InitWardenArgs, PendingAdmin, PendingAdminView, PendingServiceView,
};
use knot_warden_encoding::events::{
    AdminCancelled, AdminScheduled, DelaySet, SchedulerSet, ServiceCancelled, ServiceExecuted,
    ServiceScheduled,
};

const SERVICE_ID: ContractId = ContractId::from_bytes([0x0d; 32]);

fn archive_hex<T>(v: &T) -> String
where
    T: Serialize<AllocSerializer<4096>>,
{
    rkyv::to_bytes::<_, 4096>(v)
        .expect("archive")
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn pk0() -> BlsPublicKey {
    let mut rng = StdRng::seed_from_u64(0xa11ce_u64);
    let sk = BlsSecretKey::random(&mut rng);
    BlsPublicKey::from(&sk)
}

/// `InitWardenArgs { atlas: SERVICE_ID, scheduler: External(pk0), delay_blocks: 86400 }`.
/// Provenance: rustc 1.94.0; rkyv 0.7.39; post-`repr(C)`.
pub const GOLDEN_INIT_WARDEN_ARGS_HEX: &str = "0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0000000000000000e3a945bd7dbd51365c255b3a7851432419f20ddb7bc948f5b60d677c5b02ff9e6255228ee75c9dd8a3bd4a86751e9b14cf501c89e69b4b2a2169c189accff3afc07b7ff80a0acfc75a4e073ee006624f722dd52ef90ae1828d8bfdcb6c1e260aad4c44e90e1b5e5c2067d4363ee978a0db41fdba0f29829a1263e43f33f231a9dc20fc5acafc235d9c920f2772cbd716ddb84cca39704625b55a01a011e7eeae177ef0949bce380f2d64afd6038e15ff70e7aaf4d9b92e8bf4188696e1264e0900000000000000008051010000000000";

/// `PendingAdmin::Delay(86400)`.
/// Provenance: rustc 1.94.0; rkyv 0.7.39.
pub const GOLDEN_PENDING_ADMIN_DELAY_HEX: &str = "000000000000000080510100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";

/// `PendingAdmin::Scheduler(Account::External(pk0))`.
/// Provenance: rustc 1.94.0; rkyv 0.7.39.
pub const GOLDEN_PENDING_ADMIN_SCHEDULER_EXTERNAL_HEX: &str = "01000000000000000000000000000000e3a945bd7dbd51365c255b3a7851432419f20ddb7bc948f5b60d677c5b02ff9e6255228ee75c9dd8a3bd4a86751e9b14cf501c89e69b4b2a2169c189accff3afc07b7ff80a0acfc75a4e073ee006624f722dd52ef90ae1828d8bfdcb6c1e260aad4c44e90e1b5e5c2067d4363ee978a0db41fdba0f29829a1263e43f33f231a9dc20fc5acafc235d9c920f2772cbd716ddb84cca39704625b55a01a011e7eeae177ef0949bce380f2d64afd6038e15ff70e7aaf4d9b92e8bf4188696e1264e090000000000000000";

/// `PendingAdmin::Scheduler(Account::Contract(SERVICE_ID))`.
/// Provenance: rustc 1.94.0; rkyv 0.7.39.
pub const GOLDEN_PENDING_ADMIN_SCHEDULER_CONTRACT_HEX: &str = "0100000000000000010d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";

/// `PendingServiceView { name: "treasury-data", id: SERVICE_ID, execute_at: 100 }`.
/// Provenance: rustc 1.94.0; rkyv 0.7.39; post-`repr(C)`.
pub const GOLDEN_PENDING_SERVICE_VIEW_HEX: &str = "74726561737572792d646174610000000d000000f0ffffff0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d6400000000000000";

/// `PendingAdminView { change: Delay(86400), execute_at: 100 }`.
/// Provenance: rustc 1.94.0; rkyv 0.7.39; post-`repr(C)`.
pub const GOLDEN_PENDING_ADMIN_VIEW_HEX: &str = "0000000000000000805101000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000006400000000000000";

/// `SchedulerSet { scheduler: External(pk0) }`.
/// Provenance: rustc 1.94.0; rkyv 0.7.39; post-`repr(C)`.
pub const GOLDEN_SCHEDULER_SET_HEX: &str = "0000000000000000e3a945bd7dbd51365c255b3a7851432419f20ddb7bc948f5b60d677c5b02ff9e6255228ee75c9dd8a3bd4a86751e9b14cf501c89e69b4b2a2169c189accff3afc07b7ff80a0acfc75a4e073ee006624f722dd52ef90ae1828d8bfdcb6c1e260aad4c44e90e1b5e5c2067d4363ee978a0db41fdba0f29829a1263e43f33f231a9dc20fc5acafc235d9c920f2772cbd716ddb84cca39704625b55a01a011e7eeae177ef0949bce380f2d64afd6038e15ff70e7aaf4d9b92e8bf4188696e1264e090000000000000000";

/// `DelaySet { blocks: 86400 }`.
/// Provenance: rustc 1.94.0; rkyv 0.7.39; post-`repr(C)`.
pub const GOLDEN_DELAY_SET_HEX: &str = "8051010000000000";

/// `ServiceScheduled { name: "treasury-data", id: SERVICE_ID, execute_at: 100 }`.
/// Provenance: rustc 1.94.0; rkyv 0.7.39; post-`repr(C)`.
pub const GOLDEN_SERVICE_SCHEDULED_HEX: &str = "74726561737572792d646174610000000d000000f0ffffff0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d6400000000000000";

/// `ServiceExecuted { name: "treasury-data", id: SERVICE_ID }`.
/// Provenance: rustc 1.94.0; rkyv 0.7.39; post-`repr(C)`.
pub const GOLDEN_SERVICE_EXECUTED_HEX: &str = "74726561737572792d646174610000000d000000f0ffffff0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d";

/// `ServiceCancelled { name: "treasury-data", id: SERVICE_ID }`.
/// Provenance: rustc 1.94.0; rkyv 0.7.39; post-`repr(C)`.
pub const GOLDEN_SERVICE_CANCELLED_HEX: &str = "74726561737572792d646174610000000d000000f0ffffff0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d";

/// `AdminScheduled { change: Delay(86400), execute_at: 100 }`.
/// Provenance: rustc 1.94.0; rkyv 0.7.39; post-`repr(C)`.
pub const GOLDEN_ADMIN_SCHEDULED_HEX: &str = "0000000000000000805101000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000006400000000000000";

/// `AdminCancelled { change: Delay(86400) }`.
/// Provenance: rustc 1.94.0; rkyv 0.7.39; post-`repr(C)`.
pub const GOLDEN_ADMIN_CANCELLED_HEX: &str = "000000000000000080510100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";

fn samples() -> Vec<(&'static str, String, &'static str)> {
    let pk = pk0();
    let external = Account::External(pk);
    let delay = PendingAdmin::Delay(86400);
    let name = String::from("treasury-data");
    vec![
        (
            "InitWardenArgs",
            archive_hex(&InitWardenArgs {
                atlas: SERVICE_ID,
                scheduler: external.clone(),
                delay_blocks: 86400,
            }),
            GOLDEN_INIT_WARDEN_ARGS_HEX,
        ),
        (
            "PendingAdmin::Delay",
            archive_hex(&delay),
            GOLDEN_PENDING_ADMIN_DELAY_HEX,
        ),
        (
            "PendingAdmin::Scheduler(External)",
            archive_hex(&PendingAdmin::Scheduler(external.clone())),
            GOLDEN_PENDING_ADMIN_SCHEDULER_EXTERNAL_HEX,
        ),
        (
            "PendingAdmin::Scheduler(Contract)",
            archive_hex(&PendingAdmin::Scheduler(Account::Contract(SERVICE_ID))),
            GOLDEN_PENDING_ADMIN_SCHEDULER_CONTRACT_HEX,
        ),
        (
            "PendingServiceView",
            archive_hex(&PendingServiceView {
                name: name.clone(),
                id: SERVICE_ID,
                execute_at: 100,
            }),
            GOLDEN_PENDING_SERVICE_VIEW_HEX,
        ),
        (
            "PendingAdminView",
            archive_hex(&PendingAdminView {
                change: delay.clone(),
                execute_at: 100,
            }),
            GOLDEN_PENDING_ADMIN_VIEW_HEX,
        ),
        (
            "SchedulerSet",
            archive_hex(&SchedulerSet {
                scheduler: external,
            }),
            GOLDEN_SCHEDULER_SET_HEX,
        ),
        (
            "DelaySet",
            archive_hex(&DelaySet { blocks: 86400 }),
            GOLDEN_DELAY_SET_HEX,
        ),
        (
            "ServiceScheduled",
            archive_hex(&ServiceScheduled {
                name: name.clone(),
                id: SERVICE_ID,
                execute_at: 100,
            }),
            GOLDEN_SERVICE_SCHEDULED_HEX,
        ),
        (
            "ServiceExecuted",
            archive_hex(&ServiceExecuted {
                name: name.clone(),
                id: SERVICE_ID,
            }),
            GOLDEN_SERVICE_EXECUTED_HEX,
        ),
        (
            "ServiceCancelled",
            archive_hex(&ServiceCancelled {
                name,
                id: SERVICE_ID,
            }),
            GOLDEN_SERVICE_CANCELLED_HEX,
        ),
        (
            "AdminScheduled",
            archive_hex(&AdminScheduled {
                change: delay.clone(),
                execute_at: 100,
            }),
            GOLDEN_ADMIN_SCHEDULED_HEX,
        ),
        (
            "AdminCancelled",
            archive_hex(&AdminCancelled { change: delay }),
            GOLDEN_ADMIN_CANCELLED_HEX,
        ),
    ]
}

#[test]
fn layout_goldens_match() {
    let rows = samples();
    for (name, actual, expected) in rows {
        assert_eq!(actual, expected, "{name}");
    }
}
