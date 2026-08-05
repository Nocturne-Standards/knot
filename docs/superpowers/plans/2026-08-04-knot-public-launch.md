# Knot Public Launch Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `aichbindas/knot` public-ready: strip PM product surface, rename generic domains to `nocturne.knot.*`, clean docs/site/Lab copy, residual generic Lab quality — matching [`docs/launch-form-knot.md`](../launch-form-knot.md).

**Architecture:** Peel PM tooling and `council_resolve_*` out of knot (wen owns receive + A1–A5 fixes per paired plan). Knot keeps encoding (proposal + change_account only), registry, proposals, generic Lab/CLI, collector. Public claim is Prove-only. Docs align across repo, nocturne-docs `/v1/knot/`, and Lab UI.

**Tech Stack:** Rust 2024 / rust-version 1.94, Cargo workspace, Piecrust contracts (`make wasm` / `make test`), Axum Lab (`knot-tool`), Deno/md nocturne-docs, git worktree on `feat/launch-form-knot` (or successor).

**Paired plan:** [`/Users/leonidas/dev/aichbindas/wen/docs/superpowers/plans/2026-08-04-wen-pm-peel-and-fixes.md`](../../../../wen/docs/superpowers/plans/2026-08-04-wen-pm-peel-and-fixes.md) (absolute on disk). Run P0 strip in knot only after wen branch is ready to receive digests, or land digests in wen first then strip knot in the same calendar wave.

## Global Constraints

- Launch form L1–L9 locked; do not re-litigate Prove-only or domain strings.
- Knot must **not** depend on wen after peel.
- Domain proposal: `nocturne.knot.multisig.proposal.v2`
- Domain change_account: `nocturne.knot.multisig-registry.change_account.v2`
- Council domain lives in wen: `nocturne.wen.prediction-market.council-resolve.v3`
- No Wave/carve/§ Status novels on public README front doors.
- Policy A: commit/push feat branches; no main push without ask.
- Prefer Cursor-owned worktrees for new isolation; this work may continue on existing `knot/.worktrees/audit-2026-08-full` @ `feat/launch-form-knot`.
- Heavy cargo: `claim.py acquire` before long tests.

---

### Task 1: Inventory strip targets and add failing “no PM surface” guard

**Files:**
- Create: `crates/knot-tool/tests/no_pm_resolve_surface.rs` (will fail until Task 3)
- Modify: none yet
- Reference: `crates/knot-tool/src/main.rs`, `src/rpc.rs`, `src/pm_*.rs`, `static/pm-*`

**Interfaces:**
- Consumes: current tree with PM-resolve present
- Produces: CI-style regression test that forbids PM-resolve product surface after peel

- [ ] **Step 1: Write the failing test**

```rust
//! After public-launch peel, knot must not expose PM-resolve as a product surface.
#[test]
fn tool_crate_has_no_pm_resolve_modules() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    for rel in [
        "src/pm_resolve_types.rs",
        "src/pm_read_types.rs",
        "static/pm-resolve.html",
        "static/pm-resolve-app.js",
    ] {
        let p = std::path::Path::new(manifest_dir).join(rel);
        assert!(
            !p.exists(),
            "PM-resolve peel incomplete: {} still exists",
            p.display()
        );
    }
}

#[test]
fn main_help_text_has_no_pm_resolve_subcommand_docs() {
    // Grep source: PmResolve / pm-resolve must not appear in CLI enum after peel.
    let main = include_str!("../src/main.rs");
    assert!(
        !main.contains("PmResolve") && !main.contains("pm-resolve"),
        "main.rs still references PmResolve / pm-resolve"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p knot-tool --test no_pm_resolve_surface -- --nocapture`  
Expected: FAIL (files still exist / strings still present)

- [ ] **Step 3: Commit the failing guard**

```bash
git add crates/knot-tool/tests/no_pm_resolve_surface.rs
git commit -m "test: fail until PM-resolve surface peeled from knot tool"
git push origin HEAD
```

---

### Task 2: Coordinate with wen — digest home exists before delete

**Files:**
- Read: wen plan Task 1–2 (digest module + L9 domain)
- Modify: none in knot until wen PR/branch has `council_resolve_digest` compiling

