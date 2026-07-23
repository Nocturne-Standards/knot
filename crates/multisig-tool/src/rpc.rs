//! Local RPC + web UI server. `127.0.0.1`-only by construction (refuses any
//! other bind address), bearer-token-gated on every `/api/*` route. Signing
//! happens here, server-side, using identities decrypted into memory once at
//! startup (password prompted once) — the browser JS never receives a
//! secret key, only names/public keys/signatures/messages.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{bail, Result};
use axum::extract::{Path as AxPath, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use dusk_bytes::Serializable;
use dusk_core::abi::ContractId;
use dusk_core::signatures::bls::PublicKey as BlsPublicKey;
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::proposals_types::call_types::{
    ApproveArgs, ProposalStatus, ProposalView, ProposeArgs,
};
use crate::registry_types::call_types::{
    AccountMeta, ChangeAccountArgs, CreateAccountArgs, DiagnoseQuorumResult, MultisigAccountView,
    SignatureEntry, VerifyQuorumAggregateArgs, VerifyQuorumArgs,
};
use crate::{chain, keystore};
use multisig_tool::bls;

struct AppState {
    identities: Mutex<Vec<keystore::Identity>>,
    password: String,
    store_path: PathBuf,
    token: String,
}

pub async fn serve(bind: &str, store_path: PathBuf) -> Result<()> {
    if !(bind.starts_with("127.0.0.1:") || bind.starts_with("localhost:")) {
        bail!("refusing to bind to '{bind}' — multisig-tool only ever serves on 127.0.0.1 (see README.md)");
    }

    let password = crate::prompt_password()?;
    let identities = keystore::load(&store_path, &password)?;

    let mut token_bytes = [0u8; 32];
    OsRng.fill_bytes(&mut token_bytes);
    let token = hex::encode(token_bytes);

    let state = Arc::new(AppState {
        identities: Mutex::new(identities),
        password,
        store_path,
        token: token.clone(),
    });

    let api = Router::new()
        .route("/api/identities", get(api_list_identities).post(api_new_identity))
        .route("/api/identities/import-pk", post(api_import_pk))
        .route("/api/account/create", post(api_account_create))
        .route("/api/account/{id}", get(api_account_query))
        .route("/api/account/{id}/meta", get(api_account_meta))
        .route("/api/account/{id}/keys", get(api_account_keys))
        .route("/api/account/next-id", get(api_account_next_id))
        .route("/api/quorum/submit", post(api_quorum_submit))
        .route("/api/quorum/check", post(api_quorum_check))
        .route("/api/quorum/diagnose", post(api_quorum_diagnose))
        .route("/api/quorum-agg/submit", post(api_quorum_agg_submit))
        .route("/api/quorum-agg/check", post(api_quorum_agg_check))
        .route("/api/change-account/submit", post(api_change_account_submit))
        .route("/api/proposal/create", post(api_proposal_create))
        .route("/api/proposal/{id}/approve", post(api_proposal_approve))
        .route("/api/proposal/{id}", get(api_proposal_status))
        .route("/api/proposal/{id}/finalize", post(api_proposal_finalize))
        .route("/api/proposal/next-id", get(api_proposal_next_id))
        .route_layer(axum::middleware::from_fn_with_state(state.clone(), require_token));

    let app = Router::new()
        .route("/", get(index))
        .route("/app.js", get(app_js))
        .route("/style.css", get(style_css))
        .route("/fonts.css", get(fonts_css))
        .route("/fonts/{file}", get(serve_font))
        .merge(api)
        .with_state(state);

    let addr: std::net::SocketAddr = bind.parse()?;
    eprintln!("multisig-tool listening on http://{addr}/?token={token}");
    eprintln!("TESTNET ONLY. Open the URL above (with token) in your browser.");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn require_token(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    let ok = headers
        .get("X-Multisig-Tool-Token")
        .and_then(|v| v.to_str().ok())
        .map(|v| v == state.token)
        .unwrap_or(false);
    if !ok {
        return (StatusCode::UNAUTHORIZED, "missing/invalid token").into_response();
    }
    next.run(request).await
}

async fn index(State(state): State<Arc<AppState>>) -> Html<String> {
    let template = include_str!("../static/index.html");
    Html(template.replace("__TOKEN__", &state.token))
}

async fn app_js() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/javascript")],
        include_str!("../static/app.js"),
    )
}

