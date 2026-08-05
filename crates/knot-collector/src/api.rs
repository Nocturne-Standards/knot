// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 Nocturne Standards

//! HTTP routes: `GET /v1/health`, the proposal/partial relay surface
//! (`/v1/proposals`, `/v1/proposals/:id`, `/v1/proposals/:id/partials`), and
//! the party-finder roster (`/v1/party` — upsert-only; no DELETE).
//!
//! The collector never holds secret keys or signs on-chain transactions. It
//! recomputes proposal digests (`gate`) and verifies BLS signatures on
//! partials and party signup (`verify`) so the relay cannot grief members.

use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::json;

use crate::dto::{
    digest_to_id, normalize_hex, normalize_pk, IntentDto, PartialDto, PartySignupDto, ProposalDto,
    DIGEST_BYTES, PK_BYTES,
};
use crate::gate::gate_proposal_digest;
use crate::store::{AppendOutcome, CreateOutcome};
use crate::verify::{party_signup_preimage, verify_bls_partial, verify_bls_standard};
use crate::{
    AppState, BLS_SIG_BYTES, DEFAULT_LIST_LIMIT, MAX_BODY_BYTES, MAX_LIST_LIMIT, MAX_NOTE_CHARS,
};

/// Builds the collector's router bound to `state`.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/proposals", post(create_proposal).get(list_proposals))
        .route("/v1/proposals/{id}", get(get_proposal))
        .route("/v1/proposals/{id}/partials", post(append_partial))
        .route("/v1/party", get(list_party).post(signup_party))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .with_state(state)
}

async fn health(State(state): State<AppState>) -> Response {
    if !state.store.is_alive() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "ok": false,
                "version": env!("CARGO_PKG_VERSION"),
            })),
        )
            .into_response();
    }
    Json(json!({
        "ok": true,
        "version": env!("CARGO_PKG_VERSION"),
    }))
    .into_response()
}

fn error_response(status: StatusCode, message: impl Into<String>) -> Response {
    (status, Json(json!({ "error": message.into() }))).into_response()
}

fn internal_error(e: impl std::fmt::Display) -> Response {
    tracing::error!("internal error: {e}");
    error_response(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
}

#[derive(Debug, serde::Deserialize)]
struct ListQuery {
    limit: Option<u32>,
    offset: Option<u32>,
}

fn list_params(query: ListQuery) -> Result<(u32, u32), String> {
    let limit = query.limit.unwrap_or(DEFAULT_LIST_LIMIT);
    let offset = query.offset.unwrap_or(0);
    if limit == 0 || limit > MAX_LIST_LIMIT {
        return Err(format!("limit must be 1..={MAX_LIST_LIMIT}"));
    }
    Ok((limit, offset))
}

fn decode_sig_bytes(sig: &str) -> Result<Vec<u8>, String> {
    let sig_stripped = sig.strip_prefix("0x").unwrap_or(sig);
    let sig_bytes = hex::decode(sig_stripped)
        .map_err(|e| format!("sig: invalid hex: {e}"))?;
    if sig_bytes.len() != BLS_SIG_BYTES {
        return Err(format!(
            "sig: expected {BLS_SIG_BYTES} bytes (BLS signature), got {}",
            sig_bytes.len()
        ));
    }
    Ok(sig_bytes)
}

/// Content-addressed proposal ids are exactly 64 hex chars (32-byte digest).
fn validate_proposal_id(id: &str) -> Result<String, String> {
    let id = id.to_ascii_lowercase();
    if id.len() != 64 || !id.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("id must be exactly 64 hex characters".into());
    }
    Ok(id)
}

fn reject_overlong_text(label: &str, text: &str) -> Result<(), String> {
    let chars = text.chars().count();
    if chars > MAX_NOTE_CHARS {
        return Err(format!(
            "{label}: max {MAX_NOTE_CHARS} characters, got {chars}"
        ));
    }
    Ok(())
}

