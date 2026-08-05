---
id: 14
slug: lab-rpc-hardening
status: TODO
owner: null
deps:
  - 1
scope:
  - crates/knot-tool/src/rpc.rs
  - crates/knot-tool/src/main.rs
  - crates/knot-tool/src/chain.rs
  - crates/knot-tool/src/mock_ledger.rs
  - crates/knot-tool/static/
  - docs/internal/IMPLEMENTATION.md
acceptance:
  - "R1: OTP bootstrap → HttpOnly SameSite=Strict cookie; HTML never embeds secret; CLI opens /?code=…"
  - "R7: fail closed if no session / bootstrap"
  - "R2: quorum + change-account preview+confirm; prefer one signer per serve call; CLI twins"
  - "R3: tx_status_label submitted/propagated vs confirmed only on inclusion"
  - "R4: RPC error-code schema + fixed messages; raw wallet log stderr-only"
  - "R6: DEMO_MODE required explicit; refuse ambiguous; loud banner"
  - "R8: refuse change-account --nonce by default; dev-only latch"
  - "R9: escapeHtml every innerHTML sink; fix IMPLEMENTATION claim"
  - "R12: bind via SocketAddr is_loopback"
acceptanceDone:
  - false
  - false
  - false
  - false
  - false
  - false
  - false
  - false
  - false
---
# Phase 1b: Lab/RPC hardening (R1–R4, R6–R9, R12)

Authority: `docs/internal/IMPLEMENTATION.md` §11.

## Design notes (locked)

- **R1:** One-shot OTP in `/?code=…` (CLI `open`) → HttpOnly cookie, `SameSite=Strict`, localhost. Do **not** put session secret in HTML. Header was never the problem; HTML embed was.
- **R4:** Classify errors into a small `code` + fixed `message` map at the RPC boundary. Do not sanitize arbitrary strings. Full wallet logs → stderr only.
- Lab-only — do not copy cookie session model onto the collector.

Prefer landing after `#3 rename` if both in flight (path churn). May start on `knot-tool` paths if rename not yet done.

## Evidence (worker)

## Proposal (worker, if BLOCKED)
