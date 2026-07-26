// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 Nocturne Standards

//! `multisig-collector` binary entry: reads `MULTISIG_COLLECTOR_BIND` /
//! `MULTISIG_COLLECTOR_DB`, opens the SQLite store, and serves the axum
//! router — see `lib.rs` / `api.rs` for the route surface.

use std::path::PathBuf;

use anyhow::{Context, Result};
use multisig_collector::{
    assert_bind_allowed, store::Store, AppState, BIND_ENV, DB_ENV, DEFAULT_BIND, DEFAULT_DB_PATH,
};
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let bind = std::env::var(BIND_ENV).unwrap_or_else(|_| DEFAULT_BIND.to_string());
    assert_bind_allowed(&bind).map_err(anyhow::Error::msg)?;
    let db_path = std::env::var(DB_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(DEFAULT_DB_PATH));

    let store = Store::open(&db_path)
        .with_context(|| format!("open sqlite database {}", db_path.display()))?;
    info!(path = %db_path.display(), "sqlite store opened");

    let state = AppState::new(store);
    let app = multisig_collector::api::router(state);

    let listener = tokio::net::TcpListener::bind(&bind)
        .await
        .with_context(|| format!("bind {bind}"))?;
    info!(%bind, "multisig-collector listening");

    axum::serve(listener, app)
        .await
        .context("axum serve failed")?;

    Ok(())
}
