---
id: 8
slug: signing-tool-tcb
status: DONE
owner: audit-worker
deps:
  - 1
  - 4
scope:
  - crates/knot-tool/
  - crates/knot-collector/
  - crates/knot-encoding/
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
# Signing tool TCB (knot-tool)

Planner context…

## Evidence (worker)

## Proposal (worker, if BLOCKED)
