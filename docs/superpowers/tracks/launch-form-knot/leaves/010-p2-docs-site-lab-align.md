---
id: 10
slug: p2-docs-site-lab-align
status: IN_PROGRESS
owner: worker-p2-docs
deps: []
scope:
  - README.md
  - docs/
  - crates/multisig-tool/static/
acceptance:
  - Root README matches launch-form public claim; Prove-first; AGPL callout
  - docs/versioning.md exists; A12–A15 addressed
  - Lab copy has no treasury clash; nocturne-docs /v1/knot/ aligned or PR linked
acceptanceDone:
  - true
  - true
  - false
---
# P2 — Public docs + nocturne-docs + Lab copy alignment

Plan Tasks 7–9.

## Evidence (worker)

**Task 7 (knot repo):**
- Added `docs/versioning.md` (testnet-only, PINNED-DIFFERENT-REDEPLOYED, per-crate semver vs git tag).
- Rewrote root `README.md`: Prove-first, AGPL collector callout, crate versions from `Cargo.toml`, `aichbindas/knot` links, Status archaeology → `docs/internal/deploy-history.md`.
- Fixed A14 dead `../../../` doc links in crate READMEs/CHANGELOGs (correct `../../docs/versioning.md` or removed dead targets).
- Fixed A15: `chain.rs` + `multisig-tool/README.md` — self-contained `RUSK_WALLET_PWD` text, no `references/testnet-wallet.md`.

**Task 8 (knot repo):**
- Lab `static/index.html`: "Form the treasury" → "Form a multisig account"; "Treasury / DAO payout" → "Committee payout".

**Task 9 (nocturne-docs) — pending separate PR:**
- `nocturne-docs` repo not in this workspace; cannot edit `/v1/knot/` here. Knot-side claim is aligned; standards site still needs peel/Prove-only pass (follow-up PR in `nocturne-docs`).

## Proposal (worker, if BLOCKED)
