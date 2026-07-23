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
}
