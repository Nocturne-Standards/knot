---
id: 4
slug: encoding-v3
status: DONE
owner: cursor-agent
deps:
  - 2
scope:
  - crates/knot-encoding/
  - crates/knot-encoding/
acceptance:
  - DOMAIN_*_V3 preimages with self_id/chain_id/epoch/member_count per §2.12
  - Goldens + unit tests for H1/H2 field binding
  - v2 kept only if still needed internally
acceptanceDone:
  - true
  - true
  - true
---
# Phase 3b: encoding digests v3

Planner context…

## Evidence (worker)

- Added `DOMAIN_PROPOSAL_V3`, `DOMAIN_CHANGE_ACCOUNT_V3` and preimage/digest builders
  (`proposal_preimage_v3`, `proposal_digest_v3`, `change_account_preimage_v3`,
  `change_account_digest_v3`, `ProposalIntentV3`) per IMPLEMENTATION.md §2.12.
- v2 API unchanged (`proposal_digest`, `change_account_digest`, …).
- Goldens locked: proposal
  `2b2243ab796615051a9c4478dfd63f21acd2ebf0857ca6962447bf6f74606e80`;
  change_account
  `0d9f0b3d1d74c4805365e4c495bf6d4e40c8bc7a6732c734fb32e64048dcbf8d`.
- H1 tests: `h1_same_intent_two_contracts_differ`, `h1_same_intent_two_chain_ids_differ`.
- H2 test: `h2_change_account_chain_and_registry_binding` + `member_count` prefix test.
- `cargo test -p knot-encoding --features call-types --release` — 28 passed.
