// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Nocturne Standards

//! Dispatch named Knot events by contract identity, topic, and version.
//!
//! Payloads are the structs in `knot-encoding` and `knot-warden-encoding`.
//! A topic selects an arm only together with its contract. Version
//! [`NAMED_EVENT_VERSION`] is the only accepted generation.

use knot_encoding::events::{
    AccountChanged, AccountCreated, AuthorizedAccountSet, DataSet, PendingCancelled,
    PendingScheduled, ProposalApproved, ProposalCancelled, ProposalCreated, ProposalFinalized,
    ProposalQueued, ProposalTtlSet, Pruned, RegistrySet, TimelockSet, TombstoneSet,
};
use knot_warden_encoding::events::{
    AdminCancelled, AdminScheduled, AtlasCancelForwarded, AtlasSet, DelaySet, GuardianForwarded,
    SchedulerSet, ServiceCancelled, ServiceExecuted, ServiceScheduled, TimelockForwarded,
};

/// Named-struct event generation. Other versions are rejected.
pub const NAMED_EVENT_VERSION: u32 = 1;

pub const CONTRACT_REGISTRY: &str = "knot-registry";
pub const CONTRACT_PROPOSALS: &str = "knot-proposals";
pub const CONTRACT_WARDEN: &str = "knot-warden";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    UnsupportedVersion(u32),
    UnknownEvent { contract: String, topic: String },
    InvalidPayload(String),
}

impl core::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            DecodeError::UnsupportedVersion(version) => {
                write!(f, "unsupported event version {version}")
            }
            DecodeError::UnknownEvent { contract, topic } => {
                write!(f, "unknown event {contract}/{topic}")
            }
            DecodeError::InvalidPayload(message) => {
                write!(f, "invalid event payload: {message}")
            }
        }
    }
}

impl std::error::Error for DecodeError {}

macro_rules! from_archived {
    ($ty:ty, $bytes:expr) => {{
        let value = rkyv::from_bytes::<$ty>($bytes)
            .map_err(|err| DecodeError::InvalidPayload(err.to_string()))?;
        serde_json::to_value(&value).map_err(|err| DecodeError::InvalidPayload(err.to_string()))
    }};
}

/// Decode one event payload.
///
/// `contract` and `topic` select the arm. The same topic on two contracts is
/// two arms. Bytes are the rkyv archive of that arm's named struct.
pub fn decode(
    contract: &str,
    topic: &str,
    version: u32,
    bytes: &[u8],
) -> Result<serde_json::Value, DecodeError> {
    if version != NAMED_EVENT_VERSION {
        return Err(DecodeError::UnsupportedVersion(version));
    }
    match (contract, topic) {
        (CONTRACT_REGISTRY, "data_set") => from_archived!(DataSet, bytes),
        (CONTRACT_REGISTRY, "account_created") => from_archived!(AccountCreated, bytes),
        (CONTRACT_REGISTRY, "pending_scheduled") => from_archived!(PendingScheduled, bytes),
        (CONTRACT_REGISTRY, "account_changed") => from_archived!(AccountChanged, bytes),
        (CONTRACT_REGISTRY, "timelock_set") => from_archived!(TimelockSet, bytes),
        (CONTRACT_REGISTRY, "pending_cancelled") => from_archived!(PendingCancelled, bytes),
        (CONTRACT_PROPOSALS, "data_set") => from_archived!(DataSet, bytes),
        (CONTRACT_PROPOSALS, "registry_set") => from_archived!(RegistrySet, bytes),
        (CONTRACT_PROPOSALS, "proposal_ttl_set") => from_archived!(ProposalTtlSet, bytes),
        (CONTRACT_PROPOSALS, "tombstone_set") => from_archived!(TombstoneSet, bytes),
        (CONTRACT_PROPOSALS, "authorized_account_set") => {
            from_archived!(AuthorizedAccountSet, bytes)
        }
        (CONTRACT_PROPOSALS, "proposal_created") => from_archived!(ProposalCreated, bytes),
        (CONTRACT_PROPOSALS, "proposal_approved") => from_archived!(ProposalApproved, bytes),
        (CONTRACT_PROPOSALS, "proposal_finalized") => from_archived!(ProposalFinalized, bytes),
        (CONTRACT_PROPOSALS, "proposal_queued") => from_archived!(ProposalQueued, bytes),
        (CONTRACT_PROPOSALS, "proposal_cancelled") => from_archived!(ProposalCancelled, bytes),
        (CONTRACT_PROPOSALS, "pruned") => from_archived!(Pruned, bytes),
        (CONTRACT_WARDEN, "scheduler_set") => from_archived!(SchedulerSet, bytes),
        (CONTRACT_WARDEN, "delay_set") => from_archived!(DelaySet, bytes),
        (CONTRACT_WARDEN, "service_scheduled") => from_archived!(ServiceScheduled, bytes),
        (CONTRACT_WARDEN, "service_executed") => from_archived!(ServiceExecuted, bytes),
        (CONTRACT_WARDEN, "service_cancelled") => from_archived!(ServiceCancelled, bytes),
        (CONTRACT_WARDEN, "admin_scheduled") => from_archived!(AdminScheduled, bytes),
        (CONTRACT_WARDEN, "admin_cancelled") => from_archived!(AdminCancelled, bytes),
        (CONTRACT_WARDEN, "atlas_set") => from_archived!(AtlasSet, bytes),
        (CONTRACT_WARDEN, "guardian_forwarded") => from_archived!(GuardianForwarded, bytes),
        (CONTRACT_WARDEN, "timelock_forwarded") => from_archived!(TimelockForwarded, bytes),
        (CONTRACT_WARDEN, "atlas_cancel_forwarded") => from_archived!(AtlasCancelForwarded, bytes),
        _ => Err(DecodeError::UnknownEvent {
            contract: contract.to_owned(),
            topic: topic.to_owned(),
        }),
    }
}
