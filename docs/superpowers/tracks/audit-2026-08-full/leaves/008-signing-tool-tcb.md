---
id: 8
slug: signing-tool-tcb
status: DONE
owner: audit-worker
deps:
  - 1
  - 4
scope:
  - crates/multisig-tool/
  - crates/multisig-collector/
  - crates/multisig-encoding/
acceptance:
  - Digest re-gate after collector pull verified in code
  - Live membership/threshold before sign — not UI-only
  - Bearer/loopback/keystore bounds reviewed
  - Aggregate subset honesty reviewed
acceptanceDone:
  - true
  - true
  - true
  - true
---
# Signing tool TCB (multisig-tool)

Planner context…

## Evidence (worker)

## Proposal (worker, if BLOCKED)
