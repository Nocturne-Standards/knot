# Wave B — chain-id / encoding / contracts v3

> **For agentic workers:** Use superpowers:subagent-driven-development.

**Goal:** Hard-gate `abi::chain_id` under ephemeral VM, then land encoding digests v3 and registry+proposals contracts v3.

**Architecture:** Leaf #2 is investigative+shim only. #4 updates `knot-encoding` digests. #5 changes on-chain contracts (redeploy, burn v2). Serial deps: 2→4→5.

**Tech Stack:** Rust, piecrust/dusk-vm, knot-encoding, knot-registry, knot-proposals.

## Global Constraints

- Authority: `docs/internal/IMPLEMENTATION.md` §2 (encoding/contracts) + §10 phases 3a–3c
- Worktree: `.worktrees/public-ready-v3-rename`, branch `feat/public-ready-v3-rename`
- Claim packages before heavy cargo (encoding/registry/proposals)
- No Lab rework (Wave A done)
- Pin JSON keys stay `multisig-*` until paired pin update
- Domains: v3 tags per §2.12 — do not invent alternate separators

---

### Task B1: Confirm abi::chain_id under VM::ephemeral (#2)

**Files:** proposals contract tests, possibly `bls.rs` / docs; IMPLEMENTATION note.

**Acceptance (leaf #2):**
- Document whether `abi::chain_id` works under `VM::ephemeral`
- If unset: ship test-only shim before encoding/contracts v3
- Blocks #4/#5

**Steps:** Write probe test; record result in leaf evidence + IMPLEMENTATION; shim if needed; commit.

---

### Task B2: Encoding digests v3 (#4)

Depends on B1.

**Acceptance:** DOMAIN_*_V3 preimages with self_id/chain_id/epoch/member_count per §2.12; goldens + H1/H2 tests.

---

### Task B3: Contracts v3 (#5)

Depends on B2.

**Acceptance:** State/methods §2.10–2.11; rich events §2.13; tests §2.15; redeploy notes; burn v2.
