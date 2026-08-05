---
id: 1
slug: residual-audit
status: DONE
owner: cursor-agent
deps: []
scope:
  - crates/multisig-tool/src/rpc.rs
  - crates/multisig-tool/src/main.rs
  - crates/multisig-tool/src/chain.rs
  - crates/multisig-tool/src/store.rs
  - crates/multisig-tool/src/dto.rs
  - crates/multisig-tool/src/collector_client.rs
  - crates/multisig-tool/src/mock_ledger.rs
  - crates/multisig-tool/static/
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
