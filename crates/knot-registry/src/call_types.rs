//! Call argument / return types for `knot-registry`.
//!
//! Canonical definitions live in `knot-encoding`.
//! This module re-exports them so existing `#[path]` includes and contract
//! `use` paths keep working.

#[allow(unused_imports)]
pub use knot_encoding::call_types::{
    ChangeAccountArgs, CreateAccountArgs, MultisigAccountView, SignatureEntry,
    VerifyQuorumAggregateArgs, VerifyQuorumArgs,
};
