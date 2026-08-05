---
id: 3
slug: rename-knot
status: DONE
owner: cursor-agent
deps:
  - 1
scope:
  - crates/
  - Cargo.toml
  - Cargo.lock
  - Makefile
  - docs/
  - README.md
acceptance:
  - All crates/dirs/bin/env/header/wasm paths renamed per IMPLEMENTATION §5.7
  - Tests pass after rename
  - Pin JSON keys still multisig-*
  - Zero behaviour change
acceptanceDone:
  - true
  - true
  - true
  - true
---
# Phase 2: mechanical multisig-* → knot-* rename

Planner context…

## Evidence (worker)

- `git mv` crates: `multisig-{encoding,registry,proposals,tool,collector}` → `knot-*`
- Package names, workspace members, `#[path]` includes, wasm `include_bytes!` → `knot_*.wasm`
- Env/header already `KNOT_*` / `X-Knot-Token`; keystore default `~/.knot/` with one-release fallback `.knot-tool` / `.multisig-tool`
- Pin keys unchanged: `chain.rs` `json_key` still `"multisig-registry"` / `"multisig-proposals"`
- Domains unchanged: `nocturne.knot.multisig.*`
- Tests: `knot-encoding` (call-types), `knot-collector`, `knot-tool` (lib + ints + blob), `make wasm/test` registry + proposals — all pass
- `check-crate-version-table`, `check-repo-rules` ok

## Proposal (worker, if BLOCKED)
