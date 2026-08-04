---
id: 6
slug: monorepo-carve
status: DONE
owner: audit-worker
deps: []
scope:
  - Cargo.toml
  - Cargo.lock
  - crates/
  - vendor/
acceptance:
  - No nest path-deps; license Apache↛AGPL edges; private git pins; vendor copies checked
  - Findings with evidence
acceptanceDone:
  - true
  - true
---
# Monorepo carve invariants

Planner context…

## Evidence (worker)

## Proposal (worker, if BLOCKED)
