---
id: 4
slug: docs-site
status: TODO
owner: null
deps:
  - 5
scope:
  - /Users/leonidas/dev/aichbindas/nocturne-docs/docs/v1/knot/
acceptance:
  - "docs.nocturne-standards.org knot pages: Knot works alone; Atlas is reference directory; set Atlas timelock_blocks to 0 when using Knot delay"
acceptanceDone:
  - false
---
# nocturne-docs Atlas pairing note

## Objective

Public knot pages: Knot works alone. Atlas is the reference service-directory to pair. When both delays exist, set Atlas timelock_blocks to 0. Separate repo PR.

## Files

- create:
- modify: nocturne-docs/docs/v1/knot/{index,architecture,contracts}.md
- do not touch: knot crate source; live contract IDs

## Interfaces

n/a: behavioral only

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
rg -n "Atlas" /Users/leonidas/dev/aichbindas/nocturne-docs/docs/v1/knot/
```

## Evidence (worker)

- HEAD (`git rev-parse HEAD`): nocturne-docs feat branch (separate repo)
- status_digest (`git status --porcelain=v1 -uall | git hash-object --stdin`): see nocturne-docs
- If porcelain empty: also `git log --oneline <upstream>..HEAD`. Clean tree AND no new commits = no-op — do not `leaf_done`
- verification (quote command + output):

```
$ rg -n "Atlas" /Users/leonidas/dev/aichbindas/nocturne-docs/docs/v1/knot/
architecture.md: Knot works alone. Atlas is the reference service-directory
index.md: set Atlas timelock_blocks to 0
contracts.md: delay 0 applies now / execute / cancel
```

- Covered: standalone Knot; Atlas optional reference directory; Atlas delay 0 when paired
- Not covered: live docs.nocturne-standards.org deploy (site PR)

## Chat handoff (mid-leaf)

- Next: nocturne-docs feat PR
- Open questions: none
- Touched: nocturne-docs/docs/v1/knot/{index,architecture,contracts}.md
- Covered: pairing copy
- Not covered: site CI publish

## Chat handoff (mid-leaf)

Fill before chat end, sibling spawn, or compaction drop. Do not wait for `leaf_done`. Next chat reads this, not the old transcript.

- Next:
- Open questions:
- Touched:
- Covered:
- Not covered:

## Proposal (worker, if BLOCKED)
