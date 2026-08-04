//! Local RPC + web UI server. `127.0.0.1`-only by construction (refuses any
//! other bind address), bearer-token-gated on every `/api/*` route. Signing
//! happens here, server-side, using identities decrypted into memory once at
//! startup (password prompted once) — the browser JS never receives a
//! secret key, only names/public keys/signatures/messages.

use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

use anyhow::{bail, Result};
use axum::extract::{Path as AxPath, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use dusk_bytes::Serializable;
use dusk_core::abi::ContractId;
use dusk_core::signatures::bls::PublicKey as BlsPublicKey;
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;
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
use multisig_tool::collector_client::{self, CollectorClient};
use multisig_tool::mock_ledger::{DemoMode, MockLedger, MockProposal, MockProposalStatus};

/// Chain id baked into mock proposals (matches live testnet `init_chain_id`).
const MOCK_CHAIN_ID: u64 = 2;

const MOCK_DRAWER_MSG: &str = "mock mode: use DEMO_MODE=testnet";

struct AppState {
    identities: Mutex<Vec<keystore::Identity>>,
    password: String,
    store_path: PathBuf,
    token: String,
    demo_mode: DemoMode,
    /// In-process ledger; only read/written when `demo_mode == Mock`.
    mock: Mutex<MockLedger>,
}

#[derive(Default)]
pub struct ServeOptions {
    /// When set, open the default browser to this tab (e.g. `#proposals`).
    pub open_tab: Option<String>,
    /// Extra query string (without leading `?`/`&`), e.g. `account=1`.
    pub query_extra: Option<String>,
    /// Open the default browser after bind when `open_tab` is set.
    pub open_browser: bool,
}


pub async fn serve(bind: &str, store_path: PathBuf) -> Result<()> {
    serve_with_options(bind, store_path, ServeOptions::default()).await
}

pub async fn serve_with_options(
    bind: &str,
    store_path: PathBuf,
    opts: ServeOptions,
) -> Result<()> {
    if !(bind.starts_with("127.0.0.1:") || bind.starts_with("localhost:")) {
        bail!("refusing to bind to '{bind}' — multisig-tool only ever serves on 127.0.0.1 (see README.md)");
    }

    let password = crate::prompt_password()?;
    let identities = keystore::load(&store_path, &password)?;

    let demo_mode = DemoMode::from_env();
    eprintln!("DEMO_MODE={}", demo_mode.as_str());

    let mut token_bytes = [0u8; 32];
    OsRng.fill_bytes(&mut token_bytes);
    let state = Arc::new(AppState {
        identities: Mutex::new(identities),
        password,
        store_path,
        token: hex::encode(token_bytes),
        demo_mode,
        mock: Mutex::new(MockLedger::new()),
    });

    let app = build_router(state);

    let addr: std::net::SocketAddr = bind.parse()?;
    // Do not put the bearer token in the printed/opened URL (M8) — it is
    // injected into the local HTML as `window.MULTISIG_TOOL_TOKEN`; API
    // clients must send header `X-Multisig-Tool-Token`.
    let mut url = format!("http://{addr}/");
    if let Some(extra) = &opts.query_extra {
        if !extra.is_empty() {
            url.push('?');
            url.push_str(extra.trim_start_matches('&').trim_start_matches('?'));
        }
    }
    if let Some(tab) = &opts.open_tab {
        url.push('#');
        url.push_str(tab.trim_start_matches('#'));
    }
    eprintln!("multisig-tool listening on {url}");
    eprintln!("Authorize /api/* with header X-Multisig-Tool-Token (value injected into local HTML only).");
    eprintln!("TESTNET ONLY. Open the URL above in your browser.");
    if opts.open_browser || opts.open_tab.is_some() {
        open_default_browser(&url);
    }
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

fn open_default_browser(url: &str) {
    let result = {
        #[cfg(target_os = "macos")]
        {
            Command::new("open").arg(url).status()
        }
        #[cfg(target_os = "linux")]
        {
            Command::new("xdg-open").arg(url).status()
        }
        #[cfg(target_os = "windows")]
        {
            Command::new("cmd").args(["/C", "start", "", url]).status()
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        {
            Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "no browser opener for this OS",
            ))
        }
    };
    match result {
        Ok(status) if status.success() => {}
        Ok(status) => eprintln!("warning: browser open exited with {status}"),
        Err(e) => eprintln!("warning: could not open browser: {e}"),
    }
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
        .map(|v| {
            let got = v.as_bytes();
            let want = state.token.as_bytes();
            got.len() == want.len() && bool::from(got.ct_eq(want))
        })
        .unwrap_or(false);
    if !ok {
        return (StatusCode::UNAUTHORIZED, "missing/invalid token").into_response();
    }
    next.run(request).await
}

async fn index(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let template = include_str!("../static/index.html");
    // Token is process-scoped; never cache HTML across restarts.
    (
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        template.replace("__TOKEN__", &state.token),
    )
}

async fn app_js() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/javascript")],
        include_str!("../static/app.js"),
    )
}