async fn style_css() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/css")], include_str!("../static/style.css"))
}

async fn fonts_css() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/css")], include_str!("../static/fonts.css"))
}

async fn serve_font(AxPath(file): AxPath<String>) -> Result<Response, StatusCode> {
    const ALLOWED: &[&str] = &[
        "literata-500.woff2",
        "literata-700.woff2",
        "sora-400.woff2",
        "sora-500.woff2",
        "sora-600.woff2",
        "sora-700.woff2",
    ];
    if !ALLOWED.contains(&file.as_str()) {
        return Err(StatusCode::NOT_FOUND);
    }
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("static/fonts")
        .join(&file);
    let bytes = std::fs::read(&path).map_err(|_| StatusCode::NOT_FOUND)?;
    Ok((
        [
            (header::CONTENT_TYPE, "font/woff2"),
            (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
        ],
        bytes,
    )
        .into_response())
}

fn bs58_pk(pk: &BlsPublicKey) -> String {
    bs58::encode(pk.to_bytes()).into_string()
}

#[derive(Serialize)]
struct IdentityOut {
    name: String,
    pk_base58: String,
    pk_only: bool,
}

async fn api_list_identities(State(state): State<Arc<AppState>>) -> Json<Vec<IdentityOut>> {
    let identities = state.identities.lock().await;
    Json(
        identities
            .iter()
            .map(|i| IdentityOut {
                name: i.name.clone(),
                pk_base58: bs58_pk(&i.pk),
                pk_only: i.is_pk_only(),
            })
            .collect(),
    )
}

#[derive(Deserialize)]
struct NewIdentityReq {
    name: String,
}

async fn api_new_identity(
    State(state): State<Arc<AppState>>,
    Json(req): Json<NewIdentityReq>,
) -> Result<Json<IdentityOut>, (StatusCode, String)> {
    let mut identities = state.identities.lock().await;
    if identities.iter().any(|i| i.name == req.name) {
        return Err((StatusCode::BAD_REQUEST, format!("identity '{}' already exists", req.name)));
    }
    let identity = keystore::generate(&req.name);
    let out = IdentityOut {
        name: identity.name.clone(),
        pk_base58: bs58_pk(&identity.pk),
        pk_only: false,
    };
    identities.push(identity);
    keystore::save(&state.store_path, &state.password, &identities)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(out))
}

#[derive(Deserialize)]
struct ImportPkReq {
    name: String,
    pk: String,
}

async fn api_import_pk(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ImportPkReq>,
) -> Result<Json<IdentityOut>, (StatusCode, String)> {
    let mut identities = state.identities.lock().await;
    if identities.iter().any(|i| i.name == req.name) {
        return Err((StatusCode::BAD_REQUEST, format!("identity '{}' already exists", req.name)));
    }
    let pk = keystore::parse_pk(&req.pk).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let identity = keystore::from_pk_only(&req.name, pk);
    let out = IdentityOut {
        name: identity.name.clone(),
        pk_base58: bs58_pk(&identity.pk),
        pk_only: true,
    };
    identities.push(identity);
    keystore::save(&state.store_path, &state.password, &identities)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(out))
}

async fn find_pk(state: &AppState, name: &str) -> Result<BlsPublicKey, (StatusCode, String)> {
    let identities = state.identities.lock().await;
    identities
        .iter()
        .find(|i| i.name == name)
        .map(|i| i.pk)
        .ok_or_else(|| (StatusCode::BAD_REQUEST, format!("no identity named '{name}'")))
}

#[derive(Deserialize)]
struct CreateAccountReq {
    members: Vec<String>,
    threshold: u32,
}

