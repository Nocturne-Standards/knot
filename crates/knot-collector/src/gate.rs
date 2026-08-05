// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 Nocturne Standards

//! Digest recomputation (C1) via `knot-encoding` — no `dusk-core` on this path.

use knot_encoding::{GateError, ProposalIntent, recompute_and_verify};

use crate::dto::{BlobKind, IntentDto, ProposalDto};

/// Strip one optional `0x` prefix; reject repeated `0x`.
fn strip_single_0x(s: &str) -> Result<&str, String> {
    let t = s.trim();
    match t.strip_prefix("0x") {
        None => Ok(t),
        Some(rest) if rest.starts_with("0x") => Err("repeated 0x prefix".into()),
        Some(rest) => Ok(rest),
    }
}

fn decode_hex_field(s: &str, label: &str) -> Result<Vec<u8>, String> {
    let stripped = strip_single_0x(s)?;
    hex::decode(stripped).map_err(|e| format!("{label}: invalid hex: {e}"))
}

fn decode_hex32(s: &str, label: &str) -> Result<[u8; 32], String> {
    let bytes = decode_hex_field(s, label)?;
    bytes
        .as_slice()
        .try_into()
        .map_err(|_| format!("{label}: expected 32 bytes, got {}", bytes.len()))
}

/// Recompute the §4a digest from canonical intent fields and reject mismatches (C1).
pub fn gate_proposal_digest(dto: &ProposalDto) -> Result<[u8; 32], String> {
    if dto.kind != BlobKind::Proposals {
        return Err(format!(
            "unsupported blob kind {:?} — only proposals blobs are verified",
            dto.kind
        ));
    }
    if dto.version != 1 {
        return Err(format!(
            "unsupported proposal version {} — collector verifies version 1 proposals only",
            dto.version
        ));
    }
    let intent = dto_to_proposal_intent(dto)?;
    let claimed = decode_hex32(&dto.signed_digest, "signed_digest")?;
    match recompute_and_verify(&intent, &claimed) {
        Ok(digest) => Ok(digest),
        Err(GateError::DigestMismatch) => {
            Err("signed_digest does not match recomputed digest from intent fields".into())
        }
        Err(GateError::Encoding(e)) => Err(format!("intent encoding error: {e}")),
    }
}

fn dto_to_proposal_intent(dto: &ProposalDto) -> Result<ProposalIntent, String> {
    match &dto.intent {
        IntentDto::Proposals(i) => {
            let target_contract_id = decode_hex32(&i.target_contract_id, "target_contract_id")?;
            let call_args = decode_hex_field(&i.call_args, "call_args")?;
            Ok(ProposalIntent {
                chain_id: i.chain_id,
                committee_id: i.committee_id,
                nonce: i.nonce,
                target_contract_id,
                function_name: i.function_name.clone(),
                call_args,
                deadline: i.deadline,
            })
        }
        IntentDto::PmCouncilResolve(_) => Err("pm_council_resolve intent is not supported".into()),
    }
}
