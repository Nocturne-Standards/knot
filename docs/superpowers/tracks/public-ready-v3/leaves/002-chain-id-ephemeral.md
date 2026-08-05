---
id: 2
slug: chain-id-ephemeral
status: IN_PROGRESS
owner: cursor-agent
deps:
  - 3
scope:
  - crates/knot-proposals/tests/
  - crates/knot-tool/src/bls.rs
  - docs/internal/IMPLEMENTATION.md
acceptance:
  - Document whether abi::chain_id works under VM::ephemeral
  - "If unset: ship test-only shim before 3b/3c"
  - Blocks encoding/contract v3
acceptanceDone:
  - false
  - false
  - false
---
# Phase 3a: verify abi::chain_id under ephemeral VM

Planner context…

## Evidence (worker)

## Proposal (worker, if BLOCKED)
