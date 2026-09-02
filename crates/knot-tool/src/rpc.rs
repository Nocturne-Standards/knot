//! Local RPC + web UI server. `127.0.0.1`-only by construction (refuses any
//! other bind address), session-cookie-gated on every `/api/*` route. Signing
//! happens here, server-side, using identities decrypted into memory once at
//! startup (password prompted once) — the browser JS never receives a
//! secret key, only names/public keys/signatures/messages.
//!
//! Auth (R1): one-shot OTP in `/?code=…` sets an HttpOnly `SameSite=Strict`
//! session cookie. HTML never embeds the session secret. `/api/*` accepts the
//! cookie (primary) or `X-Knot-Token` header matching the session (secondary,
//! for programmatic tests).

use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

use anyhow::{Result, bail};
use axum::extract::{Path as AxPath, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use dusk_bytes::Serializable;
use dusk_core::abi::ContractId;
use dusk_core::signatures::bls::PublicKey as BlsPublicKey;
use rand::RngCore;
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;
use tokio::sync::Mutex;

use crate::proposals_types::call_types::{ApproveArgs, ProposalStatus, ProposalView, ProposeArgs};
use crate::registry_types::call_types::{
    ChangeAccountArgs, CreateAccountArgs, MultisigAccountView, SignatureEntry,
    VerifyQuorumAggregateArgs, VerifyQuorumArgs,
};
use crate::{chain, keystore};
use knot_encoding::call_types::DiagnoseQuorumResult;
use knot_tool::blob;
use knot_tool::bls;
use knot_tool::collector_client::{self, CollectorClient};
use knot_tool::diagnose;
use knot_tool::membership;
use knot_tool::mock_ledger::{
    DemoMode, MOCK_CHAIN_ID, MOCK_PROPOSALS_SELF_ID, MOCK_REGISTRY_SELF_ID, MockLedger,
    MockProposal, MockProposalStatus,
};

/// Chain id baked into mock proposals (matches live testnet `init_chain_id`).
const SESSION_COOKIE: &str = "knot_session";

/// R4: fixed API error catalog — variable / wallet / `Display` details go to stderr only.
#[derive(Clone, Serialize)]
struct RpcErrorBody {
    code: &'static str,
    message: &'static str,
}

struct RpcError {
    status: StatusCode,
    body: RpcErrorBody,
}

impl RpcError {
    fn catalog(status: StatusCode, code: &'static str, message: &'static str) -> Self {
        Self {
            status,
            body: RpcErrorBody { code, message },
        }
    }

    fn logged(
        status: StatusCode,
        code: &'static str,
        message: &'static str,
        detail: impl std::fmt::Display,
    ) -> Self {
        eprintln!("knot-tool rpc [{code}]: {detail}");
        Self::catalog(status, code, message)
    }

    fn unauthorized() -> Self {
        Self::catalog(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "Session missing or invalid.",
        )
    }

    fn identity_exists(name: &str) -> Self {
        Self::logged(
            StatusCode::BAD_REQUEST,
            "identity_exists",
            "Identity already exists.",
            format!("identity '{name}'"),
        )
    }

    fn identity_not_found(name: &str) -> Self {
        Self::logged(
            StatusCode::BAD_REQUEST,
            "identity_not_found",
            "Identity not found.",
            format!("identity '{name}'"),
        )
    }

    fn account_not_found(account_id: u64) -> Self {
        Self::logged(
            StatusCode::BAD_REQUEST,
            "account_not_found",
            "Account not found.",
            format!("account {account_id}"),
        )
    }

    fn proposal_not_found(id: u64) -> Self {
        Self::logged(
            StatusCode::BAD_REQUEST,
            "proposal_not_found",
            "Proposal not found.",
            format!("proposal {id}"),
        )
    }

    fn proposal_not_open(id: u64) -> Self {
        Self::logged(
            StatusCode::BAD_REQUEST,
            "proposal_not_open",
            "Proposal is not open.",
            format!("proposal {id}"),
        )
    }

    fn not_a_member(detail: impl std::fmt::Display) -> Self {
        Self::logged(
            StatusCode::FORBIDDEN,
            "not_a_member",
            "Signer is not a committee member.",
            detail,
        )
    }

    fn invalid_input(detail: impl std::fmt::Display) -> Self {
        Self::logged(
            StatusCode::BAD_REQUEST,
            "invalid_input",
            "Invalid request.",
            detail,
        )
    }

    fn invalid_hex(detail: impl std::fmt::Display) -> Self {
        Self::logged(
            StatusCode::BAD_REQUEST,
            "invalid_hex",
            "Invalid hex encoding.",
            detail,
        )
    }

    fn invalid_target() -> Self {
        Self::catalog(
            StatusCode::BAD_REQUEST,
            "invalid_target",
            "Target must be 32-byte hex.",
        )
    }

    fn digest_mismatch(detail: &'static str) -> Self {
        Self::logged(
            StatusCode::BAD_REQUEST,
            "digest_mismatch",
            "Digest does not match recomputed intent.",
            detail,
        )
    }

    fn confirm_required() -> Self {
        Self::catalog(
            StatusCode::BAD_REQUEST,
            "confirm_required",
            "Confirm required — call preview first, then POST with confirm:true.",
        )
    }

    fn live_mode_required() -> Self {
        Self::catalog(
            StatusCode::NOT_IMPLEMENTED,
            "live_mode_required",
            "This action requires live testnet mode (DEMO_MODE=testnet).",
        )
    }

    fn collector_config(detail: impl std::fmt::Display) -> Self {
        Self::logged(
            StatusCode::BAD_REQUEST,
            "collector_config",
            "Collector is not configured.",
            detail,
        )
    }

    fn internal(detail: impl std::fmt::Display) -> Self {
        Self::logged(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "An internal error occurred.",
            detail,
        )
    }
}

impl IntoResponse for RpcError {
    fn into_response(self) -> Response {
        (self.status, Json(self.body)).into_response()
    }
}

type ApiResult<T> = Result<T, RpcError>;

struct AppState {
    identities: Mutex<Vec<keystore::Identity>>,
    password: String,
    store_path: PathBuf,
    /// One-shot bootstrap code; consumed on first successful `/?code=…`.
    otp: Mutex<Option<String>>,
    /// Session secret carried in HttpOnly cookie (and optional `X-Knot-Token`).
    session_token: String,
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

/// Parse `bind` and refuse unless the address is loopback (R12).
pub fn validate_loopback_bind(bind: &str) -> Result<std::net::SocketAddr> {
    let addr: std::net::SocketAddr = bind
        .parse()
        .map_err(|e| anyhow::anyhow!("invalid bind address '{bind}': {e}"))?;
    if !addr.ip().is_loopback() {
        bail!(
            "refusing to bind to '{bind}' — knot-tool only serves on loopback \
             (127.0.0.1 or ::1; see README.md)"
        );
    }
    Ok(addr)
}

pub async fn serve_with_options(bind: &str, store_path: PathBuf, opts: ServeOptions) -> Result<()> {
    let addr = validate_loopback_bind(bind)?;

    let password = crate::prompt_password()?;
    let identities = keystore::load(&store_path, &password)?;

    let demo_mode = DemoMode::from_env().map_err(anyhow::Error::msg)?;
    eprintln!("════════════════════════════════════════════════════════");
    eprintln!(
        "  DEMO_MODE={} — {}",
        demo_mode.as_str(),
        demo_mode.serve_banner_label()
    );
    eprintln!("════════════════════════════════════════════════════════");

    let mut otp_bytes = [0u8; 16];
    OsRng.fill_bytes(&mut otp_bytes);
    let otp = hex::encode(otp_bytes);
    let mut session_bytes = [0u8; 32];
    OsRng.fill_bytes(&mut session_bytes);
    let session_token = hex::encode(session_bytes);
    let state = Arc::new(AppState {
        identities: Mutex::new(identities),
        password,
        store_path,
        otp: Mutex::new(Some(otp.clone())),
        session_token,
        demo_mode,
        mock: Mutex::new(MockLedger::new()),
    });

    let app = build_router(state);

    let mut url = format!("http://{addr}/?code={otp}");
    if let Some(extra) = &opts.query_extra
        && !extra.is_empty()
    {
        url.push('&');
        url.push_str(extra.trim_start_matches('&').trim_start_matches('?'));
    }
    if let Some(tab) = &opts.open_tab {
        url.push('#');
        url.push_str(tab.trim_start_matches('#'));
    }
    eprintln!("knot-tool listening on http://{addr}/");
    eprintln!("Bootstrap session: open {url}");
    eprintln!("/api/* requires session cookie (set by bootstrap) or X-Knot-Token header.");
    let _ = std::io::stderr().flush();
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

fn constant_time_eq(got: &str, want: &str) -> bool {
    let got = got.as_bytes();
    let want = want.as_bytes();
    got.len() == want.len() && bool::from(got.ct_eq(want))
}

fn session_from_cookie(headers: &HeaderMap) -> Option<&str> {
    let cookie_header = headers.get(header::COOKIE)?.to_str().ok()?;
    cookie_header.split(';').find_map(|pair| {
        let pair = pair.trim();
        let (name, value) = pair.split_once('=')?;
        if name == SESSION_COOKIE {
            Some(value)
        } else {
            None
        }
    })
}

fn session_authorized(headers: &HeaderMap, session_token: &str) -> bool {
    session_from_cookie(headers)
        .map(|v| constant_time_eq(v, session_token))
        .unwrap_or(false)
        || headers
            .get("X-Knot-Token")
            .and_then(|v| v.to_str().ok())
            .map(|v| constant_time_eq(v, session_token))
            .unwrap_or(false)
}

fn session_cookie_value(session_token: &str) -> Result<axum::http::HeaderValue, ()> {
    axum::http::HeaderValue::from_str(&format!(
        "{SESSION_COOKIE}={session_token}; HttpOnly; SameSite=Strict; Path=/"
    ))
    .map_err(|_| ())
}

async fn require_session(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    if !session_authorized(&headers, &state.session_token) {
        return RpcError::unauthorized().into_response();
    }
    next.run(request).await
}

#[derive(Deserialize, Default)]
struct IndexQuery {
    code: Option<String>,
}

async fn index(State(state): State<Arc<AppState>>, Query(q): Query<IndexQuery>) -> Response {
    if let Some(code) = q.code {
        let mut otp_guard = state.otp.lock().await;
        let valid = otp_guard
            .as_ref()
            .map(|otp| constant_time_eq(&code, otp))
            .unwrap_or(false);
        if !valid {
            return (
                StatusCode::UNAUTHORIZED,
                "invalid or expired bootstrap code",
            )
                .into_response();
        }
        *otp_guard = None;
        let cookie = match session_cookie_value(&state.session_token) {
            Ok(v) => v,
            Err(()) => {
                return (StatusCode::INTERNAL_SERVER_ERROR, "session cookie error").into_response();
            }
        };
        return (
            StatusCode::SEE_OTHER,
            [(header::SET_COOKIE, cookie)],
            Redirect::to("/"),
        )
            .into_response();
    }

    let template = include_str!("../static/index.html");
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        template,
    )
        .into_response()
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
    (
        [(header::CONTENT_TYPE, "text/css")],
        include_str!("../static/style.css"),
    )
}

async fn fonts_css() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css")],
        include_str!("../static/fonts.css"),
    )
}

