# Design — Knot launch form (gap map → discuss → public-ready)

Date: 2026-08-04  
Repo: `aichbindas/knot` (worktree `feat/audit-2026-08-full` or successor)  
Status: draft for operator review before gap-map execution

## Goal

Make **knot** the first product repo that can go public-facing without wave/carve/§/Status archaeology in the front door — while **keeping** technical knowledge (move, don’t delete).

Learn the playbook, then repeat on **chit**, then **ambit** (sequencing B).

End-to-end path (this design covers through gap map + launch-form settlement; later phases get their own plans):

1. Audit (done) → findings + fix leaves queued  
2. **Gap map** (this wave)  
3. Deep discussion → settle **launch form**  
4. Implementation plan (fixes + finishes + deletes + doc hygiene)  
5. Implement + test  
6. Public-ready surface

## Non-goals (this wave)

- Fix coding / Critical patches  
- Per-leaf implementation plans  
- Chit or ambit gap maps  
- Publishing / org transfer / crates.io

## Decisions locked

| Decision | Choice |
|---|---|
| Deliverable before launch-form talk | Gap map only (option C) |
| Repo order | Knot → chit → ambit (option B) |
| Dual-mode (from audit) | Prove only — on-chain re-verify; no pure-Coord product offer |
| Public docs | Strip meta talk from README/crate front doors; preserve knowledge elsewhere |

## Gap map — what it must cover

The map is **not** only “missing features.” Three equal columns of attention:

### A — Existing functionality (fix / improve)

Things that already exist in tree but are wrong, incomplete, unsafe, or unclean for public claim:

- Audit Critical/High/Medium (and intentional Low clusters)  
- UX/TCB hygiene that Prove-mode does not excuse (e.g. tool membership gates)  
- Test gaps on **shipped** paths (missing goldens, no parity tests)  
- Hand-mirrored / duplicated types (`extract`)  
- Dead code or demo-lie on paths we still expose

### B — Missing functionality — **required** for launch form

Planned or implied by public docs / security-model / README claims, but not implemented (or not wired). Must land before public tag **or** the public claim must be rewritten so we don’t overclaim.

### C — Missing functionality — **optional**

Nice-to-have / roadmap. Explicitly `defer` so launch stays thin. Must not block publish if claim is honest.

### Tags (every row)

| Tag | Meaning |
|---|---|
| `ship` | Exists, trustworthy enough to claim; maybe tiny polish only |
| `fix` | Exists; must change before claim (security or correctness) |
| `improve` | Exists; quality/cleanliness upgrade, launch-blocking only if discussion says so |
| `finish` | Half-built or unwired; required or optional per column B/C |
| `add` | Missing entirely; required or optional |
| `defer` | Not for this launch form |
| `delete` | Remove for cleanliness / honesty |
| `extract` | Shared crate / golden vectors / stop hand-mirroring |
| `doc-move` | Knowledge stays; leave public README |

### Row schema

| ID | Surface | Kind (A/B/C) | Planned cite | Code cite | Tests/goldens | Tag | Launch? (Y/N/discuss) | Notes |

`Launch?` filled tentatively in the map; **settled** only in launch-form discussion.

## Phase 0 — Knot gap map execution

**Track id:** `launch-form-knot` (separate from `audit-2026-08-full` so audit fix leaves stay untouched).

**Worktree:** prefer new `.worktrees/launch-form-knot` from `main` or branch from current audit branch if docs-only merge is simpler — default: **new worktree from `main`, cherry or merge audit report if needed for cites**. Practical default: continue on `feat/audit-2026-08-full` *only if* operator wants one PR; else new branch `feat/launch-form-knot` from latest `main` + copy/link audit report paths.

Recommended: **new worktree** `feat/launch-form-knot` from `main`, read audit report from `origin/feat/audit-2026-08-full` or merge that branch first so cites resolve in-tree.

**Deliverable file:** `docs/launch-gap-map-2026-08.md`

**Also produce:** short `docs/doc-hygiene-inventory.md` — list of README/Status/meta blobs → `keep-public` / `move-to-docs-internal` / `move-to-nocturne-docs` / `delete-duplicate`.

**Leaves (suggested):**

1. `inventory-surfaces` — crate + host surface list  
2. `map-existing-fix-improve` — Kind A rows (incl. audit cross-walk)  
3. `map-missing-required-optional` — Kind B/C rows vs plans/specs/README claims  
4. `map-tests-goldens` — what exists vs needed for `ship`/`fix` rows  
5. `doc-hygiene-inventory`  
6. `gap-map-rollup` — single `docs/launch-gap-map-2026-08.md`

## Phase 1 — Launch-form discussion (after map)

Inputs: gap map, `docs/security-audit-2026-08-04.md`, `docs/security-model.md` (Prove posture), doc-hygiene inventory.

Settle and write `docs/launch-form-knot.md`:

1. One-paragraph public claim  
2. Crates in / out of first public tag  
3. Kind A items that are launch-blocking (`fix` list)  
4. Kind B required adds/finishes  
5. Kind C explicitly deferred  
6. `delete` / `extract` list  
7. Required tests/goldens bar  
8. README shape (no Wave/carve/§ Status dump) + where knowledge lives  
9. Then: one implementation plan (not per leaf)

## Phase 2+ — Chit, then ambit

Same gap-map schema and discussion template. Knot’s `launch-form-*.md` is the template.

## Success criteria (Phase 0)

- Every major surface has ≥1 row  
- Every audit Medium+ has a Kind A cross-walk row (or explicit “wontfix / claim change”)  
- Required vs optional missing are separated  
- Doc hygiene inventory exists  
- No product code changes in Phase 0  

## Open for operator before execute

Confirm worktree strategy: new `feat/launch-form-knot` from `main` (merge audit branch for report) vs continue on audit feat branch.
