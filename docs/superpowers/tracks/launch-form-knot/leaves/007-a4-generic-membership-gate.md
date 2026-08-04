---
id: 7
slug: a4-generic-membership-gate
status: TODO
owner: null
deps: []
scope:
  - crates/multisig-tool/src/rpc.rs
  - crates/multisig-tool/src/main.rs
acceptance:
  - approve/quorum/change_account sign paths fetch live account before sign
  - Document Prove-mode on-chain mitigation remains
acceptanceDone:
  - false
  - true
---
# A4 — generic Lab membership/threshold pre-check (from audit #13)

Successor of knot audit #13. **Deferred** for first public tag (see
`DECISIONS.md` 2026-08-04 A4 deferred). Prove-mode on-chain mitigation
documented in `docs/security-model.md`.

**Supersedes:** `audit-2026-08-full` leaf `#13`.

## Evidence (worker)

- Acceptance #2: security-model Known gap (A4) + DECISIONS deferral.
- Acceptance #1: not implemented — stays open post-tag.

## Proposal (worker, if BLOCKED)
