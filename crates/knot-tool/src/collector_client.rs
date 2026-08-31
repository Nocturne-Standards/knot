// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Nocturne Standards

//! HTTP client for `knot-collector`'s untrusted proposal/partial relay.
//!
//! Deliberately **not** a Cargo dependency on `knot-collector` — this
//! talks to it over plain HTTP/JSON only, the same way any third-party
//! client would. The request/response shapes here are hand-mirrored from
//! `knot-collector/src/dto.rs`; `BlobFile`/`PartialFile` (`crate::blob`)
//! already match `ProposalDto`/`PartialDto` field-for-field (see that
//! crate's README "Wire parity" section), so they're reused directly as the
//! JSON body for the proposal endpoints instead of duplicating them again
//! here. Only the party-roster shapes (which `knot-tool` has no local
//! equivalent of) get their own small structs below.

use anyhow::{Context, Result, bail};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::blob::{BlobFile, PartialFile};

/// Collector base URL, e.g. `http://127.0.0.1:8899`. No CLI default — must
/// come from `--collector` or this env var.
pub const URL_ENV: &str = "KNOT_COLLECTOR_URL";
/// HTTP Basic Auth username. Optional — omitted requests send no `Authorization` header.
pub const USER_ENV: &str = "KNOT_COLLECTOR_USER";
/// HTTP Basic Auth password (paired with `USER_ENV`).
pub const PASSWORD_ENV: &str = "KNOT_COLLECTOR_PASSWORD";

/// `POST /v1/proposals` response — the collector never echoes partials back
/// here (it clears any caller-supplied ones), so this is deliberately
/// smaller than `BlobFile`.
#[derive(Debug, Clone, Deserialize)]
pub struct PushResponse {
    pub id: String,
    pub signed_digest: String,
}

/// One row of `GET /v1/proposals` — mirrors `knot_collector::dto::ProposalSummary`.
#[derive(Debug, Clone, Deserialize)]
pub struct ProposalSummary {
    pub id: String,
    pub signed_digest: String,
    #[serde(default = "default_summary_kind")]
    pub kind: String,
    pub threshold: u32,
    pub partials_count: usize,
    pub created_at: i64,
}

fn default_summary_kind() -> String {
    "proposals".into()
}

/// `POST /v1/party` request body — mirrors `knot_collector::dto::PartySignupDto`.
#[derive(Debug, Clone, Serialize)]
struct PartySignupRequest<'a> {
    name: &'a str,
    pk: &'a str,
    sig: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<&'a str>,
}

/// Roster row for `GET /v1/party` (collector wire shape).
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
        validate_collector_url(&base_url)?;
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
        Self::into_ok(resp)
            .await?
            .json()
            .await
            .context("parse push response")
    }

    /// `GET /v1/proposals/:id` — full blob, including all partials so far.
    pub async fn pull(&self, id: &str) -> Result<BlobFile> {
        let id = validate_proposal_id(id)?;
        let resp = self
            .auth(
                self.http
                    .get(format!("{}/v1/proposals/{id}", self.base_url)),
            )
            .send()
            .await
            .context("GET /v1/proposals/:id")?;
        Self::into_ok(resp)
            .await?
            .json()
            .await
            .context("parse proposal body")
    }

    /// `GET /v1/proposals` — summary list (no partial bodies).
    pub async fn list_proposals(&self) -> Result<Vec<ProposalSummary>> {
        let resp = self
            .auth(self.http.get(format!("{}/v1/proposals", self.base_url)))
            .send()
            .await
            .context("GET /v1/proposals")?;
        Self::into_ok(resp)
            .await?
            .json()
            .await
            .context("parse proposal list")
    }

    /// `POST /v1/proposals/:id/partials` — appends or **replaces** the partial
    /// for `signer_pk` (collector last-write-wins; never 409 on duplicate pk).
    /// Returns the full updated blob (digest unchanged).
    pub async fn append_partial(&self, id: &str, partial: &PartialFile) -> Result<BlobFile> {
        let id = validate_proposal_id(id)?;
        let resp = self
            .auth(
                self.http
                    .post(format!("{}/v1/proposals/{id}/partials", self.base_url)),
            )
            .json(partial)
            .send()
            .await
            .context("POST /v1/proposals/:id/partials")?;
        Self::into_ok(resp)
            .await?
            .json()
            .await
            .context("parse appended proposal")
    }

    /// `GET /v1/party` — full roster.
    pub async fn list_party(&self) -> Result<Vec<PartyMember>> {
        let resp = self
            .auth(self.http.get(format!("{}/v1/party", self.base_url)))
            .send()
            .await
            .context("GET /v1/party")?;
        Self::into_ok(resp)
            .await?
            .json()
            .await
            .context("parse party list")
    }

    /// `POST /v1/party` — signup, or upsert-by-`pk` if already present.
    /// `sig` must be a BLS signature over [`knot_encoding::party_signup_preimage`]
    /// for `name` and the normalized pk bytes (M12).
    pub async fn signup_party(
        &self,
        name: &str,
        pk: &str,
        sig: &str,
        note: Option<&str>,
    ) -> Result<PartyMember> {
        let resp = self
            .auth(self.http.post(format!("{}/v1/party", self.base_url)))
            .json(&PartySignupRequest {
                name,
                pk,
                sig,
                note,
            })
            .send()
            .await
            .context("POST /v1/party")?;
        Self::into_ok(resp)
            .await?
            .json()
            .await
            .context("parse party signup response")
    }
}

/// R5: collector URL must be `https://` or HTTP loopback before Basic Auth.
pub fn validate_collector_url(raw: &str) -> Result<()> {
    let parsed = reqwest::Url::parse(raw.trim()).context("invalid collector URL")?;
    match parsed.scheme() {
        "https" => Ok(()),
        "http" => match parsed.host() {
            Some(url::Host::Domain("localhost")) => Ok(()),
            Some(url::Host::Ipv4(ip)) if ip.is_loopback() => Ok(()),
            Some(url::Host::Ipv6(ip)) if ip.is_loopback() => Ok(()),
            _ => bail!(
                "refusing collector URL '{raw}': HTTP is allowed only on loopback; use https:// elsewhere (R5)"
            ),
        },
        other => bail!(
            "refusing collector URL scheme '{other}': only https:// or http://loopback are allowed (R5)"
        ),
    }
}

/// R11: content-addressed proposal ids are exactly 64 hex chars.
pub fn validate_proposal_id(id: &str) -> Result<String> {
    let id = id.to_ascii_lowercase();
    if id.len() != 64 || !id.chars().all(|c| c.is_ascii_hexdigit()) {
        bail!("proposal id must be exactly 64 hex characters");
    }
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r5_accepts_https_and_loopback_http() {
        assert!(validate_collector_url("https://collector.example.com").is_ok());
        assert!(validate_collector_url("http://127.0.0.1:8899").is_ok());
        assert!(validate_collector_url("http://localhost:8899").is_ok());
        assert!(validate_collector_url("http://[::1]:8899").is_ok());
    }

    #[test]
    fn r5_rejects_plain_http_remote() {
        let err = validate_collector_url("http://evil.example.com:8899").unwrap_err();
        assert!(err.to_string().contains("R5"), "{err}");
    }

    #[test]
    fn r11_validates_proposal_id() {
        let good = "a".repeat(64);
        assert_eq!(validate_proposal_id(&good).unwrap(), good);
        assert!(validate_proposal_id("abc").is_err());
        assert!(validate_proposal_id(&"g".repeat(64)).is_err());
    }
}
