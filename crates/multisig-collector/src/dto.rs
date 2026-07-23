// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 Leon Frenzel

//! JSON wire DTOs for `/v1/proposals`.
//!
//! Deliberately **duplicated** (not shared via `multisig-encoding`) — that
//! crate is `no_std` + Apache-2.0 and consumed by a WASM contract
//! (`multisig-proposals`); teaching it JSON/hex would bloat a size-sensitive
//! on-chain dependency for a concern only the (AGPL, off-chain) collector
//! has. See the module doc in `lib.rs` and the crate `README.md` "Wire
//! parity" section for the exact field-for-field mapping to
//! `multisig-tool/src/blob.rs`'s `BlobFile`/`IntentFile`/`PartialFile`.
//!
//! The collector never decodes `target_contract_id`/`function_name`/
//! `call_args`/`human_summary` — those pass through as opaque strings. Only
//! `signed_digest` (drives the content-addressed `id`) and `signer_pk`
//! (drives partial de-duplication) are hex-validated and normalized here.

use serde::{Deserialize, Serialize};

/// BLS public key length in bytes (96) — matches `multisig-tool`'s `hex96`.
pub const PK_BYTES: usize = 96;
/// `signed_digest` length in bytes (32) — matches `multisig-tool`'s `hex32`.
pub const DIGEST_BYTES: usize = 32;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProposalDto {
    pub version: u16,
    pub intent: IntentDto,
    pub signed_digest: String,
    pub threshold: u32,
    #[serde(default)]
    pub partials: Vec<PartialDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IntentDto {
    pub chain_id: u64,
    pub committee_id: u64,
    pub nonce: u64,
    pub target_contract_id: String,
    pub function_name: String,
    pub call_args: String,
    pub deadline: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub human_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PartialDto {
    pub signer_pk: String,
    pub sig: String,
}

/// Summary row for `GET /v1/proposals`.
#[derive(Debug, Clone, Serialize)]
pub struct ProposalSummary {
    pub id: String,
    pub signed_digest: String,
    pub threshold: u32,
    pub partials_count: usize,
    pub created_at: i64,
}

/// `POST /v1/party` request body — sign up (or refresh) on the party-finder
/// roster. `pk` accepts either hex (with or without `0x`) or base58, mirroring
/// `multisig-tool::keystore::parse_pk`'s accepted formats; this crate never
/// constructs a `BlsPublicKey` from it (no `dusk_core` dependency — see the
/// module doc in `lib.rs`), it only validates the decoded length is 96 bytes.
#[derive(Debug, Clone, Deserialize)]
pub struct PartySignupDto {
    pub name: String,
    pub pk: String,
    #[serde(default)]
    pub note: Option<String>,
}

/// Roster row for `GET /v1/party`. `pk` is always the canonical
/// `"0x"+lowercase-hex` form (see [`normalize_pk`]), regardless of which
/// format the signup request used.
#[derive(Debug, Clone, Serialize)]
pub struct PartyMemberDto {
    pub name: String,
    pub pk: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    pub joined_at: i64,
}

/// Decodes `s` as hex (optionally `0x`-prefixed) or base58, whichever
/// matches, and checks the result is exactly [`PK_BYTES`] (96) bytes long.
/// Returns the canonical `"0x"+lowercase-hex` form on success. A string of
/// only hex digits is treated as hex (never base58 — base58 excludes `0`,
/// `O`, `I`, `l` but not the other 55 alphanumerics, so this only matters
/// for all-hex-digit strings, which decode losslessly as hex anyway).
pub fn normalize_pk(s: &str) -> Result<String, String> {
    let t = s.trim();
    let bytes = if t.starts_with("0x") || t.starts_with("0X") {
        hex::decode(&t[2..]).map_err(|e| format!("invalid hex: {e}"))?
    } else if t.chars().all(|c| c.is_ascii_hexdigit()) {
        hex::decode(t).map_err(|e| format!("invalid hex: {e}"))?
    } else {
        bs58::decode(t)
            .into_vec()
            .map_err(|e| format!("invalid base58: {e}"))?
    };
    if bytes.len() != PK_BYTES {
        return Err(format!("expected {PK_BYTES} bytes, got {}", bytes.len()));
    }
    Ok(format!("0x{}", hex::encode(bytes)))
}

/// Strips an optional `0x`/`0X` prefix, hex-decodes, checks the exact byte
/// length, and returns a canonical `"0x" + lowercase-hex"` string. Used for
/// `signed_digest` (32 bytes) and `signer_pk` (96 bytes) — the two fields
/// the collector actually inspects.
pub fn normalize_hex(s: &str, expected_bytes: usize) -> Result<String, String> {
    let stripped = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")).unwrap_or(s);
    let bytes = hex::decode(stripped).map_err(|e| format!("invalid hex: {e}"))?;
    if bytes.len() != expected_bytes {
        return Err(format!(
            "expected {expected_bytes} bytes, got {}",
            bytes.len()
        ));
    }
    Ok(format!("0x{}", hex::encode(bytes)))
}

/// Content-addressed id: lowercase hex of `signed_digest`, no `0x` prefix
/// (64 hex chars for a 32-byte digest). `normalized_digest` must already be
/// in `"0x" + lowercase-hex"` form (i.e. the output of [`normalize_hex`]).
pub fn digest_to_id(normalized_digest: &str) -> String {
    normalized_digest
        .strip_prefix("0x")
        .unwrap_or(normalized_digest)
        .to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_hex_accepts_0x_prefix_and_lowercases() {
        let got = normalize_hex("0xABCD", 2).unwrap();
        assert_eq!(got, "0xabcd");
    }

    #[test]
    fn normalize_hex_accepts_missing_prefix() {
        let got = normalize_hex("abcd", 2).unwrap();
        assert_eq!(got, "0xabcd");
    }

    #[test]
    fn normalize_hex_rejects_wrong_length() {
        assert!(normalize_hex("abcd", 3).is_err());
    }

    #[test]
    fn normalize_hex_rejects_invalid_hex() {
        assert!(normalize_hex("zzzz", 2).is_err());
    }

    #[test]
    fn digest_to_id_strips_prefix() {
        assert_eq!(digest_to_id("0xabcd"), "abcd");
    }

    #[test]
    fn normalize_pk_accepts_hex_with_and_without_prefix() {
        let hex96 = "11".repeat(96);
        let got_bare = normalize_pk(&hex96).unwrap();
        let got_prefixed = normalize_pk(&format!("0x{hex96}")).unwrap();
        assert_eq!(got_bare, format!("0x{hex96}"));
        assert_eq!(got_bare, got_prefixed);
    }

    #[test]
    fn normalize_pk_accepts_base58() {
        let hex96 = "ab".repeat(96);
        let bytes = hex::decode(&hex96).unwrap();
        let b58 = bs58::encode(&bytes).into_string();
        let got = normalize_pk(&b58).unwrap();
        assert_eq!(got, format!("0x{hex96}"));
    }

    #[test]
    fn normalize_pk_rejects_wrong_length() {
        assert!(normalize_pk("abcd").is_err());
    }

    #[test]
    fn normalize_pk_rejects_garbage() {
        assert!(normalize_pk("not a valid pk at all!!").is_err());
    }
}
