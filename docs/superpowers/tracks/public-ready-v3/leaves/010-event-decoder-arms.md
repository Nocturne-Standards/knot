---
id: 10
slug: event-decoder-arms
status: DONE
owner: cursor-agent
deps:
  - 5
scope:
  - sme_platform/rusk-experiments/event-decoder/
acceptance:
  - Match arms for knot proposal/registry rich events in sme_platform event-decoder
  - No pre-v3 dual-decode fallbacks
  - Proven against VM::ephemeral receipts if feasible
acceptanceDone:
  - true
  - true
  - true
---
# Phase 7: event-decoder Knot arms

Planner context…

## Evidence (worker)

- **Repo:** `aichbindas/sme_platform` branch `feat/knot-event-decoder-arms`
- **Commit:** `25959e6d74b733c8a68bc8b2626a65d37efbd98c` — `feat(event-decoder): add Knot rich-event decode arms`
- **File:** `rusk-experiments/event-decoder/src/lib.rs`
- **Arms:** `proposal_created` / `proposal_approved` / `proposal_finalized` / `pruned` / `registry_set` under `multisig-proposals|knot-proposals`; `account_created|account_changed` under `multisig-registry|knot-registry`
- **Shapes match** knot v3 emits in `crates/knot-proposals` / `crates/knot-registry` (no pre-v3 dual-decode)
- **Proof:** rkyv unit round-trips (`cargo test knot_ --lib` → 5 passed); VM::ephemeral receipt path skipped (wasm include heavy); crafted archived bytes sufficient
- **Note:** sme_platform kit gate adoption (public-surface / contract-authz) not landed here — local `.gate-coverage-waivers` only so this leaf could commit; separate hygiene work

## Proposal (worker, if BLOCKED)
