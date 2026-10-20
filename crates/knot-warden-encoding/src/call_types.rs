// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Nocturne Standards

//! Call arguments and views for warden.
//!
//! Struct Archive types carry `#[archive_attr(repr(C))]`. `PendingAdmin` is a
//! data-carrying enum: rustc rejects `repr(C)` on archived enums. Its pin is
//! the layout golden.

use alloc::string::String;

use bytecheck::CheckBytes;
use dusk_core::abi::ContractId;
use rkyv::{Archive, Deserialize, Serialize};

pub use atlas_encoding::call_types::{Account, PendingView as AtlasPendingView, SetServiceArgs};

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[archive_attr(repr(C))]
#[cfg_attr(feature = "data-driver", derive(serde::Serialize, serde::Deserialize))]
pub struct InitWardenArgs {
    pub atlas: ContractId,
    pub scheduler: Account,
    pub delay_blocks: u64,
}

/// Scheduled replacement of the scheduler, of `delay_blocks`, or of the
/// Atlas guardian. The Atlas guardian variant waits `delay_blocks` here
/// before warden calls Atlas `set_guardian`.
///
/// Data-carrying enum: no `repr(C)`. Pin is the layout golden.
#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[cfg_attr(feature = "data-driver", derive(serde::Serialize, serde::Deserialize))]
pub enum PendingAdmin {
    Delay(u64),
    Scheduler(Account),
    AtlasGuardian(Account),
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[archive_attr(repr(C))]
#[cfg_attr(feature = "data-driver", derive(serde::Serialize, serde::Deserialize))]
pub struct PendingServiceView {
    pub name: String,
    pub id: ContractId,
    pub execute_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[archive_attr(repr(C))]
#[cfg_attr(feature = "data-driver", derive(serde::Serialize, serde::Deserialize))]
pub struct PendingAdminView {
    pub change: PendingAdmin,
    pub execute_at: u64,
}