**Interfaces:**
- Consumes: wen exports equivalent of today’s `knot_encoding::council_resolve_digest`
- Produces: go/no-go for Task 3

- [ ] **Step 1: Confirm wen branch exposes digest**

Run (from wen checkout):  
`rg -n 'nocturne.wen.prediction-market.council-resolve.v3|council_resolve_digest' crates/`  
Expected: domain string + function present.

- [ ] **Step 2: Note wen branch name / commit in knot DECISIONS**

```bash
cd /Users/leonidas/dev/aichbindas/nocturne-mcp-tracks && node --input-type=module <<'EOF'
import { decisionAdd } from './tracks/track-init.js';
import { trackDir } from './tracks/root.js';
decisionAdd(trackDir('/Users/leonidas/dev/aichbindas/knot/.worktrees/audit-2026-08-full','launch-form-knot'), {
  title: 'Wen digest ready for peel',
  body: 'Wen branch/commit: <fill>. council_resolve_digest lives in wen; knot may delete encoding helpers.',
});
EOF
git add docs/superpowers/tracks/launch-form-knot/DECISIONS.md
git commit -m "(track) launch-form-knot: wen digest ready for peel"
git push origin HEAD
```

- [ ] **Step 3: Stop if wen not ready** — do not delete knot digest until Step 1 passes.

---

### Task 3: Strip PM-resolve from `knot-tool` and collector PM kind

**Files:**
- Delete: `crates/knot-tool/src/pm_resolve_types.rs`, `pm_read_types.rs`, `static/pm-resolve.html`, `static/pm-resolve-app.js`
- Modify: `crates/knot-tool/src/main.rs` (remove `PmResolve` CLI), `src/rpc.rs` (remove `/api/pm-resolve/*`, `/api/pm/markets`, standalone_pm_resolve, pm static routes), `src/lib.rs` / `mod` declarations, `src/blob.rs` (PM blob helpers if only used by PM), `README.md`
- Modify: `crates/knot-collector/src/dto.rs`, `store.rs`, `api.rs` — remove or gate `pm_council_resolve` kind (prefer remove from knot)
- Modify: `crates/knot-tool/tests/collector_roundtrip.rs` — drop PM roundtrip test or move assertion to wen
- Test: `crates/knot-tool/tests/no_pm_resolve_surface.rs` (Task 1)

**Interfaces:**
- Consumes: Task 2 go-ahead
- Produces: knot tool = generic Lab only

- [ ] **Step 1: Remove modules and CLI/RPC routes**

Delete PM files listed above. In `main.rs`, remove the `PmResolve` subcommand enum variant and match arms. In `rpc.rs`, remove routes registered under `/api/pm-resolve` and `/api/pm/markets`, `standalone_pm_resolve` option, and handlers `api_pm_resolve_*` / `api_pm_markets`. Remove `mod pm_resolve_types` / `mod pm_read_types`.

- [ ] **Step 2: Collector — remove PM DTO kind**

In `dto.rs`, remove `pm_council_resolve` variant and related structs. Fix `api.rs` / `store.rs` tests that create PM samples. Keep generic proposal blob API.

- [ ] **Step 3: Run tests**

