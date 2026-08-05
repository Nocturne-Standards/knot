# Doc hygiene inventory — knot

Companion to [`docs/launch-gap-map-2026-08.md`](launch-gap-map-2026-08.md).
Catalogs every README/Status/meta blob and every dead cross-repo citation
found while building the gap map, and proposes where each piece of
knowledge should live for a public-ready front door: **keep-public** /
**move-to-docs-internal** / **move-to-nocturne-docs** / **delete-duplicate**.

Knowledge is never deleted outright unless it's a literal duplicate or a
dead pointer with no content behind it — per the design doc, "move, don't
delete."

## Category definitions

| Category | Meaning |
|---|---|
| `keep-public` | Stays in this repo's `README`/`docs/`, as-is or lightly edited. |
| `move-to-docs-internal` | Real content, but operator/maintainer-facing, not a first-read for a public consumer. Move to a private ops doc (parent platform `docs/`/`references/` per this repo's own root README framing) or a repo-local `docs/internal/` not linked from the public front door. |
| `move-to-nocturne-docs` | Belongs on `docs.nocturne-standards.org/v1/knot/` (the already-designated long-form public docs site) rather than duplicated/summarized in-repo `README`s. |
| `delete-duplicate` | Either a literal duplicate of content that lives elsewhere, or a dead pointer to content that no longer exists anywhere reachable — safe to remove the citation itself (not "delete knowledge," there's no knowledge behind it to preserve). |

## Root `README.md`

| Section | Category | Notes |
|---|---|---|
| Title / one-paragraph description | `keep-public` | Good as-is — clear, no jargon. |
| "For newcomers" + architecture diagram link | `keep-public` | Correctly defers detail to `docs.nocturne-standards.org`. |
| §Status — dated bullet log (2026-07-23 through 2026-08-02, contract hex ids, "Wave 7 carve target," `checked_add`/audit references) | `move-to-docs-internal` | This is exactly the "wave/carve/§ Status archaeology in the front door" the design doc names as the thing to fix. Valuable history, wrong location — a first-time public reader doesn't need 6 dated entries of internal deploy history to understand what the repo does. Move to a `docs/CHANGELOG.md` (repo-level) or parent-platform ops notes; keep only a **current-state** table (crate versions, license, one-line role) in the public README, which the table right below it already mostly is. |
| Version table (`| Surface | License | Notes |`) | `keep-public`, but **fix stale versions** (see gap map A12) before shipping. |
| "Consumer dep (encoding)" `Cargo.toml` snippet | `keep-public` | Directly useful to an integrator. |
| §Layout (directory tree) | `keep-public` | Standard, helpful. |
| §Deploy | `keep-public` | Short, accurate, points at collector README correctly. |
| §Quick commands | `keep-public` | Standard OSS convention. |
| "`docs/roadmap.md` Track 7" citation (§Status) | `delete-duplicate` | Dead link (parent-monorepo path, doesn't exist here) — removed along with the rest of the §Status archaeology above. |
| "atlas/ stays outside this nest" paragraph | `keep-public` | Accurate scoping statement, helps readers understand the boundary — keep even though `atlas/` itself isn't in this repo. |

## `docs/security-model.md`

| Section | Category | Notes |
|---|---|---|
| Whole document | `keep-public` | This is the single best-written doc in the repo — exactly the kind of trust-boundary writeup a security-conscious public launch wants front and center. No changes needed for hygiene; content gaps (B1, A9) are tracked in the gap map, not a hygiene issue. |

## `docs/security-audit-2026-08-04.md`, `docs/security-audit-2026-07-28.md`, `docs/security-audit-2026-07-28-fix-plan.md`

| Item | Category | Notes |
|---|---|---|
| All three files | `keep-public` | Publishing point-in-time audits (frozen, dated, with a visible "resolved since" trail) is a trust signal, not a liability — keep. |
| Organization | `keep-public` (minor `improve`, not a hygiene tier change) | Consider `docs/audits/2026-08-04.md` etc. once there are 3+ of these, purely for directory cleanliness — not required for this launch. |
| "root CLAUDE.md" citations inside `security-audit-2026-07-28*.md` | `delete-duplicate` | Dead link (`CLAUDE.md` doesn't exist in this worktree). These are frozen historical documents though — recommend leaving the text untouched (it's a point-in-time record) but not treating it as a live citation; no action required before launch since these files are explicitly "frozen, for current state see..." at their own top. |

## `docs/superpowers/**` (specs, tracks: `audit-2026-08-full`, `launch-form-knot`, `SPEC.md`/`STATUS.md`/`DECISIONS.md`/`leaves/*`)

| Item | Category | Notes |
|---|---|---|
| Everything under `docs/superpowers/` | `move-to-nocturne-docs` (planning process) / **exclude from public tag** | This is agent-planning scaffolding (planner/worker/reviewer model pins, leaf claim/start/done state machine, acceptance checklists) — internal process metadata, not product documentation. It should not ride along in the public GitHub carve (`aichbindas/knot` per root README §Status). Recommend: keep in the private/working tree (where it already lives and is actively used), and either `.gitignore`/exclude it from whatever produces the public mirror, or move it up to a nocturne-wide internal docs area if it needs to survive past this track's lifetime. Not a "move the knowledge to nocturne-docs.org" case — there's no end-user knowledge here, just process. |

## Root `AGENTS.md` and `.cursor/rules/*.mdc`

| Item | Category | Notes |
|---|---|---|
| `AGENTS.md` (nocturne-agent-kit workflow, claim.py, worktree policy) | `move-to-nocturne-docs` / exclude from public tag | Pure agent-tooling instructions for contributors using this specific internal kit. A public open-source consumer of `knot-encoding`/`knot-registry` has no use for `claim.py acquire` instructions. Not harmful to leave in a private repo; should not be part of the public carve's first impression. |
| `.cursor/rules/*.mdc` (ai-process-hygiene, shared-code, workflow) | `move-to-nocturne-docs` / exclude from public tag | Same reasoning — internal agent configuration, not product docs. `shared-code.mdc`'s actual *rules* (no hand-copied type mirrors, know your encoding layer) are good engineering practice worth publishing in some form, but as a CONTRIBUTING.md excerpt, not verbatim agent-rule format with Cursor-specific frontmatter. |

## Per-crate READMEs

### `crates/knot-tool/README.md` (423 lines — largest, densest)

| Section | Category | Notes |
|---|---|---|
| Title, "For newcomers," Quick start intro | `keep-public` | Necessary onboarding. |
| Quick start's env var table + `scripts/multisig-first-run.sh` invocation | `keep-public`, but fix the `references/testnet-wallet.md` dead citation (gap map A15) and the "From the **sme_platform** repo root (not this crate alone)" framing, which assumes a monorepo checkout that doesn't match this standalone repo's actual layout. |
| §Scope (rkyv wire format explanation) | `keep-public` | Genuinely useful for integrators reading the source. |
| §Security model | `keep-public` | Good, specific, matches `docs/security-model.md`. Minor overlap — could link instead of restate, but restating here is low-cost and not a hygiene problem. |
| §Known caveats | `keep-public` | Real, dated, useful engineering notes (hardfork signing scheme gotcha). Two dead citations inside it (`references/dusk-native/...`, `docs/multisig/testnet-read-lag-2026-07-22.md`) — see Dead links section below. |
| §Status — 9 dated bullets (2026-07-23 through 2026-07-28), each a mini-changelog entry with spec links, most of which are the same `../../../` dead links | `move-to-docs-internal` | This is the densest instance of the wave/carve archaeology problem. The **content** (what each feature does, how to test it) is valuable and belongs in `CHANGELOG.md` (which already exists for this crate and is far shorter/cleaner) or a repo-local `docs/tool-history.md`. The public README should keep a **current-state** summary (what `pm-resolve` does today, how to run it) without the dated "(2026-07-24)" / "(2026-07-23)" changelog framing repeated 9 times. |
| §Status result table (`Registry create/query/change_account \| Pass`, etc.) | `keep-public` | Concise, useful "does this work" signal — keep this part even after trimming the surrounding dated prose. |
| Multi-person runbook, Out-of-band fingerprint, Hardware keys, Monitoring note | `keep-public` | All genuinely useful end-user-facing content, no changes needed. |
| §Usage (Build/First run/CLI/Web UI) | `keep-public` | Standard, necessary. |
| §Explicitly out of scope | `keep-public` | Exactly the honest-scoping pattern the rest of the repo should copy (see gap map C1–C4). |

### `crates/knot-collector/README.md`

| Section | Category | Notes |
|---|---|---|
| Title, trust-model intro | `keep-public` | Good. |
| §Status — 3 dated bullets, one citing a spec doc | `move-to-docs-internal` | Smaller instance of the same pattern; low cost to trim to current-state only. Spec-doc citation (`docs/superpowers/specs/2026-07-23-knot-collector-monorepo-demo-design.md`) is itself under `docs/superpowers/` — see that section above (move-to-nocturne-docs / exclude). |
| API table, env var table, wire-parity explanation | `keep-public` | Core, accurate, necessary reference material. |
| §Run, §License | `keep-public` | Standard. |
| "VPS deploy (operator TODO)" pointer to a runbook that doesn't exist | See gap map C6 — `add` or `doc-move`, not a pure hygiene call since there's a real missing artifact, not just a misplaced one. |

### `crates/knot-registry/README.md`, `crates/knot-proposals/README.md`

| Section | Category | Notes |
|---|---|---|
| §Scope, §Functions, §Finalize/failed execute, §Build/test, §Deploy | `keep-public` | Solid contract-level API documentation, keep as-is. |
| §Status ("v0.1.5 ... Spec 23b Phase B ... PINNED-DIFFERENT-REDEPLOYED ... Contract id `3e3c...`") | `move-to-docs-internal` | Deploy-history/versioning archaeology again, smaller dose than `knot-tool`'s. The **current** version + contract id + "what changed" one-liner is fine to keep public (an integrator does need the live contract id); the "23b Phase B" / "Spec 26 source-carry paragraph cleared" internal-spec-numbering language should move — it's meaningless without access to `docs/superpowers/specs/` (which itself is flagged move-to-nocturne-docs above). |
| §Next steps (registry) | See gap map C5 — `doc-move`, the actionable item belongs in `prediction-market`'s backlog, not knot's public README. |
| "references/dusk-native/...", "root CLAUDE.md", "rusk-experiments/multisig-approval", "references/repos/multisig-contract" citations | `delete-duplicate` | Dead links, see below — content (if it still exists) lives in the parent platform repo, not reachable from here. |

### `crates/knot-encoding/README.md`

| Section | Category | Notes |
|---|---|---|
| Whole doc | `keep-public` | Shortest, cleanest crate README in the repo — good model for what the others should look like after trimming. Only issue is the spec citation (`docs/superpowers/specs/2026-07-31-shared-code/26-...`, `docs/multisig/multisig-suite-and-atlas-implementation-plan.md`) — both dead/internal, see below. |

## Dead links (found during this pass, feeds gap map A14)

All 18 occurrences resolve to a nonexistent path when checked from the
citing file's own directory (verified via direct filesystem check, not
grep-only):

| Dead target | Cited from | Disposition |
|---|---|---|
| `../../../docs/multisig/knot-collector-deploy-runbook.md` | `knot-tool/README.md` | `add` (write a real runbook here) — see gap map C6. |
| `../../../docs/multisig/testnet-read-lag-2026-07-22.md` | `knot-tool/README.md` (×2) | `delete-duplicate` — historical incident writeup, low value to a new public reader; drop the citation or fold the one-sentence lesson ("RUES free-reads need raw bodies, not hex") directly into §Known caveats, which already states the lesson. |
| `../../../docs/multisig/multisig-suite-and-atlas-implementation-plan.md` | `knot-encoding/README.md` | `move-to-nocturne-docs` if the plan still matters externally, else `delete-duplicate`. |
| `../../../docs/versioning.md` | `knot-tool/README.md`, `knot-tool/CHANGELOG.md`, `knot-collector/README.md`, `knot-collector/CHANGELOG.md` | `add` — see gap map B3; this one has real expected content missing, not just a misplaced pointer. |
| `../../../docs/superpowers/specs/2026-07-31-shared-code/26-multisig-shared-call-types.md` | `knot-encoding/README.md` | `move-to-nocturne-docs` or inline the one relevant paragraph (Layer-E call-types rationale) directly into the README, since the spec dir itself is excluded from the public tag. |
| `../../../docs/superpowers/specs/2026-07-26-multisig-website-demo-design.md` | `knot-tool/README.md` | Same as above. |
| `../../../deployments/testnet.json` | `knot-tool/README.md` (×2) | `move-to-docs-internal` — this is real operator data (which repo's tree it should live in is a monorepo-layout question outside this track's scope); at minimum the README should say where a *standalone* knot checkout gets this file from, since today it implies a file this repo doesn't ship. |
| `../../../prediction-market/docs/council-resolve-testing.md` | `knot-tool/README.md` (×2) | `delete-duplicate` from knot's side (belongs entirely to the `prediction-market` repo now) — replace with a short "see prediction-market's own docs" pointer with no relative path (a repo name, not a filesystem path, since post-carve they're siblings at best). |
| `../../../prediction-market/crates/pm-admin-tool/README.md`, `.../pm-council-tool/README.md` | `knot-tool/README.md` | Same as above. |
| `../../../references/repos/multisig-contract` | `knot-registry/README.md` | `delete-duplicate` — comparison-to-upstream-example note; nice-to-have context, safe to drop the dead link or replace with the public Dusk example's actual public URL if one exists. |
| `../../../references/dusk-native/crate-source-locations.md`, `.../dusk-vm-issue-1-ephemeral-hardfork-policy-unreachable.md` | `knot-registry/README.md`, `knot-tool/README.md` (×2), `knot-tool/src/bls.rs` module doc | `move-to-docs-internal` — these describe a real, still-relevant `dusk-vm` gotcha (`VM::ephemeral()` hardfork policy) that affects how this repo's own tests are written. Worth a short internal note (or upstreaming as a `dusk-vm` issue if not already filed) rather than a dead pointer, since the gotcha is genuinely load-bearing for anyone extending the test suite. |
| `../../../nocturne-lab/`, `bash nocturne-lab/scripts/sync-assets.sh` | `knot-tool/README.md` | `move-to-docs-internal` — asset-sync tooling for the marketing/demo Lab UI, maintainer-facing only. |
| `references/testnet-wallet.md` (no `../../../` prefix — cited as if repo-root-relative) | `knot-tool/README.md` (×2), `knot-tool/src/chain.rs:315,329` (**runtime error text**, not just docs) | See gap map A15 — this is the highest-priority dead link because it surfaces in an actual CLI error message a real user will hit. `add` (vendor the real content) or rewrite the message to be self-contained. |
| `atlas/README.md` | `knot-tool/README.md` (monitoring note) | `delete-duplicate` from knot's perspective — `atlas/` is explicitly "outside this nest" per the root README's own framing; the monitoring note is good advice but shouldn't cite a path knot doesn't ship. Reword to reference Atlas by name without a filesystem path. |
| `root CLAUDE.md` | `knot-registry/README.md`, `docs/security-audit-2026-07-28*.md` | `delete-duplicate` — internal AI-agent tooling note (dusk-forge codegen gotcha), doesn't exist here; if the underlying gotcha still matters, fold the one sentence into the README directly instead of citing an agent config file. |
| `docs/roadmap.md` Track 7 | Root `README.md` | `delete-duplicate` — see root README section above. |

## Summary counts

| Category | Count (rows above) |
|---|---|
| `keep-public` | 24 |
| `move-to-docs-internal` | 11 |
| `move-to-nocturne-docs` | 6 |
| `delete-duplicate` | 9 |

Totals count doc *sections*, not individual sentences — several sections
contain a mix (e.g. a README's §Status table gets `move-to-docs-internal`
for its dated prose but `keep-public` for its current-state result table;
both are listed under that section's row with the split called out in
Notes).