#[derive(Serialize)]
struct SubmitOut {
    log: String,
    outcome: String,
    /// confirmed | failed | unknown — for the UI status row.
    tx_status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tx_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    panic_line: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    diagnose: Option<DiagnoseOut>,
    #[serde(skip_serializing_if = "Option::is_none")]
    check: Option<bool>,
}

#[derive(Debug, Serialize)]
struct DiagnoseOut {
    exists: bool,
    threshold: u32,
    members_len: u32,
    member_matches: u32,
    sigs_ok: u32,
    free_read_untrusted: bool,
}

fn outcome_str(o: chain::WriteOutcome) -> &'static str {
    match o {
        chain::WriteOutcome::Panic => "panic",
        chain::WriteOutcome::Ok => "ok",
        chain::WriteOutcome::Unknown => "unknown",
    }
}

fn submit_from_log(log: String) -> SubmitOut {
    let outcome = chain::classify_write(&log);
    let panic_line = chain::panic_line(&log);
    let tx_hash = chain::extract_tx_hash(&log);
    let tx_status = chain::tx_status_label(outcome, &log).to_string();
    SubmitOut {
        log,
        outcome: outcome_str(outcome).to_string(),
        tx_status,
        tx_hash,
        panic_line,
        note: None,
        diagnose: None,
        check: None,
    }
}

fn to_500<E: std::fmt::Display>(e: E) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}

async fn api_account_create(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateAccountReq>,
) -> Result<Json<SubmitOut>, (StatusCode, String)> {
    let mut members = Vec::with_capacity(req.members.len());
    for name in &req.members {
        members.push(find_pk(&state, name).await?);
    }
    let args = CreateAccountArgs {
        members,
        threshold: req.threshold,
    };
    let bytes = chain::encode(&args).map_err(to_500)?;
    let result = tokio::task::spawn_blocking(move || chain::submit_call("create_account", &bytes))
        .await
        .map_err(to_500)?
        .map_err(to_500)?;
    Ok(Json(submit_from_log(result.stdout)))
}

#[derive(Serialize)]
struct AccountView {
    threshold: u32,
    nonce: u64,
    members: Vec<String>,
}

async fn api_account_query(
    AxPath(id): AxPath<u64>,
) -> Result<Json<Option<AccountView>>, (StatusCode, String)> {
    let bytes = chain::encode(&id).map_err(to_500)?;
    let view: Option<MultisigAccountView> = chain::query("account", bytes).await.map_err(to_500)?;
    Ok(Json(view.map(|v| AccountView {
        threshold: v.threshold,
        nonce: v.nonce,
        members: v.members.iter().map(bs58_pk).collect(),
    })))
}

#[derive(Serialize)]
struct MetaOut {
    threshold: u32,
    nonce: u64,
    members_len: u32,
}

async fn api_account_meta(
    AxPath(id): AxPath<u64>,
) -> Result<Json<Option<MetaOut>>, (StatusCode, String)> {
    let bytes = chain::encode(&id).map_err(to_500)?;
    let meta: Option<AccountMeta> = chain::query("account_meta", bytes).await.map_err(to_500)?;
    Ok(Json(meta.map(|m| MetaOut {
        threshold: m.threshold,
        nonce: m.nonce,
        members_len: m.members_len,
    })))
}

async fn api_account_keys(
    AxPath(id): AxPath<u64>,
) -> Result<Json<Option<Vec<String>>>, (StatusCode, String)> {
    let bytes = chain::encode(&id).map_err(to_500)?;
    let keys: Option<Vec<Vec<u8>>> = chain::query("member_key_bytes", bytes).await.map_err(to_500)?;
    Ok(Json(keys.map(|ks| ks.into_iter().map(hex::encode).collect())))
}

async fn api_account_next_id() -> Result<Json<u64>, (StatusCode, String)> {
    let bytes = chain::encode(&()).map_err(to_500)?;
    let next: u64 = chain::query("next_account_id", bytes).await.map_err(to_500)?;
    Ok(Json(next))
}

#[derive(Deserialize)]
struct QuorumSubmitReq {
    account: u64,
    msg: String,
    #[serde(default)]
    hex: bool,
    signers: Vec<String>,
}

