---
id: 10
slug: signing-tool-tcb-pm-membership-gate
status: TODO
owner: null
deps: []
scope:
  - crates/multisig-tool/src/rpc.rs
  - crates/multisig-tool/src/main.rs
  - crates/multisig-tool/static/
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

## Proposal (worker, if BLOCKED)
