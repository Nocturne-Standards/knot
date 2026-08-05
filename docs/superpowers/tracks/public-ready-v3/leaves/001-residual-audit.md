---
id: 1
slug: residual-audit
status: DONE
owner: cursor-agent
deps: []
scope:
  - crates/knot-tool/src/rpc.rs
  - crates/knot-tool/src/main.rs
  - crates/knot-tool/src/chain.rs
  - crates/knot-tool/src/store.rs
  - crates/knot-tool/src/dto.rs
  - crates/knot-tool/src/collector_client.rs
  - crates/knot-tool/src/mock_ledger.rs
  - crates/knot-tool/static/
  - docs/internal/IMPLEMENTATION.md
acceptance:
  - Full read of listed surfaces at HEAD
  - Findings folded into IMPLEMENTATION.md (new § or §7/§8) — not a second frozen audit as authority
  - No code fixes in this leaf unless trivial one-liners agreed
  - Stop for human review before phase 2
acceptanceDone:
  - true
  - true
  - true
  - true
---
# Phase 1: residual host-surface audit

Planner context…

## Evidence (worker)

## Proposal (worker, if BLOCKED)