fn msg_bytes(msg: &str, hex_flag: bool) -> Result<Vec<u8>, (StatusCode, String)> {
    if hex_flag {
        hex::decode(msg.trim_start_matches("0x")).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))
    } else {
        Ok(msg.as_bytes().to_vec())
    }
}

async fn build_sigs_locked(
    state: &AppState,
    signers: &[String],
    msg: &[u8],
) -> Result<Vec<SignatureEntry>, (StatusCode, String)> {
    let identities = state.identities.lock().await;
    let mut sigs = Vec::with_capacity(signers.len());
    for name in signers {
        let id = identities
            .iter()
            .find(|i| &i.name == name)
            .ok_or_else(|| (StatusCode::BAD_REQUEST, format!("no identity named '{name}'")))?;
        let sk = id
            .require_sk()
            .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
        sigs.push(SignatureEntry {
            signer: id.pk,
            signature: bls::sign(sk, msg),
        });
    }
    Ok(sigs)
}

fn diagnose_to_out(d: &DiagnoseQuorumResult) -> DiagnoseOut {
    DiagnoseOut {
        exists: d.exists,
        threshold: d.threshold,
        members_len: d.members_len,
        member_matches: d.member_matches,
        sigs_ok: d.sigs_ok,
        free_read_untrusted: d.member_matches > 0 && d.sigs_ok == 0,
    }
}

async fn free_read_quorum(args: &VerifyQuorumArgs) -> Result<(Option<DiagnoseOut>, Option<bool>), (StatusCode, String)> {
    let bytes = chain::encode(args).map_err(to_500)?;
    let diagnose = match chain::query::<DiagnoseQuorumResult>("diagnose_quorum", bytes.clone()).await {
        Ok(d) => Some(diagnose_to_out(&d)),
        Err(_) => None,
    };
    let check = chain::query::<bool>("verify_quorum", bytes).await.ok();
    Ok((diagnose, check))
}

async fn api_quorum_submit(
    State(state): State<Arc<AppState>>,
    Json(req): Json<QuorumSubmitReq>,
) -> Result<Json<SubmitOut>, (StatusCode, String)> {
    let msg = msg_bytes(&req.msg, req.hex)?;
    let sigs = build_sigs_locked(&state, &req.signers, &msg).await?;
    let args = VerifyQuorumArgs {
        account_id: req.account,
        msg,
        sigs,
    };
    let bytes = chain::encode(&args).map_err(to_500)?;
    let result = tokio::task::spawn_blocking(move || chain::submit_call("verify_quorum", &bytes))
        .await
        .map_err(to_500)?
        .map_err(to_500)?;
    let mut out = submit_from_log(result.stdout);
    out.note = Some(
        "verify_quorum returns bool with no event. Free-read follow-up may be untrusted on live testnet."
            .into(),
    );
    if let Ok((d, c)) = free_read_quorum(&args).await {
        out.diagnose = d;
        out.check = c;
    }
    Ok(Json(out))
}

async fn api_quorum_check(
    State(state): State<Arc<AppState>>,
    Json(req): Json<QuorumSubmitReq>,
) -> Result<Json<SubmitOut>, (StatusCode, String)> {
    let msg = msg_bytes(&req.msg, req.hex)?;
    let sigs = build_sigs_locked(&state, &req.signers, &msg).await?;
    let args = VerifyQuorumArgs {
        account_id: req.account,
        msg,
        sigs,
    };
    let (diagnose, check) = free_read_quorum(&args).await?;
    let mut note = None;
    if diagnose.as_ref().map(|d| d.free_read_untrusted).unwrap_or(false) {
        note = Some(
            "Free-read verify looks untrusted (members matched, sigs_ok=0). Prefer change_account counters."
                .into(),
        );
    }
    Ok(Json(SubmitOut {
        log: format!("check={check:?} diagnose={diagnose:?}"),
        outcome: "ok".into(),
        tx_status: "n/a".into(),
        tx_hash: None,
        panic_line: None,
        note,
        diagnose,
        check,
    }))
}

