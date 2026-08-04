# DECISIONS — launch-form-knot

_Append-only. Use decision_add / planner._

## 2026-08-04T14:43:20.455Z — Continue on branched tip from audit (no merge from main)

feat/launch-form-knot created from feat/audit-2026-08-full tip in existing worktree. Cursor-owned-worktrees rule: do not hand-roll a second worktree + move from main. Overview (audit+design) preserved linearly.

## 2026-08-04T15:16:33.173Z — Domain tags → nocturne.knot.* (all three) + coordinated redeploy

Locked 2026-08-04.

Rename all three encoding domain tags off `sme-platform.*` to `nocturne.knot.*`, with a version bump in the suffix so old testnet partials fail loudly:

- DOMAIN_PROPOSAL_V1 → nocturne.knot.multisig.proposal.v2 (or keep v1 only if never mainnet; prefer bump)
- DOMAIN_CHANGE_ACCOUNT_V1 → nocturne.knot.multisig-registry.change_account.v2
- DOMAIN_COUNCIL_RESOLVE_V2 → nocturne.knot.prediction-market.council-resolve.v3

Exact final strings to be pinned in implementation plan; prefix MUST be `nocturne.knot.`.

Coordinate: multisig-encoding goldens/tests + wen/prediction-market-logic matching bytes + any tool fixtures. Redeploy required — accepted (redeploy wave anyway).

Related: PM-specific tooling moves to wen/prediction-market; generic Lab stays in knot.

## 2026-08-04T15:27:28.207Z — council_resolve_digest leaves encoding; PM tooling → wen

Locked 2026-08-04.

- Move PM-specific host tooling (pm-resolve CLI/UI/RPC, mirrored ResolveArgs/read types, standalone PM UI, collector PM blob kind if purely PM-shaped — review at impl) to wen/prediction-market.
- Move `council_resolve_digest` / `council_resolve_message` / `DOMAIN_COUNCIL_RESOLVE_*` out of `multisig-encoding` into the PM/wen side (shared types crate or prediction-market-logic encoding module). Knot encoding keeps only generic proposal + change_account digests (still renamed to nocturne.knot.*).
- Wen depends on knot for registry verify_quorum + generic encoding; knot does not depend on wen.
- Goal: knot stays clean general multisig; no PM message layout in encoding.

## 2026-08-04T15:31:14.855Z — Council-resolve domain string pinned

Locked 2026-08-04: after peel to wen, DOMAIN = `nocturne.wen.prediction-market.council-resolve.v3` (byte-exact). Knot generic digests stay `nocturne.knot.*`. Coordinated redeploy with wen.

## 2026-08-04T16:11:32.988Z — PM audit fixes owned by wen plan; docs include website + nocturne-docs

Locked 2026-08-04.

- A1–A5 are not dropped: peel moves code to wen; wen paired plan must fix them (acceptance from audit leaves 010–014) before wen publicly claims pm-resolve.
- Knot public launch plan covers strip + P1–P4 only; references moved findings.
- P2 expanded: align repo READMEs, nocturne-docs /v1/knot/, and Lab/website copy to the same public claim.

## Wen digest ready for peel (2026-08-04)

Wen branch `feat/wen-pm-peel-plan` @ `7ae9728`. Digest in `pm-council-encoding`
(`nocturne.wen.prediction-market.council-resolve.v3`). Knot may delete encoding helpers.

## 2026-08-04 — nocturne-docs /v1/knot/ alignment PR

P2 standards-site pass: https://github.com/aichbindas/nocturne-docs/pull/3

Rewrites `/v1/knot/` for Prove-only generic multisig, `nocturne.knot.*` domains,
AGPL collector split, `aichbindas/knot` links; removes pm-resolve-in-knot from
wen admin cross-link.

## 2026-08-04T16:24:57.856Z — Plan leaves P0–P4 + migrated audit #13

PM fix successors live on wen track pm-peel-and-fixes. Audit #10–14 superseded after this.

## 2026-08-04 — A4 deferred for first public tag

Optional generic Lab live-membership pre-check before approve/quorum/
change-account sign (audit #13 / leaf `007-a4-generic-membership-gate`) is
**not** required for first public tag. Prove-mode on-chain mitigation remains
the guarantee. Recorded so launch is not blocked; leaf stays TODO or deferred.

## 2026-08-04 — Wen A1–A5 successors tracked

Wen track `pm-peel-and-fixes` on `feat/wen-pm-peel-plan`: A1/A2/A3/A5 re-verify
**Pass** (`docs/superpowers/specs/2026-08-04-pm-council-a1-a5-checklist.md`,
commit `7ae9728`). Knot tag not blocked on wen DONE; council UX SSOT is wen.
