---
id: 13
slug: signing-tool-tcb-generic-membership-gate
status: superseded
owner: planner-migrate
deps: []
scope:
  - crates/knot-tool/src/rpc.rs
  - crates/knot-tool/src/main.rs
acceptance:
  - approve/quorum/change_account sign paths fetch live account before sign
  - Document Prove-mode on-chain mitigation remains
acceptanceDone:
  - false
  - false
---
# Generic sign paths: live membership/threshold pre-check

Planner context…

## Evidence (worker)

Work relocated per launch-form L3/L7. Goals preserved on successor leaf. Successor: knot://launch-form-knot#7 a4-generic-membership-gate. Old leaf kept in git as finding record; must not be claimed for knot-local PM tooling fixes.

## Proposal (worker, if BLOCKED)

Supersede this leaf. Execute successor knot://launch-form-knot#7 a4-generic-membership-gate instead.
