---
id: 4
slug: map-tests-goldens
status: DONE
owner: gap-map-worker
deps:
  - 2
  - 3
scope:
  - crates/
  - "**/tests/"
acceptance:
  - ship/fix rows note existing vs needed tests/goldens
acceptanceDone:
  - true
---
# Tests and goldens coverage map

Planner context…

## Evidence (worker)

`docs/launch-gap-map-2026-08.md` §Tests / goldens coverage map. Counted
`#[test]` occurrences per source file (`grep -c '#\[test\]'`) and
integration-test file line counts (`wc -l`) across all 5 crates:
`knot-encoding` (12 + 5 unit, `layout_goldens.rs` 155 lines),
`knot-registry` (14 integration tests under `VM::ephemeral()`,
`layout_goldens.rs` 230 lines), `knot-proposals` (16 integration
tests incl. CEI/reentrancy, `layout_goldens.rs` 183 lines),
`knot-collector` (11 unit in `store.rs`, 3 `tokio::test` HTTP
integration), `knot-tool` (`blob.rs` 10, `mock_ledger.rs` 7,
`keystore.rs` 1, `chain.rs` 3, plus 2+1 integration tests). Headline
finding: `knot-tool/src/rpc.rs` (2011 lines) — the file containing
every audited Critical/High/Medium handler (A1/A2/A4/A5 in the gap map) —
has **zero** tests of any kind, direct or integration; existing
integration tests (`collector_roundtrip.rs`, `blob_aggregate_local.rs`)
exercise `blob.rs`/`collector_client.rs` only, never the axum handlers.
Same zero-coverage gap confirmed for `pm_resolve_types.rs`/
`pm_read_types.rs` (no golden vector against `prediction-market`'s copy,
feeds row A3) and `main.rs` CLI layer (feeds row A11). Fed directly into
gap map rows A10 (fix, Launch=N) and A3/A11 (extract, Launch=discuss).

## Proposal (worker, if BLOCKED)
