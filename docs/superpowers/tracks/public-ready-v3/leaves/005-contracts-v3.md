---
id: 5
slug: contracts-v3
status: DONE
owner: cursor-agent
deps:
  - 4
scope:
  - crates/knot-registry/
  - crates/knot-proposals/
  - crates/knot-registry/
  - crates/knot-proposals/
acceptance:
  - State/methods per §2.10–2.11
  - Rich events §2.13
  - Tests §2.15
  - Redeploy registry then proposals; burn v2
acceptanceDone:
  - true
  - true
  - true
  - true
---
# Phase 3c: registry + proposals contracts v3

Planner context…

## Evidence (worker)

**Date:** 2026-08-05 · **Branch:** `feat/public-ready-v3-rename`

### Implementation
- `knot-proposals/src/state.rs` — full v3 per §2.10–2.11
- `knot-registry/src/state.rs` — `change_account_message_v3`
- `knot-encoding/call_types.rs` — `ProposeArgs.nonce`, `ProposalView.epoch`

### Tests
| Suite | Result |
|-------|--------|
| `knot-registry` cargo test --release | 22 passed |
| `knot-proposals` make test | 27 passed |

Critical §2.15: H1 digest binding, M3 parallel finalize, consumed replay block,
L2 `deadline==height`, M1 ttl ceiling, epoch invalidation, prune+consumed retain,
finalize self panic, merge identical open digest.

### Redeploy
Registry v3 first, then proposals v3 + `init_registry`. Burn v2 signatures.

### Deferred
`knot-tool` compile break → leaf #6. Event-decoder arms → leaf #10.

## Proposal (worker, if BLOCKED)
