// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Nocturne Standards

//! Multisig registry: BLS M-of-N quorum accounts other contracts can point
//! at instead of re-implementing committee/threshold logic themselves.
//!
//! Scope is deliberately narrow: this is a *verification* registry, not a
//! custody wallet — it never holds Dusk or any token (unlike Dusk's own
//! `multisig-contract` example, which does deposit/transfer). It answers
//! one question — "did enough of this account's members sign this
//! message?" — for callers like `prediction-market`'s dispute council or a
//! future `compliance-gate` operator council to build authorization on top
//! of. See README.md.

#![cfg_attr(target_family = "wasm", no_std)]
#![cfg(target_family = "wasm")]

#[cfg(not(any(feature = "contract", feature = "data-driver")))]
compile_error!("Enable either 'contract' or 'data-driver' feature for WASM builds");

extern crate alloc;
extern crate self as knot_registry;

pub(crate) mod call_types;

#[cfg(target_family = "wasm")]
mod state;