async fn api_quorum_diagnose(
    State(state): State<Arc<AppState>>,
    Json(req): Json<QuorumSubmitReq>,
) -> Result<Json<SubmitOut>, (StatusCode, String)> {
    api_quorum_check(State(state), Json(req)).await
}

async fn api_quorum_agg_submit(
    State(state): State<Arc<AppState>>,
    Json(req): Json<QuorumSubmitReq>,
) -> Result<Json<SubmitOut>, (StatusCode, String)> {
    let msg = msg_bytes(&req.msg, req.hex)?;
    let identities = state.identities.lock().await;
    let mut signer_keys = Vec::with_capacity(req.signers.len());
    let mut per_signer_sigs = Vec::with_capacity(req.signers.len());
    for name in &req.signers {
        let id = identities
            .iter()
            .find(|i| &i.name == name)
            .ok_or_else(|| (StatusCode::BAD_REQUEST, format!("no identity named '{name}'")))?;
        let sk = id
            .require_sk()
            .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
        signer_keys.push(id.pk);
        per_signer_sigs.push(bls::sign_multisig(sk, &id.pk, &msg));
    }
    drop(identities);
    let aggregate_sig = bls::aggregate(&per_signer_sigs);

    let args = VerifyQuorumAggregateArgs {
        account_id: req.account,
        msg,
        signer_keys,
        aggregate_sig,
    };
    let bytes = chain::encode(&args).map_err(to_500)?;
    let result =
        tokio::task::spawn_blocking(move || chain::submit_call("verify_quorum_aggregate", &bytes))
            .await
            .map_err(to_500)?
            .map_err(to_500)?;
    let mut out = submit_from_log(result.stdout);
    out.note = Some("Aggregate path: bool return not in wallet log; free-read check may be untrusted.".into());
    let check_bytes = chain::encode(&args).map_err(to_500)?;
    out.check = chain::query::<bool>("verify_quorum_aggregate", check_bytes).await.ok();
    Ok(Json(out))
}

async fn api_quorum_agg_check(
    State(state): State<Arc<AppState>>,
    Json(req): Json<QuorumSubmitReq>,
) -> Result<Json<SubmitOut>, (StatusCode, String)> {
    let msg = msg_bytes(&req.msg, req.hex)?;
    let identities = state.identities.lock().await;
    let mut signer_keys = Vec::with_capacity(req.signers.len());
    let mut per_signer_sigs = Vec::with_capacity(req.signers.len());
    for name in &req.signers {
        let id = identities
            .iter()
            .find(|i| &i.name == name)
            .ok_or_else(|| (StatusCode::BAD_REQUEST, format!("no identity named '{name}'")))?;
        let sk = id
            .require_sk()
            .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
        signer_keys.push(id.pk);
        per_signer_sigs.push(bls::sign_multisig(sk, &id.pk, &msg));
    }
    drop(identities);
    let aggregate_sig = bls::aggregate(&per_signer_sigs);
    let args = VerifyQuorumAggregateArgs {
        account_id: req.account,
        msg,
        signer_keys,
        aggregate_sig,
    };
    let bytes = chain::encode(&args).map_err(to_500)?;
    let check = chain::query::<bool>("verify_quorum_aggregate", bytes).await.ok();
    Ok(Json(SubmitOut {
        log: format!("check={check:?}"),
        outcome: "ok".into(),
        tx_status: "n/a".into(),
        tx_hash: None,
        panic_line: None,
        note: Some("Free-read aggregate check may be untrusted on live testnet.".into()),
        diagnose: None,
        check,
    }))
}

#[derive(Deserialize)]
struct ChangeAccountReq {
    account: u64,
    new_members: Vec<String>,
    new_threshold: u32,
    signers: Vec<String>,
}

