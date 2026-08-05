---
id: 2
slug: chain-id-ephemeral
status: IN_PROGRESS
owner: cursor-agent
deps:
  - 3
scope:
  - crates/knot-proposals/tests/
  - crates/knot-tool/src/bls.rs
  - docs/internal/IMPLEMENTATION.md
acceptance:
  - Document whether abi::chain_id works under VM::ephemeral
  - "If unset: ship test-only shim before 3b/3c"
  - Blocks encoding/contract v3
acceptanceDone:
  - false
  - false
  - false
---
# Phase 3a: verify abi::chain_id under ephemeral VM

Planner context…

## Evidence (worker)

**Date:** 2026-08-05 · **Result:** `abi::chain_id()` **works** under the repo's
`VM::ephemeral()` test harness.

| Check | Outcome |
|---|---|
| `VM::ephemeral()` + `genesis_session(0xCA)` sets `Metadata::CHAIN_ID` | Yes — `dusk-vm` 1.6.0 `vm/src/lib.rs:261-272` |
| Contract `abi::chain_id()` returns genesis value | Yes — `0xCA` |
| Shim required | **No** |
| Probe test | `knot-proposals/tests/contract.rs::abi_chain_id_available_under_ephemeral_vm` |
| Probe contract method | `proposals-test-target::chain_id()` → `abi::chain_id()` |

**Caveat:** bare `VM::ephemeral()` without `genesis_session(chain_id)` leaves
metadata unset and `abi::chain_id()` panics. All Knot contract tests already use
`genesis_session`; v3 contracts may call `abi::chain_id()` directly in tests.

**IMPLEMENTATION.md:** §2.5 + §2.14 item 1 amended (stamp `276b19e`).

## Proposal (worker, if BLOCKED)