async fn mock_ledger_js() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/javascript")],
        include_str!("../static/mock-ledger.js"),
    )
}

async fn style_css() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/css")], include_str!("../static/style.css"))
}

async fn fonts_css() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/css")], include_str!("../static/fonts.css"))
}

async fn lab_fonts_css() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/css")], include_str!("../static/lab/fonts.css"))
}

async fn lab_tokens_css() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/css")], include_str!("../static/lab/tokens.css"))
}

async fn lab_layout_css() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/css")], include_str!("../static/lab/layout.css"))
}

async fn lab_components_css() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/css")], include_str!("../static/lab/components.css"))
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

async fn serve_lab_font(AxPath(file): AxPath<String>) -> Result<Response, StatusCode> {
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
        .join("static/lab/fonts")
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

pub(crate) fn bs58_pk(pk: &BlsPublicKey) -> String {
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

#[derive(Serialize)]
struct SetupStatusOut {
    store_path: String,
    identities_count: usize,
    collector_configured: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    collector_url: Option<String>,
    collector_user_configured: bool,
    /// `"mock"` | `"testnet"` — mirrors `DEMO_MODE` (default mock).
    demo_mode: &'static str,
}

/// Server-side-only env read — the URL is shown to the browser (harmless),
/// but `MULTISIG_COLLECTOR_PASSWORD` never is; only whether a user was set.
async fn api_setup_status(State(state): State<Arc<AppState>>) -> Json<SetupStatusOut> {
    let identities = state.identities.lock().await;
    let collector_url = std::env::var(collector_client::URL_ENV).ok();
    let collector_user_configured = std::env::var(collector_client::USER_ENV)
        .map(|v| !v.is_empty())
        .unwrap_or(false);
    Json(SetupStatusOut {
        store_path: state.store_path.display().to_string(),
        identities_count: identities.len(),
        collector_configured: collector_url.is_some(),
        collector_url,
        collector_user_configured,
        demo_mode: state.demo_mode.as_str(),
    })
}

fn to_400<E: std::fmt::Display>(e: E) -> (StatusCode, String) {
    (StatusCode::BAD_REQUEST, e.to_string())
}

fn mock_drawer_unavailable(state: &AppState) -> Result<(), (StatusCode, String)> {
    if state.demo_mode == DemoMode::Mock {
        Err((StatusCode::NOT_IMPLEMENTED, MOCK_DRAWER_MSG.into()))
    } else {
        Ok(())
    }
}

fn mock_ok_submit(log: String, tx_hash: String) -> SubmitOut {
    SubmitOut {
        log,
        outcome: "ok".into(),
        tx_status: "confirmed".into(),
        tx_hash: Some(tx_hash),
        panic_line: None,
        note: Some("DEMO_MODE=mock — no chain submit".into()),
        diagnose: None,
        check: None,
    }
}

fn mock_proposal_preview(p: &MockProposal) -> Result<SignPreviewOut, (StatusCode, String)> {
    if p.status != MockProposalStatus::Open {
        return Err((StatusCode::BAD_REQUEST, "proposal is not Open".into()));
    }
    let intent = multisig_encoding::ProposalIntent {
        chain_id: p.chain_id,
        committee_id: p.registry_account_id,
        nonce: p.nonce,
        target_contract_id: p.target,
        function_name: p.function_name.clone(),
        call_args: p.call_args.clone(),
        deadline: p.deadline,
    };
    let digest = multisig_encoding::recompute_and_verify(&intent, &p.digest).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "REFUSING: mock digest does not match recomputed intent".into(),
        )
    })?;
    Ok(SignPreviewOut {
        digest_hex: format!("0x{}", hex::encode(digest)),
        digest_mnemonic: multisig_encoding::digest_mnemonic(&digest),
        digest_safety_number: multisig_encoding::digest_safety_number(&digest),
        chain_id: intent.chain_id,
        committee_id: intent.committee_id,
        nonce: intent.nonce,
        target_hex: format!("0x{}", hex::encode(intent.target_contract_id)),
        function_name: intent.function_name,
        call_args_hex: format!("0x{}", hex::encode(&intent.call_args)),
        deadline: intent.deadline,
    })
}