async fn lab_fonts_css() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css")],
        include_str!("../static/lab/fonts.css"),
    )
}

async fn lab_tokens_css() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css")],
        include_str!("../static/lab/tokens.css"),
    )
}

async fn lab_layout_css() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css")],
        include_str!("../static/lab/layout.css"),
    )
}

async fn lab_components_css() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css")],
        include_str!("../static/lab/components.css"),
    )
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
) -> ApiResult<Json<IdentityOut>> {
    let mut identities = state.identities.lock().await;
    if identities.iter().any(|i| i.name == req.name) {
        return Err(RpcError::identity_exists(&req.name));
    }
    let identity = keystore::generate(&req.name);
    let out = IdentityOut {
        name: identity.name.clone(),
        pk_base58: bs58_pk(&identity.pk),
        pk_only: false,
    };
    identities.push(identity);
    keystore::save(&state.store_path, &state.password, &identities).map_err(RpcError::internal)?;
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
) -> ApiResult<Json<IdentityOut>> {
    let mut identities = state.identities.lock().await;
    if identities.iter().any(|i| i.name == req.name) {
        return Err(RpcError::identity_exists(&req.name));
    }
    let pk = keystore::parse_pk(&req.pk).map_err(RpcError::invalid_input)?;
    let identity = keystore::from_pk_only(&req.name, pk);
    let out = IdentityOut {
        name: identity.name.clone(),
        pk_base58: bs58_pk(&identity.pk),
        pk_only: true,
    };
    identities.push(identity);
    keystore::save(&state.store_path, &state.password, &identities).map_err(RpcError::internal)?;
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
    /// `"mock"` | `"testnet"` — matches active `DEMO_MODE` (must be set explicitly).
    demo_mode: &'static str,
}

/// Server-side-only env read — the URL is shown to the browser (harmless),
/// but `KNOT_COLLECTOR_PASSWORD` never is; only whether a user was set.
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

