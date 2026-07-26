// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 Nocturne Standards

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
    assert_eq!(body["ok"], true);
    assert_eq!(body["version"], env!("CARGO_PKG_VERSION"));
}

/// Same real-listener setup as above, but exercises the full
/// create → get → append partial → list flow over actual HTTP requests
/// (not `tower::Service::oneshot`), proving the router + JSON wiring for
/// the whole proposal surface, not just handler logic.
#[tokio::test]
async fn proposal_lifecycle_over_real_http() {
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

    let base = format!("http://{addr}");
    let client = reqwest::Client::new();

    let digest = format!("0x{}", "ab".repeat(32));
    let create_body = serde_json::json!({
        "version": 1,
        "intent": {
            "chain_id": 1,
            "committee_id": 7,
            "nonce": 3,
            "target_contract_id": "0x11",
            "function_name": "set_service",
            "call_args": "0x0001",
            "deadline": 1000,
            "human_summary": "hint"
        },
        "signed_digest": digest,
        "threshold": 2,
        "partials": []
    });

    let create_resp = client
        .post(format!("{base}/v1/proposals"))
        .json(&create_body)
        .send()
        .await
        .expect("POST /v1/proposals");
    assert_eq!(create_resp.status(), reqwest::StatusCode::CREATED);
    let created: serde_json::Value = create_resp.json().await.expect("parse create body");
    let id = created["id"].as_str().expect("id field").to_string();
    assert_eq!(id.len(), 64, "id must be lowercase hex of 32-byte digest");

    let get_resp = client
        .get(format!("{base}/v1/proposals/{id}"))
        .send()
        .await
        .expect("GET /v1/proposals/:id");
    assert_eq!(get_resp.status(), reqwest::StatusCode::OK);
    let fetched: serde_json::Value = get_resp.json().await.expect("parse get body");
    assert_eq!(fetched["signed_digest"], digest);
    assert_eq!(fetched["partials"].as_array().unwrap().len(), 0);

    let pk = format!("0x{}", "11".repeat(96));
    let sig = format!("0x{}", "22".repeat(48));
    let append_resp = client
        .post(format!("{base}/v1/proposals/{id}/partials"))
        .json(&serde_json::json!({ "signer_pk": pk, "sig": sig }))
        .send()
        .await
        .expect("POST /v1/proposals/:id/partials");
    assert_eq!(append_resp.status(), reqwest::StatusCode::OK);
    let appended: serde_json::Value = append_resp.json().await.expect("parse append body");
    assert_eq!(appended["partials"].as_array().unwrap().len(), 1);
    assert_eq!(appended["signed_digest"], digest, "append must not mutate digest");

    let list_resp = client
        .get(format!("{base}/v1/proposals"))
        .send()
        .await
        .expect("GET /v1/proposals");
    assert_eq!(list_resp.status(), reqwest::StatusCode::OK);
    let list: serde_json::Value = list_resp.json().await.expect("parse list body");
    let arr = list.as_array().expect("list is an array");
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["id"], id);
    assert_eq!(arr[0]["partials_count"], 1);
}

/// Real-listener coverage for the party-finder roster: signup, upsert-by-pk,
/// list — DELETE is intentionally absent (operator clears DB if needed).
#[tokio::test]
async fn party_roster_lifecycle_over_real_http() {
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

    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let pk = format!("0x{}", "55".repeat(96));

    let signup_resp = client
        .post(format!("{base}/v1/party"))
        .json(&serde_json::json!({ "name": "Alice", "pk": pk, "note": "demo" }))
        .send()
        .await
        .expect("POST /v1/party");
    assert_eq!(signup_resp.status(), reqwest::StatusCode::OK);

    let update_resp = client
        .post(format!("{base}/v1/party"))
        .json(&serde_json::json!({ "name": "Alice Renamed", "pk": pk }))
        .send()
        .await
        .expect("POST /v1/party (upsert)");
    assert_eq!(update_resp.status(), reqwest::StatusCode::OK);

    let list_resp = client
        .get(format!("{base}/v1/party"))
        .send()
        .await
        .expect("GET /v1/party");
    assert_eq!(list_resp.status(), reqwest::StatusCode::OK);
    let list: serde_json::Value = list_resp.json().await.expect("parse party list body");
    let arr = list.as_array().expect("party list is an array");
    assert_eq!(arr.len(), 1, "upsert must not duplicate the roster row");
    assert_eq!(arr[0]["name"], "Alice Renamed");
    assert_eq!(arr[0]["pk"], pk);

    let delete_resp = client
        .delete(format!("{base}/v1/party/{pk}"))
        .send()
        .await
        .expect("DELETE /v1/party/:pk");
    assert_eq!(
        delete_resp.status(),
        reqwest::StatusCode::NOT_FOUND,
        "DELETE route must be absent"
    );
}
