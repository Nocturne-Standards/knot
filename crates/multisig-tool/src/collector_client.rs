// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Leon Frenzel

//! HTTP client for `multisig-collector`'s untrusted proposal/partial relay.
//!
//! Deliberately **not** a Cargo dependency on `multisig-collector` — this
//! talks to it over plain HTTP/JSON only, the same way any third-party
//! client would. The request/response shapes here are hand-mirrored from
//! `multisig-collector/src/dto.rs`; `BlobFile`/`PartialFile` (`crate::blob`)
//! already match `ProposalDto`/`PartialDto` field-for-field (see that
//! crate's README "Wire parity" section), so they're reused directly as the
//! JSON body for the proposal endpoints instead of duplicating them again
//! here. Only the party-roster shapes (which `multisig-tool` has no local
//! equivalent of) get their own small structs below.

use anyhow::{bail, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::blob::{BlobFile, PartialFile};

/// Collector base URL, e.g. `http://127.0.0.1:8899`. No CLI default — must
/// come from `--collector` or this env var.
pub const URL_ENV: &str = "MULTISIG_COLLECTOR_URL";
/// HTTP Basic Auth username. Optional — omitted requests send no `Authorization` header.
pub const USER_ENV: &str = "MULTISIG_COLLECTOR_USER";
/// HTTP Basic Auth password (paired with `USER_ENV`).
pub const PASSWORD_ENV: &str = "MULTISIG_COLLECTOR_PASSWORD";

/// `POST /v1/proposals` response — the collector never echoes partials back
/// here (it clears any caller-supplied ones), so this is deliberately
/// smaller than `BlobFile`.
#[derive(Debug, Clone, Deserialize)]
pub struct PushResponse {
    pub id: String,
    pub signed_digest: String,
}

/// One row of `GET /v1/proposals` — mirrors `multisig_collector::dto::ProposalSummary`.
#[derive(Debug, Clone, Deserialize)]
pub struct ProposalSummary {
    pub id: String,
    pub signed_digest: String,
    #[serde(default)]
    pub kind: crate::blob::BlobKind,
    pub threshold: u32,
    pub partials_count: usize,
    pub created_at: i64,
}

/// `POST /v1/party` request body — mirrors `multisig_collector::dto::PartySignupDto`.
#[derive(Debug, Clone, Serialize)]
struct PartySignupRequest<'a> {
    name: &'a str,
    pk: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<&'a str>,
}

/// Roster row — mirrors `multisig_collector::dto::PartyMemberDto`.
#[derive(Debug, Clone, Deserialize)]
pub struct PartyMember {
    pub name: String,
    pub pk: String,
    #[serde(default)]
    pub note: Option<String>,
    pub joined_at: i64,
}

/// Thin HTTP client. Cheap to construct per-invocation — the CLI builds one
/// per subcommand rather than threading it through `Cli`.
pub struct CollectorClient {
    base_url: String,
    user: Option<String>,
    password: Option<String>,
    http: Client,
}

impl CollectorClient {
    /// Resolves the base URL from `cli_url` (highest priority, i.e.
    /// `--collector`) or [`URL_ENV`]; credentials always come from
    /// [`USER_ENV`]/[`PASSWORD_ENV`] — there is no `--user`/`--password`
    /// flag, so a password never has to appear in shell history/`ps`.
    pub fn resolve(cli_url: Option<&str>) -> Result<Self> {
        let base_url = cli_url
            .map(str::to_string)
            .or_else(|| std::env::var(URL_ENV).ok())
            .ok_or_else(|| {
                anyhow::anyhow!("no collector URL: pass --collector or set {URL_ENV}")
            })?;
        let user = std::env::var(USER_ENV).ok().filter(|s| !s.is_empty());
        let password = std::env::var(PASSWORD_ENV).ok();
        Ok(Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            user,
            password,
            http: Client::new(),
        })
    }

    fn auth(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.user {
            Some(user) => builder.basic_auth(user, self.password.as_deref()),
            None => builder,
        }
    }

    /// Maps a non-2xx response to an `Err` carrying the collector's
    /// `{"error": "..."}"` message (or the raw body if it isn't that shape).
    async fn into_ok(resp: reqwest::Response) -> Result<reqwest::Response> {
        if resp.status().is_success() {
            return Ok(resp);
        }
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        let message = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(str::to_string))
            .unwrap_or(body);
        bail!("collector returned {status}: {message}");
    }

    /// `POST /v1/proposals` — creates (or idempotently re-affirms) a
    /// proposal. Any partials already in `blob` are ignored server-side.
    pub async fn push(&self, blob: &BlobFile) -> Result<PushResponse> {
        let resp = self
            .auth(self.http.post(format!("{}/v1/proposals", self.base_url)))
            .json(blob)
            .send()
            .await
            .context("POST /v1/proposals")?;
        Self::into_ok(resp).await?.json().await.context("parse push response")
    }

    /// `GET /v1/proposals/:id` — full blob, including all partials so far.
    pub async fn pull(&self, id: &str) -> Result<BlobFile> {
        let resp = self
            .auth(self.http.get(format!("{}/v1/proposals/{id}", self.base_url)))
            .send()
            .await
            .context("GET /v1/proposals/:id")?;
        Self::into_ok(resp).await?.json().await.context("parse proposal body")
    }

    /// `GET /v1/proposals` — summary list (no partial bodies).
    pub async fn list_proposals(&self) -> Result<Vec<ProposalSummary>> {
        let resp = self
            .auth(self.http.get(format!("{}/v1/proposals", self.base_url)))
            .send()
            .await
            .context("GET /v1/proposals")?;
        Self::into_ok(resp).await?.json().await.context("parse proposal list")
    }

    /// `POST /v1/proposals/:id/partials` — returns the full updated blob
    /// (all partials so far, digest unchanged).
    pub async fn append_partial(&self, id: &str, partial: &PartialFile) -> Result<BlobFile> {
        let resp = self
            .auth(self.http.post(format!("{}/v1/proposals/{id}/partials", self.base_url)))
            .json(partial)
            .send()
            .await
            .context("POST /v1/proposals/:id/partials")?;
        Self::into_ok(resp).await?.json().await.context("parse appended proposal")
    }

    /// `GET /v1/party` — full roster.
    pub async fn list_party(&self) -> Result<Vec<PartyMember>> {
        let resp = self
            .auth(self.http.get(format!("{}/v1/party", self.base_url)))
            .send()
            .await
            .context("GET /v1/party")?;
        Self::into_ok(resp).await?.json().await.context("parse party list")
    }

    /// `POST /v1/party` — signup, or upsert-by-`pk` if already present.
    pub async fn signup_party(&self, name: &str, pk: &str, note: Option<&str>) -> Result<PartyMember> {
        let resp = self
            .auth(self.http.post(format!("{}/v1/party", self.base_url)))
            .json(&PartySignupRequest { name, pk, note })
            .send()
            .await
            .context("POST /v1/party")?;
        Self::into_ok(resp).await?.json().await.context("parse party signup response")
    }

    /// `DELETE /v1/party/:pk` — removes one roster row.
    pub async fn leave_party(&self, pk: &str) -> Result<()> {
        let resp = self
            .auth(self.http.delete(format!("{}/v1/party/{pk}", self.base_url)))
            .send()
            .await
            .context("DELETE /v1/party/:pk")?;
        Self::into_ok(resp).await?;
        Ok(())
    }
}
