// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 Nocturne Standards

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
pub mod dto;
pub mod store;

use std::sync::Arc;

use store::Store;

/// Default bind address when `MULTISIG_COLLECTOR_BIND` is unset.
pub const DEFAULT_BIND: &str = "127.0.0.1:8899";
/// Env var overriding the bind address.
pub const BIND_ENV: &str = "MULTISIG_COLLECTOR_BIND";
/// Opt-in escape hatch to bind outside loopback (reverse-proxy / lab only).
/// Must be exactly `1` — any other value (or unset) keeps the loopback guard.
pub const ALLOW_NON_LOOPBACK_ENV: &str = "MULTISIG_COLLECTOR_ALLOW_NON_LOOPBACK";

/// Default SQLite path when `MULTISIG_COLLECTOR_DB` is unset.
pub const DEFAULT_DB_PATH: &str = "./collector.sqlite";
/// Env var overriding the SQLite database path.
pub const DB_ENV: &str = "MULTISIG_COLLECTOR_DB";

/// Axum request body cap (bytes).
pub const MAX_BODY_BYTES: usize = 64 * 1024;
/// Max distinct partials stored per proposal.
pub const MAX_PARTIALS: usize = 32;
/// Max Unicode scalar values for party `note` / intent `human_summary`.
pub const MAX_NOTE_CHARS: usize = 512;
/// BLS signature length in bytes (`dusk_core::signatures::bls::Signature` /
/// `Serializable::SIZE` — confirmed 48; collector never verifies, only caps).
pub const BLS_SIG_BYTES: usize = 48;

/// Refuses non-loopback binds unless [`ALLOW_NON_LOOPBACK_ENV`]=`1`.
/// Loopback means `127.0.0.1:` or `localhost:` prefix (same style as
/// `multisig-tool`'s RPC guard).
pub fn assert_bind_allowed(bind: &str) -> Result<(), String> {
    let allow = matches!(std::env::var(ALLOW_NON_LOOPBACK_ENV), Ok(v) if v == "1");
    assert_bind_allowed_with(bind, allow)
}

/// Testable core of [`assert_bind_allowed`] — `allow_non_loopback` stands in
/// for `MULTISIG_COLLECTOR_ALLOW_NON_LOOPBACK=1`.
pub fn assert_bind_allowed_with(bind: &str, allow_non_loopback: bool) -> Result<(), String> {
    let loopback = bind.starts_with("127.0.0.1:") || bind.starts_with("localhost:");
    if loopback || allow_non_loopback {
        return Ok(());
    }
    Err(format!(
        "refusing to bind to '{bind}' — set {ALLOW_NON_LOOPBACK_ENV}=1 to allow non-loopback \
         (prefer reverse-proxy → 127.0.0.1; see README.md)"
    ))
}

#[cfg(test)]
mod bind_tests {
    use super::*;

    #[test]
    fn bind_allows_loopback() {
        assert!(assert_bind_allowed_with("127.0.0.1:8899", false).is_ok());
        assert!(assert_bind_allowed_with("localhost:8899", false).is_ok());
    }

    #[test]
    fn bind_rejects_non_loopback_without_escape() {
        let err = assert_bind_allowed_with("0.0.0.0:8899", false).expect_err("must refuse");
        assert!(err.contains("refusing to bind"), "{err}");
    }

    #[test]
    fn bind_allows_non_loopback_with_escape() {
        assert!(assert_bind_allowed_with("0.0.0.0:8899", true).is_ok());
    }
}

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
