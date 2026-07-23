// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 Leon Frenzel

//! Smoke test: spin the real axum router on an ephemeral port backed by an
//! in-memory SQLite store, then hit `GET /v1/health` over a real HTTP
//! connection (not a `tower::Service::oneshot` in-process call) — proves the
//! binary's actual listener + JSON wiring, not just handler logic.

use multisig_collector::{store::Store, AppState};

#[tokio::test]
async fn health_endpoint_returns_ok_true() {
    let store = Store::open_in_memory().expect("open in-memory sqlite store");
    let state = AppState::new(store);
    let app = multisig_collector::api::router(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let addr = listener.local_addr().expect("local addr");

    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve");
    });

    let url = format!("http://{addr}/v1/health");
    let resp = reqwest::get(&url).await.expect("GET /v1/health");
    assert_eq!(resp.status(), reqwest::StatusCode::OK);

    let body: serde_json::Value = resp.json().await.expect("parse json body");
    assert_eq!(body, serde_json::json!({ "ok": true }));
}
