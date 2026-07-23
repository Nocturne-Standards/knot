// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 Leon Frenzel

//! HTTP routes. Only `GET /v1/health` this task — `/v1/proposals` and
//! `/v1/party` land in Tasks 5–6 per the design doc §2.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde_json::json;

use crate::AppState;

/// Builds the collector's router bound to `state`.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/v1/health", get(health))
        .with_state(state)
}

async fn health(State(state): State<AppState>) -> Response {
    if !state.store.is_alive() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "ok": false })),
        )
            .into_response();
    }
    Json(json!({ "ok": true })).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Store;
    use tower::ServiceExt;

    #[tokio::test]
    async fn health_ok_when_store_alive() {
        let store = Store::open_in_memory().expect("open store");
        let state = AppState::new(store);
        let app = router(state);

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/v1/health")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}
