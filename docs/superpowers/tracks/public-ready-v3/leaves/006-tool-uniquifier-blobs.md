---
id: 6
slug: tool-uniquifier-blobs
status: IN_PROGRESS
owner: cursor-agent
deps:
  - 4
scope:
  - crates/knot-tool/
  - crates/knot-tool/
acceptance:
  - CSPRNG uniquifier + --nonce for proposals (caller-supplied; not change-account)
  - M8 fetch threshold / honest offline label
  - M9 verify partials locally
  - L7 Result aggregate; L8 atomic blob write; L14 typed errors
  - R5 collector URL allowlist (loopback or https only) before Basic Auth
  - R11 client validate proposal id as 64-hex before path use
acceptanceDone:
  - true
  - true
  - true
  - true
  - true
  - true
---
# Phase 4a: tool uniquifier + blob hardening (+ R5, R11)

Authority: `docs/internal/IMPLEMENTATION.md` §2.6 uniquifier, §4.1, §11 R5/R11.

Lab session/cookie work is **#14**, not this leaf.

## Evidence (worker)

- Commit: `feat(knot-tool): CSPRNG uniquifier + blob M8/M9/L7/L8/L14 + R5/R11`
- Tests: `cargo test -p knot-tool` — 55 passed
- CSPRNG: `blob::resolve_proposal_nonce` (CLI + RPC); explicit `--nonce` preserved
- M8: `threshold_guard_for_blob` + `ThresholdGuard` verified vs unverified messages
- M9: `bls::verify_multisig` + drop invalid partials in `aggregate_partials`
- L7/L8/L14: `bls::aggregate` Result, `write_atomic`, `GateError`
- R5/R11: `validate_collector_url`, `validate_proposal_id` in collector client

### Fix-pass (review L14 shared-code)

- Commit: `9159e7e` — `fix(encoding): typed GateError at source for digest gate`
- Tests: `knot-encoding` 25 passed; `knot-tool` 55 passed
- `GateError` in `knot-encoding`; `knot-tool::blob::gate_blob` delegates to `gate_blob_for_signing`

## Proposal (worker, if BLOCKED)