fn mock_proposal_status_out(id: u64, p: MockProposal) -> ProposalStatusOut {
    let status = match p.status {
        MockProposalStatus::Open => "Open",
        MockProposalStatus::Finalized => "Executed",
    };
    ProposalStatusOut {
        id,
        status: status.into(),
        registry_account_id: p.registry_account_id,
        chain_id: p.chain_id,
        nonce: p.nonce,
        target: format!("0x{}", hex::encode(p.target)),
        function: p.function_name,
        call_args_hex: format!("0x{}", hex::encode(&p.call_args)),
        deadline: p.deadline,
        digest_hex: format!("0x{}", hex::encode(p.digest)),
        approvals_len: p.approvals.len(),
        approvals: p
            .approvals
            .iter()
            .map(|pk| bs58::encode(pk).into_string())
            .collect(),
    }
}

#[derive(Serialize)]
struct PartyMemberOut {
    name: String,
    pk: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
    joined_at: i64,
}

impl From<collector_client::PartyMember> for PartyMemberOut {
    fn from(m: collector_client::PartyMember) -> Self {
        PartyMemberOut {
            name: m.name,
            pk: m.pk,
            note: m.note,
            joined_at: m.joined_at,
        }
    }
}

/// Party finder proxy — `multisig-tool serve` holds `MULTISIG_COLLECTOR_*`
/// in its own process env; the browser only ever sees names/pks/notes here,
/// never the collector's Basic Auth password.
async fn api_party_list(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<PartyMemberOut>>, (StatusCode, String)> {
    mock_drawer_unavailable(&state)?;
    let client = CollectorClient::resolve(None).map_err(to_400)?;
    let members = client.list_party().await.map_err(to_500)?;
    Ok(Json(members.into_iter().map(PartyMemberOut::from).collect()))
}

#[derive(Deserialize)]
struct PartySignupReq {
    /// Local identity name whose *public* key gets published to the roster.
    name: String,
    #[serde(default)]
    note: Option<String>,
}

async fn api_party_signup(
    State(state): State<Arc<AppState>>,
    Json(req): Json<PartySignupReq>,
) -> Result<Json<PartyMemberOut>, (StatusCode, String)> {
    mock_drawer_unavailable(&state)?;
    let pk = find_pk(&state, &req.name).await?;
    let client = CollectorClient::resolve(None).map_err(to_400)?;
    let member = client
        .signup_party(&req.name, &bs58_pk(&pk), req.note.as_deref())
        .await
        .map_err(to_500)?;
    Ok(Json(member.into()))
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
    if state.demo_mode == DemoMode::Mock {
        let member_bytes: Vec<[u8; 96]> = members.iter().map(|pk| pk.to_bytes()).collect();
        let mut mock = state.mock.lock().await;
        let id = mock
            .create_account(member_bytes, req.threshold)
            .map_err(to_400)?;
        return Ok(Json(mock_ok_submit(
            format!("mock: create_account id={id} threshold={}", req.threshold),
            format!("mock-create-account-{id}"),
        )));
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
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<u64>,
) -> Result<Json<Option<AccountView>>, (StatusCode, String)> {
    if state.demo_mode == DemoMode::Mock {
        let mock = state.mock.lock().await;
        return Ok(Json(mock.account(id).map(|v| AccountView {
            threshold: v.threshold,
            nonce: v.nonce,
            members: v
                .members
                .iter()
                .map(|pk| bs58::encode(pk).into_string())
                .collect(),
        })));
    }
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
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<u64>,
) -> Result<Json<Option<MetaOut>>, (StatusCode, String)> {
    if state.demo_mode == DemoMode::Mock {
        let mock = state.mock.lock().await;
        return Ok(Json(mock.account_meta(id).map(|m| MetaOut {
            threshold: m.threshold,
            nonce: m.nonce,
            members_len: m.member_count,
        })));
    }
    let bytes = chain::encode(&id).map_err(to_500)?;
    let meta: Option<AccountMeta> = chain::query("account_meta", bytes).await.map_err(to_500)?;
    Ok(Json(meta.map(|m| MetaOut {
        threshold: m.threshold,
        nonce: m.nonce,
        members_len: m.members_len,
    })))
}

async fn api_account_keys(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<u64>,
) -> Result<Json<Option<Vec<String>>>, (StatusCode, String)> {
    if state.demo_mode == DemoMode::Mock {
        let mock = state.mock.lock().await;
        return Ok(Json(mock.account(id).map(|a| {
            a.members
                .into_iter()
                .map(|pk| hex::encode(pk))
                .collect()
        })));
    }
    let bytes = chain::encode(&id).map_err(to_500)?;
    let keys: Option<Vec<Vec<u8>>> = chain::query("member_key_bytes", bytes).await.map_err(to_500)?;
    Ok(Json(keys.map(|ks| ks.into_iter().map(hex::encode).collect())))
}

async fn api_account_next_id(
    State(state): State<Arc<AppState>>,
) -> Result<Json<u64>, (StatusCode, String)> {
    if state.demo_mode == DemoMode::Mock {
        let mock = state.mock.lock().await;
        return Ok(Json(mock.next_account_id()));
    }
    let bytes = chain::encode(&()).map_err(to_500)?;
    let next: u64 = chain::query("next_account_id", bytes).await.map_err(to_500)?;
    Ok(Json(next))
}

#[derive(Deserialize)]
struct RegistryAccountsQuery {
    /// Cap how many accounts to scan from 0 (default 64, max 256).
    #[serde(default = "default_account_scan")]
    limit: u64,
}

fn default_account_scan() -> u64 {
    64
}

#[derive(Serialize)]
struct RegistryAccountRow {
    id: u64,
    threshold: u32,
    nonce: u64,
    members_len: usize,
    members_base58: Vec<String>,
    /// Local identity names matched to member keys (same order; empty if unknown).
    member_names: Vec<String>,
    label: String,
}

async fn api_registry_accounts(
    State(state): State<Arc<AppState>>,
    Query(q): Query<RegistryAccountsQuery>,
) -> Result<Json<Vec<RegistryAccountRow>>, (StatusCode, String)> {
    mock_drawer_unavailable(&state)?;
    let scan = q.limit.clamp(1, 256);
    let next_bytes = chain::encode(&()).map_err(to_500)?;
    let next: u64 = chain::query("next_account_id", next_bytes)
        .await
        .map_err(to_500)?;
    let end = next.min(scan);
    let identities = state.identities.lock().await;
    let mut rows = Vec::new();
    for id in 0..end {
        let bytes = chain::encode(&id).map_err(to_500)?;
        let view: Option<MultisigAccountView> =
            chain::query("account", bytes).await.map_err(to_500)?;
        let Some(v) = view else {
            continue;
        };
        let members_base58: Vec<String> = v.members.iter().map(bs58_pk).collect();
        let member_names: Vec<String> = v
            .members
            .iter()
            .map(|pk| {
                identities
                    .iter()
                    .find(|i| i.pk.to_bytes() == pk.to_bytes())
                    .map(|i| i.name.clone())
                    .unwrap_or_default()
            })
            .collect();
        let named: Vec<&str> = member_names
            .iter()
            .map(|n| if n.is_empty() { "?" } else { n.as_str() })
            .collect();
        let label = format!(
            "account {id} · {thr}-of-{n} · {}",
            named.join(", "),
            thr = v.threshold,
            n = v.members.len()
        );
        rows.push(RegistryAccountRow {
            id,
            threshold: v.threshold,
            nonce: v.nonce,
            members_len: v.members.len(),
            members_base58,
            member_names,
            label,
        });
    }
    Ok(Json(rows))
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
    mock_drawer_unavailable(&state)?;
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
    mock_drawer_unavailable(&state)?;
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
    mock_drawer_unavailable(&state)?;
    api_quorum_check(State(state), Json(req)).await
}

async fn api_quorum_agg_submit(
    State(state): State<Arc<AppState>>,
    Json(req): Json<QuorumSubmitReq>,
) -> Result<Json<SubmitOut>, (StatusCode, String)> {
    mock_drawer_unavailable(&state)?;
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
    mock_drawer_unavailable(&state)?;
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
    mock_drawer_unavailable(&state)?;
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
    State(state): State<Arc<AppState>>,
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
    if state.demo_mode == DemoMode::Mock {
        let mut mock = state.mock.lock().await;
        let before = mock.next_proposal_id();
        let id = mock
            .create_proposal(
                req.account,
                target_bytes,
                req.function,
                call_args,
                req.deadline,
                MOCK_CHAIN_ID,
            )
            .map_err(to_400)?;
        return Ok(Json(ProposalCreateOut {
            submit: mock_ok_submit(
                format!("mock: propose id={id} account={}", req.account),
                format!("mock-propose-{id}"),
            ),
            allocated_id_hint: before,
        }));
    }
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
    /// Must be true — preview first, then confirm before signing.
    #[serde(default)]
    confirm: bool,
}

#[derive(Serialize)]
struct SignPreviewOut {
    digest_hex: String,
    digest_mnemonic: String,
    digest_safety_number: String,
    chain_id: u64,
    committee_id: u64,
    nonce: u64,
    target_hex: String,
    function_name: String,
    call_args_hex: String,
    deadline: u64,
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
    digest_mnemonic: String,
    digest_safety_number: String,
}

fn proposal_preview_from_view(view: &ProposalView) -> Result<SignPreviewOut, (StatusCode, String)> {
    if view.status != ProposalStatus::Open {
        return Err((
            StatusCode::BAD_REQUEST,
            "proposal is not Open".into(),
        ));
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
            "REFUSING: on-chain digest does not match recomputed intent".into(),
        )
    })?;
    Ok(SignPreviewOut {
        digest_hex: format!("0x{}", hex::encode(digest)),
        digest_mnemonic: multisig_encoding::digest_mnemonic(&digest),
        digest_safety_number: multisig_encoding::digest_safety_number(&digest),
        chain_id: intent.chain_id,
        committee_id: intent.committee_id,
        nonce: intent.nonce,
        target_hex: format!("0x{}", hex::encode(intent.target_contract_id)),
        function_name: intent.function_name,
        call_args_hex: format!("0x{}", hex::encode(&intent.call_args)),
        deadline: intent.deadline,
    })
}

