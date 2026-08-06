# Field guide rules

**Vendored artifact.** Cap: 30 rules per tier. Findings live only in the kit
(`FIELD_GUIDE/findings/`). Cite findings by ID, never by private `path:line`.

**Version:** see sibling `.rules-version` / `.rules-manifest` in adopting repos.

**Schema (each rule):**

```yaml
id: R-NNN
tier: T1|T2|T3
title: …
rule: >
  …
evidence: [F-…]          # ≥2 distinct repos, or estate convention noted
enforcement: gate:… | review:… | unenforced
verified-against: …      # T1 only, when applicable
links:                   # supersedes | caused-by | instance-of only
  - …
```

---

## T1 — Platform facts

### R-001
```yaml
id: R-001
tier: T1
title: Every pub fn on a contract state struct is a public entry point
rule: >
  #[dusk_forge::contract] emits an extern "C" wrapper for every public method.
  Every exported method declares who may call it in .contract-authz-baseline.
  PUBLIC is a valid answer; silence is not. When you fix one instance, sweep
  the class in the same change.
evidence: [F-002, F-012]
enforcement: gate:check-contract-authz.sh
verified-against: dusk-forge-contract 0.2.0
links:
  - caused-by: R-019
```

### R-006
```yaml
id: R-006
tier: T1
title: Do not use legacy *_insecure BLS paths for new signatures
rule: >
  Grep for *_insecure, *_unchecked, from_raw_*, _unsafe. Each hit carries a
  justifying comment or is replaced. After every dependency bump that adds a
  secure variant beside a legacy one, re-grep — existing code silently keeps
  the legacy path.
evidence: [F-001]
enforcement: review:contract-checklist-crypto
verified-against: bls12_381-bls (insecure-v1-signing feature)
links:
  - instance-of: C6
```

Note: F-001 spans chit/knot/wen (three repos) — one finding, three sightings.

---

## T2 — Estate conventions

### R-010
```yaml
id: R-010
tier: T2
title: Host bind, CORS, and auth are three separate decisions
rule: >
  Default bind is loopback. Non-loopback requires an explicit env var and
  refuses to start without auth configured. CORS is an allow-list —
  CorsLayer::permissive() never ships. Demo/operator routes feature-gated off
  by default. citadel-prover is the in-house reference shape.
evidence: [F-008]
enforcement: review:host-checklist
```

### R-011
```yaml
id: R-011
tier: T2
title: Opaque client errors; bounded requests
rule: >
  Map errors to opaque client messages; log detail server-side. Body limit,
  request timeout, and concurrency cap on every public router.
evidence: [F-009]
enforcement: review:host-checklist
```

### R-014
```yaml
id: R-014
tier: T2
title: #[path] includer set updates same commit
rule: >
  #[path] is a textual include. Every new use inside a path-included file must
  land in every includer's Cargo.toml in the same commit. Re-derive the
  includer set before merging.
evidence: [F-013]
enforcement: review:shared-code
```

### R-015
```yaml
id: R-015
tier: T2
title: Path-dep change refreshes every resolving Cargo.lock
rule: >
  Add/remove path dependency → regenerate every committed Cargo.lock that
  resolves it, same commit. Vestigial member-crate locks: delete.
evidence: [F-014]
enforcement: review:shared-code
```

### R-016
```yaml
id: R-016
tier: T2
title: main-guard escape needs env + commit trailer
rule: >
  Escape hatch is DEV_WORKFLOW_ALLOW_MAIN plus allow-main: in the commit
  message. One convention — do not invent a second. Hook reads message via
  commit-msg $1 (then GIT_COMMIT_MSG / COMMIT_EDITMSG fallbacks).
evidence: [F-015]
enforcement: gate:check-main-guard.sh
```

### R-017
```yaml
id: R-017
tier: T2
title: Hand-copied mirrors need a repo-rules baseline
rule: >
  New hand-mirrored shared files are declared in the repo-rules baseline or
  rejected. Silence is not an answer.
evidence: []
enforcement: gate:check-repo-rules.sh
```

