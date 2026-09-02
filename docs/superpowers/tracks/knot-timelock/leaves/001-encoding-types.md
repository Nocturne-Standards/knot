---
id: 1
slug: encoding-types
status: IN_PROGRESS
owner: knot-timelock-session
deps: []
scope:
  - crates/knot-encoding/src/lib.rs
  - crates/knot-encoding/src/call_types.rs
  - crates/knot-encoding/src/layout_goldens.rs
  - crates/knot-encoding/README.md
  - crates/knot-encoding/CHANGELOG.md
acceptance:
  - set_timelock.v1, cancel_pending.v1, cancel_proposal.v1 domains live only in knot-encoding
  - MultisigAccountView/AccountMeta carry timelock_blocks + pending; ProposalStatus Queued=3 Cancelled=4; ProposalView.execute_at
  - Layout + preimage goldens updated; change_account v3 and proposal v3 digests unchanged
acceptanceDone:
  - false
  - false
  - false
---
# Encoding domains, views, status

## Objective

Declare set_timelock / cancel_pending / cancel_proposal domains and preimage helpers once in knot-encoding. Extend MultisigAccountView, AccountMeta, ProposalStatus, ProposalView for delay. Keep change_account v3 and proposal v3 digests unchanged. Goldens for new preimages and view layout.

## Files

- create:
- modify: crates/knot-encoding/src/lib.rs, call_types.rs, layout_goldens.rs, README.md, CHANGELOG.md
- do not touch: proposal_digest_v3 / change_account_digest_v3 known vectors; contract behavior

## Interfaces

- `DOMAIN_SET_TIMELOCK_V1` = `nocturne.knot.multisig-registry.set_timelock.v1`
- `DOMAIN_CANCEL_PENDING_V1` = `nocturne.knot.multisig-registry.cancel_pending.v1`
- `DOMAIN_CANCEL_PROPOSAL_V1` = `nocturne.knot.multisig.proposal.cancel.v1`
- `ProposalStatus::{Queued=3, Cancelled=4}`; `ProposalView.execute_at`
- `MultisigAccountView.{timelock_blocks, pending}`; `AccountMeta.{timelock_blocks, pending_execute_at}`

## Constraints

- Honor only this leaf's scope + acceptance
- Worker: `leaf_*` and Write on `leaves/` for this leaf id only; planner owns siblings
- Policy A: commit/push on feat/* inside the track worktree; no git writes on primary/main
- No silent SPEC shrink; `leaf_block` if blocked
- Work only inside the assigned worktree
- Swap: parked X; live Y — when a pin, constraint, or working-set item leaves the stage, write the exchange. Unspoken drop is shrink.

## Verification

Exact command(s) the worker runs and the parent re-runs:

```bash
cargo test -p knot-encoding --features call-types
```

## Evidence (worker)

- HEAD (`git rev-parse HEAD`): f3370bab1277ebf391672e8ed82df15c3779c1ea
- status_digest (`git status --porcelain=v1 -uall | git hash-object --stdin`): e69de29bb2d1d6434b8b29ae775ad8c2e48c5391
- If porcelain empty: also `git log --oneline <upstream>..HEAD`. Clean tree AND no new commits = no-op — do not `leaf_done`
- verification (quote command + output):

```
$ cargo test -p knot-encoding --features call-types
cargo test: 33 passed (2 suites, 0.12s)
```

- Covered: domains live only in knot-encoding; views/status fields; layout + preimage goldens; v3 digests unchanged (`sample_intent_v3_digest_golden`, `change_account_digest_v3_known_vector`)
- Not covered: contract behavior (leaves 2–3)

## Chat handoff (mid-leaf)

- Next: leaf_done after feat commit
- Open questions: none
- Touched: crates/knot-encoding/src/{lib,call_types,layout_goldens}.rs README CHANGELOG
- Covered: encoding types + goldens
- Not covered: wasm contracts

## Chat handoff (mid-leaf)

Fill before chat end, sibling spawn, or compaction drop. Do not wait for `leaf_done`. Next chat reads this, not the old transcript.

- Next:
- Open questions:
- Touched:
- Covered:
- Not covered:

## Proposal (worker, if BLOCKED)