async fn api_proposal_preview(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<u64>,
) -> Result<Json<SignPreviewOut>, (StatusCode, String)> {
    if state.demo_mode == DemoMode::Mock {
        let mock = state.mock.lock().await;
        let p = mock
            .proposal(id)
            .ok_or_else(|| (StatusCode::BAD_REQUEST, format!("proposal {id} not found")))?;
        return Ok(Json(mock_proposal_preview(&p)?));
    }
    let view: Option<ProposalView> = chain::query_contract(
        chain::Contract::Proposals,
        "proposal",
        chain::encode(&id).map_err(to_500)?,
    )
    .await
    .map_err(to_500)?;
    let view = view.ok_or_else(|| (StatusCode::BAD_REQUEST, format!("proposal {id} not found")))?;
    Ok(Json(proposal_preview_from_view(&view)?))
}

/// Preview a collector proposals-kind blob (no signing).
async fn api_blob_preview(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<String>,
) -> Result<Json<SignPreviewOut>, (StatusCode, String)> {
    mock_drawer_unavailable(&state)?;
    let client = CollectorClient::resolve(None).map_err(to_500)?;
    let file = client.pull(&id).await.map_err(to_500)?;
    let proposal = file.to_proposal_blob().map_err(to_500)?;
    let digest = multisig_encoding::gate_blob_for_signing(&proposal).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "REFUSING: signed_digest does not match recomputed §4a digest".into(),
        )
    })?;
    let i = &proposal.intent.intent;
    Ok(Json(SignPreviewOut {
        digest_hex: format!("0x{}", hex::encode(digest)),
        digest_mnemonic: multisig_encoding::digest_mnemonic(&digest),
        digest_safety_number: multisig_encoding::digest_safety_number(&digest),
        chain_id: i.chain_id,
        committee_id: i.committee_id,
        nonce: i.nonce,
        target_hex: format!("0x{}", hex::encode(i.target_contract_id)),
        function_name: i.function_name.clone(),
        call_args_hex: format!("0x{}", hex::encode(&i.call_args)),
        deadline: i.deadline,
    }))
}

