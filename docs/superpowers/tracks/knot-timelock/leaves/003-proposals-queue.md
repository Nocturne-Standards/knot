---
id: 3
slug: proposals-queue
status: TODO
owner: null
deps:
  - 1
  - 2
scope:
  - crates/knot-proposals/src/state.rs
  - crates/knot-proposals/src/call_types.rs
  - crates/knot-proposals/tests/contract.rs
  - crates/knot-proposals/tests/layout_goldens.rs
  - crates/knot-proposals/README.md
  - crates/knot-proposals/CHANGELOG.md
  - .contract-authz-baseline
acceptance:
  - delay 0 finalize still call_raw in-call
  - delay > 0 queues; execute after eta call_raw; execute before eta panics
  - finalize panics if now+delay > deadline
  - cancel immediate with quorum; digest stays consumed until deadline; prune keeps Queued
acceptanceDone:
  - false
  - false
  - false
  - false
---
# Proposals queue execute cancel

## Objective

Split finalize: quorum as today; delay 0 call_raw now; else Queued until permissionless execute. Immediate cancel quorum over cancel domain. Panic if now+delay > deadline. Digest stays consumed after cancel.

## Files

- create:
- modify: crates/knot-proposals/src/state.rs, call_types.rs, tests/, README.md, CHANGELOG.md, .contract-authz-baseline
- do not touch: proposal signing digest v3; registry schedule logic except reading timelock_blocks

## Interfaces

- `finalize(proposal_id)` queues or executes
- `execute(proposal_id)` permissionless after execute_at
- `cancel(CancelProposalArgs)` immediate
- `ProposalView.execute_at`; `ProposalStatus::{Queued,Cancelled}`

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
(cd crates/knot-proposals && make wasm && make test)
```

## Evidence (worker)

- HEAD (`git rev-parse HEAD`): f3370bab1277ebf391672e8ed82df15c3779c1ea
- status_digest (`git status --porcelain=v1 -uall | git hash-object --stdin`): e69de29bb2d1d6434b8b29ae775ad8c2e48c5391
- If porcelain empty: also `git log --oneline <upstream>..HEAD`. Clean tree AND no new commits = no-op — do not `leaf_done`
- verification (quote command + output):

```
$ (cd crates/knot-proposals && make wasm && make test)
test result: ok. 25 passed
layout_goldens: 7 passed
delay_zero_finalize_still_call_raw ... ok
delay_queues_then_execute_after_eta ... ok
finalize_panics_if_delay_exceeds_deadline ... ok
cancel_queued_is_immediate_and_digest_stays_consumed ... ok
prune_keeps_queued_until_deadline ... ok
```

- Covered: delay 0 call_raw; queue/execute; deadline invariant; cancel + consumed digest; prune keeps Queued; GOLDEN_PROPOSAL_VIEW_HEX
- Not covered: Lab CLI (leaf 5)

## Chat handoff (mid-leaf)

- Next: leaf_done after feat commit
- Open questions: none
- Touched: crates/knot-proposals/src/state.rs tests README CHANGELOG
- Covered: finalize/execute/cancel
- Not covered: tool

## Chat handoff (mid-leaf)

Fill before chat end, sibling spawn, or compaction drop. Do not wait for `leaf_done`. Next chat reads this, not the old transcript.

- Next:
- Open questions:
- Touched:
- Covered:
- Not covered:

## Proposal (worker, if BLOCKED)
