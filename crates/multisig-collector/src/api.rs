// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 Leon Frenzel

//! HTTP routes: `GET /v1/health` plus the proposal/partial relay surface
//! (`/v1/proposals`, `/v1/proposals/:id`, `/v1/proposals/:id/partials`).
//! `/v1/party` lands in Task 6.
//!
//! This module never imports `SecretKey`/`sign_multisig` or `dusk_core` —
//! it only hex-decodes `signed_digest`/`signer_pk` far enough to validate
//! length and normalize case; it never verifies a signature or recomputes
//! the §4a digest (that anti-blind-signing check stays in `multisig-tool`,
//! which is trusted with keys — see `lib.rs` module doc).

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::json;

use crate::dto::{digest_to_id, normalize_hex, PartialDto, ProposalDto, DIGEST_BYTES, PK_BYTES};
use crate::store::{AppendOutcome, CreateOutcome};
use crate::AppState;

/// Builds the collector's router bound to `state`.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/proposals", post(create_proposal).get(list_proposals))
        .route("/v1/proposals/{id}", get(get_proposal))
        .route("/v1/proposals/{id}/partials", post(append_partial))
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

fn error_response(status: StatusCode, message: impl Into<String>) -> Response {
    (status, Json(json!({ "error": message.into() }))).into_response()
}

