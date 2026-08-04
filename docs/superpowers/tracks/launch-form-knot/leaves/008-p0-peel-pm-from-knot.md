---
id: 8
slug: p0-peel-pm-from-knot
status: DONE
owner: worker
deps: []
scope:
  - crates/multisig-tool/
  - crates/multisig-collector/
  - crates/multisig-encoding/
acceptance:
  - no_pm_resolve_surface tests pass
  - council_resolve_* removed from multisig-encoding
  - Collector has no pm_council_resolve product kind (or documented relocate)
acceptanceDone:
  - true
  - true
  - true
---

# P0 — Peel PM surface from knot tool/collector/encoding

Plan Tasks 1–4. Prefer wen council-resolve-digest-l9 DONE before deleting digest.

## Evidence (worker)

- Deleted `pm_resolve_types.rs`, `pm_read_types.rs`, `static/pm-resolve.html`, `static/pm-resolve-app.js`.
- Removed `PmResolve` CLI, `/api/pm-resolve/*`, `/api/pm/markets`, `/api/deployments/pm`, standalone PM UI paths from `multisig-tool`.
- Stripped `council_resolve_digest` / `DOMAIN_COUNCIL_RESOLVE_V2` from `multisig-encoding`.
- Collector retains `pm_council_resolve` DTO kind; README notes wen wire compatibility + `pm-council-tool` product UX.
- Guard test: `crates/multisig-tool/tests/no_pm_resolve_surface.rs`.
- Tests: `cargo test -p multisig-encoding`, `cargo test -p multisig-tool --test no_pm_resolve_surface`, `cargo test -p multisig-tool`, `cargo test -p multisig-collector` — all PASS.

## Proposal (worker, if BLOCKED)