### R-018
```yaml
id: R-018
tier: T2
title: Package claims must be acquired before heavy work
rule: >
  One worktree owns a package claim. Acquire before hang-prone cargo; release
  after. Duplicate suites in parallel are forbidden.
evidence: []
enforcement: gate:check-claims.sh
```

### R-020
```yaml
id: R-020
tier: T2
title: Crate versions stay consistent across the workspace table
rule: >
  Version table / workspace version drift is a release blocker. Gate the table.
evidence: []
enforcement: gate:check-crate-version-table.sh
```

### R-024
```yaml
id: R-024
tier: T2
title: Push Policy A — feat free, main gated
rule: >
  Agents may commit and push non-main branches without asking. Ask (or set
  DEV_WORKFLOW_ALLOW_MAIN_PUSH=1) before updating main/master. Prefer PR merge
  onto main. Enforced by hooks/pre-push (not a bin/check-*.sh gate).
evidence: [F-016]
enforcement: review:push-policy
```

### R-025
```yaml
id: R-025
tier: T2
title: Ask before commit; session grant is explicit
rule: >
  Default: ask before every commit. git status first; reconcile staged files
  with this session's work. A prior yes does not carry to unrelated later work.
  Session-wide commit permission must be explicit and scoped; when the task
  ends, revert to ask-first.
evidence: [F-017]
enforcement: review:workflow
```

### R-027
```yaml
id: R-027
tier: T2
title: Testnet wallet lives under rusk-wallet/
rule: >
  Canonical path is ~/.dusk/rusk-wallet/wallet.dat — not ~/.dusk/wallet.dat.
  Before claiming deploy is blocked, ls that path and use testnet endpoints.
  Password often from .env.testnet via load_repo_env, not the interactive shell.
evidence: [F-019]
enforcement: review:deploy
```

---

## T3 — Recurrence classes (review checklist)

### R-002
```yaml
id: R-002
tier: T3
title: Digests bind domain, chain, instance, and all varying fields
rule: >
  Any signed or hashed authorization binds at minimum: domain tag, chain id,
  contract instance id, and every field the caller can vary. Fix before first
  issuance — later is a breaking change to material in the wild.
evidence: [F-003]
enforcement: review:contract-checklist
links:
  - instance-of: C2
```

### R-003
```yaml
id: R-003
tier: T3
title: Repeatable bearer auth carries a nonce or sequence
rule: >
  Any signature that authorizes a repeatable action carries a nonce or
  sequence, checked against contract state, strictly increasing. State the
  replay policy in the type's doc comment.
evidence: [F-004]
enforcement: review:contract-checklist
links:
  - instance-of: C3
```

### R-004
```yaml
id: R-004
tier: T3
title: Sign the whole field, never a prefix
rule: >
  Sign the whole field or a hash of the whole field. Never a prefix or
  truncation. One canonical encoding per type, enforced at the decoder.
evidence: [F-005]
enforcement: review:contract-checklist
links:
  - instance-of: C4
```

### R-005
```yaml
id: R-005
tier: T3
title: Permissionless insert requires a cap, epoch, or prune
rule: >
  Unbounded Vec + linear scan raises gas for everyone. Index by key. Cap,
  epoch, or prune permissionless inserts.
evidence: [F-006]
enforcement: review:contract-checklist
links:
  - instance-of: C5
```

### R-007
```yaml
id: R-007
tier: T3
title: Trust-boundary claims stay true when keys appear
rule: >
  Trust-boundary claims live in security-model.md and are re-read whenever a
  component gains a key, wallet, or network listener. Doc true-when-written
  that becomes false is a finding.
evidence: [F-007]
enforcement: review:security-model
links:
  - instance-of: C7
```

Single-repo evidence — kept as loud checklist item; promote bar soft for doc
drift until second repo files.

### R-012
```yaml
id: R-012
tier: T3
title: Audits open on a fresh remote HEAD
rule: >
  Any audit, design, or release-readiness doc opens with git fetch --prune and
  hard-fails unless HEAD == origin/HEAD. Stamp the SHA in the first ten lines.
evidence: [F-010]
enforcement: gate:check-audit-sha.sh
links:
  - instance-of: P1
```

Also run `check-fresh-remote.sh` on release workflows (release-only, not per-PR).