async fn create_proposal(
    State(state): State<AppState>,
    Json(mut dto): Json<ProposalDto>,
) -> Response {
    let digest = match normalize_hex(&dto.signed_digest, DIGEST_BYTES) {
        Ok(d) => d,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, format!("signed_digest: {e}")),
    };
    dto.signed_digest = digest.clone();
    // "partials empty or ignored on create" — never trust caller-supplied
    // partials at creation time; they must go through the append endpoint.
    dto.partials.clear();
    let id = digest_to_id(&digest);

    match state.store.create_proposal(&id, &digest, &dto) {
        Ok(CreateOutcome::Created) => (
            StatusCode::CREATED,
            Json(json!({ "id": id, "signed_digest": digest })),
        )
            .into_response(),
        Ok(CreateOutcome::AlreadyExists) => {
            Json(json!({ "id": id, "signed_digest": digest })).into_response()
        }
        Ok(CreateOutcome::Conflict) => error_response(
            StatusCode::CONFLICT,
            "a different proposal already exists under this content-addressed id",
        ),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

async fn list_proposals(State(state): State<AppState>) -> Response {
    match state.store.list_proposals() {
        Ok(summaries) => Json(summaries).into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

async fn get_proposal(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let id = id.to_ascii_lowercase();
    match state.store.get_proposal(&id) {
        Ok(Some(dto)) => Json(dto).into_response(),
        Ok(None) => error_response(StatusCode::NOT_FOUND, "no proposal with this id"),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

async fn append_partial(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(partial): Json<PartialDto>,
) -> Response {
    let id = id.to_ascii_lowercase();
    let signer_pk = match normalize_hex(&partial.signer_pk, PK_BYTES) {
        Ok(pk) => pk,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, format!("signer_pk: {e}")),
    };
    let sig_stripped = partial.sig.strip_prefix("0x").unwrap_or(&partial.sig);
    let sig_bytes = match hex::decode(sig_stripped) {
        Ok(b) => b,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, format!("sig: invalid hex: {e}")),
    };
    let sig = format!("0x{}", hex::encode(sig_bytes));

    match state
        .store
        .append_partial(&id, PartialDto { signer_pk, sig })
    {
        Ok(AppendOutcome::Appended(dto)) => Json(dto).into_response(),
        Ok(AppendOutcome::NotFound) => error_response(StatusCode::NOT_FOUND, "no proposal with this id"),
        Ok(AppendOutcome::DuplicatePk) => error_response(
            StatusCode::CONFLICT,
            "this signer_pk already has a partial on this proposal",
        ),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::IntentDto;
    use crate::store::Store;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    #[tokio::test]
    async fn health_ok_when_store_alive() {
        let store = Store::open_in_memory().expect("open store");
        let state = AppState::new(store);
        let app = router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    fn sample_dto(digest: &str) -> ProposalDto {
        ProposalDto {
            version: 1,
            intent: IntentDto {
                chain_id: 1,
                committee_id: 7,
                nonce: 3,
                target_contract_id: "0x11".to_string(),
                function_name: "set_service".to_string(),
                call_args: "0x0001".to_string(),
                deadline: 1000,
                human_summary: Some("hint".to_string()),
            },
            signed_digest: digest.to_string(),
            threshold: 2,
            partials: Vec::new(),
        }
    }

    async fn body_json(response: Response) -> serde_json::Value {
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    fn json_request(method: &str, uri: &str, body: serde_json::Value) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap()
    }

    #[tokio::test]
    async fn create_then_get_then_append_partial_then_list() {
        let store = Store::open_in_memory().expect("open store");
        let state = AppState::new(store);
        let app = router(state);

        let digest = format!("0x{}", "ab".repeat(32));
        let dto = sample_dto(&digest);

        let create_resp = app
            .clone()
            .oneshot(json_request(
                "POST",
                "/v1/proposals",
                serde_json::to_value(&dto).unwrap(),
            ))
            .await
            .unwrap();
        assert_eq!(create_resp.status(), StatusCode::CREATED);
        let created = body_json(create_resp).await;
        let id = created["id"].as_str().unwrap().to_string();
        assert_eq!(id.len(), 64);
        assert_eq!(id, id.to_ascii_lowercase());
        assert_eq!(created["signed_digest"], digest);

        let get_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/proposals/{id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(get_resp.status(), StatusCode::OK);
        let fetched = body_json(get_resp).await;
        assert_eq!(fetched["signed_digest"], digest);
        assert_eq!(fetched["partials"].as_array().unwrap().len(), 0);

        let pk = format!("0x{}", "11".repeat(96));
        let sig = format!("0x{}", "22".repeat(48));
        let partial_body = json!({ "signer_pk": pk, "sig": sig });

        let append_resp = app
            .clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/proposals/{id}/partials"),
                partial_body.clone(),
            ))
            .await
            .unwrap();
        assert_eq!(append_resp.status(), StatusCode::OK);
        let appended = body_json(append_resp).await;
        assert_eq!(appended["partials"].as_array().unwrap().len(), 1);
        // Appending a partial must never mutate the stored digest.
        assert_eq!(appended["signed_digest"], digest);

        let dup_resp = app
            .clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/proposals/{id}/partials"),
                partial_body,
            ))
            .await
            .unwrap();
        assert_eq!(dup_resp.status(), StatusCode::CONFLICT);

        let list_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/proposals")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(list_resp.status(), StatusCode::OK);
        let list = body_json(list_resp).await;
        let arr = list.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["id"], id);
        assert_eq!(arr[0]["partials_count"], 1);
    }

    #[tokio::test]
    async fn get_unknown_id_is_404() {
        let store = Store::open_in_memory().expect("open store");
        let state = AppState::new(store);
        let app = router(state);

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/v1/proposals/deadbeef")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn append_partial_to_unknown_id_is_404() {
        let store = Store::open_in_memory().expect("open store");
        let state = AppState::new(store);
        let app = router(state);

        let pk = format!("0x{}", "11".repeat(96));
        let sig = format!("0x{}", "22".repeat(48));
        let resp = app
            .oneshot(json_request(
                "POST",
                "/v1/proposals/deadbeef/partials",
                json!({ "signer_pk": pk, "sig": sig }),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn create_rejects_bad_digest_hex() {
        let store = Store::open_in_memory().expect("open store");
        let state = AppState::new(store);
        let app = router(state);

        let mut dto = sample_dto(&format!("0x{}", "ab".repeat(32)));
        dto.signed_digest = "not-hex".to_string();

        let resp = app
            .oneshot(json_request(
                "POST",
                "/v1/proposals",
                serde_json::to_value(&dto).unwrap(),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn create_ignores_caller_supplied_partials() {
        let store = Store::open_in_memory().expect("open store");
        let state = AppState::new(store);
        let app = router(state);

        let digest = format!("0x{}", "ab".repeat(32));
        let mut dto = sample_dto(&digest);
        dto.partials.push(PartialDto {
            signer_pk: format!("0x{}", "11".repeat(96)),
            sig: format!("0x{}", "22".repeat(48)),
        });

        let create_resp = app
            .clone()
            .oneshot(json_request(
                "POST",
                "/v1/proposals",
                serde_json::to_value(&dto).unwrap(),
            ))
            .await
            .unwrap();
        let created = body_json(create_resp).await;
        let id = created["id"].as_str().unwrap().to_string();

        let get_resp = app
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/proposals/{id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let fetched = body_json(get_resp).await;
        assert_eq!(fetched["partials"].as_array().unwrap().len(), 0);
    }
}
