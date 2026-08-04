---
id: 3
slug: map-missing-required-optional
status: DONE
owner: gap-map-worker
deps:
  - 1
scope:
  - README.md
  - docs/
  - crates/
acceptance:
  - Required missing (B) separated from optional (C)
  - Each row has Planned cite or claim-rewrite note
acceptanceDone:
  - true
  - true
---
# Kind B/C — missing required vs optional

Planner context…

## Evidence (worker)

`docs/launch-gap-map-2026-08.md` §Kind B (3 rows) and §Kind C (6 rows).
Kind B — required for honest launch, each with a claim-rewrite note per
row schema: B1 Prove-only posture stated only in `security-model.md`'s
appendix, not surfaced as a top-level product claim; B2 PM-resolve feature
ships without the caveat (present in the audit's own text) that the target
contract's on-chain re-verification is unverified from this repo; B3
`docs/versioning.md` cited by 4+ files as the versioning-policy SSOT but
does not exist anywhere in this worktree (confirmed via direct file check).
Kind C — separated required vs optional by checking each "not implemented"
claim against whether the repo's own docs already honestly scope it as
deferred: C1 (QR transfer), C2 (hardware keys), C3 (wallet-extension alt
signer), C4 (hosted Lab subdomain) are all already honestly deferred
in-repo, tagged `defer`/Launch=Y — kept as positive examples rather than
flagged as gaps. C5 (registry README "Next steps" pointing at
prediction-market's own backlog, now a separate repo) and C6 (collector
VPS runbook cited but missing) needed a `doc-move`/`add` tag and discussion
note since the claim location is presently wrong or the artifact is
missing.

## Proposal (worker, if BLOCKED)
