---
id: 12
slug: p4-verify-public-ready
status: DONE
owner: cursor-grok
deps: []
scope:
  - docs/launch-form-knot.md
acceptance:
  - Launch-form success criteria checklist complete
  - Wen A1/A2/A3/A5 successors tracked (link in DECISIONS); knot tag not blocked on wen DONE
acceptanceDone:
  - true
  - true
---
# P4 — Verify checklist for public-ready tag

Plan Task 12.

## Evidence (worker)

- `cargo test -p multisig-encoding` 16 PASS; `multisig-collector` PASS
- `multisig-tool` lib + `no_pm_resolve_surface` + `rpc_generic_smoke` + `collector_roundtrip` PASS
- `multisig-registry` / `multisig-proposals` `make test` PASS
- `rg`: no `PmResolve` / `council_resolve_digest` / old `sme-platform.multisig.proposal` in live crates (guard test strings only)
- Wen A1–A5: DECISIONS link to `pm-peel-and-fixes` / `7ae9728`
- Operator still: redeploy registry+proposals per `docs/internal/redeploy-2026-08-domains.md`; tag when ready

## Proposal (worker, if BLOCKED)