async fn api_proposal_approve(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<u64>,
    Json(req): Json<ProposalApproveReq>,
) -> Result<Json<ProposalApproveOut>, (StatusCode, String)> {
    if !req.confirm {
        return Err((
            StatusCode::BAD_REQUEST,
            "confirm required — call /preview first, then POST with confirm:true".into(),
        ));
    }

    if state.demo_mode == DemoMode::Mock {
        let mock_p = {
            let mock = state.mock.lock().await;
            mock.proposal(id)
                .ok_or_else(|| (StatusCode::BAD_REQUEST, format!("proposal {id} not found")))?
        };
        if mock_p.status != MockProposalStatus::Open {
            return Err((StatusCode::BAD_REQUEST, format!("proposal {id} is not Open")));
        }
        let intent = multisig_encoding::ProposalIntent {
            chain_id: mock_p.chain_id,
            committee_id: mock_p.registry_account_id,
            nonce: mock_p.nonce,
            target_contract_id: mock_p.target,
            function_name: mock_p.function_name.clone(),
            call_args: mock_p.call_args.clone(),
            deadline: mock_p.deadline,
        };
        let digest =
            multisig_encoding::recompute_and_verify(&intent, &mock_p.digest).map_err(|_| {
                (
                    StatusCode::BAD_REQUEST,
                    "REFUSING TO SIGN: mock digest does not match recomputed intent".into(),
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
            digest_mnemonic: multisig_encoding::digest_mnemonic(&digest),
            digest_safety_number: multisig_encoding::digest_safety_number(&digest),
        };

        let identities = state.identities.lock().await;
        let id_rec = identities
            .iter()
            .find(|i| i.name == req.signer)
            .ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    format!("no identity named '{}'", req.signer),
                )
            })?;
        let sk = id_rec
            .require_sk()
            .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
        // Real secure BLS sign of the digest (signature discarded after membership record).
        let _signature = bls::sign(sk, &digest);
        let pk_bytes = id_rec.pk.to_bytes();
        drop(identities);

        let mut mock = state.mock.lock().await;
        mock.approve(id, pk_bytes).map_err(to_400)?;
        return Ok(Json(ProposalApproveOut {
            submit: mock_ok_submit(
                format!("mock: approve proposal {id} by {}", req.signer),
                format!("mock-approve-{id}"),
            ),
            intent: intent_out,
        }));
    }

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
        digest_mnemonic: multisig_encoding::digest_mnemonic(&digest),
        digest_safety_number: multisig_encoding::digest_safety_number(&digest),
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
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<u64>,
) -> Result<Json<Option<ProposalStatusOut>>, (StatusCode, String)> {
    if state.demo_mode == DemoMode::Mock {
        let mock = state.mock.lock().await;
        return Ok(Json(mock.proposal(id).map(|p| mock_proposal_status_out(id, p))));
    }
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
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<u64>,
) -> Result<Json<SubmitOut>, (StatusCode, String)> {
    if state.demo_mode == DemoMode::Mock {
        let mut mock = state.mock.lock().await;
        mock.finalize(id).map_err(to_400)?;
        return Ok(Json(mock_ok_submit(
            format!("mock: finalize proposal {id}"),
            format!("mock-finalize-{id}"),
        )));
    }
    let bytes = chain::encode(&id).map_err(to_500)?;
    let result =
        tokio::task::spawn_blocking(move || chain::submit_call_to(chain::Contract::Proposals, "finalize", &bytes))
            .await
            .map_err(to_500)?
            .map_err(to_500)?;
    Ok(Json(submit_from_log(result.stdout)))
}

