# Tool uniquifier + blob hardening (#6)

> **For agentic workers:** Use superpowers:subagent-driven-development.

**Goal:** Land leaf #6 — CSPRNG proposal uniquifier, blob M8/M9/L7/L8/L14, R5/R11.

**Architecture:** `knot-tool` only. Contracts already v3 (`ProposeArgs.nonce`). Minimal compile fix `4a60ecc` landed; this leaf finishes real uniquifier + blob/collector client hardening. Lab cookie session is #14 (done) — do not reopen.

**Tech Stack:** Rust knot-tool, knot-encoding v3 digests, existing chain query helpers.

## Global Constraints

- Authority: IMPLEMENTATION.md §2.6, §4.1, §11 R5/R11; leaf `006-tool-uniquifier-blobs.md`
- Worktree: `.worktrees/public-ready-v3-rename`, branch `feat/public-ready-v3-rename`
- claim.py acquire `knot-tool` before cargo test
- Change-account nonce latch from #14 stays (refuse `--nonce` unless latch) — proposal uniquifier `--nonce` is separate
- Commit + push feat/*

---

### Task 1: Proposal uniquifier (CSPRNG + `--nonce`)

- Default: CSPRNG u64 uniquifier into `ProposeArgs.nonce` / blob intent
- CLI `--nonce` for proposals only (not change-account without latch)
- Digests use encoding v3 helpers
- Tests for default random + explicit `--nonce`

### Task 2: Blob M8/M9/L7/L8/L14

- M8: fetch registry threshold before aggregate; offline = honest label (no fake REFUSING)
- M9: verify each partial locally; drop+name invalids
- L7: `bls::aggregate` → `Result`
- L8: atomic blob write (tmp+rename pattern per §3.2 as available)
- L14: typed errors from recompute/gate (encoding + blob callers)

### Task 3: R5 + R11

- R5: collector URL allowlist — loopback or `https://` only before Basic Auth
- R11: validate proposal id as 64-hex before path use (client)

### Task 4: Leaf close-out

- Evidence on leaf #6; leaf_done; track commit