async fn api_change_account_submit(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ChangeAccountReq>,
) -> Result<Json<SubmitOut>, (StatusCode, String)> {
    let mut new_members = Vec::with_capacity(req.new_members.len());
    for name in &req.new_members {
        new_members.push(find_pk(&state, name).await?);
    }

    let current: Option<MultisigAccountView> =
        chain::query("account", chain::encode(&req.account).map_err(to_500)?)
            .await
            .map_err(to_500)?;
    let current =
        current.ok_or_else(|| (StatusCode::BAD_REQUEST, format!("account {} not found", req.account)))?;

    let msg = bls::change_account_message(req.account, current.nonce, &new_members, req.new_threshold);
    let sigs = build_sigs_locked(&state, &req.signers, &msg).await?;

    let args = ChangeAccountArgs {
        account_id: req.account,
        new_members,
        new_threshold: req.new_threshold,
        sigs,
    };
    let bytes = chain::encode(&args).map_err(to_500)?;
    let result = tokio::task::spawn_blocking(move || chain::submit_call("change_account", &bytes))
        .await
        .map_err(to_500)?
        .map_err(to_500)?;
    Ok(Json(submit_from_log(result.stdout)))
}

#[derive(Deserialize)]
struct ProposalCreateReq {
    account: u64,
    target: String,
    function: String,
    #[serde(default)]
    args_hex: String,
    #[serde(default)]
    deadline: u64,
}

#[derive(Serialize)]
struct ProposalCreateOut {
    #[serde(flatten)]
    submit: SubmitOut,
    allocated_id_hint: u64,
}

async fn api_proposal_create(
    Json(req): Json<ProposalCreateReq>,
) -> Result<Json<ProposalCreateOut>, (StatusCode, String)> {
    let target_bytes: [u8; 32] = hex::decode(req.target.trim_start_matches("0x"))
        .map_err(to_500)?
        .as_slice()
        .try_into()
        .map_err(|_| (StatusCode::BAD_REQUEST, "target must be 32-byte hex".into()))?;
    let call_args = if req.args_hex.is_empty() {
        Vec::new()
    } else {
        hex::decode(req.args_hex.trim_start_matches("0x")).map_err(to_500)?
    };
    let before: u64 = chain::query_contract(
        chain::Contract::Proposals,
        "next_proposal_id",
        chain::encode(&()).map_err(to_500)?,
    )
    .await
    .map_err(to_500)?;
    let args = ProposeArgs {
        registry_account_id: req.account,
        target: ContractId::from_bytes(target_bytes),
        function_name: req.function,
        call_args,
        deadline: req.deadline,
    };
    let bytes = chain::encode(&args).map_err(to_500)?;
    let result =
        tokio::task::spawn_blocking(move || chain::submit_call_to(chain::Contract::Proposals, "propose", &bytes))
            .await
            .map_err(to_500)?
            .map_err(to_500)?;
    Ok(Json(ProposalCreateOut {
        submit: submit_from_log(result.stdout),
        allocated_id_hint: before,
    }))
}

#[derive(Deserialize)]
struct ProposalApproveReq {
    signer: String,
}

#[derive(Serialize)]
struct ProposalApproveOut {
    #[serde(flatten)]
    submit: SubmitOut,
    intent: IntentDisplay,
}

#[derive(Serialize)]
struct IntentDisplay {
    chain_id: u64,
    committee_id: u64,
    nonce: u64,
    target: String,
    function: String,
    call_args_hex: String,
    deadline: u64,
    digest_hex: String,
}

