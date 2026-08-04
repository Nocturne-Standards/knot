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
