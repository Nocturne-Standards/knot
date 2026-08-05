---
id: 9
slug: registry-diagnostics-offchain
status: DONE
owner: cursor-agent
deps:
  - 5
scope:
  - crates/knot-registry/
  - crates/knot-tool/
acceptance:
  - Delete diagnose_quorum/account_meta/member_key_bytes; keep next_account_id
  - Reimplement diagnose in knot-tool
  - CHANGELOG notes why
acceptanceDone:
  - true
  - true
  - true
---
# Phase 6: registry diagnostics off-chain

Planner context…

## Evidence (worker)

## Proposal (worker, if BLOCKED)
