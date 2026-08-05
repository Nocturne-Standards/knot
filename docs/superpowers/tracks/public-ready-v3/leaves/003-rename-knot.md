---
id: 3
slug: rename-knot
status: TODO
owner: null
deps:
  - 1
scope:
  - crates/
  - Cargo.toml
  - Cargo.lock
  - Makefile
  - docs/
  - README.md
acceptance:
  - All crates/dirs/bin/env/header/wasm paths renamed per IMPLEMENTATION §5.7
  - Tests pass after rename
  - Pin JSON keys still multisig-*
  - Zero behaviour change
acceptanceDone:
  - false
  - false
  - false
  - false
---
# Phase 2: mechanical multisig-* → knot-* rename

Planner context…

## Evidence (worker)

## Proposal (worker, if BLOCKED)
