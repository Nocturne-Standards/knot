---
id: 14
slug: signing-tool-tcb-pm-threshold-warn
status: superseded
owner: planner-migrate
deps: []
scope:
  - crates/multisig-tool/src/rpc.rs
acceptance:
  - api_pm_resolve_init cross-checks or hard-warns on threshold mismatch vs live account
acceptanceDone:
  - false
---
# PM resolve init: warn/reject blob threshold vs live registry

Planner context…

## Evidence (worker)

Work relocated per launch-form L3/L7. Goals preserved on successor leaf. Successor: wen://pm-peel-and-fixes#5 a5-threshold-live-check. Old leaf kept in git as finding record; must not be claimed for knot-local PM tooling fixes.

## Proposal (worker, if BLOCKED)

Supersede this leaf. Execute successor wen://pm-peel-and-fixes#5 a5-threshold-live-check instead.
