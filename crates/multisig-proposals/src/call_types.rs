//! Call argument / return types for `multisig-proposals` (v0.3).
//!
//! Canonical definitions live in `multisig-encoding` (spec 26 + Wave 7).
//! This module re-exports them so existing `#[path]` includes and contract
//! `use` paths keep working.

pub use multisig_encoding::call_types::{
    ApproveArgs, MultisigAccountView, ProposalStatus, ProposalView, ProposeArgs, SignatureEntry,
    VerifyQuorumArgs,
};
