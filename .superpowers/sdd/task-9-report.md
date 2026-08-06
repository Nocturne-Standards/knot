# Task 9 — registry diagnostics off-chain

**Status:** DONE  
**Authority:** IMPLEMENTATION.md §4.3 L3

## Summary

Removed on-chain `diagnose_quorum`, `account_meta`, `member_key_bytes` from `knot-registry`; kept `next_account_id`. Reimplemented diagnose in `knot-tool/src/diagnose.rs` from `account()` + local `pk.verify`. CLI/RPC meta/keys/diagnose paths use account view. CHANGELOG updated.

## Evidence

See `task-9-test-evidence.txt`. Registry: 14 contract + 6 golden tests green; wasm rebuilt. knot-tool: `diagnose_matches_member_and_sig_counts` green.

## Files

- `crates/knot-registry/src/state.rs` — delete 3 methods
- `crates/knot-tool/src/diagnose.rs` — new off-chain diagnose
- `crates/knot-tool/src/main.rs`, `rpc.rs` — wire local diagnose
- `.contract-authz-baseline` — drop 3 entries
- `crates/knot-tool/CHANGELOG.md`
