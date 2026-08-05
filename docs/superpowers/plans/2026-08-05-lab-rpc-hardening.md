# Lab/RPC hardening (#14) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Harden `knot-tool` Lab/RPC per IMPLEMENTATION.md §11 findings R1–R4, R6–R9, R12 before Lab is shared.

**Architecture:** Lab-only changes in `knot-tool` (`rpc.rs`, `main.rs`, `chain.rs`, `mock_ledger.rs`, `static/`). Session moves from HTML-embedded bearer to one-shot OTP → HttpOnly cookie. RPC returns fixed error codes; wallet logs stay on stderr. Collector unchanged.

**Tech Stack:** Rust, axum 0.8, existing `subtle` ct_eq, vanilla JS Lab UI.

## Global Constraints

- Authority: `docs/internal/IMPLEMENTATION.md` §11 (R1–R4, R6–R9, R12) and leaf `014-lab-rpc-hardening.md`
- Lab-only — do **not** put cookie session on `knot-collector`
- Pin JSON keys and signing domains untouched
- Branch: `feat/public-ready-v3-rename` in worktree `.worktrees/public-ready-v3-rename`
- Claim packages with `claim.py` before heavy `cargo test`/`cargo build` on `knot-tool`
- Commit on this branch; push Policy A for `feat/*`
- Prefer TDD: failing test → implement → green → commit
- Do not invent pre-v3 deployment compatibility

---

### Task 1: Bind + DEMO_MODE + change-account nonce latch (R12, R6, R8)

**Files:**
- Modify: `crates/knot-tool/src/rpc.rs` (bind check)
- Modify: `crates/knot-tool/src/mock_ledger.rs` (`DemoMode::from_env`)
- Modify: `crates/knot-tool/src/main.rs` (change-account `--nonce` + latch)
- Modify: `crates/knot-tool/src/rpc.rs` / chain API if change-account nonce exposed over RPC
- Test: `crates/knot-tool/src/mock_ledger.rs` unit tests; `rpc`/`main` tests as needed
- Docs: brief README note that `DEMO_MODE` must be set

**Acceptance:**
- Bind via parsed `SocketAddr`; refuse unless `ip().is_loopback()` (not string prefix)
- `DEMO_MODE` unset or unknown → refuse start with clear error (no silent default to mock)
- Explicit `DEMO_MODE=mock` or `testnet` only; loud banner on serve
- Change-account CLI `--nonce` refused by default; require explicit env latch (e.g. `KNOT_ALLOW_CHANGE_ACCOUNT_NONCE=1`) for diagnostics
- Tests cover refuse paths

**Steps:**
1. Write failing tests for DemoMode refuse-when-unset and bind non-loopback
2. Implement R6/R12/R8
3. Run `cargo test -p knot-tool --lib` (+ any touched ints)
4. Commit

---

### Task 2: OTP → HttpOnly session cookie + fail-closed (R1, R7)

**Files:**
- Modify: `crates/knot-tool/src/rpc.rs` (index, auth middleware, bootstrap route)
- Modify: `crates/knot-tool/static/index.html` (no `__TOKEN__` secret embed)
- Modify: `crates/knot-tool/static/app.js` (credentials/cookies; fail closed)
- Modify: `crates/knot-tool/tests/rpc_generic_smoke.rs`
- Possibly: `Cargo.toml` if cookie jar helpers needed (axum extractors preferred)

**Acceptance:**
- Serve generates one-shot OTP; CLI opens `http://127.0.0.1:PORT/?code=OTP` (and hash/tab as today)
- Valid OTP sets HttpOnly `SameSite=Strict` session cookie; OTP not reusable; HTML never embeds session secret
- `/api/*` auth via session cookie (header may remain as secondary for tests — prefer cookie; document)
- Missing/invalid session → 401; frontend without session fails closed (no silent `KNOT_FRONTEND_MOCK` from leftover `__TOKEN__`)
- Static Cloudflare Lab mock path: only when explicitly `KNOT_FRONTEND_MOCK=true` (or equivalent), never from missing token placeholder

**Steps:**
1. Failing smoke: HTML body must not contain session secret; `/?code=` establishes cookie
2. Implement OTP + cookie + middleware
3. Update smoke tests
4. Commit

---

### Task 3: RPC error-code schema (R4)

**Files:**
- Modify: `crates/knot-tool/src/rpc.rs` (error JSON shape)
- Modify: `crates/knot-tool/static/app.js` (display fixed messages)
- Test: unit or smoke covering error body shape

**Acceptance:**
- API errors: JSON `{ "code": "...", "message": "..." }` with fixed message catalog
- Raw wallet / `e.to_string()` only on stderr, never as browser message
- Unknown/internal → generic code + fixed message

**Steps:**
1. Define small code enum/map
2. Failing test asserting no raw panic string in JSON body
3. Wire handlers; commit

---

### Task 4: Quorum / change-account preview+confirm (R2)

**Files:**
- Modify: `crates/knot-tool/src/rpc.rs` (preview endpoints or confirm flags)
- Modify: `crates/knot-tool/static/` (confirm UX matching proposal approve)
- Modify: `crates/knot-tool/src/main.rs` (CLI twins)
- Prefer: one signer identity per `serve` call when signing quorum/change (document if soft)

**Acceptance:**
- Same preview+confirm gate as proposal approve for quorum verify / change-account multi-sign paths
- CLI has matching confirm/preview behavior
- Tests cover reject-without-confirm

**Steps:**
1. Locate proposal approve confirm pattern; mirror
2. TDD + UI + CLI
3. Commit

---

### Task 5: Status labels + escapeHtml (R3, R9)

**Files:**
- Modify: `crates/knot-tool/src/chain.rs` (`tx_status_label`)
- Modify: `crates/knot-tool/static/app.js` (all innerHTML sinks)
- Modify: tests in `chain.rs`
- Update IMPLEMENTATION claim if it overstates escape coverage

**Acceptance:**
- Propagate/preverify → `submitted` or `propagated`; `confirmed` only when log shows block inclusion
- Every user-controlled / server-string interpolation through `escapeHtml` (or textContent)
- Tests updated for label classification

**Steps:**
1. Fix `tx_status_label` + tests
2. Audit `app.js` sinks; escape
3. Commit

---

### Task 6: Leaf close-out

**Files:**
- Modify: leaf `014-lab-rpc-hardening.md` evidence
- Track `leaf_done` via nocturne MCP (controller)

**Acceptance:**
- All leaf acceptance checkboxes true
- Focused suite green; README serve docs match new auth + DEMO_MODE

**Steps:**
1. Final `cargo test -p knot-tool` (claim first)
2. Update leaf evidence; mark done
3. Commit track docs if needed