```bash
cargo test -p knot-tool
cargo test -p knot-collector
cargo test -p knot-tool --test no_pm_resolve_surface
```

Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add -A crates/knot-tool crates/knot-collector
git commit -m "refactor: peel PM-resolve surface out of knot tool and collector"
git push origin HEAD
```

---

### Task 4: Remove `council_resolve_*` from `knot-encoding`

**Files:**
- Modify: `crates/knot-encoding/src/lib.rs` — delete `DOMAIN_COUNCIL_RESOLVE_V2`, `council_resolve_digest`, `council_resolve_message`, and their unit tests
- Modify: `crates/knot-encoding/README.md` — remove council-resolve API section
- Modify: any `layout_goldens` / exports that mention council resolve
- Grep whole repo: `council_resolve|DOMAIN_COUNCIL|prediction-market.council`

**Interfaces:**
- Consumes: wen owns digest (Task 2)
- Produces: encoding = proposal + change_account (+ fingerprint/call-types) only

- [ ] **Step 1: Grep and delete**

```bash
rg -n 'council_resolve|DOMAIN_COUNCIL_RESOLVE|council-resolve' crates/
```

Remove all knot hits. Leave no `pub use` of council helpers.

- [ ] **Step 2: Test**

```bash
cargo test -p knot-encoding
```

Expected: PASS; no council tests remain.

- [ ] **Step 3: Commit**

```bash
git add crates/knot-encoding
git commit -m "refactor: move council_resolve digest ownership out of encoding"
git push origin HEAD
```

---

### Task 5: Rename generic domains to `nocturne.knot.*`

**Files:**
- Modify: `crates/knot-encoding/src/lib.rs` constants + goldens/tests that pin digest hex
- Modify: any fixtures under `crates/multisig-*/tests/` that hardcode old domain preimages
- Modify: `crates/knot-encoding/README.md`

**Interfaces:**
- Produces:

```rust
pub const DOMAIN_PROPOSAL_V2: &[u8] = b"nocturne.knot.multisig.proposal.v2";
pub const DOMAIN_CHANGE_ACCOUNT_V2: &[u8] =
    b"nocturne.knot.multisig-registry.change_account.v2";
