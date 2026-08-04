---
id: 11
slug: p3-residual-lab-tests
status: DONE
owner: worker
deps: []
scope:
  - crates/multisig-tool/
acceptance:
  - Focused tests cover remaining generic RPC/CLI paths after PM peel
  - A4 leaf either DONE or explicitly deferred in DECISIONS
acceptanceDone:
  - true
  - true
---
# P3 — Residual generic Lab RPC tests

Plan Tasks 10–11.

## Evidence (worker)

- `crates/multisig-tool/src/rpc.rs`: `build_router` + `generic_rpc_smoke` axum oneshot tests — token gate, `DEMO_MODE=mock` setup/status, account create, proposal create/preview/approve/finalize, `confirm` required on approve.
- `crates/multisig-tool/tests/rpc_generic_smoke.rs`: spawns real `multisig-tool serve` (mock ledger), full HTTP flow + `/api/pm-resolve/status` → 404 (no PM surface).
- `cargo test -p multisig-tool --lib` — 12 passed.
- `cargo test -p multisig-tool` — all bins/tests passed (incl. `rpc_generic_smoke`, `no_pm_resolve_surface`, `collector_roundtrip`, `blob_aggregate_local`).
- **A4 (leaf 007):** skipped — live membership pre-check before approve/quorum sign not implemented; Prove-mode on-chain mitigation unchanged. Deferred per plan Task 10 optional scope.

## Proposal (worker, if BLOCKED)
