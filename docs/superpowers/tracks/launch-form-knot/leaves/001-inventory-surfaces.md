---
id: 1
slug: inventory-surfaces
status: DONE
owner: gap-map-worker
deps: []
scope:
  - crates/
  - README.md
  - docs/
acceptance:
  - Crate + host surface list in gap map or leaf evidence
acceptanceDone:
  - true
---
# Inventory crates and host surfaces

Planner context…

## Evidence (worker)

Full crate + host surface inventory landed in
`docs/launch-gap-map-2026-08.md` §Surface inventory: 5 workspace crates
(`multisig-encoding` 0.1.2, `multisig-registry` 0.1.5, `multisig-proposals`
0.3.2, `multisig-tool` 0.2.0, `multisig-collector` 0.2.0 AGPL) cross-checked
against root `README.md`'s Status table (found stale versions there — feeds
gap map row A12), plus 7 host surfaces (tool CLI, tool loopback RPC, tool
web UI, collector HTTP API, registry contract, proposals contract, and the
out-of-repo PM contract that `pm-resolve` depends on). Read root `README.md`,
`AGENTS.md`, `docs/security-model.md`, `docs/security-audit-2026-08-04.md`,
every `crates/*/README.md`, `crates/*/CHANGELOG.md`, and `crates/*/src/`
listings via `ls`/`wc -l`/`grep` to confirm structure before rows were
written.

## Proposal (worker, if BLOCKED)
