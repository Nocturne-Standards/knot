---
id: 2
slug: registry-timelock
status: TODO
owner: null
deps:
  - 1
scope:
  - crates/knot-registry/src/state.rs
  - crates/knot-registry/src/call_types.rs
  - crates/knot-registry/tests/contract.rs
  - crates/knot-registry/tests/layout_goldens.rs
  - crates/knot-registry/README.md
  - crates/knot-registry/CHANGELOG.md
  - .contract-authz-baseline
acceptance:
  - delay 0 change_account still applies in-call and bumps nonce
  - delay > 0 membership not live until execute_pending; execute before eta panics
  - set_timelock from 0 applies now; later shorten is delayed
  - cancel_pending immediate; bound to this pending; authz baseline updated
acceptanceDone:
  - false
  - false
  - false
  - false
---
# Registry per-account schedule/execute

## Objective

Per-account `timelock_blocks` on the registry. `change_account` / `set_timelock` schedule; delay 0 applies in-call; `execute_pending` permissionless after eta; `cancel_pending` immediate current-member quorum. Authz baseline for new pub methods.

## Files

- create:
- modify: crates/knot-registry/src/state.rs, call_types.rs, tests/, README.md, CHANGELOG.md, .contract-authz-baseline
- do not touch: change_account v3 digest; proposals contract

## Interfaces

- `set_timelock(SetTimelockArgs)`
- `cancel_pending(CancelPendingArgs)`
- `execute_pending(account_id: u64)`
- delay 0: schedule then apply in the same call

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
(cd crates/knot-registry && make wasm && make test)
```

## Evidence (worker)

- HEAD (`git rev-parse HEAD`): f3370bab1277ebf391672e8ed82df15c3779c1ea
- status_digest (`git status --porcelain=v1 -uall | git hash-object --stdin`): e69de29bb2d1d6434b8b29ae775ad8c2e48c5391
- If porcelain empty: also `git log --oneline <upstream>..HEAD`. Clean tree AND no new commits = no-op — do not `leaf_done`
- verification (quote command + output):

```
$ (cd crates/knot-registry && make wasm && make test)
test result: ok. 17 passed
layout_goldens: 6 passed
delay_zero_change_account_still_applies_in_call ... ok
set_timelock_from_zero_applies_now_later_change_is_delayed ... ok
cancel_pending_is_immediate_and_bound_to_this_pending ... ok
```

- Covered: delay 0 in-call; delayed membership + execute before eta panics; raise from 0 now; shorten delayed; cancel immediate; authz baseline
- Not covered: proposals queue (leaf 3)

## Chat handoff (mid-leaf)

- Next: leaf_done after feat commit
- Open questions: none
- Touched: crates/knot-registry/src/state.rs tests README CHANGELOG .contract-authz-baseline
- Covered: registry schedule/execute/cancel
- Not covered: proposals

## Chat handoff (mid-leaf)

Fill before chat end, sibling spawn, or compaction drop. Do not wait for `leaf_done`. Next chat reads this, not the old transcript.

- Next:
- Open questions:
- Touched:
- Covered:
- Not covered:

## Proposal (worker, if BLOCKED)
