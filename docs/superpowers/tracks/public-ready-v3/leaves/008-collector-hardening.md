---
id: 8
slug: collector-hardening
status: DONE
owner: cursor-agent
deps:
  - 4
scope:
  - crates/knot-collector/
  - crates/knot-collector/
acceptance:
  - L9–L12 + M11 first
  - C1 digest recompute via knot-encoding
  - M10+M12 signature verification with dusk-core
  - Rewrite no-dusk_core docs to never-holds-keys / never-signs / never-submits
  - R10 cap party name like MAX_NOTE_CHARS
acceptanceDone:
  - true
  - true
  - true
  - true
  - true
---
# Phase 5: collector C1/M10–M12 + caps (+ R10)

Authority: `docs/internal/IMPLEMENTATION.md` §4.2, §11 R10.

Do not import Lab cookie session design here — collector remains untrusted relay + proxy auth.

## Evidence (worker)

- L9–L12 + M11: pagination/caps/TTL, loopback parse, graceful shutdown, PRAGMA FULL+fullfsync, generic 500s
- C1: `gate.rs` + `knot-encoding` digest recompute; Conflict warn log
- M10/M12: `verify.rs` BLS partial + party signup sig
- Docs rewritten (never-holds-keys / never-signs / never-submits)
- R10: party name cap
- Tests: `cargo test -p knot-collector` 49 passed
- Report: `.superpowers/sdd/task-8-report.md`

## Proposal (worker, if BLOCKED)
