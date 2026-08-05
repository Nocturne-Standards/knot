---
id: 10
slug: signing-tool-tcb-pm-membership-gate
status: superseded
owner: planner-migrate
deps: []
scope:
  - crates/knot-tool/src/rpc.rs
  - crates/knot-tool/src/main.rs
  - crates/knot-tool/static/
acceptance:
  - api_pm_resolve_sign and CLI Sign reject non-member PK after live registry account fetch
  - UI disables Sign until membership check succeeds
  - Test covers non-member rejection
acceptanceDone:
  - false
  - false
  - false
---
# PM resolve sign: live membership gate before BLS partial

Planner context…

## Evidence (worker)

Work relocated per launch-form L3/L7. Goals preserved on successor leaf. Successor: wen://pm-peel-and-fixes#2 a1-membership-gate-sign. Old leaf kept in git as finding record; must not be claimed for knot-local PM tooling fixes.

## Proposal (worker, if BLOCKED)

Supersede this leaf. Execute successor wen://pm-peel-and-fixes#2 a1-membership-gate-sign instead.