### R-013
```yaml
id: R-013
tier: T3
title: Inventories of secrets are secrets
rule: >
  Findings cite <redacted — see secret manager> and a location, never the
  value. Rotation is the fix; deletion is cleanup. Record rotation date, not
  the credential.
evidence: [F-011]
enforcement: gate:check-public-surface.sh
links:
  - instance-of: P2
```

### R-019
```yaml
id: R-019
tier: T3
title: Point-fix-not-swept
rule: >
  When a failure class is fixed in one site, sweep every sibling export /
  endpoint / digest in the same change. A same-day reintroduction of the same
  class means prose failed — write or tighten the gate.
evidence: [F-012, F-002]
enforcement: review:contract-checklist
links:
  - caused-by: R-001
```

### R-021
```yaml
id: R-021
tier: T3
title: Secret scans must not fail open
rule: >
  gitleaks runs locally when installed and always in CI. A failing scan is a
  hard fail. Missing binary exits 2 (warn); a run that finds secrets must
  never be swallowed with || true.
evidence: [F-011]
enforcement: gate:check-gitleaks.sh
```

### R-022
```yaml
id: R-022
tier: T3
title: Release workflows verify fresh remote
rule: >
  check-fresh-remote.sh gates release readiness, not every PR. Audits and
  release docs must not ship against a stale origin/HEAD.
evidence: [F-010]
enforcement: gate:check-fresh-remote.sh
```

### R-023
```yaml
id: R-023
tier: T3
title: Absence of a required gate is a failure
rule: >
  A repo with core.hooksPath set but zero scripts/check-*.sh must not pass
  silently. check-gate-coverage.sh makes forgetting loud. Waivers need a
  reason string (Track C will require a finding ID).
evidence: [F-011]
enforcement: gate:check-gate-coverage.sh
```

### R-026
```yaml
id: R-026
tier: T3
title: Do not leave origin.fetch on a tag-only refspec
rule: >
  remote.origin.fetch must update heads (typically
  +refs/heads/*:refs/remotes/origin/*). After tag/release dances, check
  git config --get remote.origin.fetch and compare origin/main to
  git ls-remote. Prefer git fetch origin tag <name> without rewriting fetch.
evidence: [F-018]
enforcement: review:workflow
```

---

## Contract review checklist (T3 rollup)

For every exported method, in order:

1. **Who may call this?** Written in `.contract-authz-baseline` (R-001).
2. **What does the caller control?** Validated or inside the signed digest (R-002).
3. **If it verifies a signature** — is the key bound to something authoritative,
   or supplied alongside the signature?
4. **If it is replayable** — where is the nonce? (R-003)
5. **Does the digest bind** domain, chain, contract instance, and all varying
   fields? (R-002)
6. **If it inserts** — what bounds growth? (R-005)
7. **Arithmetic** — checked, saturating, or proven unreachable, and stated which.
8. **Is there a negative test?** A fix without a rejected-case test is not done.
9. **Crypto path** — any `*_insecure` justified? (R-006)

---

## Enforcement index

| Rule | enforcement |
|------|-------------|
| R-001 | gate:check-contract-authz.sh |
| R-002 | review:contract-checklist |
| R-003 | review:contract-checklist |
| R-004 | review:contract-checklist |
| R-005 | review:contract-checklist |
| R-006 | review:contract-checklist-crypto |
| R-007 | review:security-model |
| R-010 | review:host-checklist |
| R-011 | review:host-checklist |
| R-012 | gate:check-audit-sha.sh |
| R-013 | gate:check-public-surface.sh |
| R-014 | review:shared-code |
| R-015 | review:shared-code |
| R-016 | gate:check-main-guard.sh |
| R-017 | gate:check-repo-rules.sh |
| R-018 | gate:check-claims.sh |
| R-019 | review:contract-checklist |
| R-020 | gate:check-crate-version-table.sh |
| R-021 | gate:check-gitleaks.sh |
| R-022 | gate:check-fresh-remote.sh |
| R-023 | gate:check-gate-coverage.sh |
| R-024 | review:push-policy |
| R-025 | review:workflow |
| R-026 | review:workflow |
| R-027 | review:deploy |
