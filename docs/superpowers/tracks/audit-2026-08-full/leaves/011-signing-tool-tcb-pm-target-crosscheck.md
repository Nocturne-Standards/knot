---
id: 11
slug: signing-tool-tcb-pm-target-crosscheck
status: TODO
owner: null
deps: []
scope:
  - crates/multisig-tool/src/rpc.rs
  - crates/multisig-tool/src/main.rs
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

## Proposal (worker, if BLOCKED)
