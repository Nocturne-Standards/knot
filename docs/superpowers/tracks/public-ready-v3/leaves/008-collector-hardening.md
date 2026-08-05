---
id: 8
slug: collector-hardening
status: TODO
owner: null
deps:
  - 4
scope:
  - crates/knot-collector/
  - crates/multisig-collector/
acceptance:
  - L9–L12 + M11 first
  - C1 digest recompute via knot-encoding
  - M10+M12 verify with dusk-core
  - Rewrite no-dusk_core docs to never-holds-keys guarantee
acceptanceDone:
  - false
  - false
  - false
  - false
---
# Phase 5: collector C1/M10–M12 + caps

Planner context…

## Evidence (worker)

## Proposal (worker, if BLOCKED)