fn check_summary_caps(dto: &ProposalDto) -> Result<(), String> {
    match &dto.intent {
        IntentDto::Proposals(i) => {
            if let Some(s) = &i.human_summary {
                reject_overlong_text("human_summary", s)?;
            }
        }
        IntentDto::PmCouncilResolve(i) => {
            if let Some(s) = &i.human_summary {
                reject_overlong_text("human_summary", s)?;
            }
        }
    }
    Ok(())
}

async fn create_proposal(
    State(state): State<AppState>,
    Json(mut dto): Json<ProposalDto>,
) -> Response {
    if let Err(e) = gate_proposal_digest(&dto) {
        return error_response(StatusCode::BAD_REQUEST, e);
    }
    let digest = match normalize_hex(&dto.signed_digest, DIGEST_BYTES) {
        Ok(d) => d,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, format!("signed_digest: {e}")),
    };
    dto.signed_digest = digest.clone();
    if let Err(e) = check_summary_caps(&dto) {
        return error_response(StatusCode::BAD_REQUEST, e);
    }
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
        Ok(CreateOutcome::RowCapReached) => error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "proposal storage cap reached — try again after TTL sweep or operator cleanup",
        ),
        Err(e) => internal_error(e),
    }
}

async fn list_proposals(State(state): State<AppState>, Query(query): Query<ListQuery>) -> Response {
    let (limit, offset) = match list_params(query) {
        Ok(p) => p,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, e),
    };
    match state.store.list_proposals(limit, offset) {
        Ok(summaries) => Json(summaries).into_response(),
        Err(e) => internal_error(e),
    }
}

async fn get_proposal(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let id = match validate_proposal_id(&id) {
        Ok(id) => id,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, e),
    };
    match state.store.get_proposal(&id) {
        Ok(Some(dto)) => Json(dto).into_response(),
        Ok(None) => error_response(StatusCode::NOT_FOUND, "no proposal with this id"),
        Err(e) => internal_error(e),
    }
}

async fn append_partial(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(partial): Json<PartialDto>,
) -> Response {
    let id = match validate_proposal_id(&id) {
        Ok(id) => id,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, e),
    };
    let signer_pk = match normalize_hex(&partial.signer_pk, PK_BYTES) {
        Ok(pk) => pk,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, format!("signer_pk: {e}")),
    };
    let sig_bytes = match decode_sig_bytes(&partial.sig) {
        Ok(b) => b,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, e),
    };

    let stored_digest = match state.store.get_proposal(&id) {
        Ok(Some(dto)) => dto.signed_digest,
        Ok(None) => return error_response(StatusCode::NOT_FOUND, "no proposal with this id"),
        Err(e) => return internal_error(e),
    };
    let digest_bytes: [u8; 32] = match normalize_hex(&stored_digest, DIGEST_BYTES) {
        Ok(d) => {
            let stripped = d.strip_prefix("0x").unwrap_or(&d);
            let raw = hex::decode(stripped).expect("stored digest normalized");
            raw.as_slice()
                .try_into()
                .expect("stored digest is 32 bytes")
        }
        Err(e) => return internal_error(format!("stored digest corrupt: {e}")),
    };
    let pk_stripped = signer_pk.strip_prefix("0x").unwrap_or(&signer_pk);
    let pk_bytes: [u8; 96] = hex::decode(pk_stripped)
        .expect("normalized pk")
        .as_slice()
        .try_into()
        .expect("pk is 96 bytes");
    if !verify_bls_partial(&pk_bytes, &digest_bytes, &sig_bytes) {
        return error_response(StatusCode::BAD_REQUEST, "sig: BLS verification failed");
    }
    let sig = format!("0x{}", hex::encode(&sig_bytes));

    match state
        .store
        .append_partial(&id, PartialDto { signer_pk, sig })
    {
        Ok(AppendOutcome::Appended(dto)) => Json(dto).into_response(),
        Ok(AppendOutcome::NotFound) => {
            error_response(StatusCode::NOT_FOUND, "no proposal with this id")
        }
        Ok(AppendOutcome::TooManyPartials) => error_response(
            StatusCode::BAD_REQUEST,
            format!("proposal already has the maximum of {} partials", crate::MAX_PARTIALS),
        ),
        Err(e) => internal_error(e),
    }
}

