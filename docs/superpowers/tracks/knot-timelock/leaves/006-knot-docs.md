---
id: 6
slug: knot-docs
status: DONE
owner: knot-timelock-session
deps:
  - 2
  - 3
scope:
  - docs/security-model.md
  - docs/design-notes.md
  - README.md
  - crates/knot-registry/README.md
  - crates/knot-proposals/README.md
  - crates/knot-encoding/README.md
acceptance:
  - security-model and design-notes explain delay vs deadline and immediate cancel
  - crate READMEs label PINNED-DIFFERENT-REDEPLOYED
  - "knot README: standalone; Atlas optional; Atlas delay 0 when paired"
acceptanceDone:
  - true
  - true
  - true
---
# Knot in-repo timelock docs

## Objective

In-repo docs: delay vs deadline; cancel immediate; PINNED-DIFFERENT-REDEPLOYED on encoding/registry/proposals; Knot standalone; Atlas optional with Atlas delay 0 when paired.

## Files

- create:
- modify: docs/security-model.md, docs/design-notes.md, README.md, crate READMEs
- do not touch: nocturne-docs (leaf 4)

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
rg -n "PINNED-DIFFERENT-REDEPLOYED|timelock_blocks|delay vs deadline" README.md docs/security-model.md docs/design-notes.md crates/knot-encoding/README.md crates/knot-registry/README.md crates/knot-proposals/README.md
```

## Evidence (worker)

- HEAD (`git rev-parse HEAD`): f3370bab1277ebf391672e8ed82df15c3779c1ea
- status_digest (`git status --porcelain=v1 -uall | git hash-object --stdin`): e69de29bb2d1d6434b8b29ae775ad8c2e48c5391
- If porcelain empty: also `git log --oneline <upstream>..HEAD`. Clean tree AND no new commits = no-op — do not `leaf_done`
- verification (quote command + output):

```
$ rg -n "PINNED-DIFFERENT-REDEPLOYED|timelock_blocks|delay vs deadline" README.md docs/security-model.md docs/design-notes.md crates/knot-encoding/README.md crates/knot-registry/README.md crates/knot-proposals/README.md
```

Hits in security-model, design-notes section 4, README Atlas pairing, crate READMEs PINNED-DIFFERENT-REDEPLOYED.

- Covered: delay vs deadline; immediate cancel; PINNED labels; standalone Knot + Atlas delay 0
- Not covered: nocturne-docs (leaf 4)

## Chat handoff (mid-leaf)

- Next: leaf_done after feat commit
- Open questions: none
- Touched: docs/security-model.md docs/design-notes.md README.md crate READMEs
- Covered: in-repo docs
- Not covered: public site

## Chat handoff (mid-leaf)

Fill before chat end, sibling spawn, or compaction drop. Do not wait for `leaf_done`. Next chat reads this, not the old transcript.

- Next:
- Open questions:
- Touched:
- Covered:
- Not covered:

## Proposal (worker, if BLOCKED)
