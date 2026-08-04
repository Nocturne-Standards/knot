# Knot launch form (settling)

Inputs: [`launch-gap-map-2026-08.md`](launch-gap-map-2026-08.md),
[`doc-hygiene-inventory.md`](doc-hygiene-inventory.md),
[`security-audit-2026-08-04.md`](security-audit-2026-08-04.md),
[`security-model.md`](security-model.md).

**Status:** in discussion — locked items below; open items marked TODO.

## Public claim (draft — TODO finalize wording)

Knot is an M-of-N BLS multisig suite for Dusk: on-chain registry + proposals
(`call_raw`), canonical encoding, local signing Lab, optional untrusted
collector. Authorization is **Prove mode** (chain re-verifies quorum). The Lab
is convenience/UX, not the final authority.

Prediction-market council resolve is a **consumer** of the registry; its host
tooling lives with that product (wen), not as a first-class knot surface.

## Locked decisions

| ID | Decision |
|---|---|
| L1 | Sequencing: knot public-ready playbook first, then chit, then ambit |
| L2 | Dual posture: **Prove only** (no pure-Coord product offer) |
| L3 | **Move PM-specific tooling** (pm-resolve CLI/UI/RPC, mirrored PM ABI types, standalone PM UI) to the **wen / prediction-market** repo |
| L4 | Keep in knot: encoding, registry, proposals, **generic** Lab/CLI, collector |
| L5 | Domain tags: rename **all three** off `sme-platform.*` to prefix **`nocturne.knot.`**, version-bump suffixes, coordinated redeploy with wen (accepted) |
| L6 | Lab demo is generic proposals walkthrough — **not** a wen demo; rename “treasury” UI copy to avoid clash with wen treasury contracts (intent locked; exact copy TODO) |

### Domain rename targets (prefix locked; exact strings pin in impl plan)

| Constant (today) | Direction |
|---|---|
| `DOMAIN_PROPOSAL_V1` = `sme-platform.multisig.proposal.v1` | → `nocturne.knot.multisig.proposal.v2` |
| `DOMAIN_CHANGE_ACCOUNT_V1` = `sme-platform.multisig-registry.change_account.v1` | → `nocturne.knot.multisig-registry.change_account.v2` |
| `DOMAIN_COUNCIL_RESOLVE_V2` = `sme-platform.prediction-market.council-resolve.v2` | → `nocturne.knot.prediction-market.council-resolve.v3` |

`council_resolve_*` may remain in `multisig-encoding` as a shared preimage helper for wen, or move next to PM types when tooling moves — **TODO** (packaging only; bytes must match either way).

## Open (TODO)

- [ ] Exact public README one-paragraph claim
- [ ] Crates in first public tag (collector AGPL callout?)
- [ ] Launch-blocking Kind A after PM peel (A1–A5 largely move with tooling; A10/A12–A15 remain)
- [ ] Kind B docs (Prove claim at top level, versioning.md, dead links)
- [ ] Kind C defer list final
- [ ] Where `council_resolve_digest` lives after peel
- [ ] Required tests/goldens bar for public tag
- [ ] Doc hygiene: what moves to `docs/internal/` vs nocturne-docs

## Out of scope this launch form

Chit/ambit maps; external firm audit; crates.io.
