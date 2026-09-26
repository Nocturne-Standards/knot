// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Nocturne Standards

//! Host-visible event payloads for registry and proposals logic.
//!
//! The book contracts do not emit. Topic strings are the first argument of
//! `abi::emit` in the logic contracts. Struct Archive types carry
//! `#[archive_attr(repr(C))]`. Payloads that nest [`RegistryPendingChange`]
//! keep that enum's own layout.

use alloc::string::String;
use alloc::vec::Vec;

use bytecheck::CheckBytes;
use dusk_core::abi::ContractId;
use dusk_core::signatures::bls::{PublicKey as BlsPublicKey, Signature as BlsSignature};
use rkyv::{Archive, Deserialize, Serialize};

use crate::call_types::RegistryPendingChange;

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[archive_attr(repr(C))]
#[cfg_attr(feature = "data-driver", derive(serde::Serialize, serde::Deserialize))]
pub struct DataSet {
    pub data: ContractId,
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[archive_attr(repr(C))]
#[cfg_attr(feature = "data-driver", derive(serde::Serialize, serde::Deserialize))]
pub struct AccountCreated {
    pub id: u64,
    pub members: Vec<BlsPublicKey>,
    pub threshold: u32,
    pub timelock_blocks: u64,
    pub nonce: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[archive_attr(repr(C))]
#[cfg_attr(feature = "data-driver", derive(serde::Serialize, serde::Deserialize))]
pub struct PendingScheduled {
    pub account_id: u64,
    pub execute_at: u64,
    pub change: RegistryPendingChange,
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[archive_attr(repr(C))]
#[cfg_attr(feature = "data-driver", derive(serde::Serialize, serde::Deserialize))]
pub struct AccountChanged {
    pub account_id: u64,
    pub members: Vec<BlsPublicKey>,
    pub threshold: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[archive_attr(repr(C))]
#[cfg_attr(feature = "data-driver", derive(serde::Serialize, serde::Deserialize))]
pub struct TimelockSet {
    pub account_id: u64,
    pub blocks: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[archive_attr(repr(C))]
#[cfg_attr(feature = "data-driver", derive(serde::Serialize, serde::Deserialize))]
pub struct PendingCancelled {
    pub account_id: u64,
    pub execute_at: u64,
    pub change: RegistryPendingChange,
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[archive_attr(repr(C))]
#[cfg_attr(feature = "data-driver", derive(serde::Serialize, serde::Deserialize))]
pub struct RegistrySet {
    pub registry: ContractId,
    pub epoch: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[archive_attr(repr(C))]
#[cfg_attr(feature = "data-driver", derive(serde::Serialize, serde::Deserialize))]
pub struct ProposalTtlSet {
    pub blocks: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[archive_attr(repr(C))]
#[cfg_attr(feature = "data-driver", derive(serde::Serialize, serde::Deserialize))]
pub struct TombstoneSet {
    pub tombstone: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[archive_attr(repr(C))]
#[cfg_attr(feature = "data-driver", derive(serde::Serialize, serde::Deserialize))]
pub struct AuthorizedAccountSet {
    pub account_id: u64,
    pub auth_generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[archive_attr(repr(C))]
#[cfg_attr(feature = "data-driver", derive(serde::Serialize, serde::Deserialize))]
pub struct ProposalCreated {
    pub proposal_id: u64,
    pub signed_digest: [u8; 32],
    pub registry_account_id: u64,
    pub deadline: u64,
    pub epoch: u64,
    pub nonce: u64,
    pub auth_generation: u64,
    pub target: ContractId,
    pub function_name: String,
    pub call_args: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[archive_attr(repr(C))]
#[cfg_attr(feature = "data-driver", derive(serde::Serialize, serde::Deserialize))]
pub struct ProposalApproved {
    pub proposal_id: u64,
    pub signed_digest: [u8; 32],
    pub signer: BlsPublicKey,
    pub signature: BlsSignature,
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[archive_attr(repr(C))]
#[cfg_attr(feature = "data-driver", derive(serde::Serialize, serde::Deserialize))]
pub struct ProposalFinalized {
    pub proposal_id: u64,
    pub signed_digest: [u8; 32],
    pub registry_account_id: u64,
    pub target: ContractId,
    pub function_name: String,
    pub call_args: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[archive_attr(repr(C))]
#[cfg_attr(feature = "data-driver", derive(serde::Serialize, serde::Deserialize))]
pub struct ProposalQueued {
    pub proposal_id: u64,
    pub signed_digest: [u8; 32],
    pub registry_account_id: u64,
    pub execute_at: u64,
    pub target: ContractId,
    pub function_name: String,
    pub call_args: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[archive_attr(repr(C))]
#[cfg_attr(feature = "data-driver", derive(serde::Serialize, serde::Deserialize))]
pub struct ProposalCancelled {
    pub proposal_id: u64,
    pub signed_digest: [u8; 32],
    pub registry_account_id: u64,
}

/// Ids removed by one `prune`. Same fields as [`crate::call_types::PruneReport`],
/// which is the book return. This is the event type so the contract macro
/// can register it next to the other payloads.
#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[archive_attr(repr(C))]
#[cfg_attr(feature = "data-driver", derive(serde::Serialize, serde::Deserialize))]
pub struct Pruned {
    pub proposal_ids: Vec<u64>,
    pub digest_keys: Vec<[u8; 32]>,
}

#[cfg(feature = "contract-events")]
mod topics {
    use dusk_forge::ContractEvent;

    use super::*;

    impl ContractEvent for DataSet {
        const TOPICS: &'static [&'static str] = &["data_set"];
    }
    impl ContractEvent for AccountCreated {
        const TOPICS: &'static [&'static str] = &["account_created"];
    }
    impl ContractEvent for PendingScheduled {
        const TOPICS: &'static [&'static str] = &["pending_scheduled"];
    }
    impl ContractEvent for AccountChanged {
        const TOPICS: &'static [&'static str] = &["account_changed"];
    }
    impl ContractEvent for TimelockSet {
        const TOPICS: &'static [&'static str] = &["timelock_set"];
    }
    impl ContractEvent for PendingCancelled {
        const TOPICS: &'static [&'static str] = &["pending_cancelled"];
    }
    impl ContractEvent for RegistrySet {
        const TOPICS: &'static [&'static str] = &["registry_set"];
    }
    impl ContractEvent for ProposalTtlSet {
        const TOPICS: &'static [&'static str] = &["proposal_ttl_set"];
    }
    impl ContractEvent for TombstoneSet {
        const TOPICS: &'static [&'static str] = &["tombstone_set"];
    }
    impl ContractEvent for AuthorizedAccountSet {
        const TOPICS: &'static [&'static str] = &["authorized_account_set"];
    }
    impl ContractEvent for ProposalCreated {
        const TOPICS: &'static [&'static str] = &["proposal_created"];
    }
    impl ContractEvent for ProposalApproved {
        const TOPICS: &'static [&'static str] = &["proposal_approved"];
    }
    impl ContractEvent for ProposalFinalized {
        const TOPICS: &'static [&'static str] = &["proposal_finalized"];
    }
    impl ContractEvent for ProposalQueued {
        const TOPICS: &'static [&'static str] = &["proposal_queued"];
    }
    impl ContractEvent for ProposalCancelled {
        const TOPICS: &'static [&'static str] = &["proposal_cancelled"];
    }
    impl ContractEvent for Pruned {
        const TOPICS: &'static [&'static str] = &["pruned"];
    }
}
