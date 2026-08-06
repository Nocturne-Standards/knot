# Task 2 Review: OTP session cookie + fix-pass

## Prior findings (Important)

1. **`rpc_generic_smoke` stderr reader** — `spawn_serve` used `read_to_string` on piped stderr; buffer fills only at EOF while `knot-tool serve` runs indefinitely, so `wait_for_serve` never saw `code=` and integration tests hung/failed.
2. **`reqwest` `cookies` in prod deps** — `cookie_store` pulled into shipped `knot-tool` binary; only integration tests need `cookie_store(true)`.

## Fixes applied

1. **Incremental stderr read** — `BufReader::lines()` appends each line to shared `stderr_buf` as serve prints bootstrap URL; `wait_for_serve` observes `code=` while process live.
2. **`cookies` → dev-dependencies** — prod `reqwest` keeps `rustls-tls` + `json` only; test client gets `cookies` via `[dev-dependencies]`. Verified: `cargo tree -p knot-tool -e normal -i cookie_store` prints nothing.

## Acceptance (unchanged, now verified end-to-end)

- OTP bootstrap → HttpOnly `SameSite=Strict` session cookie; HTML never embeds secret
- `/api/*` gated on cookie (primary) or `X-Knot-Token` (secondary)
- Fail-closed frontend; `KNOT_FRONTEND_MOCK` only when explicitly set
- Integration smoke: cookie jar bootstrap, OTP one-shot, full mock proposal flow

## Residual (out of scope)

- README still describes old bearer-token-in-HTML model
