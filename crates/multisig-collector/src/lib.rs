// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 Leon Frenzel

//! Library surface for `multisig-collector` — an untrusted off-chain
//! signature/proposal relay (Safe Transaction Service analogue). Holds only
//! serialized blobs: no keystore, no BLS signing, no wallet, no chain
//! submit. See `docs/superpowers/specs/2026-07-23-multisig-collector-monorepo-demo-design.md`
//! §2 for the full trust model and API surface.
//!
//! This crate intentionally has **no** dependency on `dusk_core` or any BLS
//! secret-key type — compromise of this service can at worst cause a DoS or
//! a rejected (digest-mismatched) blob, never a forged signature.

pub mod api;
pub mod store;

use std::sync::Arc;

use store::Store;

/// Default bind address when `MULTISIG_COLLECTOR_BIND` is unset.
pub const DEFAULT_BIND: &str = "127.0.0.1:8899";
/// Env var overriding the bind address.
pub const BIND_ENV: &str = "MULTISIG_COLLECTOR_BIND";

/// Default SQLite path when `MULTISIG_COLLECTOR_DB` is unset.
pub const DEFAULT_DB_PATH: &str = "./collector.sqlite";
/// Env var overriding the SQLite database path.
pub const DB_ENV: &str = "MULTISIG_COLLECTOR_DB";

/// Shared axum state — cheap to clone (single `Arc`).
#[derive(Clone)]
pub struct AppState {
    pub store: Arc<Store>,
}

impl AppState {
    pub fn new(store: Store) -> Self {
        Self {
            store: Arc::new(store),
        }
    }
}
