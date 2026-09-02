---
id: 5
slug: tool-lab
status: DONE
owner: knot-timelock-session
deps:
  - 1
  - 2
  - 3
scope:
  - crates/knot-tool/src/
  - crates/knot-tool/README.md
  - crates/knot-tool/CHANGELOG.md
acceptance:
  - CLI/RPC can set_timelock, execute_pending, cancel_pending, proposal execute/cancel
  - mock ledger shows eta and queued status
  - account meta includes timelock_blocks
acceptanceDone:
  - true
  - true
  - true
---
# Lab CLI/RPC/mock for delay

## Objective

Lab CLI/RPC/mock: set_timelock, execute_pending, cancel_pending, proposal execute/cancel. Show eta and queued status. Account meta includes timelock_blocks.

## Files

- create:
- modify: crates/knot-tool/src/{main,rpc,mock_ledger,bls,membership,diagnose}.rs, README.md, CHANGELOG.md
- do not touch: wasm contracts; encoding domains (consume only)

## Interfaces

- `knot-tool account set-timelock|execute-pending|cancel-pending`
- `knot-tool proposal execute|cancel`
- RPC `/api/account/{id}/set-timelock|execute-pending|cancel-pending`
- RPC `/api/proposal/{id}/execute|cancel`

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
cargo test -p knot-tool
```

## Evidence (worker)

- HEAD (`git rev-parse HEAD`): f3370bab1277ebf391672e8ed82df15c3779c1ea
- status_digest (`git status --porcelain=v1 -uall | git hash-object --stdin`): e69de29bb2d1d6434b8b29ae775ad8c2e48c5391
- If porcelain empty: also `git log --oneline <upstream>..HEAD`. Clean tree AND no new commits = no-op — do not `leaf_done`
- verification (quote command + output):

```
$ cargo test -p knot-tool
cargo test: 73 passed (7 suites, 55.96s)
delay_zero_finalize_still_immediate ... ok
delay_queues_until_execute ... ok
set_timelock_shorten_is_pending_until_execute ... ok
```

- Covered: CLI/RPC methods; mock eta/queued; account meta timelock_blocks; static mock-ledger.js routes
- Not covered: live testnet signing round-trip

## Chat handoff (mid-leaf)

- Next: leaf_done after feat commit
- Open questions: none
- Touched: crates/knot-tool/src/* static/mock-ledger.js README CHANGELOG
- Covered: Lab mock/RPC/CLI
- Not covered: hosted mock UI click-through

## Chat handoff (mid-leaf)

Fill before chat end, sibling spawn, or compaction drop. Do not wait for `leaf_done`. Next chat reads this, not the old transcript.

- Next:
- Open questions:
- Touched:
- Covered:
- Not covered:

## Proposal (worker, if BLOCKED)