async fn api_proposal_approve(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<u64>,
    Json(req): Json<ProposalApproveReq>,
) -> Result<Json<ProposalApproveOut>, (StatusCode, String)> {
    let view: Option<ProposalView> = chain::query_contract(
        chain::Contract::Proposals,
        "proposal",
        chain::encode(&id).map_err(to_500)?,
    )
    .await
    .map_err(to_500)?;
    let view = view.ok_or_else(|| (StatusCode::BAD_REQUEST, format!("proposal {id} not found")))?;
    if view.status != ProposalStatus::Open {
        return Err((StatusCode::BAD_REQUEST, format!("proposal {id} is not Open")));
    }

    let intent = multisig_encoding::ProposalIntent {
        chain_id: view.chain_id,
        committee_id: view.registry_account_id,
        nonce: view.nonce,
        target_contract_id: view.target.to_bytes(),
        function_name: view.function_name.clone(),
        call_args: view.call_args.clone(),
        deadline: view.deadline,
    };
    let digest = multisig_encoding::recompute_and_verify(&intent, &view.signed_digest).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "REFUSING TO SIGN: on-chain digest does not match recomputed intent".into(),
        )
    })?;
    let intent_out = IntentDisplay {
        chain_id: intent.chain_id,
        committee_id: intent.committee_id,
        nonce: intent.nonce,
        target: format!("0x{}", hex::encode(intent.target_contract_id)),
        function: intent.function_name.clone(),
        call_args_hex: format!("0x{}", hex::encode(&intent.call_args)),
        deadline: intent.deadline,
        digest_hex: format!("0x{}", hex::encode(digest)),
    };

    let identities = state.identities.lock().await;
    let id_rec = identities
        .iter()
        .find(|i| i.name == req.signer)
        .ok_or_else(|| (StatusCode::BAD_REQUEST, format!("no identity named '{}'", req.signer)))?;
    let sk = id_rec
        .require_sk()
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let args = ApproveArgs {
        proposal_id: id,
        signer: id_rec.pk,
        signature: bls::sign(sk, &digest),
    };
    drop(identities);

    let bytes = chain::encode(&args).map_err(to_500)?;
    let result =
        tokio::task::spawn_blocking(move || chain::submit_call_to(chain::Contract::Proposals, "approve", &bytes))
            .await
            .map_err(to_500)?
            .map_err(to_500)?;
    Ok(Json(ProposalApproveOut {
        submit: submit_from_log(result.stdout),
        intent: intent_out,
    }))
}

#[derive(Serialize)]
struct ProposalStatusOut {
    id: u64,
    status: String,
    registry_account_id: u64,
    chain_id: u64,
    nonce: u64,
    target: String,
    function: String,
    call_args_hex: String,
    deadline: u64,
    digest_hex: String,
    approvals: Vec<String>,
    approvals_len: usize,
}

async fn api_proposal_status(
    AxPath(id): AxPath<u64>,
) -> Result<Json<Option<ProposalStatusOut>>, (StatusCode, String)> {
    let view: Option<ProposalView> = chain::query_contract(
        chain::Contract::Proposals,
        "proposal",
        chain::encode(&id).map_err(to_500)?,
    )
    .await
    .map_err(to_500)?;
    Ok(Json(view.map(|v| {
        let status = match v.status {
            ProposalStatus::Open => "Open",
            ProposalStatus::Executed => "Executed",
            ProposalStatus::Tombstoned => "Tombstoned",
        };
        ProposalStatusOut {
            id,
            status: status.into(),
            registry_account_id: v.registry_account_id,
            chain_id: v.chain_id,
            nonce: v.nonce,
            target: format!("0x{}", hex::encode(v.target.to_bytes())),
            function: v.function_name,
            call_args_hex: format!("0x{}", hex::encode(&v.call_args)),
            deadline: v.deadline,
            digest_hex: format!("0x{}", hex::encode(v.signed_digest)),
            approvals_len: v.approvals.len(),
            approvals: v.approvals.iter().map(bs58_pk).collect(),
        }
    })))
}

async fn api_proposal_finalize(
    AxPath(id): AxPath<u64>,
) -> Result<Json<SubmitOut>, (StatusCode, String)> {
    let bytes = chain::encode(&id).map_err(to_500)?;
    let result =
        tokio::task::spawn_blocking(move || chain::submit_call_to(chain::Contract::Proposals, "finalize", &bytes))
            .await
            .map_err(to_500)?
            .map_err(to_500)?;
    Ok(Json(submit_from_log(result.stdout)))
}

async fn api_proposal_next_id() -> Result<Json<u64>, (StatusCode, String)> {
    let next: u64 = chain::query_contract(
        chain::Contract::Proposals,
        "next_proposal_id",
        chain::encode(&()).map_err(to_500)?,
    )
    .await
    .map_err(to_500)?;
    Ok(Json(next))
}
