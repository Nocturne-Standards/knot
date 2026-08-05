# Task B2 report — encoding digests v3 (leaf #4)

**Date:** 2026-08-05  
**Branch:** `feat/public-ready-v3-rename`  
**Status:** DONE

## Delivered

- `DOMAIN_PROPOSAL_V3`, `DOMAIN_CHANGE_ACCOUNT_V3`
- `proposal_preimage_v3` / `proposal_digest_v3` + `ProposalIntentV3`
- `change_account_preimage_v3` / `change_account_digest_v3` / `change_account_message_v3`
- v2 APIs retained unchanged

## Layout

Per IMPLEMENTATION.md §2.12: proposal adds `self_id:[u8;32]` + `epoch:u64_le` after
`chain_id`; change_account adds `chain_id`, registry `self_id`, and `member_count:u32_le`
before member pks.

## Tests

```
cargo test -p knot-encoding --features call-types --release
# 28 passed
```

H1: different `self_id` / `chain_id` → different proposal digests.  
H2: change_account binds chain + registry instance; `member_count` prefix prevents
field-shifting.

## Goldens

| Kind | Hex |
|------|-----|
| proposal v3 (sample) | `2b2243ab796615051a9c4478dfd63f21acd2ebf0857ca6962447bf6f74606e80` |
| change_account v3 | `0d9f0b3d1d74c4805365e4c495bf6d4e40c8bc7a6732c734fb32e64048dcbf8d` |

## Unblocks

Leaf #5 (contracts v3) — contracts can switch to v3 digest calls.

## Concerns

None. Tool/contracts still on v2 until #5/#6; additive API only.
