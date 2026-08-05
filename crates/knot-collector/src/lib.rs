// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 Nocturne Standards

//! Library surface for `knot-collector` — an untrusted off-chain
//! signature/proposal relay (Safe Transaction Service analogue). Holds only
//! serialized blobs: no keystore, no BLS signing, no wallet, no chain
//! submit. See `docs/superpowers/specs/2026-07-23-knot-collector-monorepo-demo-design.md`
//! §2 for the full trust model and API surface.
//!
//! The collector **never holds secret keys, never signs, and never submits
//! on-chain transactions**. It may verify public BLS signatures and
//! recompute digests via `knot-encoding` so it cannot be used as an
//! unauthenticated griefing relay.

pub mod api;
pub mod dto;
pub mod gate;
pub mod store;
pub mod verify;

use std::str::FromStr;
use std::sync::Arc;

use store::Store;

/// Default bind address when `KNOT_COLLECTOR_BIND` is unset.
pub const DEFAULT_BIND: &str = "127.0.0.1:8899";
/// Env var overriding the bind address.
pub const BIND_ENV: &str = "KNOT_COLLECTOR_BIND";
/// Opt-in escape hatch to bind outside loopback (reverse-proxy / lab only).
/// Must be exactly `1` — any other value (or unset) keeps the loopback guard.
pub const ALLOW_NON_LOOPBACK_ENV: &str = "KNOT_COLLECTOR_ALLOW_NON_LOOPBACK";

/// Default SQLite path when `KNOT_COLLECTOR_DB` is unset.
pub const DEFAULT_DB_PATH: &str = "./collector.sqlite";
/// Env var overriding the SQLite database path.
pub const DB_ENV: &str = "KNOT_COLLECTOR_DB";

/// Axum request body cap (bytes).
pub const MAX_BODY_BYTES: usize = 64 * 1024;
/// Max distinct partials stored per proposal.
pub const MAX_PARTIALS: usize = 32;
/// Max Unicode scalar values for party `name` / `note` / intent `human_summary`.
pub const MAX_NOTE_CHARS: usize = 512;
/// BLS signature length in bytes (`dusk_core::signatures::bls::Signature` /
/// `Serializable::SIZE` — confirmed 48).
pub const BLS_SIG_BYTES: usize = 48;
/// Default `GET /v1/proposals` / `GET /v1/party` page size (M11).
pub const DEFAULT_LIST_LIMIT: u32 = 50;
/// Hard cap on `?limit` query parameter (M11).
pub const MAX_LIST_LIMIT: u32 = 200;
/// Max stored proposal rows before new creates are rejected (M11).
pub const MAX_PROPOSAL_ROWS: usize = 10_000;
/// Max party roster rows before new signups are rejected (M11).
pub const MAX_PARTY_ROWS: usize = 1_000;
/// Proposal rows older than this many seconds may be swept (M11 TTL).
pub const PROPOSAL_RETENTION_SECS: i64 = 90 * 24 * 3600;

/// Refuses non-loopback binds unless [`ALLOW_NON_LOOPBACK_ENV`]=`1`.
/// Parses `bind` as a [`std::net::SocketAddr`] and requires a loopback IP.
pub fn assert_bind_allowed(bind: &str) -> Result<(), String> {
    let allow = matches!(std::env::var(ALLOW_NON_LOOPBACK_ENV), Ok(v) if v == "1");
    assert_bind_allowed_with(bind, allow)
}

/// Testable core of [`assert_bind_allowed`] — `allow_non_loopback` stands in
/// for `KNOT_COLLECTOR_ALLOW_NON_LOOPBACK=1`.
pub fn assert_bind_allowed_with(bind: &str, allow_non_loopback: bool) -> Result<(), String> {
    let loopback = std::net::SocketAddr::from_str(bind)
        .map(|addr| addr.ip().is_loopback())
        .unwrap_or(false);
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
        assert!(assert_bind_allowed_with("[::1]:8899", false).is_ok());
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
