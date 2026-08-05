# Task B3 report — contracts v3 (leaf #5)

**Date:** 2026-08-05  
**Branch:** `feat/public-ready-v3-rename`  
**Status:** DONE_WITH_CONCERNS

## Delivered

### knot-encoding
- `ProposeArgs.nonce` (caller uniquifier)
- `ProposalView.epoch` replaces `chain_id`

### knot-proposals v3
- State: `epoch`, `DigestRecord{consumed}`, removed `chain_id`/`committee_nonces`
- `proposal_digest_v3` via `abi::chain_id()` + `abi::self_id()`
- `init_registry` bumps epoch (no wipe); removed `init_chain_id`/`wipe_open_proposals`
- `set_proposal_ttl` no wipe; `MAX_PROPOSAL_TTL` = 100_000
- `prune(limit)` permissionless batch (max 128)
- Rich events §2.13 tuples on create/approve/finalize/pruned
- `finalize` marks `consumed`, blocks self-target

### knot-registry v3
- `change_account` uses `change_account_message_v3` (chain_id + self_id + member_count)

### Docs
- READMEs: deploy registry then proposals; burn v2 signatures

## Tests

```
cd crates/knot-registry && make wasm && cargo test --release   # 22 passed
cd crates/knot-proposals && make test                          # 27 passed (20 contract + 7 layout)
```

§2.15 covered: H1 (digest), M3 parallel, consumed replay, L2 deadline==height,
M1 ttl exceed, zero deadline, set_proposal_ttl bounds, epoch bump, prune+consumed,
finalize self, init_registry O(1) after 200 proposals, merge.

Not in-contract: H1 two chain ids (encoding golden only); prune past deadline
then re-propose (partial — consumed-before-deadline tested).

## Concerns / deferred

- ~~**knot-tool does not compile** (`ProposeArgs.nonce`, `ProposalView.epoch`) — leaf #6~~
  **Fixed 2026-08-05:** `knot-tool` compiles against v3 types; mock/RPC use
  `proposal_digest_v3` + `change_account_message_v3`; `ProposeArgs.nonce`
  defaults to 0; `ProposalView.epoch` in status/preview APIs. Full R5/R11
  CSPRNG uniquifier still leaf #6.
- **event-decoder arms** not added (emit-only per §2.13; leaf #10)
- Blob CLI still v2 `ProposalIntent` shape until #6

## Redeploy

1. Registry v3 → proposals v3 → `init_registry`
2. Re-create councils; all v2 signatures burned
