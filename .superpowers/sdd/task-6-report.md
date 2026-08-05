# Task 6 report — tool uniquifier + blob hardening

**Leaf:** #6 `tool-uniquifier-blobs`  
**Branch:** `feat/public-ready-v3-rename`  
**Status:** DONE

## Acceptance

| Item | Done | Evidence |
|------|------|----------|
| CSPRNG uniquifier + `--nonce` proposals | yes | `blob::random_proposal_nonce` / `resolve_proposal_nonce`; CLI `proposal create` + `blob create` + RPC `ProposalCreateReq` use `Option<u64>` |
| M8 fetch threshold / honest offline label | yes | `main::threshold_guard_for_blob` fetches registry `account`; `ThresholdGuard` + `check_partial_count` — verified uses REFUSING, offline uses honest label |
| M9 verify partials locally | yes | `bls::verify_multisig` (V2 then V1); `aggregate_partials` drops invalid/malformed with named stderr |
| L7 Result aggregate | yes | `bls::aggregate` returns `Result`; callers use `?` |
| L8 atomic blob write | yes | `blob::write_atomic` (tmp+rename+dir fsync); `write_file` uses it |
| L14 typed errors | yes | `blob::GateError` (DigestMismatch / Encoding); `gate_blob` + RPC mapping |
| R5 collector URL allowlist | yes | `validate_collector_url` in `collector_client::resolve` |
| R11 proposal id 64-hex | yes | `validate_proposal_id` on `pull` / `append_partial` |

## Tests

```
cargo test -p knot-tool  → 55 passed (lib + integration)
```

New unit tests: CSPRNG nonce, gate typed errors, M8/M9 aggregate guards, atomic write, R5/R11 collector client, empty aggregate.

## Files

- `crates/knot-tool/src/blob.rs` — uniquifier, gate, atomic write, M8/M9 aggregate
- `crates/knot-tool/src/bls.rs` — Result aggregate, verify_multisig
- `crates/knot-tool/src/collector_client.rs` — R5, R11
- `crates/knot-tool/src/main.rs` — CLI nonce + threshold fetch on aggregate
- `crates/knot-tool/src/rpc.rs` — RPC nonce + typed gate

## Notes

- change-account `--nonce` latch (#14) untouched
- collector server (#8) untouched
- `knot-encoding` `gate_blob_for_signing` still returns `()`; tool uses local `gate_blob` with typed errors (leaf scope knot-tool only)
