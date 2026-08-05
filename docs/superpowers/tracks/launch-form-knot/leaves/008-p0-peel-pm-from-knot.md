---
id: 8
slug: p0-peel-pm-from-knot
status: DONE
owner: worker
deps: []
scope:
  - crates/knot-tool/
  - crates/knot-collector/
  - crates/knot-encoding/
acceptance:
  - no_pm_resolve_surface tests pass
  - council_resolve_* removed from knot-encoding
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
- Removed `PmResolve` CLI, `/api/pm-resolve/*`, `/api/pm/markets`, `/api/deployments/pm`, standalone PM UI paths from `knot-tool`.
- Stripped `council_resolve_digest` / `DOMAIN_COUNCIL_RESOLVE_V2` from `knot-encoding`.
- Collector retains `pm_council_resolve` DTO kind; README notes wen wire compatibility + `pm-council-tool` product UX.
- Guard test: `crates/knot-tool/tests/no_pm_resolve_surface.rs`.
- Tests: `cargo test -p knot-encoding`, `cargo test -p knot-tool --test no_pm_resolve_surface`, `cargo test -p knot-tool`, `cargo test -p knot-collector` — all PASS.

## Proposal (worker, if BLOCKED)