async fn api_proposal_next_id(
    State(state): State<Arc<AppState>>,
) -> Result<Json<u64>, (StatusCode, String)> {
    if state.demo_mode == DemoMode::Mock {
        let mock = state.mock.lock().await;
        return Ok(Json(mock.next_proposal_id()));
    }
    let next: u64 = chain::query_contract(
        chain::Contract::Proposals,
        "next_proposal_id",
        chain::encode(&()).map_err(to_500)?,
    )
    .await
    .map_err(to_500)?;
    Ok(Json(next))
}

fn build_router(state: Arc<AppState>) -> Router {
    let api = Router::new()
        .route("/api/setup/status", get(api_setup_status))
        .route("/api/party", get(api_party_list).post(api_party_signup))
        .route("/api/identities", get(api_list_identities).post(api_new_identity))
        .route("/api/identities/import-pk", post(api_import_pk))
        .route("/api/account/create", post(api_account_create))
        .route("/api/account/{id}", get(api_account_query))
        .route("/api/account/{id}/meta", get(api_account_meta))
        .route("/api/account/{id}/keys", get(api_account_keys))
        .route("/api/account/next-id", get(api_account_next_id))
        .route("/api/registry/accounts", get(api_registry_accounts))
        .route("/api/quorum/submit", post(api_quorum_submit))
        .route("/api/quorum/check", post(api_quorum_check))
        .route("/api/quorum/diagnose", post(api_quorum_diagnose))
        .route("/api/quorum-agg/submit", post(api_quorum_agg_submit))
        .route("/api/quorum-agg/check", post(api_quorum_agg_check))
        .route("/api/change-account/submit", post(api_change_account_submit))
        .route("/api/proposal/create", post(api_proposal_create))
        .route("/api/proposal/{id}/preview", get(api_proposal_preview))
        .route("/api/proposal/{id}/approve", post(api_proposal_approve))
        .route("/api/proposal/{id}", get(api_proposal_status))
        .route("/api/proposal/{id}/finalize", post(api_proposal_finalize))
        .route("/api/proposal/next-id", get(api_proposal_next_id))
        .route("/api/blob/{id}/preview", get(api_blob_preview))
        .route_layer(axum::middleware::from_fn_with_state(state.clone(), require_token));

    Router::new()
        .route("/", get(index))
        .route("/app.js", get(app_js))
        .route("/mock-ledger.js", get(mock_ledger_js))
        .route("/style.css", get(style_css))
        .route("/fonts.css", get(fonts_css))
        .route("/fonts/{file}", get(serve_font))
        .route("/lab/fonts.css", get(lab_fonts_css))
        .route("/lab/tokens.css", get(lab_tokens_css))
        .route("/lab/layout.css", get(lab_layout_css))
        .route("/lab/components.css", get(lab_components_css))
        .route("/lab/fonts/{file}", get(serve_lab_font))
        .merge(api)
        .with_state(state)
}

