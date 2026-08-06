# Task 1 Report: Bind + DEMO_MODE + change-account nonce latch (R12, R6, R8)

## Status: DONE

## Commits

- `edd8524` — feat(knot-tool): harden bind, DEMO_MODE, and change-account nonce

## Changes

### R12 — Loopback bind (`rpc.rs`)
- Added `validate_loopback_bind()`: parse `SocketAddr`, refuse unless `ip().is_loopback()`.
- Accepts `127.0.0.1:*` and `[::1]:*`; refuses `0.0.0.0`, LAN IPs, invalid strings.
- Unit tests in `rpc::bind_validation`.

### R6 — Explicit DEMO_MODE (`mock_ledger.rs`, `rpc.rs`, `main.rs`)
- `DemoMode::from_env()` now returns `Result`; unset or unknown values refuse with clear error.
- Only `mock` and `testnet` (case-insensitive) accepted.
- Loud banner on serve (`════ DEMO_MODE=… ════`).
- README updated: `DEMO_MODE` required before `serve`.

### R8 — Change-account nonce latch (`main.rs`, `mock_ledger.rs`)
- CLI `change-account --nonce` refused unless `KNOT_ALLOW_CHANGE_ACCOUNT_NONCE=1`.
- RPC path unchanged (already uses on-chain free-read nonce only).
- Unit test for latch default-off / latch-on.

## Tests

```
cargo test -p knot-tool --lib   → 16 passed
cargo test -p knot-tool --tests → 32 passed (6 suites)
```

New tests: `demo_mode_from_env_requires_explicit_value`, `change_account_nonce_latch_refuses_by_default`, `bind_validation::{accepts_ipv4_and_ipv6_loopback, refuses_non_loopback_and_invalid}`.

## Concerns

None. `localhost:8877` no longer passes prefix check before parse — it always failed `SocketAddr::parse` anyway; use `127.0.0.1` or `[::1]`.

## Files touched

- `crates/knot-tool/src/rpc.rs`
- `crates/knot-tool/src/mock_ledger.rs`
- `crates/knot-tool/src/main.rs`
- `crates/knot-tool/README.md`

---

## Review fix pass (2026-08-05)

**Status:** DONE

**Commit:** `b856590` — fix(knot-tool): mode-aware serve banner and DEMO_MODE docs

**Fixes:**
- Banner: `DemoMode::serve_banner_label()` — mock says "in-process mock ledger (no chain writes)"; testnet says "TESTNET ONLY (live chain writes)". No more hardcoded TESTNET ONLY for mock.
- `Serve` CLI doc (`main.rs`): requires explicit `DEMO_MODE=mock` or `testnet`; removed stale "(default)" wording.
- README: removed "(default for local demos)" contradiction.

**Tests:** `cargo test -p knot-tool --lib` → 16 passed (added banner label assertions in `demo_mode_as_str_for_setup_status`).

---

## Review fix pass 2 (2026-08-05)

**Status:** DONE

**Commit:** `9c79617` — fix(knot-tool): drop redundant TESTNET ONLY from serve open hint

**Fixes:**
- `rpc.rs` line 131: removed hardcoded `TESTNET ONLY` from open-URL hint; banner above already mode-aware via `serve_banner_label()`.

**Tests:** `cargo test -p knot-tool --lib` → 16 passed
