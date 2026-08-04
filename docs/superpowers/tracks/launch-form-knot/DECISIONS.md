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
