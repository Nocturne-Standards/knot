# Task 3 Report: RPC error-code schema (R4)

## Status: DONE

## Commits

- `8315a2e` — feat(knot-tool): RPC error-code schema at API boundary (R4)
- `0452bb4` — fix(knot-tool): proposal create bad hex returns 400 (review Important)

## Changes

### R4 — Error-code schema (`rpc.rs`)
- Added `RpcError` / `RpcErrorBody` with fixed `{code, message}` catalog.
- All `/api/*` handler errors return JSON; raw `Display` / wallet / parse details logged via `eprintln!` only.
- Session gate (`require_session`) returns `unauthorized` JSON schema.
- Catalog codes: `identity_exists`, `identity_not_found`, `account_not_found`, `proposal_not_found`, `proposal_not_open`, `not_a_member`, `confirm_required`, `digest_mismatch`, `invalid_input`, `invalid_hex`, `invalid_target`, `collector_config`, `live_mode_required`, `internal_error`.

### Frontend (`app.js`)
- `api()` reads `body.message` from JSON error responses (falls back to string body for non-API routes).

### Tests
- Unit (`rpc::generic_rpc_smoke`): updated confirm/non-member assertions; added `api_errors_use_code_schema_not_raw_details` — **8 passed**; review fix adds `proposal_create_rejects_invalid_hex` — **9 passed**.
- Integration (`rpc_generic_smoke.rs`): non-member error asserts JSON schema (not re-run this session).

## Files touched

- `crates/knot-tool/src/rpc.rs`
- `crates/knot-tool/static/app.js`
- `crates/knot-tool/tests/rpc_generic_smoke.rs`

## Concerns

- Bootstrap/index non-API errors still plain text (out of R4 scope).
- `SubmitOut.log` still carries wallet stdout on success paths (R3 scope).

## Verification

```bash
cargo test -p knot-tool --bin knot-tool generic_rpc_smoke
```

## Review fix — Important (proposal create hex)

**Issue:** `api_proposal_create` mapped `hex::decode` failures on `target` / `args_hex` to `RpcError::internal` (500) instead of `RpcError::invalid_hex` (400).

**Fix:** Both decode sites now use `RpcError::invalid_hex`, matching `msg_bytes` and rest of catalog.

**Test:** Added `proposal_create_rejects_invalid_hex` in `generic_rpc_smoke` — bad target and bad `args_hex` assert 400 + `invalid_hex` code.

**Verification:** `cargo test -p knot-tool --bin knot-tool generic_rpc_smoke` — **9 passed**.
