// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Nocturne Standards

//! Host-visible event payloads for warden mutations.
//!
//! Struct Archive types carry `#[archive_attr(repr(C))]`. Nested
//! `PendingAdmin` skips that attribute on its own definition.

use alloc::string::String;

use bytecheck::CheckBytes;
use dusk_core::abi::ContractId;
use rkyv::{Archive, Deserialize, Serialize};

use crate::call_types::{Account, PendingAdmin};

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[archive_attr(repr(C))]
#[cfg_attr(feature = "data-driver", derive(serde::Serialize, serde::Deserialize))]
pub struct SchedulerSet {
    pub scheduler: Account,
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[archive_attr(repr(C))]
#[cfg_attr(feature = "data-driver", derive(serde::Serialize, serde::Deserialize))]
pub struct DelaySet {
    pub blocks: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[archive_attr(repr(C))]
#[cfg_attr(feature = "data-driver", derive(serde::Serialize, serde::Deserialize))]
pub struct ServiceScheduled {
    pub name: String,
    pub id: ContractId,
    pub execute_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[archive_attr(repr(C))]
#[cfg_attr(feature = "data-driver", derive(serde::Serialize, serde::Deserialize))]
pub struct ServiceExecuted {
    pub name: String,
    pub id: ContractId,
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[archive_attr(repr(C))]
#[cfg_attr(feature = "data-driver", derive(serde::Serialize, serde::Deserialize))]
pub struct ServiceCancelled {
    pub name: String,
    pub id: ContractId,
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[archive_attr(repr(C))]
#[cfg_attr(feature = "data-driver", derive(serde::Serialize, serde::Deserialize))]
pub struct AdminScheduled {
    pub change: PendingAdmin,
    pub execute_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[archive_attr(repr(C))]
#[cfg_attr(feature = "data-driver", derive(serde::Serialize, serde::Deserialize))]
pub struct AdminCancelled {
    pub change: PendingAdmin,
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[archive_attr(repr(C))]
#[cfg_attr(feature = "data-driver", derive(serde::Serialize, serde::Deserialize))]
pub struct AtlasSet {
    pub atlas: ContractId,
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[archive_attr(repr(C))]
#[cfg_attr(feature = "data-driver", derive(serde::Serialize, serde::Deserialize))]
pub struct GuardianForwarded {
    pub guardian: Account,
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[archive_attr(repr(C))]
#[cfg_attr(feature = "data-driver", derive(serde::Serialize, serde::Deserialize))]
pub struct TimelockForwarded {
    pub blocks: u64,
}

/// Warden holds none of the Atlas pending change. The topic is the record
/// that `cancel_pending` was forwarded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[archive_attr(repr(C))]
#[cfg_attr(feature = "data-driver", derive(serde::Serialize, serde::Deserialize))]
pub struct AtlasCancelForwarded;

/// `ContractEvent` topic impls, needed only by the contract macro.
/// Host consumers decoding events leave the `contract-events` feature off.
#[cfg(feature = "contract-events")]
mod topics {
    use super::*;
    use dusk_forge::ContractEvent;

    impl ContractEvent for SchedulerSet {
        const TOPICS: &'static [&'static str] = &["scheduler_set"];
    }

    impl ContractEvent for DelaySet {
        const TOPICS: &'static [&'static str] = &["delay_set"];
    }

    impl ContractEvent for ServiceScheduled {
        const TOPICS: &'static [&'static str] = &["service_scheduled"];
    }

    impl ContractEvent for ServiceExecuted {
        const TOPICS: &'static [&'static str] = &["service_executed"];
    }

    impl ContractEvent for ServiceCancelled {
        const TOPICS: &'static [&'static str] = &["service_cancelled"];
    }

    impl ContractEvent for AdminScheduled {
        const TOPICS: &'static [&'static str] = &["admin_scheduled"];
    }

    impl ContractEvent for AdminCancelled {
        const TOPICS: &'static [&'static str] = &["admin_cancelled"];
    }

    impl ContractEvent for AtlasSet {
        const TOPICS: &'static [&'static str] = &["atlas_set"];
    }

    impl ContractEvent for GuardianForwarded {
        const TOPICS: &'static [&'static str] = &["guardian_forwarded"];
    }

    impl ContractEvent for TimelockForwarded {
        const TOPICS: &'static [&'static str] = &["timelock_forwarded"];
    }

    impl ContractEvent for AtlasCancelForwarded {
        const TOPICS: &'static [&'static str] = &["atlas_cancel_forwarded"];
    }
}