#[cfg(test)]
mod generic_rpc_smoke {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    const TEST_TOKEN: &str = "fixed-smoke-test-token";

    fn test_state_with_identities(names: &[&str]) -> Arc<AppState> {
        let identities: Vec<keystore::Identity> = names
            .iter()
            .map(|name| keystore::generate(name))
            .collect();
        Arc::new(AppState {
            identities: Mutex::new(identities),
            password: "smoke-test-password".into(),
            store_path: PathBuf::from("/tmp/multisig-tool-smoke-identities.dat"),
            token: TEST_TOKEN.into(),
            demo_mode: DemoMode::Mock,
            mock: Mutex::new(MockLedger::new()),
        })
    }

    fn token_header() -> (&'static str, &'static str) {
        ("X-Multisig-Tool-Token", TEST_TOKEN)
    }

    async fn oneshot_json(
        app: Router,
        method: &str,
        uri: &str,
        body: Option<String>,
    ) -> (StatusCode, String) {
        let (name, value) = token_header();
        let mut builder = Request::builder().method(method).uri(uri).header(name, value);
        let req = if let Some(json) = body {
            builder = builder.header("content-type", "application/json");
            builder.body(Body::from(json)).unwrap()
        } else {
            builder.body(Body::empty()).unwrap()
        };
        let resp = app.oneshot(req).await.expect("oneshot");
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("body");
        (status, String::from_utf8(bytes.to_vec()).expect("utf8 body"))
    }

