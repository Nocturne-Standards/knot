# Task B1 report — abi::chain_id under VM::ephemeral (leaf #2)

**Date:** 2026-08-05  
**Branch:** `feat/public-ready-v3-rename`  
**Status:** DONE

## Result

`abi::chain_id()` **works** under the repo's `VM::ephemeral()` contract-test harness.

## Method

1. Added `chain_id()` export to `proposals-test-target` calling `abi::chain_id()`.
2. Added integration test `abi_chain_id_available_under_ephemeral_vm` in
   `knot-proposals/tests/contract.rs` using existing `initialize()` helper
   (`VM::ephemeral()` + `genesis_session(0xCA)`).
3. Confirmed against `dusk-vm` 1.6.0 source: `genesis_session` inserts
   `Metadata::CHAIN_ID`.

## Shim

**Not required.** Metadata is set by `genesis_session(chain_id)`; all Knot
contract tests already follow this pattern.

## Caveat

Bare `VM::ephemeral()` without `genesis_session` leaves `CHAIN_ID` unset;
`abi::chain_id()` panics. v3 contract work must keep using `genesis_session`.

## Tests

```
cargo test --release abi_chain_id_available_under_ephemeral_vm
# 1 passed
```

## Docs

- `docs/internal/IMPLEMENTATION.md` §2.5, §2.14 item 1 (stamp `160ec67`)
- `docs/superpowers/tracks/public-ready-v3/leaves/002-chain-id-ephemeral.md` Evidence

## Unblocks

Leaves #4 (encoding v3) and #5 (contracts v3).
