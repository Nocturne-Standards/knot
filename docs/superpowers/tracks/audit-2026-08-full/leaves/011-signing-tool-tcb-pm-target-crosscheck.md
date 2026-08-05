---
id: 11
slug: signing-tool-tcb-pm-target-crosscheck
status: superseded
owner: planner-migrate
deps: []
scope:
  - crates/knot-tool/src/rpc.rs
  - crates/knot-tool/src/main.rs
acceptance:
  - Submit rejects blob pm_contract_id that does not match live-resolved PM contract id
  - Test covers stale-target rejection
acceptanceDone:
  - false
  - false
---
# PM resolve submit: cross-check pm_contract_id vs live deployment

Planner context…

## Evidence (worker)

Work relocated per launch-form L3/L7. Goals preserved on successor leaf. Successor: wen://pm-peel-and-fixes#3 a2-submit-target-crosscheck. Old leaf kept in git as finding record; must not be claimed for knot-local PM tooling fixes.

## Proposal (worker, if BLOCKED)

Supersede this leaf. Execute successor wen://pm-peel-and-fixes#3 a2-submit-target-crosscheck instead.