    #[tokio::test]
    async fn setup_status_mock_mode_and_token_gate() {
        let state = test_state_with_identities(&["alice"]);
        let app = build_router(state);

        let no_token = Request::builder()
            .method("GET")
            .uri("/api/setup/status")
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(no_token).await.expect("oneshot");
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        let (status, body) = oneshot_json(app, "GET", "/api/setup/status", None).await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).expect("json");
        assert_eq!(json["demo_mode"], "mock");
        assert_eq!(json["identities_count"], 1);
    }

    #[tokio::test]
    async fn mock_account_proposal_preview_approve_finalize_smoke() {
        let state = test_state_with_identities(&["alice", "bob", "carol"]);
        let app = build_router(state);

        let create_account = serde_json::json!({
            "members": ["alice", "bob", "carol"],
            "threshold": 2
        });
        let (status, body) = oneshot_json(
            app.clone(),
            "POST",
            "/api/account/create",
            Some(create_account.to_string()),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "create account: {body}");
        let submit: serde_json::Value = serde_json::from_str(&body).expect("submit json");
        assert_eq!(submit["outcome"], "ok");
        assert!(submit["tx_hash"]
            .as_str()
            .unwrap_or("")
            .starts_with("mock-create-account-"));

        let (status, next_body) = oneshot_json(app.clone(), "GET", "/api/proposal/next-id", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(next_body.trim(), "0");

        let target = format!("0x{}", "11".repeat(32));
        let create_proposal = serde_json::json!({
            "account": 0,
            "target": target,
            "function": "set_value",
            "args_hex": "0x0708",
            "deadline": 1000
        });
        let (status, body) = oneshot_json(
            app.clone(),
            "POST",
            "/api/proposal/create",
            Some(create_proposal.to_string()),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "create proposal: {body}");
        let created: serde_json::Value = serde_json::from_str(&body).expect("created json");
        assert_eq!(created["outcome"], "ok");
        assert_eq!(created["allocated_id_hint"], 0);

        let (status, preview_body) =
            oneshot_json(app.clone(), "GET", "/api/proposal/0/preview", None).await;
        assert_eq!(status, StatusCode::OK, "preview: {preview_body}");
        let preview: serde_json::Value = serde_json::from_str(&preview_body).expect("preview json");
        assert!(preview["digest_hex"].as_str().unwrap_or("").starts_with("0x"));
        assert_eq!(preview["function_name"], "set_value");

        let approve_alice = serde_json::json!({ "signer": "alice", "confirm": true });
        let (status, body) = oneshot_json(
            app.clone(),
            "POST",
            "/api/proposal/0/approve",
            Some(approve_alice.to_string()),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "approve alice: {body}");
        let approved: serde_json::Value = serde_json::from_str(&body).expect("approve json");
        assert_eq!(approved["outcome"], "ok");
        assert_eq!(approved["intent"]["function"], "set_value");

        let (status, status_body) = oneshot_json(app.clone(), "GET", "/api/proposal/0", None).await;
        assert_eq!(status, StatusCode::OK);
        let prop: serde_json::Value = serde_json::from_str(&status_body).expect("status json");
        assert_eq!(prop["status"], "Open");
        assert_eq!(prop["approvals_len"], 1);

        let approve_bob = serde_json::json!({ "signer": "bob", "confirm": true });
        let (status, body) = oneshot_json(
            app.clone(),
            "POST",
            "/api/proposal/0/approve",
            Some(approve_bob.to_string()),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "approve bob: {body}");

        let (status, body) = oneshot_json(app.clone(), "POST", "/api/proposal/0/finalize", None).await;
        assert_eq!(status, StatusCode::OK, "finalize: {body}");
        let finalized: serde_json::Value = serde_json::from_str(&body).expect("finalize json");
        assert_eq!(finalized["tx_hash"], "mock-finalize-0");

        let (status, status_body) = oneshot_json(app, "GET", "/api/proposal/0", None).await;
        assert_eq!(status, StatusCode::OK);
        let prop: serde_json::Value = serde_json::from_str(&status_body).expect("final status json");
        assert_eq!(prop["status"], "Executed");
        assert_eq!(prop["approvals_len"], 2);
    }

    #[tokio::test]
    async fn approve_without_confirm_is_rejected() {
        let state = test_state_with_identities(&["alice"]);
        let app = build_router(state);

        let target = format!("0x{}", "22".repeat(32));
        let create_proposal = serde_json::json!({
            "account": 0,
            "target": target,
            "function": "noop",
            "args_hex": "",
            "deadline": 0
        });
        let create_account = serde_json::json!({
            "members": ["alice"],
            "threshold": 1
        });
        oneshot_json(
            app.clone(),
            "POST",
            "/api/account/create",
            Some(create_account.to_string()),
        )
        .await;
        oneshot_json(
            app.clone(),
            "POST",
            "/api/proposal/create",
            Some(create_proposal.to_string()),
        )
        .await;

        let approve = serde_json::json!({ "signer": "alice", "confirm": false });
        let (status, body) = oneshot_json(
            app,
            "POST",
            "/api/proposal/0/approve",
            Some(approve.to_string()),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(body.contains("confirm required"));
    }
}
