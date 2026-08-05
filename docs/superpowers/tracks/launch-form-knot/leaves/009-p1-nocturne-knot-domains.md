---
id: 9
slug: p1-nocturne-knot-domains
status: DONE
owner: worker
deps: []
scope:
  - crates/knot-encoding/
  - docs/internal/
acceptance:
  - DOMAIN proposal/change_account use nocturne.knot.* v2 strings
  - Encoding goldens/tests updated
  - Internal redeploy checklist landed
acceptanceDone:
  - true
  - true
  - true
---
# P1 — Rename generic domains to nocturne.knot.* + redeploy notes

Plan Tasks 5–6.

## Evidence (worker)

- Renamed `DOMAIN_PROPOSAL_V1` → `DOMAIN_PROPOSAL_V2` =
  `b"nocturne.knot.multisig.proposal.v2"`.
- Renamed `DOMAIN_CHANGE_ACCOUNT_V1` → `DOMAIN_CHANGE_ACCOUNT_V2` =
  `b"nocturne.knot.multisig-registry.change_account.v2"`.
- Pinned goldens:
  - proposal (`sample_intent`):
    `8426c1fa5895fe6b2e3a3fe0e3588eaff4b123fde07b075352264f41dfd9c9dd`
  - `change_account` (account=1, nonce=0, 2×96-byte pks, threshold=2):
    `ab2fc0f6d9b490a645b0b5768bcfbfabfce53392251f28bc776e10b6ad22c457`
- Redeploy note: `docs/internal/redeploy-2026-08-domains.md`
- Tests:
  - `cargo test -p knot-encoding` — 16 passed
  - `make test` in `knot-registry` — 22 passed (14 contract + 8 layout)
  - `make test` in `knot-proposals` — 23 passed (16 contract + 7 layout)
- No live digest-path `sme-platform.multisig` hits remain in `crates/` (only
  historical references in audit/gap-map docs).

## Proposal (worker, if BLOCKED)
