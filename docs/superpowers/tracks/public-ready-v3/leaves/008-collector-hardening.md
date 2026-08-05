---
id: 8
slug: collector-hardening
status: TODO
owner: null
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
  - false
  - false
  - false
  - false
  - false
---
# Phase 5: collector C1/M10–M12 + caps (+ R10)

Authority: `docs/internal/IMPLEMENTATION.md` §4.2, §11 R10.

Do not import Lab cookie session design here — collector remains untrusted relay + proxy auth.

## Evidence (worker)

## Proposal (worker, if BLOCKED)