async fn list_party(State(state): State<AppState>, Query(query): Query<ListQuery>) -> Response {
    let (limit, offset) = match list_params(query) {
        Ok(p) => p,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, e),
    };
    match state.store.list_party(limit, offset) {
        Ok(members) => Json(members).into_response(),
        Err(e) => internal_error(e),
    }
}

async fn signup_party(
    State(state): State<AppState>,
    Json(dto): Json<PartySignupDto>,
) -> Response {
    let pk = match normalize_pk(&dto.pk) {
        Ok(pk) => pk,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, format!("pk: {e}")),
    };
    let name = dto.name.trim();
    if name.is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "name must not be empty");
    }
    if let Err(e) = reject_overlong_text("name", name) {
        return error_response(StatusCode::BAD_REQUEST, e);
    }
    if let Some(note) = &dto.note
        && let Err(e) = reject_overlong_text("note", note)
    {
        return error_response(StatusCode::BAD_REQUEST, e);
    }
    let sig_bytes = match decode_sig_bytes(&dto.sig) {
        Ok(b) => b,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, e),
    };
    let pk_stripped = pk.strip_prefix("0x").unwrap_or(&pk);
    let pk_bytes: [u8; 96] = hex::decode(pk_stripped)
        .expect("normalized pk")
        .as_slice()
        .try_into()
        .expect("pk is 96 bytes");
    let preimage = party_signup_preimage(name, &pk_bytes);
    if !verify_bls_standard(&pk_bytes, &preimage, &sig_bytes) {
        return error_response(StatusCode::BAD_REQUEST, "sig: BLS verification failed");
    }

    match state
        .store
        .upsert_party_member(&pk, name, dto.note.as_deref())
    {
        Ok(member) => Json(member).into_response(),
        Err(e) => internal_error(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::{BlobKind, ProposalsIntentDto};
    use crate::store::Store;
    use crate::verify::party_signup_preimage;
    use axum::body::Body;
    use axum::http::Request;
    use dusk_bytes::Serializable;
    use dusk_core::signatures::bls::{
        PublicKey as BlsPublicKey, SecretKey as BlsSecretKey,
    };
    use knot_encoding::{ProposalIntent, proposal_digest};
    use rand::rngs::StdRng;
    use rand::SeedableRng;
    use tower::ServiceExt;

    fn keypair(rng: &mut StdRng) -> (BlsSecretKey, BlsPublicKey) {
        let sk = BlsSecretKey::random(rng);
        let pk = BlsPublicKey::from(&sk);
        (sk, pk)
    }

    fn sample_intent() -> ProposalIntent {
        ProposalIntent {
            chain_id: 1,
            committee_id: 7,
            nonce: 3,
            target_contract_id: [0x11; 32],
            function_name: "set_service".to_string(),
            call_args: vec![0x00, 0x01],
            deadline: 1000,
        }
    }

    fn sample_dto() -> ProposalDto {
        let intent = sample_intent();
        let digest_bytes = proposal_digest(
            intent.chain_id,
            intent.committee_id,
            intent.nonce,
            &intent.target_contract_id,
            intent.function_name.as_bytes(),
            &intent.call_args,
            intent.deadline,
        )
        .expect("digest");
        let digest = format!("0x{}", hex::encode(digest_bytes));
        ProposalDto {
            version: 1,
            kind: BlobKind::Proposals,
            intent: IntentDto::Proposals(ProposalsIntentDto {
                chain_id: intent.chain_id,
                committee_id: intent.committee_id,
                nonce: intent.nonce,
                target_contract_id: format!("0x{}", hex::encode(intent.target_contract_id)),
                function_name: intent.function_name.clone(),
                call_args: format!("0x{}", hex::encode(&intent.call_args)),
                deadline: intent.deadline,
                human_summary: Some("hint".to_string()),
            }),
            signed_digest: digest,
            threshold: 2,
            partials: Vec::new(),
        }
    }

    fn digest_bytes_from_dto(dto: &ProposalDto) -> [u8; 32] {
        let stripped = dto.signed_digest.strip_prefix("0x").unwrap_or(&dto.signed_digest);
        let raw = hex::decode(stripped).expect("digest hex");
        raw.as_slice().try_into().expect("32 bytes")
    }

    fn sign_partial(sk: &BlsSecretKey, pk: &BlsPublicKey, digest: &[u8; 32]) -> String {
        let sig = sk.sign_multisig(pk, digest);
        format!("0x{}", hex::encode(sig.to_bytes()))
    }

    fn sign_party(sk: &BlsSecretKey, pk: &BlsPublicKey, name: &str) -> String {
        let pk_bytes = pk.to_bytes();
        let preimage = party_signup_preimage(name, &pk_bytes);
        let sig = sk.sign(&preimage);
        format!("0x{}", hex::encode(sig.to_bytes()))
    }

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
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["ok"], true);
        assert_eq!(json["version"], env!("CARGO_PKG_VERSION"));
    }

    fn sample_dto_legacy(_digest: &str) -> ProposalDto {
        sample_dto()
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
        let mut rng = StdRng::seed_from_u64(42);

        let dto = sample_dto();
        let digest = dto.signed_digest.clone();
        let digest_bytes = digest_bytes_from_dto(&dto);

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

        let (sk, pk) = keypair(&mut rng);
        let pk_hex = format!("0x{}", hex::encode(pk.to_bytes()));
        let sig = sign_partial(&sk, &pk, &digest_bytes);
        let partial_body = json!({ "signer_pk": pk_hex, "sig": sig });

        let append_resp = app
            .clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/proposals/{id}/partials"),
                partial_body,
            ))
            .await
            .unwrap();
        assert_eq!(append_resp.status(), StatusCode::OK);
        let appended = body_json(append_resp).await;
        assert_eq!(appended["partials"].as_array().unwrap().len(), 1);
        assert_eq!(appended["signed_digest"], digest);

        // Last-write-wins: same pk with a different valid sig replaces (200).
        let sig_later = sign_partial(&sk, &pk, &digest_bytes);
        let replace_resp = app
            .clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/proposals/{id}/partials"),
                json!({ "signer_pk": pk_hex, "sig": sig_later }),
            ))
            .await
            .unwrap();
        assert_eq!(replace_resp.status(), StatusCode::OK);
        let replaced = body_json(replace_resp).await;
        assert_eq!(replaced["partials"].as_array().unwrap().len(), 1);
        assert_eq!(replaced["partials"][0]["sig"], sig_later);
        assert_eq!(replaced["signed_digest"], digest);

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
    async fn append_rejects_wrong_sig_length() {
        let store = Store::open_in_memory().expect("open store");
        let state = AppState::new(store);
        let app = router(state);

        let dto = sample_dto();
        let create_resp = app
            .clone()
            .oneshot(json_request(
                "POST",
                "/v1/proposals",
                serde_json::to_value(&dto).unwrap(),
            ))
            .await
            .unwrap();
        let id = body_json(create_resp).await["id"].as_str().unwrap().to_string();

        let pk = format!("0x{}", "11".repeat(96));
        let bad_sig = format!("0x{}", "22".repeat(47)); // 47 bytes, not 48
        let resp = app
            .oneshot(json_request(
                "POST",
                &format!("/v1/proposals/{id}/partials"),
                json!({ "signer_pk": pk, "sig": bad_sig }),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn append_rejects_invalid_bls_sig() {
        let store = Store::open_in_memory().expect("open store");
        let state = AppState::new(store);
        let app = router(state);

        let dto = sample_dto();
        let create_resp = app
            .clone()
            .oneshot(json_request(
                "POST",
                "/v1/proposals",
                serde_json::to_value(&dto).unwrap(),
            ))
            .await
            .unwrap();
        let id = body_json(create_resp).await["id"].as_str().unwrap().to_string();

        let pk = format!("0x{}", "11".repeat(96));
        let junk_sig = format!("0x{}", "22".repeat(48));
        let resp = app
            .oneshot(json_request(
                "POST",
                &format!("/v1/proposals/{id}/partials"),
                json!({ "signer_pk": pk, "sig": junk_sig }),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn append_rejects_33rd_distinct_pk() {
        let store = Store::open_in_memory().expect("open store");
        let state = AppState::new(store);
        let app = router(state);
        let mut rng = StdRng::seed_from_u64(99);

        let dto = sample_dto();
        let digest_bytes = digest_bytes_from_dto(&dto);
        let create_resp = app
            .clone()
            .oneshot(json_request(
                "POST",
                "/v1/proposals",
                serde_json::to_value(&dto).unwrap(),
            ))
            .await
            .unwrap();
        let id = body_json(create_resp).await["id"].as_str().unwrap().to_string();

        for i in 0..crate::MAX_PARTIALS {
            let (sk, pk) = keypair(&mut rng);
            let pk_hex = format!("0x{}", hex::encode(pk.to_bytes()));
            let sig = sign_partial(&sk, &pk, &digest_bytes);
            let resp = app
                .clone()
                .oneshot(json_request(
                    "POST",
                    &format!("/v1/proposals/{id}/partials"),
                    json!({ "signer_pk": pk_hex, "sig": sig }),
                ))
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::OK, "pk #{i}");
        }

        let (overflow_sk, overflow_pk) = keypair(&mut rng);
        let overflow_pk_hex = format!("0x{}", hex::encode(overflow_pk.to_bytes()));
        let overflow = app
            .clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/proposals/{id}/partials"),
                json!({
                    "signer_pk": overflow_pk_hex,
                    "sig": sign_partial(&overflow_sk, &overflow_pk, &digest_bytes),
                }),
            ))
            .await
            .unwrap();
        assert_eq!(overflow.status(), StatusCode::BAD_REQUEST);

        // Replace of an existing pk still succeeds at the cap.
        let (sk0, pk0) = keypair(&mut StdRng::seed_from_u64(99));
        let replace = app
            .oneshot(json_request(
                "POST",
                &format!("/v1/proposals/{id}/partials"),
                json!({
                    "signer_pk": format!("0x{}", hex::encode(pk0.to_bytes())),
                    "sig": sign_partial(&sk0, &pk0, &digest_bytes),
                }),
            ))
            .await
            .unwrap();
        assert_eq!(replace.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn pm_create_rejected_without_verified_digest() {
        let store = Store::open_in_memory().expect("open store");
        let state = AppState::new(store);
        let app = router(state);

        let digest = format!("0x{}", "99".repeat(32));
        let dto = ProposalDto {
            version: 2,
            kind: BlobKind::PmCouncilResolve,
            intent: IntentDto::PmCouncilResolve(crate::dto::PmCouncilResolveIntentDto {
                market_id: 5,
                winning_outcome: 0,
                pm_contract_id: format!("0x{}", "ab".repeat(32)),
                registry_account_id: 2,
                human_summary: None,
            }),
            signed_digest: digest.clone(),
            threshold: 2,
            partials: Vec::new(),
        };

        let create_resp = app
            .oneshot(json_request(
                "POST",
                "/v1/proposals",
                serde_json::to_value(&dto).unwrap(),
            ))
            .await
            .unwrap();
        assert_eq!(create_resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn get_bad_id_is_400_not_404() {
        let store = Store::open_in_memory().expect("open store");
        let state = AppState::new(store);
        let app = router(state);

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/v1/proposals/zz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn get_unknown_id_is_404() {
        let store = Store::open_in_memory().expect("open store");
        let state = AppState::new(store);
        let app = router(state);

        let id = "ab".repeat(32);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/proposals/{id}"))
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
        let mut rng = StdRng::seed_from_u64(7);
        let (sk, pk) = keypair(&mut rng);
        let pk_hex = format!("0x{}", hex::encode(pk.to_bytes()));
        let digest = [0xabu8; 32];
        let sig = sign_partial(&sk, &pk, &digest);
        let resp = app
            .oneshot(json_request(
                "POST",
                &format!("/v1/proposals/{}/partials", "ab".repeat(32)),
                json!({ "signer_pk": pk_hex, "sig": sig }),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn append_partial_bad_id_is_400() {
        let store = Store::open_in_memory().expect("open store");
        let state = AppState::new(store);
        let app = router(state);

        let resp = app
            .oneshot(json_request(
                "POST",
                "/v1/proposals/zz/partials",
                json!({ "signer_pk": "0x11", "sig": "0x22" }),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn create_rejects_bad_digest_hex() {
        let store = Store::open_in_memory().expect("open store");
        let state = AppState::new(store);
        let app = router(state);

        let mut dto = sample_dto();
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
    async fn create_rejects_digest_mismatch() {
        let store = Store::open_in_memory().expect("open store");
        let state = AppState::new(store);
        let app = router(state);

        let mut dto = sample_dto();
        dto.signed_digest = format!("0x{}", "ab".repeat(32));

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
    async fn create_rejects_overlong_human_summary() {
        let store = Store::open_in_memory().expect("open store");
        let state = AppState::new(store);
        let app = router(state);

        let mut dto = sample_dto();
        if let IntentDto::Proposals(ref mut i) = dto.intent {
            i.human_summary = Some("x".repeat(MAX_NOTE_CHARS + 1));
        }

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
        let mut rng = StdRng::seed_from_u64(55);

        let mut dto = sample_dto();
        let (sk, pk) = keypair(&mut rng);
        dto.partials.push(PartialDto {
            signer_pk: format!("0x{}", hex::encode(pk.to_bytes())),
            sig: sign_partial(&sk, &pk, &digest_bytes_from_dto(&dto)),
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

    #[tokio::test]
    async fn party_signup_visible_on_second_list_call() {
        let store = Store::open_in_memory().expect("open store");
        let state = AppState::new(store);
        let app = router(state);
        let mut rng = StdRng::seed_from_u64(11);
        let (sk, pk) = keypair(&mut rng);
        let pk_hex = format!("0x{}", hex::encode(pk.to_bytes()));

        let signup_resp = app
            .clone()
            .oneshot(json_request(
                "POST",
                "/v1/party",
                json!({
                    "name": "Alice",
                    "pk": pk_hex,
                    "sig": sign_party(&sk, &pk, "Alice"),
                    "note": "council lead",
                }),
            ))
            .await
            .unwrap();
        assert_eq!(signup_resp.status(), StatusCode::OK);
        let signed_up = body_json(signup_resp).await;
        assert_eq!(signed_up["name"], "Alice");
        assert_eq!(signed_up["pk"], pk_hex);
        assert_eq!(signed_up["note"], "council lead");

        for _ in 0..2 {
            let list_resp = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri("/v1/party")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(list_resp.status(), StatusCode::OK);
            let list = body_json(list_resp).await;
            let arr = list.as_array().unwrap();
            assert_eq!(arr.len(), 1);
            assert_eq!(arr[0]["name"], "Alice");
            assert_eq!(arr[0]["pk"], pk_hex);
        }
    }

    #[tokio::test]
    async fn party_upsert_by_pk_updates_name_without_duplicating() {
        let store = Store::open_in_memory().expect("open store");
        let state = AppState::new(store);
        let app = router(state);
        let mut rng = StdRng::seed_from_u64(22);
        let (sk, pk) = keypair(&mut rng);
        let pk_hex = format!("0x{}", hex::encode(pk.to_bytes()));

        app.clone()
            .oneshot(json_request(
                "POST",
                "/v1/party",
                json!({
                    "name": "Bob",
                    "pk": pk_hex,
                    "sig": sign_party(&sk, &pk, "Bob"),
                }),
            ))
            .await
            .unwrap();

        let update_resp = app
            .clone()
            .oneshot(json_request(
                "POST",
                "/v1/party",
                json!({
                    "name": "Bob Renamed",
                    "pk": pk_hex,
                    "sig": sign_party(&sk, &pk, "Bob Renamed"),
                }),
            ))
            .await
            .unwrap();
        assert_eq!(update_resp.status(), StatusCode::OK);
        let updated = body_json(update_resp).await;
        assert_eq!(updated["name"], "Bob Renamed");

        let list_resp = app
            .oneshot(
                Request::builder()
                    .uri("/v1/party")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let list = body_json(list_resp).await;
        let arr = list.as_array().unwrap();
        assert_eq!(arr.len(), 1, "upsert must not create a duplicate roster row");
        assert_eq!(arr[0]["name"], "Bob Renamed");
    }

    #[tokio::test]
    async fn party_signup_accepts_base58_pk() {
        let store = Store::open_in_memory().expect("open store");
        let state = AppState::new(store);
        let app = router(state);
        let mut rng = StdRng::seed_from_u64(33);
        let (sk, pk) = keypair(&mut rng);
        let pk_bytes = pk.to_bytes();
        let pk_b58 = bs58::encode(&pk_bytes).into_string();

        let signup_resp = app
            .oneshot(json_request(
                "POST",
                "/v1/party",
                json!({
                    "name": "Carol",
                    "pk": pk_b58,
                    "sig": sign_party(&sk, &pk, "Carol"),
                }),
            ))
            .await
            .unwrap();
        assert_eq!(signup_resp.status(), StatusCode::OK);
        let signed_up = body_json(signup_resp).await;
        assert_eq!(signed_up["pk"], format!("0x{}", hex::encode(pk_bytes)));
    }

    #[tokio::test]
    async fn party_signup_rejects_bad_pk() {
        let store = Store::open_in_memory().expect("open store");
        let state = AppState::new(store);
        let app = router(state);

        let resp = app
            .oneshot(json_request(
                "POST",
                "/v1/party",
                json!({
                    "name": "Eve",
                    "pk": "not-a-valid-key",
                    "sig": format!("0x{}", "aa".repeat(48)),
                }),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn party_signup_rejects_overlong_note() {
        let store = Store::open_in_memory().expect("open store");
        let state = AppState::new(store);
        let app = router(state);
        let mut rng = StdRng::seed_from_u64(44);
        let (sk, pk) = keypair(&mut rng);
        let pk_hex = format!("0x{}", hex::encode(pk.to_bytes()));

        let resp = app
            .oneshot(json_request(
                "POST",
                "/v1/party",
                json!({
                    "name": "Dave",
                    "pk": pk_hex,
                    "sig": sign_party(&sk, &pk, "Dave"),
                    "note": "n".repeat(MAX_NOTE_CHARS + 1),
                }),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn party_signup_rejects_overlong_name() {
        let store = Store::open_in_memory().expect("open store");
        let state = AppState::new(store);
        let app = router(state);
        let mut rng = StdRng::seed_from_u64(45);
        let (sk, pk) = keypair(&mut rng);
        let pk_hex = format!("0x{}", hex::encode(pk.to_bytes()));
        let long_name = "n".repeat(MAX_NOTE_CHARS + 1);

        let resp = app
            .oneshot(json_request(
                "POST",
                "/v1/party",
                json!({
                    "name": long_name,
                    "pk": pk_hex,
                    "sig": sign_party(&sk, &pk, &long_name),
                }),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn party_delete_route_is_gone() {
        let store = Store::open_in_memory().expect("open store");
        let state = AppState::new(store);
        let app = router(state);
        let mut rng = StdRng::seed_from_u64(44);
        let (sk, pk) = keypair(&mut rng);
        let pk_hex = format!("0x{}", hex::encode(pk.to_bytes()));

        app.clone()
            .oneshot(json_request(
                "POST",
                "/v1/party",
                json!({
                    "name": "Dave",
                    "pk": pk_hex,
                    "sig": sign_party(&sk, &pk, "Dave"),
                }),
            ))
            .await
            .unwrap();

        let delete_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/v1/party/{pk_hex}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            delete_resp.status(),
            StatusCode::NOT_FOUND,
            "DELETE /v1/party/:pk route must be absent"
        );

        // Roster still intact — no grief-delete path.
        let list_resp = app
            .oneshot(
                Request::builder()
                    .uri("/v1/party")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let list = body_json(list_resp).await;
        assert_eq!(list.as_array().unwrap().len(), 1);
    }
}
