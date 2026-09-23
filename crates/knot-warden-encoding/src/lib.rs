// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Nocturne Standards

//! Host-visible types for the warden contract.
//!
//! `Account` and `SetServiceArgs` are re-exported from `atlas-encoding` so a
//! `schedule_service` argument is the same bytes Atlas `set_service` accepts.

#![no_std]

extern crate alloc;

pub mod call_types;
pub mod events;