```

(Rename symbols from `_V1` → `_V2` or keep names and change bytes — prefer renaming symbols to match version.)

- [ ] **Step 1: Update constants and recompute goldens**

Change domain bytes. Run encoding tests; update expected digests in unit tests / `layout_goldens.rs` to new keccak outputs (run once, capture, pin).

- [ ] **Step 2: Contract tests**

```bash
(cd crates/knot-registry && make test)
(cd crates/knot-proposals && make test)
```

Expected: PASS against new digests (host-side encoding only for change_account/proposal; on-chain redeploy comes in Task 6).

- [ ] **Step 3: Commit**

```bash
git add crates/knot-encoding crates/knot-registry crates/knot-proposals
git commit -m "feat: nocturne.knot domain tags for proposal and change_account"
git push origin HEAD
```

---

### Task 6: Redeploy notes + Status pin (operator)

**Files:**
- Create or update: `docs/internal/redeploy-2026-08-domains.md` (internal — not front door)
- Modify: only internal/Status notes, not public README novel

- [ ] **Step 1: Write redeploy checklist**

Document: rebuild wasm, deploy registry + proposals, update `deployments/testnet.json` (or successor), invalidate old committee messages signed under `sme-platform.*`.

- [ ] **Step 2: Operator executes redeploy** (human) — agent records new contract IDs in `docs/internal/`.

- [ ] **Step 3: Commit internal pin**

```bash
git add docs/internal/redeploy-2026-08-domains.md
git commit -m "docs: internal redeploy checklist for nocturne.knot domains"
git push origin HEAD
```

---

### Task 7: Public README + versioning.md + A12–A15

**Files:**
- Modify: `README.md`, each `crates/*/README.md`, `crates/*/CHANGELOG.md` as needed
- Create: `docs/versioning.md`
- Modify: `crates/knot-tool/src/chain.rs` error strings (drop dead `references/testnet-wallet.md`)

- [ ] **Step 1: Add `docs/versioning.md`**

Short SSOT: testnet-only, `PINNED-DIFFERENT-REDEPLOYED` meaning, per-crate semver vs git tag.

- [ ] **Step 2: Rewrite root README front door**

Use launch-form public claim. Prove-first. AGPL collector callout. Fix version table to match Cargo.toml. Fix GitHub links to `aichbindas/knot`. Remove Wave/carve Status archaeology (move survivors to `docs/internal/` per hygiene inventory).

- [ ] **Step 3: Fix dead `../../../` links** (A14) per `docs/doc-hygiene-inventory.md`

- [ ] **Step 4: Fix CLI error text** (A15) — self-contained `RUSK_WALLET_PWD` message, no missing path.

- [ ] **Step 5: Commit**

```bash
git add README.md docs/versioning.md docs/ crates/
git commit -m "docs: public front door, versioning.md, kill dead monorepo links"
git push origin HEAD
```

---

### Task 8: Lab UI copy — drop “treasury”

**Files:**
- Modify: `crates/knot-tool/static/index.html` and related JS copy strings
- Modify: tool README Lab sections

- [ ] **Step 1: Replace “treasury” / “Form the treasury”** with “committee” / “multisig account” language.

- [ ] **Step 2: Manual eyeball** Lab in `DEMO_MODE=mock` if practical.

- [ ] **Step 3: Commit**

```bash
git add crates/knot-tool/static crates/knot-tool/README.md
git commit -m "docs: Lab copy — committee not treasury"
git push origin HEAD
```

---

### Task 9: nocturne-docs `/v1/knot/` alignment

**Files (nocturne-docs repo):**
- Modify: `docs/v1/knot/index.md` and any sibling knot pages
- Modify: any wen/PM pages that say pm-resolve lives in knot

**Worktree:** separate Cursor worktree / checkout of `nocturne-docs` on a feat branch.

- [ ] **Step 1: Rewrite knot index** to match public claim; Prove-only; no PM-resolve-in-knot; link wen for council resolve.

- [ ] **Step 2: Grep site**

```bash
rg -n 'pm-resolve|sme_platform|Wave 7|agent-pay|carve' docs/v1/knot docs/v1 -g '*.md'
```

Fix stale hits.

- [ ] **Step 3: PR nocturne-docs**; link PR URL in knot `DECISIONS.md`.

---

### Task 10: Residual generic Lab tests (A10 remainder) + optional A4

**Files:**
- Create: `crates/knot-tool/tests/rpc_generic_smoke.rs` (or unit tests with axum `oneshot`)
- Modify: optional shared helper extract for approve/sign if touching A4

- [ ] **Step 1: Add failing test** — e.g. mock-mode health/status or proposal preview handler returns 200 with bearer token.

- [ ] **Step 2: Implement minimal coverage** for remaining generic routes (not PM).

- [ ] **Step 3: Optional** — live membership **warn** on generic approve (A4); do not block launch if skipped — record in DECISIONS.

- [ ] **Step 4: Commit**

```bash
git add crates/knot-tool
git commit -m "test: generic Lab RPC smoke after PM peel"
git push origin HEAD
```

---

### Task 11: security-model + A8/A9 doc polish

**Files:**
- Modify: `docs/security-model.md` — Prove dual-posture already present; ensure front-door consistent; add `require_owner` / `public_sender` footnote (A9); member-pk ordering note (A8)

- [ ] **Step 1: Edit security-model**
- [ ] **Step 2: Commit**

```bash
git add docs/security-model.md
git commit -m "docs: security-model polish for public Prove claim"
git push origin HEAD
```

---

### Task 12: Verify bar + mark launch-form complete

**Files:**
- Modify: `docs/launch-form-knot.md` — check success criteria
- Modify: track `launch-form-knot` STATUS via leaf_done if leaves exist for plan execution

- [ ] **Step 1: Run verify checklist**

```bash
cargo test -p knot-encoding -p knot-tool -p knot-collector
(cd crates/knot-registry && make test)
(cd crates/knot-proposals && make test)
rg -n 'pm-resolve|PmResolve|council_resolve_digest|sme-platform\.multisig' crates/ README.md || true
```

Expected: tests PASS; rg shows no PM-resolve product surface / no old generic domains in encoding.

- [ ] **Step 2: Confirm wen paired plan** closed or tracking A1–A5 (link in DECISIONS).

- [ ] **Step 3: Commit “launch form ready for tag” note** (tag itself is operator).

```bash
git commit --allow-empty -m "chore: knot public-launch verify checklist green"
git push origin HEAD
```

---

## Spec coverage self-check

| Launch-form item | Task |
|---|---|
| P0 peel tooling | 1, 3 |
| P0 peel digest | 2, 4 |
| P0 collector PM kind | 3 |
| P1 domains + redeploy | 5, 6 |
| P2 README/versioning/A12–15 | 7 |
| P2 Lab copy | 8 |
| P2 nocturne-docs + website | 8–9 |
| P3 residual tests / A4 | 10 |
| P3 A8/A9 | 11 |
| P4 verify | 12 |
| A1–A5 fixes | **Wen plan** (not closed here) |
