// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Nocturne Standards

//! Warden: the delaying guardian in front of Atlas.
//!
//! Atlas's guardian slot points here. This contract calls Atlas. Atlas does
//! not call it. `delay_blocks` waits on service repoints and on replacing
//! the scheduler or the delay. `set_guardian`, `set_timelock`, and
//! `cancel_atlas_pending` are forwarded in the same transaction; Atlas
//! applies its own delay to the first two.
//!
//! Spec: `docs/warden.md`.

#![cfg_attr(target_family = "wasm", no_std)]
#![cfg(target_family = "wasm")]

#[cfg(not(any(feature = "contract", feature = "data-driver")))]
compile_error!("Enable either 'contract' or 'data-driver' feature for WASM builds");

extern crate alloc;

#[cfg(target_family = "wasm")]
mod state;
