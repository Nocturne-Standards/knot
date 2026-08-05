# Audit lessons → tooling design

**Written against `7e58d4c`.** Spec for process gates from `IMPLEMENTATION.md` §6
and `PUBLIC-REPO-STANDARD.md` §5/§7b. Product fixes (H1/H2, keystore, …) out of
scope.

**Authoritative for:** where each gate lives and how surfaces call it.
Homes locked 2026-08-05 (plan: audit lessons tooling map).

---

## Placement rule

| Surface | Job |
|---|---|
| **Kit** (`nocturne-agent-kit`) | Canonical CLI scripts under `bin/`. Hook contract via `compose-repo-hooks.sh`. |
| **MCP gates** (`nocturne-mcp-gates`) | Agent-facing wrappers that **exec the same scripts** (no parallel regex). |
| **Knot** | `scripts/` copies (CI-independent), allowlists, CI jobs. |
| **GitHub org** | Secret scanning + push protection (manual; not scriptable here). |

Shared scripts are the SSOT. MCP/CI must not grow a second regex set.

---

## Interfaces

### `check-audit-sha.sh`

- **Inputs:** optional globs via `AUDIT_SHA_GLOBS` (default:
  `docs/internal/AUDIT-* docs/internal/DESIGN-* docs/internal/IMPLEMENTATION.md`
  plus `docs/**/AUDIT-*` / `DESIGN-*` when those dirs exist).
- **Rule:** each matched file’s first 10 lines contain a 7+ hex SHA; `git cat-file -e <sha>^{commit}` succeeds.
- **Exit:** 0 ok, 1 missing/unresolvable stamp.
- **Hook/CI:** yes. **MCP:** `preflight_change` kind `audit_doc`; tool `audit_doc_stamp`.

### `check-fresh-remote.sh`

- **Inputs:** optional ref via `FRESH_REMOTE_REF` (default `origin/HEAD`).
- **Rule:** `git fetch --prune`, then `HEAD == $(git rev-parse $FRESH_REMOTE_REF)`.
- **Exit:** 0 equal, 1 stale/diverged. Network required.
- **Hook/CI:** optional `workflow_dispatch` / release only — **not** every PR.
- **MCP:** tool `release_readiness` (runs this + checklist pointers).

### `check-crate-version-table.sh`

- **Inputs:** `VERSION_TABLE_FILE` (default `README.md`), crate name → version rows
  matching `` `| \`name\` | ... | x.y.z |` `` (Knot README shape).
- **Rule:** each named workspace crate’s table version equals `cargo metadata` version.
- **Exit:** 0 match, 1 drift.
- **Hook/CI:** yes on Knot. **MCP:** not required for v1.

### `check-public-surface.sh`

- **Inputs:** regex checks from PUBLIC-REPO-STANDARD §5; allowlist file
  `.public-surface-allowlist` (substring lines); `ALLOW_PRIVATE_TIER=1` softens
  internal-vocab / private-path checks to warnings (credentials + absolute paths
  still fail).
- **Exit:** 0 no hard failures, 1 otherwise.
- **Hook/CI:** yes. **MCP:** `preflight_change` kind `public_surface`; tool `leak_scan`.

### Gitleaks

- **Kit:** document optional pre-commit if `gitleaks` on PATH.
- **Knot CI:** `gitleaks/gitleaks-action` (or `gitleaks detect`) on PRs.
- **Org:** enable secret scanning + push protection before first public push.
- **MCP:** checklist item only inside `release_readiness`.

### `adopt.sh` path hygiene

- Adoption marker must **not** write absolute local paths (`kit_absolute=…`).
- Prefer relative `kit_path`; else sentinel `kit_path=NOCTURNE_AGENT_KIT`.

---

## Build order (done / tracking)

1. Kit scripts + adopt fix + compose-repo-hooks extension
2. MCP: `audit_doc`, `public_surface`, `release_readiness` / `leak_scan` / `audit_doc_stamp`
3. Knot: `scripts/` + allowlist + CI jobs + this spec

---

## Org checklist (manual)

- [ ] GitHub org/repo: secret scanning enabled
- [ ] GitHub org/repo: push protection enabled
- [ ] Before public launch: unset `ALLOW_PRIVATE_TIER` in CI; tighten allowlist
