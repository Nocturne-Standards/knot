//! Call argument / return types for `multisig-registry`.
//!
//! Canonical definitions live in `multisig-encoding` (spec 26 + Wave 7).
//! This module re-exports them so existing `#[path]` includes and contract
//! `use` paths keep working.

pub use multisig_encoding::call_types::{
    AccountMeta, ChangeAccountArgs, CreateAccountArgs, DiagnoseQuorumResult, MultisigAccountView,
    SignatureEntry, VerifyQuorumAggregateArgs, VerifyQuorumArgs,
};