fn mock_drawer_unavailable(state: &AppState) -> ApiResult<()> {
    if state.demo_mode == DemoMode::Mock {
        Err(RpcError::live_mode_required())
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

fn mock_proposal_preview(p: &MockProposal) -> ApiResult<SignPreviewOut> {
    if p.status != MockProposalStatus::Open {
        return Err(RpcError::catalog(
            StatusCode::BAD_REQUEST,
            "proposal_not_open",
            "Proposal is not open.",
        ));
    }
    let intent = knot_encoding::ProposalIntentV3 {
        chain_id: MOCK_CHAIN_ID,
        self_id: MOCK_PROPOSALS_SELF_ID,
        epoch: p.epoch,
        committee_id: p.registry_account_id,
        nonce: p.nonce,
        target_contract_id: p.target,
        function_name: p.function_name.clone(),
        call_args: p.call_args.clone(),
        deadline: p.deadline,
    };
    let digest = knot_encoding::recompute_and_verify_v3(&intent, &p.digest)
        .map_err(|_| RpcError::digest_mismatch("mock digest mismatch"))?;
    Ok(SignPreviewOut {
        digest_hex: format!("0x{}", hex::encode(digest)),
        digest_mnemonic: knot_encoding::digest_mnemonic(&digest),
        digest_safety_number: knot_encoding::digest_safety_number(&digest),
        chain_id: intent.chain_id,
        epoch: intent.epoch,
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
        MockProposalStatus::Queued => "Queued",
        MockProposalStatus::Cancelled => "Cancelled",
    };
    ProposalStatusOut {
        id,
        status: status.into(),
        registry_account_id: p.registry_account_id,
        epoch: p.epoch,
        nonce: p.nonce,
        target: format!("0x{}", hex::encode(p.target)),
        function: p.function_name,
        call_args_hex: format!("0x{}", hex::encode(&p.call_args)),
        deadline: p.deadline,
        execute_at: p.execute_at,
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

/// Party finder proxy — `knot-tool serve` holds `KNOT_COLLECTOR_*`
/// in its own process env; the browser only ever sees names/pks/notes here,
/// never the collector's Basic Auth password.
async fn api_party_list(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<Vec<PartyMemberOut>>> {
    mock_drawer_unavailable(&state)?;
    let client = CollectorClient::resolve(None).map_err(RpcError::collector_config)?;
    let members = client.list_party().await.map_err(RpcError::internal)?;
    Ok(Json(
        members.into_iter().map(PartyMemberOut::from).collect(),
    ))
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
) -> ApiResult<Json<PartyMemberOut>> {
    mock_drawer_unavailable(&state)?;
    let sig = {
        let identities = state.identities.lock().await;
        let identity = identities
            .iter()
            .find(|i| i.name == req.name)
            .ok_or_else(|| RpcError::identity_not_found(&req.name))?;
        let sk = identity
            .require_sk()
            .map_err(|e| RpcError::invalid_input(e.to_string()))?;
        bls::party_signup_sig_hex(sk, &identity.pk, &req.name)
            .map_err(|e| RpcError::invalid_input(e.to_string()))?
    };
    let pk = find_pk(&state, &req.name).await?;
    let client = CollectorClient::resolve(None).map_err(RpcError::collector_config)?;
    let member = client
        .signup_party(&req.name, &bs58_pk(&pk), &sig, req.note.as_deref())
        .await
        .map_err(RpcError::internal)?;
    Ok(Json(member.into()))
}

async fn find_pk(state: &AppState, name: &str) -> ApiResult<BlsPublicKey> {
    let identities = state.identities.lock().await;
    identities
        .iter()
        .find(|i| i.name == name)
        .map(|i| i.pk)
        .ok_or_else(|| RpcError::identity_not_found(name))
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

async fn api_account_create(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateAccountReq>,
) -> ApiResult<Json<SubmitOut>> {
    let mut members = Vec::with_capacity(req.members.len());
    for name in &req.members {
        members.push(find_pk(&state, name).await?);
    }
    if state.demo_mode == DemoMode::Mock {
        let member_bytes: Vec<[u8; 96]> = members.iter().map(|pk| pk.to_bytes()).collect();
        let mut mock = state.mock.lock().await;
        let id = mock
            .create_account(member_bytes, req.threshold)
            .map_err(RpcError::invalid_input)?;
        return Ok(Json(mock_ok_submit(
            format!("mock: create_account id={id} threshold={}", req.threshold),
            format!("mock-create-account-{id}"),
        )));
    }
    let args = CreateAccountArgs {
        members,
        threshold: req.threshold,
    };
    let bytes = chain::encode(&args).map_err(RpcError::internal)?;
    let result = tokio::task::spawn_blocking(move || chain::submit_call("create_account", &bytes))
        .await
        .map_err(RpcError::internal)?
        .map_err(RpcError::internal)?;
    Ok(Json(submit_from_log(result.stdout)))
}

#[derive(Serialize)]
struct AccountView {
    threshold: u32,
    nonce: u64,
    timelock_blocks: u64,
    pending_execute_at: u64,
    members: Vec<String>,
}

async fn api_account_query(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<u64>,
) -> ApiResult<Json<Option<AccountView>>> {
    if state.demo_mode == DemoMode::Mock {
        let mock = state.mock.lock().await;
        return Ok(Json(mock.account(id).map(|v| {
            AccountView {
                threshold: v.threshold,
                nonce: v.nonce,
                timelock_blocks: v.timelock_blocks,
                pending_execute_at: v.pending_execute_at,
                members: v
                    .members
                    .iter()
                    .map(|pk| bs58::encode(pk).into_string())
                    .collect(),
            }
        })));
    }
    let bytes = chain::encode(&id).map_err(RpcError::internal)?;
    let view: Option<MultisigAccountView> = chain::query("account", bytes)
        .await
        .map_err(RpcError::internal)?;
    Ok(Json(view.map(|v| AccountView {
        threshold: v.threshold,
        nonce: v.nonce,
        timelock_blocks: v.timelock_blocks,
        pending_execute_at: v.pending.as_ref().map(|p| p.execute_at).unwrap_or(0),
        members: v.members.iter().map(bs58_pk).collect(),
    })))
}

#[derive(Serialize)]
struct MetaOut {
    threshold: u32,
    nonce: u64,
    members_len: u32,
    timelock_blocks: u64,
    pending_execute_at: u64,
}

async fn api_account_meta(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<u64>,
) -> ApiResult<Json<Option<MetaOut>>> {
    if state.demo_mode == DemoMode::Mock {
        let mock = state.mock.lock().await;
        return Ok(Json(mock.account_meta(id).map(|m| MetaOut {
            threshold: m.threshold,
            nonce: m.nonce,
            members_len: m.member_count,
            timelock_blocks: m.timelock_blocks,
            pending_execute_at: m.pending_execute_at,
        })));
    }
    let view = fetch_registry_account(state.as_ref(), id).await.ok();
    Ok(Json(view.map(|v| MetaOut {
        threshold: v.threshold,
        nonce: v.nonce,
        members_len: v.members.len() as u32,
        timelock_blocks: v.timelock_blocks,
        pending_execute_at: v.pending.as_ref().map(|p| p.execute_at).unwrap_or(0),
    })))
}

async fn api_account_keys(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<u64>,
) -> ApiResult<Json<Option<Vec<String>>>> {
    if state.demo_mode == DemoMode::Mock {
        let mock = state.mock.lock().await;
        return Ok(Json(
            mock.account(id)
                .map(|a| a.members.into_iter().map(hex::encode).collect()),
        ));
    }
    let view = fetch_registry_account(state.as_ref(), id).await.ok();
    Ok(Json(view.map(|v| {
        v.members
            .iter()
            .map(|pk| hex::encode(pk.to_bytes()))
            .collect()
    })))
}

async fn api_account_next_id(State(state): State<Arc<AppState>>) -> ApiResult<Json<u64>> {
    if state.demo_mode == DemoMode::Mock {
        let mock = state.mock.lock().await;
        return Ok(Json(mock.next_account_id()));
    }
    let bytes = chain::encode(&()).map_err(RpcError::internal)?;
    let next: u64 = chain::query("next_account_id", bytes)
        .await
        .map_err(RpcError::internal)?;
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
) -> ApiResult<Json<Vec<RegistryAccountRow>>> {
    mock_drawer_unavailable(&state)?;
    let scan = q.limit.clamp(1, 256);
    let next_bytes = chain::encode(&()).map_err(RpcError::internal)?;
    let next: u64 = chain::query("next_account_id", next_bytes)
        .await
        .map_err(RpcError::internal)?;
    let end = next.min(scan);
    let identities = state.identities.lock().await;
    let mut rows = Vec::new();
    for id in 0..end {
        let bytes = chain::encode(&id).map_err(RpcError::internal)?;
        let view: Option<MultisigAccountView> = chain::query("account", bytes)
            .await
            .map_err(RpcError::internal)?;
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
    /// Must be true on submit — call preview first, then confirm before signing.
    #[serde(default)]
    confirm: bool,
}

#[derive(Serialize)]
struct QuorumSignPreviewOut {
    account_id: u64,
    msg_hex: String,
    digest_hex: String,
    digest_mnemonic: String,
    digest_safety_number: String,
    signers: Vec<String>,
    /// Soft hint when multiple local signers are requested in one serve call.
    note: Option<String>,
}

#[derive(Serialize)]
struct ChangeAccountPreviewOut {
    account_id: u64,
    nonce: u64,
    new_members: Vec<String>,
    new_threshold: u32,
    signers: Vec<String>,
    digest_hex: String,
    digest_mnemonic: String,
    digest_safety_number: String,
    note: Option<String>,
}

fn multi_signer_serve_note(signers: &[String]) -> Option<String> {
    if signers.len() > 1 {
        Some(
            "Prefer one signer identity per serve process — run separate serve instances per member when possible."
                .into(),
        )
    } else {
        None
    }
}

fn quorum_preview_out(account_id: u64, msg: &[u8], signers: &[String]) -> QuorumSignPreviewOut {
    let (digest_hex, digest_mnemonic, digest_safety_number) = bls::message_fingerprint_display(msg);
    QuorumSignPreviewOut {
        account_id,
        msg_hex: format!("0x{}", hex::encode(msg)),
        digest_hex,
        digest_mnemonic,
        digest_safety_number,
        signers: signers.to_vec(),
        note: multi_signer_serve_note(signers),
    }
}

fn msg_bytes(msg: &str, hex_flag: bool) -> ApiResult<Vec<u8>> {
    if hex_flag {
        hex::decode(msg.trim_start_matches("0x")).map_err(RpcError::invalid_hex)
    } else {
        Ok(msg.as_bytes().to_vec())
    }
}

async fn fetch_registry_account(
    state: &AppState,
    account_id: u64,
) -> ApiResult<MultisigAccountView> {
    if state.demo_mode == DemoMode::Mock {
        let mock = state.mock.lock().await;
        let account = mock
            .account(account_id)
            .ok_or_else(|| RpcError::account_not_found(account_id))?;
        return Ok(account.to_account_view());
    }
    let bytes = chain::encode(&account_id).map_err(RpcError::internal)?;
    let view: Option<MultisigAccountView> = chain::query("account", bytes)
        .await
        .map_err(RpcError::internal)?;
    view.ok_or_else(|| RpcError::account_not_found(account_id))
}

async fn resolve_signer_pks_locked(
    state: &AppState,
    signer_names: &[String],
) -> ApiResult<Vec<BlsPublicKey>> {
    let identities = state.identities.lock().await;
    let mut pks = Vec::with_capacity(signer_names.len());
    for name in signer_names {
        let id = identities
            .iter()
            .find(|i| &i.name == name)
            .ok_or_else(|| RpcError::identity_not_found(name))?;
        pks.push(id.pk);
    }
    Ok(pks)
}

async fn ensure_signers_are_members(
    state: &AppState,
    account_id: u64,
    signer_names: &[String],
) -> ApiResult<()> {
    let view = fetch_registry_account(state, account_id).await?;
    let pks = resolve_signer_pks_locked(state, signer_names).await?;
    membership::ensure_pks_are_members(account_id, &pks, &view).map_err(RpcError::not_a_member)
}

fn ensure_pks_are_members_view(
    account_id: u64,
    signer_pks: &[BlsPublicKey],
    view: &MultisigAccountView,
) -> ApiResult<()> {
    membership::ensure_pks_are_members(account_id, signer_pks, view).map_err(RpcError::not_a_member)
}

async fn build_sigs_locked(
    state: &AppState,
    signers: &[String],
    msg: &[u8],
) -> ApiResult<Vec<SignatureEntry>> {
    let identities = state.identities.lock().await;
    let mut sigs = Vec::with_capacity(signers.len());
    for name in signers {
        let id = identities
            .iter()
            .find(|i| &i.name == name)
            .ok_or_else(|| RpcError::identity_not_found(name))?;
        let sk = id.require_sk().map_err(RpcError::invalid_input)?;
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

async fn free_read_quorum(
    state: &AppState,
    args: &VerifyQuorumArgs,
) -> ApiResult<(Option<DiagnoseOut>, Option<bool>)> {
    let view = fetch_registry_account(state, args.account_id).await.ok();
    let d = diagnose::diagnose_quorum(view.as_ref(), args);
    let diagnose = Some(diagnose_to_out(&d));
    let bytes = chain::encode(args).map_err(RpcError::internal)?;
    let check = chain::query::<bool>("verify_quorum", bytes).await.ok();
    Ok((diagnose, check))
}

async fn api_quorum_preview(
    State(state): State<Arc<AppState>>,
    Json(req): Json<QuorumSubmitReq>,
) -> ApiResult<Json<QuorumSignPreviewOut>> {
    ensure_signers_are_members(&state, req.account, &req.signers).await?;
    let msg = msg_bytes(&req.msg, req.hex)?;
    Ok(Json(quorum_preview_out(req.account, &msg, &req.signers)))
}

async fn api_quorum_submit(
    State(state): State<Arc<AppState>>,
    Json(req): Json<QuorumSubmitReq>,
) -> ApiResult<Json<SubmitOut>> {
    if !req.confirm {
        return Err(RpcError::confirm_required());
    }
    mock_drawer_unavailable(&state)?;
    ensure_signers_are_members(&state, req.account, &req.signers).await?;
    let msg = msg_bytes(&req.msg, req.hex)?;
    let sigs = build_sigs_locked(&state, &req.signers, &msg).await?;
    let args = VerifyQuorumArgs {
        account_id: req.account,
        msg,
        sigs,
    };
    let bytes = chain::encode(&args).map_err(RpcError::internal)?;
    let result = tokio::task::spawn_blocking(move || chain::submit_call("verify_quorum", &bytes))
        .await
        .map_err(RpcError::internal)?
        .map_err(RpcError::internal)?;
    let mut out = submit_from_log(result.stdout);
    out.note = Some(
        "verify_quorum returns bool with no event. Free-read follow-up may be untrusted on live testnet."
            .into(),
    );
    if let Ok((d, c)) = free_read_quorum(&state, &args).await {
        out.diagnose = d;
        out.check = c;
    }
    Ok(Json(out))
}

async fn api_quorum_check(
    State(state): State<Arc<AppState>>,
    Json(req): Json<QuorumSubmitReq>,
) -> ApiResult<Json<SubmitOut>> {
    mock_drawer_unavailable(&state)?;
    ensure_signers_are_members(&state, req.account, &req.signers).await?;
    let msg = msg_bytes(&req.msg, req.hex)?;
    let sigs = build_sigs_locked(&state, &req.signers, &msg).await?;
    let args = VerifyQuorumArgs {
        account_id: req.account,
        msg,
        sigs,
    };
    let (diagnose, check) = free_read_quorum(&state, &args).await?;
    let mut note = None;
    if diagnose
        .as_ref()
        .map(|d| d.free_read_untrusted)
        .unwrap_or(false)
    {
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
) -> ApiResult<Json<SubmitOut>> {
    mock_drawer_unavailable(&state)?;
    api_quorum_check(State(state), Json(req)).await
}

async fn api_quorum_agg_preview(
    State(state): State<Arc<AppState>>,
    Json(req): Json<QuorumSubmitReq>,
) -> ApiResult<Json<QuorumSignPreviewOut>> {
    ensure_signers_are_members(&state, req.account, &req.signers).await?;
    let msg = msg_bytes(&req.msg, req.hex)?;
    Ok(Json(quorum_preview_out(req.account, &msg, &req.signers)))
}

async fn api_quorum_agg_submit(
    State(state): State<Arc<AppState>>,
    Json(req): Json<QuorumSubmitReq>,
) -> ApiResult<Json<SubmitOut>> {
    if !req.confirm {
        return Err(RpcError::confirm_required());
    }
    mock_drawer_unavailable(&state)?;
    ensure_signers_are_members(&state, req.account, &req.signers).await?;
    let msg = msg_bytes(&req.msg, req.hex)?;
    let identities = state.identities.lock().await;
    let mut signer_keys = Vec::with_capacity(req.signers.len());
    let mut per_signer_sigs = Vec::with_capacity(req.signers.len());
    for name in &req.signers {
        let id = identities
            .iter()
            .find(|i| &i.name == name)
            .ok_or_else(|| RpcError::identity_not_found(name))?;
        let sk = id.require_sk().map_err(RpcError::invalid_input)?;
        signer_keys.push(id.pk);
        per_signer_sigs.push(bls::sign_multisig(sk, &id.pk, &msg));
    }
    drop(identities);
    let aggregate_sig = bls::aggregate(&per_signer_sigs).map_err(RpcError::invalid_input)?;

    let args = VerifyQuorumAggregateArgs {
        account_id: req.account,
        msg,
        signer_keys,
        aggregate_sig,
    };
    let bytes = chain::encode(&args).map_err(RpcError::internal)?;
    let result =
        tokio::task::spawn_blocking(move || chain::submit_call("verify_quorum_aggregate", &bytes))
            .await
            .map_err(RpcError::internal)?
            .map_err(RpcError::internal)?;
    let mut out = submit_from_log(result.stdout);
    out.note = Some(
        "Aggregate path: bool return not in wallet log; free-read check may be untrusted.".into(),
    );
    let check_bytes = chain::encode(&args).map_err(RpcError::internal)?;
    out.check = chain::query::<bool>("verify_quorum_aggregate", check_bytes)
        .await
        .ok();
    Ok(Json(out))
}

async fn api_quorum_agg_check(
    State(state): State<Arc<AppState>>,
    Json(req): Json<QuorumSubmitReq>,
) -> ApiResult<Json<SubmitOut>> {
    mock_drawer_unavailable(&state)?;
    ensure_signers_are_members(&state, req.account, &req.signers).await?;
    let msg = msg_bytes(&req.msg, req.hex)?;
    let identities = state.identities.lock().await;
    let mut signer_keys = Vec::with_capacity(req.signers.len());
    let mut per_signer_sigs = Vec::with_capacity(req.signers.len());
    for name in &req.signers {
        let id = identities
            .iter()
            .find(|i| &i.name == name)
            .ok_or_else(|| RpcError::identity_not_found(name))?;
        let sk = id.require_sk().map_err(RpcError::invalid_input)?;
        signer_keys.push(id.pk);
        per_signer_sigs.push(bls::sign_multisig(sk, &id.pk, &msg));
    }
    drop(identities);
    let aggregate_sig = bls::aggregate(&per_signer_sigs).map_err(RpcError::invalid_input)?;
    let args = VerifyQuorumAggregateArgs {
        account_id: req.account,
        msg,
        signer_keys,
        aggregate_sig,
    };
    let bytes = chain::encode(&args).map_err(RpcError::internal)?;
    let check = chain::query::<bool>("verify_quorum_aggregate", bytes)
        .await
        .ok();
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
    #[serde(default)]
    confirm: bool,
}

async fn api_change_account_preview(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ChangeAccountReq>,
) -> ApiResult<Json<ChangeAccountPreviewOut>> {
    let mut new_members = Vec::with_capacity(req.new_members.len());
    for name in &req.new_members {
        new_members.push(find_pk(&state, name).await?);
    }

    let current = if state.demo_mode == DemoMode::Mock {
        let mock = state.mock.lock().await;
        let account = mock
            .account(req.account)
            .ok_or_else(|| RpcError::account_not_found(req.account))?;
        account.to_account_view()
    } else {
        let current: Option<MultisigAccountView> = chain::query(
            "account",
            chain::encode(&req.account).map_err(RpcError::internal)?,
        )
        .await
        .map_err(RpcError::internal)?;
        current.ok_or_else(|| RpcError::account_not_found(req.account))?
    };

    let registry_self_id = if state.demo_mode == DemoMode::Mock {
        MOCK_REGISTRY_SELF_ID
    } else {
        chain::contract_self_id_bytes(chain::Contract::Registry).map_err(RpcError::internal)?
    };

    let msg = bls::change_account_message(
        &registry_self_id,
        req.account,
        current.nonce,
        &new_members,
        req.new_threshold,
    );
    ensure_pks_are_members_view(
        req.account,
        &resolve_signer_pks_locked(&state, &req.signers).await?,
        &current,
    )?;
    let (digest_hex, digest_mnemonic, digest_safety_number) =
        bls::message_fingerprint_display(&msg);
    let note = multi_signer_serve_note(&req.signers);
    Ok(Json(ChangeAccountPreviewOut {
        account_id: req.account,
        nonce: current.nonce,
        new_members: req.new_members,
        new_threshold: req.new_threshold,
        signers: req.signers,
        digest_hex,
        digest_mnemonic,
        digest_safety_number,
        note,
    }))
}

async fn api_change_account_submit(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ChangeAccountReq>,
) -> ApiResult<Json<SubmitOut>> {
    if !req.confirm {
        return Err(RpcError::confirm_required());
    }
    mock_drawer_unavailable(&state)?;
    let mut new_members = Vec::with_capacity(req.new_members.len());
    for name in &req.new_members {
        new_members.push(find_pk(&state, name).await?);
    }

    let current: Option<MultisigAccountView> = chain::query(
        "account",
        chain::encode(&req.account).map_err(RpcError::internal)?,
    )
    .await
    .map_err(RpcError::internal)?;
    let current = current.ok_or_else(|| RpcError::account_not_found(req.account))?;

    let registry_self_id =
        chain::contract_self_id_bytes(chain::Contract::Registry).map_err(RpcError::internal)?;

    let msg = bls::change_account_message(
        &registry_self_id,
        req.account,
        current.nonce,
        &new_members,
        req.new_threshold,
    );
    ensure_pks_are_members_view(
        req.account,
        &resolve_signer_pks_locked(&state, &req.signers).await?,
        &current,
    )?;
    let sigs = build_sigs_locked(&state, &req.signers, &msg).await?;

    let args = ChangeAccountArgs {
        account_id: req.account,
        new_members,
        new_threshold: req.new_threshold,
        sigs,
    };
    let bytes = chain::encode(&args).map_err(RpcError::internal)?;
    let result = tokio::task::spawn_blocking(move || chain::submit_call("change_account", &bytes))
        .await
        .map_err(RpcError::internal)?
        .map_err(RpcError::internal)?;
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
    /// Caller uniquifier (v3); CSPRNG default when omitted.
    #[serde(default)]
    nonce: Option<u64>,
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
) -> ApiResult<Json<ProposalCreateOut>> {
    let target_bytes: [u8; 32] = hex::decode(req.target.trim_start_matches("0x"))
        .map_err(RpcError::invalid_hex)?
        .as_slice()
        .try_into()
        .map_err(|_| RpcError::invalid_target())?;
    let call_args = if req.args_hex.is_empty() {
        Vec::new()
    } else {
        hex::decode(req.args_hex.trim_start_matches("0x")).map_err(RpcError::invalid_hex)?
    };
    if state.demo_mode == DemoMode::Mock {
        let mut mock = state.mock.lock().await;
        let before = mock.next_proposal_id();
        let nonce = blob::resolve_proposal_nonce(req.nonce);
        let id = mock
            .create_proposal(
                req.account,
                target_bytes,
                req.function,
                call_args,
                req.deadline,
                nonce,
            )
            .map_err(RpcError::invalid_input)?;
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
        chain::encode(&()).map_err(RpcError::internal)?,
    )
    .await
    .map_err(RpcError::internal)?;
    let nonce = blob::resolve_proposal_nonce(req.nonce);
    let args = ProposeArgs {
        registry_account_id: req.account,
        target: ContractId::from_bytes(target_bytes),
        function_name: req.function,
        call_args,
        nonce,
        deadline: req.deadline,
    };
    let bytes = chain::encode(&args).map_err(RpcError::internal)?;
    let result = tokio::task::spawn_blocking(move || {
        chain::submit_call_to(chain::Contract::Proposals, "propose", &bytes)
    })
    .await
    .map_err(RpcError::internal)?
    .map_err(RpcError::internal)?;
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
    epoch: u64,
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
    epoch: u64,
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

fn proposal_preview_from_view(
    view: &ProposalView,
    proposals_self_id: &[u8; 32],
) -> ApiResult<SignPreviewOut> {
    if view.status != ProposalStatus::Open {
        return Err(RpcError::catalog(
            StatusCode::BAD_REQUEST,
            "proposal_not_open",
            "Proposal is not open.",
        ));
    }
    let intent = bls::proposal_intent_v3_from_view(view, bls::digest_chain_id(), proposals_self_id);
    let digest = knot_encoding::recompute_and_verify_v3(&intent, &view.signed_digest)
        .map_err(|_| RpcError::digest_mismatch("on-chain digest mismatch"))?;
    Ok(SignPreviewOut {
        digest_hex: format!("0x{}", hex::encode(digest)),
        digest_mnemonic: knot_encoding::digest_mnemonic(&digest),
        digest_safety_number: knot_encoding::digest_safety_number(&digest),
        chain_id: intent.chain_id,
        epoch: intent.epoch,
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
) -> ApiResult<Json<SignPreviewOut>> {
    if state.demo_mode == DemoMode::Mock {
        let mock = state.mock.lock().await;
        let p = mock
            .proposal(id)
            .ok_or_else(|| RpcError::proposal_not_found(id))?;
        return Ok(Json(mock_proposal_preview(&p)?));
    }
    let view: Option<ProposalView> = chain::query_contract(
        chain::Contract::Proposals,
        "proposal",
        chain::encode(&id).map_err(RpcError::internal)?,
    )
    .await
    .map_err(RpcError::internal)?;
    let view = view.ok_or_else(|| RpcError::proposal_not_found(id))?;
    let proposals_self_id =
        chain::contract_self_id_bytes(chain::Contract::Proposals).map_err(RpcError::internal)?;
    Ok(Json(proposal_preview_from_view(&view, &proposals_self_id)?))
}

/// Preview a collector proposals-kind blob (no signing).
async fn api_blob_preview(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<String>,
) -> ApiResult<Json<SignPreviewOut>> {
    mock_drawer_unavailable(&state)?;
    let client = CollectorClient::resolve(None).map_err(RpcError::collector_config)?;
    let file = client.pull(&id).await.map_err(RpcError::internal)?;
    let proposal = file.to_proposal_blob().map_err(RpcError::internal)?;
    let digest = blob::gate_blob(&proposal).map_err(|e| match e {
        blob::GateError::DigestMismatch => RpcError::digest_mismatch("blob §4a digest mismatch"),
        blob::GateError::Encoding(enc) => RpcError::invalid_input(enc.to_string()),
    })?;
    let i = &proposal.intent.intent;
    Ok(Json(SignPreviewOut {
        digest_hex: format!("0x{}", hex::encode(digest)),
        digest_mnemonic: knot_encoding::digest_mnemonic(&digest),
        digest_safety_number: knot_encoding::digest_safety_number(&digest),
        chain_id: i.chain_id,
        epoch: 0,
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
) -> ApiResult<Json<ProposalApproveOut>> {
    if !req.confirm {
        return Err(RpcError::confirm_required());
    }

    if state.demo_mode == DemoMode::Mock {
        let mock_p = {
            let mock = state.mock.lock().await;
            mock.proposal(id)
                .ok_or_else(|| RpcError::proposal_not_found(id))?
        };
        if mock_p.status != MockProposalStatus::Open {
            return Err(RpcError::proposal_not_open(id));
        }
        let intent = knot_encoding::ProposalIntentV3 {
            chain_id: MOCK_CHAIN_ID,
            self_id: MOCK_PROPOSALS_SELF_ID,
            epoch: mock_p.epoch,
            committee_id: mock_p.registry_account_id,
            nonce: mock_p.nonce,
            target_contract_id: mock_p.target,
            function_name: mock_p.function_name.clone(),
            call_args: mock_p.call_args.clone(),
            deadline: mock_p.deadline,
        };
        let digest = knot_encoding::recompute_and_verify_v3(&intent, &mock_p.digest)
            .map_err(|_| RpcError::digest_mismatch("mock approve digest mismatch"))?;
        ensure_signers_are_members(
            &state,
            mock_p.registry_account_id,
            std::slice::from_ref(&req.signer),
        )
        .await?;
        let intent_out = IntentDisplay {
            chain_id: intent.chain_id,
            epoch: intent.epoch,
            committee_id: intent.committee_id,
            nonce: intent.nonce,
            target: format!("0x{}", hex::encode(intent.target_contract_id)),
            function: intent.function_name.clone(),
            call_args_hex: format!("0x{}", hex::encode(&intent.call_args)),
            deadline: intent.deadline,
            digest_hex: format!("0x{}", hex::encode(digest)),
            digest_mnemonic: knot_encoding::digest_mnemonic(&digest),
            digest_safety_number: knot_encoding::digest_safety_number(&digest),
        };

        let identities = state.identities.lock().await;
        let id_rec = identities
            .iter()
            .find(|i| i.name == req.signer)
            .ok_or_else(|| RpcError::identity_not_found(&req.signer))?;
        let sk = id_rec.require_sk().map_err(RpcError::invalid_input)?;
        // Real secure BLS sign of the digest (signature discarded after membership record).
        let _signature = bls::sign(sk, &digest);
        let pk_bytes = id_rec.pk.to_bytes();
        drop(identities);

        let mut mock = state.mock.lock().await;
        mock.approve(id, pk_bytes)
            .map_err(RpcError::invalid_input)?;
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
        chain::encode(&id).map_err(RpcError::internal)?,
    )
    .await
    .map_err(RpcError::internal)?;
    let view = view.ok_or_else(|| RpcError::proposal_not_found(id))?;
    if view.status != ProposalStatus::Open {
        return Err(RpcError::proposal_not_open(id));
    }

    let proposals_self_id =
        chain::contract_self_id_bytes(chain::Contract::Proposals).map_err(RpcError::internal)?;
    let intent =
        bls::proposal_intent_v3_from_view(&view, bls::digest_chain_id(), &proposals_self_id);
    let digest = knot_encoding::recompute_and_verify_v3(&intent, &view.signed_digest)
        .map_err(|_| RpcError::digest_mismatch("approve on-chain digest mismatch"))?;
    ensure_signers_are_members(
        &state,
        view.registry_account_id,
        std::slice::from_ref(&req.signer),
    )
    .await?;
    let intent_out = IntentDisplay {
        chain_id: intent.chain_id,
        epoch: intent.epoch,
        committee_id: intent.committee_id,
        nonce: intent.nonce,
        target: format!("0x{}", hex::encode(intent.target_contract_id)),
        function: intent.function_name.clone(),
        call_args_hex: format!("0x{}", hex::encode(&intent.call_args)),
        deadline: intent.deadline,
        digest_hex: format!("0x{}", hex::encode(digest)),
        digest_mnemonic: knot_encoding::digest_mnemonic(&digest),
        digest_safety_number: knot_encoding::digest_safety_number(&digest),
    };

    let identities = state.identities.lock().await;
    let id_rec = identities
        .iter()
        .find(|i| i.name == req.signer)
        .ok_or_else(|| RpcError::identity_not_found(&req.signer))?;
    let sk = id_rec.require_sk().map_err(RpcError::invalid_input)?;
    let args = ApproveArgs {
        proposal_id: id,
        signer: id_rec.pk,
        signature: bls::sign(sk, &digest),
    };
    drop(identities);

    let bytes = chain::encode(&args).map_err(RpcError::internal)?;
    let result = tokio::task::spawn_blocking(move || {
        chain::submit_call_to(chain::Contract::Proposals, "approve", &bytes)
    })
    .await
    .map_err(RpcError::internal)?
    .map_err(RpcError::internal)?;
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
    epoch: u64,
    nonce: u64,
    target: String,
    function: String,
    call_args_hex: String,
    deadline: u64,
    execute_at: u64,
    digest_hex: String,
    approvals: Vec<String>,
    approvals_len: usize,
}

async fn api_proposal_status(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<u64>,
) -> ApiResult<Json<Option<ProposalStatusOut>>> {
    if state.demo_mode == DemoMode::Mock {
        let mock = state.mock.lock().await;
        return Ok(Json(
            mock.proposal(id).map(|p| mock_proposal_status_out(id, p)),
        ));
    }
    let view: Option<ProposalView> = chain::query_contract(
        chain::Contract::Proposals,
        "proposal",
        chain::encode(&id).map_err(RpcError::internal)?,
    )
    .await
    .map_err(RpcError::internal)?;
    Ok(Json(view.map(|v| {
        let status = match v.status {
            ProposalStatus::Open => "Open",
            ProposalStatus::Executed => "Executed",
            ProposalStatus::Tombstoned => "Tombstoned",
            ProposalStatus::Queued => "Queued",
            ProposalStatus::Cancelled => "Cancelled",
        };
        ProposalStatusOut {
            id,
            status: status.into(),
            registry_account_id: v.registry_account_id,
            epoch: v.epoch,
            nonce: v.nonce,
            target: format!("0x{}", hex::encode(v.target.to_bytes())),
            function: v.function_name,
            call_args_hex: format!("0x{}", hex::encode(&v.call_args)),
            deadline: v.deadline,
            execute_at: v.execute_at,
            digest_hex: format!("0x{}", hex::encode(v.signed_digest)),
            approvals_len: v.approvals.len(),
            approvals: v.approvals.iter().map(bs58_pk).collect(),
        }
    })))
}

async fn api_proposal_finalize(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<u64>,
) -> ApiResult<Json<SubmitOut>> {
    if state.demo_mode == DemoMode::Mock {
        let mut mock = state.mock.lock().await;
        mock.finalize(id).map_err(RpcError::invalid_input)?;
        return Ok(Json(mock_ok_submit(
            format!("mock: finalize proposal {id}"),
            format!("mock-finalize-{id}"),
        )));
    }
    let bytes = chain::encode(&id).map_err(RpcError::internal)?;
    let result = tokio::task::spawn_blocking(move || {
        chain::submit_call_to(chain::Contract::Proposals, "finalize", &bytes)
    })
    .await
    .map_err(RpcError::internal)?
    .map_err(RpcError::internal)?;
    Ok(Json(submit_from_log(result.stdout)))
}

async fn api_proposal_execute(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<u64>,
) -> ApiResult<Json<SubmitOut>> {
    if state.demo_mode == DemoMode::Mock {
        let mut mock = state.mock.lock().await;
        mock.execute(id).map_err(RpcError::invalid_input)?;
        return Ok(Json(mock_ok_submit(
            format!("mock: execute proposal {id}"),
            format!("mock-execute-{id}"),
        )));
    }
    let bytes = chain::encode(&id).map_err(RpcError::internal)?;
    let result = tokio::task::spawn_blocking(move || {
        chain::submit_call_to(chain::Contract::Proposals, "execute", &bytes)
    })
    .await
    .map_err(RpcError::internal)?
    .map_err(RpcError::internal)?;
    Ok(Json(submit_from_log(result.stdout)))
}

#[derive(Deserialize)]
struct SetTimelockReq {
    blocks: u64,
}

async fn api_set_timelock(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<u64>,
    Json(req): Json<SetTimelockReq>,
) -> ApiResult<Json<SubmitOut>> {
    if state.demo_mode == DemoMode::Mock {
        let mut mock = state.mock.lock().await;
        mock.set_timelock(id, req.blocks)
            .map_err(RpcError::invalid_input)?;
        return Ok(Json(mock_ok_submit(
            format!("mock: set_timelock account={id} blocks={}", req.blocks),
            format!("mock-set-timelock-{id}"),
        )));
    }
    Err(RpcError::catalog(
        StatusCode::BAD_REQUEST,
        "use_cli",
        "Testnet set_timelock requires CLI signing (knot-tool account set-timelock).",
    ))
}

async fn api_execute_pending(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<u64>,
) -> ApiResult<Json<SubmitOut>> {
    if state.demo_mode == DemoMode::Mock {
        let mut mock = state.mock.lock().await;
        mock.execute_pending(id).map_err(RpcError::invalid_input)?;
        return Ok(Json(mock_ok_submit(
            format!("mock: execute_pending account={id}"),
            format!("mock-execute-pending-{id}"),
        )));
    }
    let bytes = chain::encode(&id).map_err(RpcError::internal)?;
    let result = tokio::task::spawn_blocking(move || chain::submit_call("execute_pending", &bytes))
        .await
        .map_err(RpcError::internal)?
        .map_err(RpcError::internal)?;
    Ok(Json(submit_from_log(result.stdout)))
}

async fn api_cancel_pending(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<u64>,
) -> ApiResult<Json<SubmitOut>> {
    if state.demo_mode == DemoMode::Mock {
        let mut mock = state.mock.lock().await;
        mock.cancel_pending(id).map_err(RpcError::invalid_input)?;
        return Ok(Json(mock_ok_submit(
            format!("mock: cancel_pending account={id}"),
            format!("mock-cancel-pending-{id}"),
        )));
    }
    Err(RpcError::catalog(
        StatusCode::BAD_REQUEST,
        "use_cli",
        "Testnet cancel_pending requires CLI signing (knot-tool account cancel-pending).",
    ))
}

async fn api_proposal_cancel(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<u64>,
) -> ApiResult<Json<SubmitOut>> {
    if state.demo_mode == DemoMode::Mock {
        let mut mock = state.mock.lock().await;
        mock.cancel_proposal(id).map_err(RpcError::invalid_input)?;
        return Ok(Json(mock_ok_submit(
            format!("mock: cancel proposal {id}"),
            format!("mock-cancel-{id}"),
        )));
    }
    Err(RpcError::catalog(
        StatusCode::BAD_REQUEST,
        "use_cli",
        "Testnet proposal cancel requires CLI signing (knot-tool proposal cancel).",
    ))
}

async fn api_proposal_next_id(State(state): State<Arc<AppState>>) -> ApiResult<Json<u64>> {
    if state.demo_mode == DemoMode::Mock {
        let mock = state.mock.lock().await;
        return Ok(Json(mock.next_proposal_id()));
    }
    let next: u64 = chain::query_contract(
        chain::Contract::Proposals,
        "next_proposal_id",
        chain::encode(&()).map_err(RpcError::internal)?,
    )
    .await
    .map_err(RpcError::internal)?;
    Ok(Json(next))
}

fn build_router(state: Arc<AppState>) -> Router {
    let api = Router::new()
        .route("/api/setup/status", get(api_setup_status))
        .route("/api/party", get(api_party_list).post(api_party_signup))
        .route(
            "/api/identities",
            get(api_list_identities).post(api_new_identity),
        )
        .route("/api/identities/import-pk", post(api_import_pk))
        .route("/api/account/create", post(api_account_create))
        .route("/api/account/{id}", get(api_account_query))
        .route("/api/account/{id}/meta", get(api_account_meta))
        .route("/api/account/{id}/keys", get(api_account_keys))
        .route("/api/account/next-id", get(api_account_next_id))
        .route("/api/registry/accounts", get(api_registry_accounts))
        .route("/api/quorum/preview", post(api_quorum_preview))
        .route("/api/quorum/submit", post(api_quorum_submit))
        .route("/api/quorum/check", post(api_quorum_check))
        .route("/api/quorum/diagnose", post(api_quorum_diagnose))
        .route("/api/quorum-agg/preview", post(api_quorum_agg_preview))
        .route("/api/quorum-agg/submit", post(api_quorum_agg_submit))
        .route("/api/quorum-agg/check", post(api_quorum_agg_check))
        .route(
            "/api/change-account/preview",
            post(api_change_account_preview),
        )
        .route(
            "/api/change-account/submit",
            post(api_change_account_submit),
        )
        .route("/api/proposal/create", post(api_proposal_create))
        .route("/api/proposal/{id}/preview", get(api_proposal_preview))
        .route("/api/proposal/{id}/approve", post(api_proposal_approve))
        .route("/api/proposal/{id}", get(api_proposal_status))
        .route("/api/proposal/{id}/finalize", post(api_proposal_finalize))
        .route("/api/proposal/{id}/execute", post(api_proposal_execute))
        .route("/api/proposal/{id}/cancel", post(api_proposal_cancel))
        .route("/api/proposal/next-id", get(api_proposal_next_id))
        .route("/api/account/{id}/set-timelock", post(api_set_timelock))
        .route(
            "/api/account/{id}/execute-pending",
            post(api_execute_pending),
        )
        .route("/api/account/{id}/cancel-pending", post(api_cancel_pending))
        .route("/api/blob/{id}/preview", get(api_blob_preview))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            require_session,
        ));

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
mod bind_validation {
    use super::validate_loopback_bind;

    #[test]
    fn accepts_ipv4_and_ipv6_loopback() {
        assert!(validate_loopback_bind("127.0.0.1:8877").is_ok());
        assert!(validate_loopback_bind("[::1]:8877").is_ok());
    }

    #[test]
    fn refuses_non_loopback_and_invalid() {
        let err = validate_loopback_bind("0.0.0.0:8877").unwrap_err();
        assert!(
            err.to_string().contains("refusing to bind"),
            "unexpected: {err}"
        );
        let err = validate_loopback_bind("192.168.1.1:8877").unwrap_err();
        assert!(
            err.to_string().contains("refusing to bind"),
            "unexpected: {err}"
        );
        assert!(validate_loopback_bind("not-an-address").is_err());
    }
}

#[cfg(test)]
mod generic_rpc_smoke {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    const TEST_SESSION: &str = "fixed-smoke-test-session-token";
    const TEST_OTP: &str = "fixed-smoke-test-otp";

    fn test_state_with_identities(names: &[&str]) -> Arc<AppState> {
        let identities: Vec<keystore::Identity> =
            names.iter().map(|name| keystore::generate(name)).collect();
        Arc::new(AppState {
            identities: Mutex::new(identities),
            password: "smoke-test-password".into(),
            store_path: PathBuf::from("/tmp/knot-tool-smoke-identities.dat"),
            otp: Mutex::new(Some(TEST_OTP.into())),
            session_token: TEST_SESSION.into(),
            demo_mode: DemoMode::Mock,
            mock: Mutex::new(MockLedger::new()),
        })
    }

    fn session_cookie_header() -> (&'static str, String) {
        ("cookie", format!("{SESSION_COOKIE}={TEST_SESSION}"))
    }

    async fn bootstrap_cookie(app: Router) -> String {
        let req = Request::builder()
            .method("GET")
            .uri(format!("/?code={TEST_OTP}"))
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.expect("bootstrap");
        assert_eq!(resp.status(), StatusCode::SEE_OTHER, "bootstrap redirect");
        let set_cookie = resp
            .headers()
            .get(header::SET_COOKIE)
            .expect("Set-Cookie")
            .to_str()
            .expect("cookie utf8")
            .to_string();
        assert!(set_cookie.contains("HttpOnly"));
        assert!(set_cookie.contains("SameSite=Strict"));
        set_cookie
    }

    async fn oneshot_json(
        app: Router,
        method: &str,
        uri: &str,
        body: Option<String>,
    ) -> (StatusCode, String) {
        let (name, value) = session_cookie_header();
        let mut builder = Request::builder()
            .method(method)
            .uri(uri)
            .header(name, value);
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
        (
            status,
            String::from_utf8(bytes.to_vec()).expect("utf8 body"),
        )
    }

    #[tokio::test]
    async fn index_html_never_embeds_session_secret() {
        let state = test_state_with_identities(&["alice"]);
        let secret = state.session_token.clone();
        let app = build_router(state);

        let req = Request::builder()
            .method("GET")
            .uri("/")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.expect("oneshot");
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("body");
        let html = String::from_utf8(bytes.to_vec()).expect("utf8 body");
        assert!(!html.contains(&secret));
        assert!(!html.contains("KNOT_TOOL_TOKEN"));
        assert!(!html.contains("__TOKEN__"));
    }

    #[tokio::test]
    async fn bootstrap_code_sets_cookie_and_consumes_otp() {
        let state = test_state_with_identities(&[]);
        let app = build_router(state);

        let cookie = bootstrap_cookie(app.clone()).await;
        assert!(cookie.contains(TEST_SESSION));

        let req = Request::builder()
            .method("GET")
            .uri(format!("/?code={TEST_OTP}"))
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.expect("reuse otp");
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn setup_status_mock_mode_and_session_gate() {
        let state = test_state_with_identities(&["alice"]);
        let app = build_router(state);

        let no_session = Request::builder()
            .method("GET")
            .uri("/api/setup/status")
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(no_session).await.expect("oneshot");
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        let (status, body) = oneshot_json(app, "GET", "/api/setup/status", None).await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).expect("json");
        assert_eq!(json["demo_mode"], "mock");
        assert_eq!(json["identities_count"], 1);
    }

    #[tokio::test]
    async fn header_token_secondary_auth_for_tests() {
        let state = test_state_with_identities(&["alice"]);
        let app = build_router(state);

        let req = Request::builder()
            .method("GET")
            .uri("/api/setup/status")
            .header("X-Knot-Token", TEST_SESSION)
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.expect("oneshot");
        assert_eq!(resp.status(), StatusCode::OK);
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
        assert!(
            submit["tx_hash"]
                .as_str()
                .unwrap_or("")
                .starts_with("mock-create-account-")
        );

        let (status, next_body) =
            oneshot_json(app.clone(), "GET", "/api/proposal/next-id", None).await;
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
        assert!(
            preview["digest_hex"]
                .as_str()
                .unwrap_or("")
                .starts_with("0x")
        );
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

        let (status, body) =
            oneshot_json(app.clone(), "POST", "/api/proposal/0/finalize", None).await;
        assert_eq!(status, StatusCode::OK, "finalize: {body}");
        let finalized: serde_json::Value = serde_json::from_str(&body).expect("finalize json");
        assert_eq!(finalized["tx_hash"], "mock-finalize-0");

        let (status, status_body) = oneshot_json(app, "GET", "/api/proposal/0", None).await;
        assert_eq!(status, StatusCode::OK);
        let prop: serde_json::Value =
            serde_json::from_str(&status_body).expect("final status json");
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
        let err: serde_json::Value = serde_json::from_str(&body).expect("error json");
        assert_eq!(err["code"], "confirm_required");
        assert!(
            err["message"]
                .as_str()
                .unwrap_or("")
                .contains("Confirm required")
        );
    }

    #[tokio::test]
    async fn api_errors_use_code_schema_not_raw_details() {
        let state = test_state_with_identities(&["alice"]);
        let app = build_router(state);

        let dup = serde_json::json!({ "name": "alice" });
        let (status, body) = oneshot_json(
            app.clone(),
            "POST",
            "/api/identities",
            Some(dup.to_string()),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "duplicate identity: {body}"
        );
        let err: serde_json::Value = serde_json::from_str(&body).expect("error json");
        assert_eq!(err["code"], "identity_exists");
        assert_eq!(err["message"], "Identity already exists.");
        assert!(!body.contains("invalid BlsPublicKey"));

        let bad_pk = serde_json::json!({
            "name": "ghost",
            "pk": "not-valid-bls-key-material-xyz"
        });
        let (status, body) = oneshot_json(
            app,
            "POST",
            "/api/identities/import-pk",
            Some(bad_pk.to_string()),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "bad pk: {body}");
        let err: serde_json::Value = serde_json::from_str(&body).expect("error json");
        assert_eq!(err["code"], "invalid_input");
        assert_eq!(err["message"], "Invalid request.");
        assert!(!body.contains("not-valid-bls-key-material-xyz"));
        assert!(!body.contains("invalid BlsPublicKey"));
    }

    #[tokio::test]
    async fn approve_rejects_non_member() {
        let state = test_state_with_identities(&["alice", "bob", "carol"]);
        let app = build_router(state);

        let create_account = serde_json::json!({
            "members": ["alice", "bob"],
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

        let target = format!("0x{}", "33".repeat(32));
        let create_proposal = serde_json::json!({
            "account": 0,
            "target": target,
            "function": "set_value",
            "args_hex": "0x01",
            "deadline": 500
        });
        let (status, body) = oneshot_json(
            app.clone(),
            "POST",
            "/api/proposal/create",
            Some(create_proposal.to_string()),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "create proposal: {body}");

        let approve_carol = serde_json::json!({ "signer": "carol", "confirm": true });
        let (status, body) = oneshot_json(
            app.clone(),
            "POST",
            "/api/proposal/0/approve",
            Some(approve_carol.to_string()),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "non-member approve: {body}");
        let err: serde_json::Value = serde_json::from_str(&body).expect("error json");
        assert_eq!(err["code"], "not_a_member");
        assert_eq!(err["message"], "Signer is not a committee member.");
        assert!(!body.contains("registry account"));

        let approve_alice = serde_json::json!({ "signer": "alice", "confirm": true });
        let (status, body) = oneshot_json(
            app,
            "POST",
            "/api/proposal/0/approve",
            Some(approve_alice.to_string()),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "member approve: {body}");
    }

    #[tokio::test]
    async fn proposal_create_rejects_invalid_hex() {
        let state = test_state_with_identities(&["alice"]);
        let app = build_router(state);

        let bad_target = serde_json::json!({
            "account": 0,
            "target": "0xZZZZ",
            "function": "noop",
            "args_hex": "",
            "deadline": 0
        });
        let (status, body) = oneshot_json(
            app.clone(),
            "POST",
            "/api/proposal/create",
            Some(bad_target.to_string()),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "bad target: {body}");
        let err: serde_json::Value = serde_json::from_str(&body).expect("error json");
        assert_eq!(err["code"], "invalid_hex");
        assert_eq!(err["message"], "Invalid hex encoding.");

        let bad_args = serde_json::json!({
            "account": 0,
            "target": format!("0x{}", "44".repeat(32)),
            "function": "noop",
            "args_hex": "0xGG",
            "deadline": 0
        });
        let (status, body) = oneshot_json(
            app,
            "POST",
            "/api/proposal/create",
            Some(bad_args.to_string()),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "bad args_hex: {body}");
        let err: serde_json::Value = serde_json::from_str(&body).expect("error json");
        assert_eq!(err["code"], "invalid_hex");
        assert_eq!(err["message"], "Invalid hex encoding.");
    }

    fn test_state_testnet_with_identities(names: &[&str]) -> Arc<AppState> {
        let identities: Vec<keystore::Identity> =
            names.iter().map(|name| keystore::generate(name)).collect();
        Arc::new(AppState {
            identities: Mutex::new(identities),
            password: "smoke-test-password".into(),
            store_path: PathBuf::from("/tmp/knot-tool-smoke-identities.dat"),
            otp: Mutex::new(Some(TEST_OTP.into())),
            session_token: TEST_SESSION.into(),
            demo_mode: DemoMode::Testnet,
            mock: Mutex::new(MockLedger::new()),
        })
    }

    #[tokio::test]
    async fn quorum_submit_without_confirm_is_rejected() {
        let state = test_state_testnet_with_identities(&["alice", "bob"]);
        let app = build_router(state);

        let body = serde_json::json!({
            "account": 0,
            "msg": "hello",
            "signers": ["alice"],
            "confirm": false
        });
        let (status, resp_body) =
            oneshot_json(app, "POST", "/api/quorum/submit", Some(body.to_string())).await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "quorum submit: {resp_body}"
        );
        let err: serde_json::Value = serde_json::from_str(&resp_body).expect("error json");
        assert_eq!(err["code"], "confirm_required");
        assert!(
            err["message"]
                .as_str()
                .unwrap_or("")
                .contains("Confirm required")
        );
    }

    #[tokio::test]
    async fn change_account_submit_without_confirm_is_rejected() {
        let state = test_state_testnet_with_identities(&["alice", "bob"]);
        let app = build_router(state);

        let body = serde_json::json!({
            "account": 0,
            "new_members": ["alice", "bob"],
            "new_threshold": 2,
            "signers": ["alice"],
            "confirm": false
        });
        let (status, resp_body) = oneshot_json(
            app,
            "POST",
            "/api/change-account/submit",
            Some(body.to_string()),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "change-account submit: {resp_body}"
        );
        let err: serde_json::Value = serde_json::from_str(&resp_body).expect("error json");
        assert_eq!(err["code"], "confirm_required");
    }

    #[tokio::test]
    async fn quorum_preview_returns_fingerprint() {
        let state = test_state_with_identities(&["alice", "bob"]);
        let app = build_router(state);

        let create_account = serde_json::json!({
            "members": ["alice", "bob"],
            "threshold": 2
        });
        oneshot_json(
            app.clone(),
            "POST",
            "/api/account/create",
            Some(create_account.to_string()),
        )
        .await;

        let preview_req = serde_json::json!({
            "account": 0,
            "msg": "hello",
            "signers": ["alice", "bob"]
        });
        let (status, body) = oneshot_json(
            app,
            "POST",
            "/api/quorum/preview",
            Some(preview_req.to_string()),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "quorum preview: {body}");
        let preview: serde_json::Value = serde_json::from_str(&body).expect("preview json");
        assert!(
            preview["digest_hex"]
                .as_str()
                .unwrap_or("")
                .starts_with("0x")
        );
        assert!(!preview["digest_mnemonic"].as_str().unwrap_or("").is_empty());
        assert_eq!(preview["msg_hex"], "0x68656c6c6f");
        assert!(
            preview["note"]
                .as_str()
                .unwrap_or("")
                .contains("one signer")
        );
    }

    #[tokio::test]
    async fn change_account_preview_returns_fingerprint() {
        let state = test_state_with_identities(&["alice", "bob"]);
        let app = build_router(state);

        let create_account = serde_json::json!({
            "members": ["alice", "bob"],
            "threshold": 2
        });
        oneshot_json(
            app.clone(),
            "POST",
            "/api/account/create",
            Some(create_account.to_string()),
        )
        .await;

        let preview_req = serde_json::json!({
            "account": 0,
            "new_members": ["alice"],
            "new_threshold": 1,
            "signers": ["alice", "bob"]
        });
        let (status, body) = oneshot_json(
            app,
            "POST",
            "/api/change-account/preview",
            Some(preview_req.to_string()),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "change-account preview: {body}");
        let preview: serde_json::Value = serde_json::from_str(&body).expect("preview json");
        assert!(
            preview["digest_hex"]
                .as_str()
                .unwrap_or("")
                .starts_with("0x")
        );
        assert!(!preview["digest_mnemonic"].as_str().unwrap_or("").is_empty());
        assert_eq!(preview["new_threshold"], 1);
    }
}
