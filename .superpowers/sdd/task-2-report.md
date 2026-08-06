# Task 2 Report: OTP → HttpOnly session cookie + fail-closed (R1, R7)

## Status: DONE

## Commits

- `94ad248` — feat(knot-tool): OTP bootstrap + HttpOnly session cookie (R1, R7)

## Changes

### R1 — OTP bootstrap → session cookie (`rpc.rs`)
- Replaced HTML-embedded bearer token with one-shot OTP (`/?code=…`) + HttpOnly `knot_session` cookie (`SameSite=Strict`, `Path=/`).
- CLI opens bootstrap URL (`?code=` + optional hash/query); session secret never in HTML.
- OTP consumed on first successful bootstrap; reuse returns 401.
- `/api/*` auth: session cookie (primary) or `X-Knot-Token` header matching session (secondary, documented for programmatic tests).

### R7 — Fail closed (`app.js`, `index.html`)
- Removed `window.KNOT_TOOL_TOKEN` / `__TOKEN__` embed from `index.html`.
- `USE_FRONTEND_MOCK` only when `window.KNOT_FRONTEND_MOCK === true` (static Cloudflare Lab).
- `fetch` uses `credentials: "include"`; missing session shows banner, no silent mock fallback.

### Tests
- Unit (`rpc::generic_rpc_smoke`): HTML secret absence, OTP bootstrap + cookie attrs, OTP one-shot, session gate, header secondary auth, full mock proposal flow — **7 passed**.
- Integration (`rpc_generic_smoke.rs`): cookie jar bootstrap via stderr OTP parse, no secret in HTML, OTP reuse 401, full flow — updated (not re-run after stderr-reader fix; unit suite green).

## Files touched

- `crates/knot-tool/src/rpc.rs`
- `crates/knot-tool/static/index.html`
- `crates/knot-tool/static/app.js`
- `crates/knot-tool/tests/rpc_generic_smoke.rs`
- `crates/knot-tool/Cargo.toml` (reqwest `cookies` feature)

## Concerns

- README still describes old bearer-token-in-HTML model (out of task scope).
- Integration tests parse OTP from serve stderr; background thread reader added for pipe buffering.

## Verification

```bash
cargo test -p knot-tool --bin knot-tool generic_rpc_smoke
cargo test -p knot-tool --test rpc_generic_smoke -- --test-threads=1
```

## Fix-pass evidence (2026-08-05)

Claim: `claim.py acquire knot-tool` OK; released after tests.

### Changes

- `tests/rpc_generic_smoke.rs`: `spawn_serve` stderr via `BufReader::lines()` incremental append (fixes EOF-only `read_to_string` hang).
- `Cargo.toml`: `reqwest` `cookies` moved from `[dependencies]` to `[dev-dependencies]`; prod tree has no `cookie_store`.

### Test output

```
cargo test -p knot-tool --test rpc_generic_smoke -- --test-threads=1
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 98.95s

cargo test -p knot-tool --lib
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
```

Prod dep check: `cargo tree -p knot-tool -e normal -i cookie_store` → nothing (no cookie_store in shipped binary).
