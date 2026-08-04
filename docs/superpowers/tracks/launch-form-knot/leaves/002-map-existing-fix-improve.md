---
id: 2
slug: map-existing-fix-improve
status: DONE
owner: gap-map-worker
deps:
  - 1
scope:
  - docs/security-audit-2026-08-04.md
  - crates/
acceptance:
  - Every audit Medium+ has Kind A row or explicit wontfix/claim-change
  - Other shipped-path fix/improve rows listed
acceptanceDone:
  - true
  - true
---
# Kind A — existing fix/improve

Planner context…

## Evidence (worker)

`docs/launch-gap-map-2026-08.md` §Kind A — 16 rows (A1–A16). All 5 audit
Medium+ findings from `docs/security-audit-2026-08-04.md` cross-walked
(A1 Critical, A2/A3 High, A4/A5 Medium), each linked to its already-queued
fix leaf in `docs/superpowers/tracks/audit-2026-08-full/leaves/010-014-*.md`
— confirmed via `cat docs/superpowers/tracks/audit-2026-08-full/STATUS.md`
that all 5 are `TODO`/unclaimed, so links are live, not stale. Added 4
audit-Low rows (A6–A9, "intentional Low clusters" per design doc) and 7
non-audit shipped-path quality rows found during this pass: `rpc.rs`
(2011 lines, hosts every Critical/High/Medium finding) has zero tests
(A10); CLI (`main.rs`, 1588 lines) independently duplicates the same
sign/submit logic as `rpc.rs` (A11); root README version-drift vs
`Cargo.toml` (A12); front-door GitHub link points at pre-carve
`aichbindas/sme_platform` monorepo instead of the carve-target
`aichbindas/knot` the same README names (A13); 18 dead `../../../`
cross-repo links verified by direct filesystem check from each citing
file's own directory, not grep-only (A14); a live CLI error message
(`chain.rs:315,329`) citing a nonexistent doc (A15); and a cosmetic-but-
permanent `sme-platform.` prefix baked into all 3 on-chain crypto domain
tags (A16).

## Proposal (worker, if BLOCKED)
