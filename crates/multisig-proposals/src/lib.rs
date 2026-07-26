// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Nocturne Standards

//! Multisig proposals: on-chain propose → approve → finalize+execute
//! (`call_raw`) using `multisig-registry` for membership/threshold.
//!
//! Signed message is the §4a digest from `multisig-encoding`. See README.md.

#![cfg_attr(target_family = "wasm", no_std)]
#![cfg(target_family = "wasm")]

#[cfg(not(any(feature = "contract", feature = "data-driver")))]
compile_error!("Enable either 'contract' or 'data-driver' feature for WASM builds");

extern crate alloc;
extern crate self as multisig_proposals;

pub(crate) mod call_types;

#[cfg(target_family = "wasm")]
mod state;
