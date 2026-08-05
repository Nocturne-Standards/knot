---
id: 12
slug: monorepo-carve-pm-abi-shared-crate
status: superseded
owner: planner-migrate
deps: []
scope:
  - crates/knot-tool/src/pm_resolve_types.rs
  - crates/knot-tool/src/pm_read_types.rs
acceptance:
  - PM ABI types come from shared dep or golden-vector parity test fails on drift
  - Carve docs note the boundary
acceptanceDone:
  - false
  - false
---
# Replace hand-mirrored PM ABI types with shared crate or goldens

Planner context…

## Evidence (worker)

Work relocated per launch-form L3/L7. Goals preserved on successor leaf. Successor: wen://pm-peel-and-fixes#4 a3-pm-abi-parity. Old leaf kept in git as finding record; must not be claimed for knot-local PM tooling fixes.

## Proposal (worker, if BLOCKED)

Supersede this leaf. Execute successor wen://pm-peel-and-fixes#4 a3-pm-abi-parity instead.
