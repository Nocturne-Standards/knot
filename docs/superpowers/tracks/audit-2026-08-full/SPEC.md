---
planner_model: cursor-grok-4.5-medium
worker_model: composer-2.5
reviewer_model: claude-sonnet-5-thinking-high
---
# SPEC — audit-2026-08-full

## Goal

Full security+logic audit of knot (BLS M-of-N multisig) post-carve. Elevate signing-tool TCB (posture D). Explore dual mode Coord vs Prove. Findings + fix leaves only.

## Scope

Crates: encoding, registry, proposals, tool, collector.
Attack surface D: every host that gates digests, aggregates, or submits — primary TCB knot-tool.
July 28 audit = cross-check only; re-prove every finding.

## Non-goals

Fix PRs this wave; full Atlas audit (validate-before-submit pointers only).

## Dual posture (explore)

- **Coord:** knot-tool validates last-mile; tool must be airtight.
- **Prove:** on-chain verify_quorum / verify_quorum_aggregate (Moonlight-bound BLS).
Goal: document whether both can be offered; implement later.

## Acceptance (track-level)

- docs/security-audit-2026-08-04.md landed with ranked findings
- All review leaves DONE; Critical/High re-verified
- Fix leaves queued for Medium+
