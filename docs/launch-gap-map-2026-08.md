# Knot launch gap map — 2026-08

Track: `docs/superpowers/tracks/launch-form-knot/`. Design:
[`docs/superpowers/specs/2026-08-04-knot-launch-form-design.md`](superpowers/specs/2026-08-04-knot-launch-form-design.md).
Inputs: [`docs/security-audit-2026-08-04.md`](security-audit-2026-08-04.md),
[`docs/security-model.md`](security-model.md), root `README.md`, every
`crates/*/README.md`, every `crates/*/CHANGELOG.md`, source tree.

**No product fix code in this pass.** This is the gap map only — inputs for
the Phase 1 launch-form discussion (`docs/launch-form-knot.md`, not written
yet). `Launch?` is **tentative**; settled only in that discussion.

## How to read this

- **Kind A** — exists in tree today, needs fix/improve before an honest
  public claim (includes every audit Medium+, plus other shipped-path
  quality issues found during this pass).
- **Kind B** — implied by docs/security-model/README claims but not
  implemented or not wired. Land it, or rewrite the claim.
- **Kind C** — optional / roadmap. `defer` unless discussion overrides.
- **Tag** meanings: `ship` exists & trustworthy · `fix` must change before
  claim · `improve` quality/cleanliness · `finish` half-built · `add`
  missing entirely · `defer` not this launch · `delete` remove · `extract`
  shared crate / golden vectors · `doc-move` knowledge stays, leaves the
  public front door.

## Counts

| Kind | Rows | Launch=Y | Launch=discuss | Launch=N |
|---|---|---|---|---|
| A (fix/improve existing) | 16 | 2 | 7 | 7 |
| B (missing, required) | 3 | 0 | 3 | 0 |
| C (missing, optional) | 6 | 4 | 2 | 0 |
| **Total** | **25** | **6** | **12** | **7** |

Every audit **Medium+** (Critical ×1, High ×2, Medium ×2) has a Kind A row
below (A1–A5), each linking the existing queued fix leaf in
`docs/superpowers/tracks/audit-2026-08-full/leaves/`. Intentional **Low**
clusters the audit calls "unchanged by design" are also rowed (A6–A9) so the
map has a home for them instead of silently dropping them.

## Surface inventory

**Crates** (all `crates/*`, one Cargo workspace, `resolver = "2"`,
`rust-version = "1.94"`, edition 2024):

| Crate | Version (Cargo.toml) | Version (root README claim) | License | Role |
|---|---|---|---|---|
| `knot-encoding` | 0.1.2 | 0.1.1 (stale) | Apache-2.0 | Canonical §4a digest / blob / fingerprint / Layer-E call types |
| `knot-registry` | 0.1.5 | 0.1.4 (stale) | Apache-2.0 | On-chain BLS M-of-N quorum verification (no custody) |
| `knot-proposals` | 0.3.2 | 0.3.1 (stale) | Apache-2.0 | On-chain propose→approve→finalize `call_raw` |
| `knot-tool` | 0.2.0 | 0.2.0 | Apache-2.0 | Local signing CLI + loopback web UI; designated primary TCB (posture D) |
| `knot-collector` | 0.2.0 | 0.2.0 | AGPL-3.0-only | Untrusted off-chain blob/partial relay |

**Host surfaces** (where a human or another process crosses a trust
boundary):

| Surface | Where | Trust posture |
|---|---|---|
| `knot-tool` CLI | `crates/knot-tool/src/main.rs` (1588 lines) | Holds keys locally; primary TCB |
| `knot-tool` loopback RPC | `crates/knot-tool/src/rpc.rs` (2011 lines), bearer-token gated | Holds keys locally; primary TCB; **every audited finding lives here or in its CLI twin** |
| `knot-tool` web UI | `crates/knot-tool/static/*.js`, `pm-resolve-app.js` | Browser never sees secret keys; UI-level gating only, no server round-trip for membership |
| `knot-collector` HTTP API | `crates/knot-collector/src/api.rs` | Untrusted by design, no `dusk_core` dep, length/shape checks only |
| `knot-registry` contract | `crates/knot-registry/src/state.rs` | On-chain Prove: independently re-verifies membership+threshold+BLS |
| `knot-proposals` contract | `crates/knot-proposals/src/state.rs` | On-chain Prove: re-verifies membership, re-derives quorum at finalize |
| PM contract (`resolve()`) | separate repo `prediction-market`, **out of this tree** | Assumed Prove per docs; **not independently verified here** (see B2) |

## Kind A — existing functionality (fix / improve)

| ID | Surface | Kind | Planned cite | Code cite | Tests/goldens | Tag | Launch? | Notes |
|---|---|---|---|---|---|---|---|---|
| A1 | `knot-tool` PM-resolve sign | A | `docs/security-model.md` "Known gap" §Off-chain rules/Tool | `crates/knot-tool/src/rpc.rs:1919-1986` (`api_pm_resolve_sign`); `src/main.rs` `PmResolveCmd::Sign`; `static/pm-resolve-app.js` `sign()` L294-320 | None (rpc.rs has zero tests, see A10) | fix | **N** | Audit **Critical**. Zero membership gate — any local process with the bearer token can sign for any identity, member or not. Fix leaf queued: `docs/superpowers/tracks/audit-2026-08-full/leaves/010-signing-tool-tcb-pm-membership-gate.md`. |
| A2 | `knot-tool` PM-resolve submit | A | `docs/security-model.md` "Known gap" | `crates/knot-tool/src/rpc.rs:1988-2011` (`api_pm_resolve_submit`); `src/main.rs:1367-1386` | None | fix | **N** | Audit **High**. `pm_contract_id` taken verbatim from untrusted collector blob, never cross-checked against `chain::contract_id_hex(Contract::PredictionMarket)` (the same helper `api_deployments_pm` already uses). Fix leaf: `.../leaves/011-signing-tool-tcb-pm-target-crosscheck.md`. |
| A3 | `knot-tool` PM ABI types | A | `AGENTS.md` / `.cursor/rules/shared-code.mdc` shared-code rule | `crates/knot-tool/src/pm_resolve_types.rs`, `src/pm_read_types.rs` | None — no golden-vector/parity test exists (checked: `grep -r "golden\|parity"` only hits `collector_client.rs` module doc, unrelated) | extract | discuss | Audit **High**. Cross-repo hand-mirrored rkyv types (`ResolveArgs`/`CouncilSigEntry`/`MarketInfo`/`MarketStatus`), "kept in sync by hand," feeds the live `resolve()` tx bytes. `registry_types.rs`/`proposals_types.rs` in the same crate do this correctly via `#[path=...]` same-repo inclusion — PM can't, it's a separate repo post-carve. Fix leaf: `.../leaves/012-monorepo-carve-pm-abi-shared-crate.md` (shared crate or golden-vector stopgap). |
| A4 | `knot-tool` generic sign paths | A | `docs/security-model.md` "Known gap" | `crates/knot-tool/src/rpc.rs` — `api_proposal_approve` (1427-1571), `api_quorum_submit` (1022-1049), `api_quorum_agg_submit` (1091-1131), `api_change_account_submit` (1183-1215); CLI twins in `main.rs` | None | fix | discuss | Audit **Medium**. No live membership/threshold pre-check; **mitigated** — `knot-registry::verify_quorum`/`verify_quorum_aggregate` (`state.rs:140-183`) and `knot-proposals::approve`/`finalize` (`state.rs:191-326`) independently re-verify on-chain (Prove mode confirmed). Failure mode today is a safe revert, not a bypass — burns a signing round + collector slot with no warning. Fix leaf: `.../leaves/013-signing-tool-tcb-generic-membership-gate.md`. |
| A5 | `knot-tool` PM-resolve init | A | `docs/security-model.md` "Known gap" | `crates/knot-tool/src/rpc.rs:1693-1731` (`api_pm_resolve_init`) | None | fix | discuss | Audit **Medium**. `req.threshold` baked into the digest with no live cross-check against the registry account's current threshold; `status_out_from_file`'s `registry_warn` (rpc.rs:1802-1836) checks signer PKs but discards `view.threshold`. Fix leaf: `.../leaves/014-signing-tool-tcb-pm-threshold-warn.md` (not yet in `docs/superpowers/tracks/launch-form-knot/`; lives in the audit track). |
| A6 | `knot-collector` row growth | A | `docs/security-model.md` "Collector" §Off-chain rules | `crates/knot-collector/src/store.rs` | `crates/knot-collector/src/store.rs` unit tests (11) don't cover unbounded growth (expected — it's permissionless by design) | defer | Y | Audit **Low**, unchanged since 2026-07-28. No cap on total proposal/party row count. Matches the crate's documented "unauthenticated relay by design" framing; an operator fronting it with auth/rate-limit (already required by docs) mitigates. Acceptable to ship as-is; note in launch form as an ops responsibility, not a code gap. |
| A7 | `knot-registry`/`knot-proposals` unbounded counts | A | `knot-registry/README.md` §Functions (`create_account` permissionless) | `crates/knot-registry/src/state.rs`, `crates/knot-proposals/src/state.rs` | `tests/contract.rs` (14/16 tests) don't target this — intentional | defer | Y | Audit **Low**, unchanged, matches documented intent (permissionless create). No action needed for launch. |
| A8 | `knot-encoding` member-pk ordering | A | `docs/security-model.md` §Integrator checklist item 6 | `crates/knot-encoding/src/lib.rs` (`change_account_digest`, `council_resolve_digest`) | 12 unit tests in `lib.rs`, none assert canonical ordering | improve | discuss | Audit **Low/Info**, unchanged. Ordering is doc-stated, not type-enforced. Cheap: either add a canonicalizing helper + test, or add one explicit sentence to `security-model.md` under Integrator checklist. Not launch-blocking either way but should be a conscious discussion choice, not silence. |
| A9 | `knot-proposals` `require_owner()` footnote | A | `docs/security-model.md` (footnote **absent** — this is the gap) | `crates/knot-proposals/src/state.rs` `require_owner()` (checks `abi::public_sender()`, not `abi::caller()`) | `tests/contract.rs` reentrancy/CEI tests pass; no test asserts the `public_sender` vs `caller` distinction | doc-move | discuss | Audit itself flags this as a "low-priority doc gap" carried over from July-28, still not folded into `security-model.md`. One-paragraph fix, zero code risk — good discussion candidate for "cheap, do it now" vs "defer to next doc pass." |
| A10 | `knot-tool` RPC layer has zero tests | A | (not an audit finding — found during this pass, leaf 4) | `crates/knot-tool/src/rpc.rs` (2011 lines) | **0** `#[test]` in `rpc.rs` itself. Only indirect coverage via `tests/collector_roundtrip.rs` (2 `tokio::test`, exercises blob push/pull/aggregate, **not** the axum handlers) and `tests/blob_aggregate_local.rs` (1 test, exercises `blob.rs`, not `rpc.rs`) | fix | **N** | Every Critical/High/Medium finding above (A1–A5) lives in this exact file, and it has no direct test coverage at all — not even a happy-path test, let alone the missing-gate regression tests the fix leaves will need. Any fix to A1–A5 should land with `rpc.rs`-level tests (axum `TestServer`/`tower::ServiceExt` or equivalent), not just manual QA. Treat as a prerequisite for confidently closing A1/A2/A4/A5, not an independent nice-to-have. |
| A11 | CLI/RPC logic duplication | A | (found during this pass) | `crates/knot-tool/src/main.rs` (1588 lines) vs `src/rpc.rs` (2011 lines) — independent implementations of the same sign/submit/approve flows, not a shared function | Same gap noted twice by the audit itself ("identical gap in CLI ... same crate") | extract | discuss | CLI and RPC each re-implement PM-resolve sign/submit, proposal approve, quorum submit/agg, change-account submit. A single missed spot doubles both the fix surface and the test surface for A1/A2/A4/A5. Worth discussing whether the fix-leaf wave should also extract one shared "gate-then-sign" function called by both, rather than patching four call sites × 2 entry points = 8 places by hand. |
| A12 | Root README version drift | A | Root `README.md` §Status table | `README.md` (states encoding 0.1.1 / registry 0.1.4 / proposals 0.3.1) vs `Cargo.toml` + per-crate README (encoding 0.1.2 / registry 0.1.5 / proposals 0.3.2 — each crate's own README already correctly says "PINNED-DIFFERENT-REDEPLOYED" at the newer version) | N/A (docs) | fix | **N** | The very first table a visitor reads states stale versions. Trivial fix, but a wrong version claim on the front door of a "public-ready" repo is exactly the kind of thing this launch form exists to catch. |
| A13 | Front-door GitHub link points to pre-carve org | A | Root `README.md` §Status ("Wave 7 carve target: private GitHub repo `aichbindas/knot`") | `README.md:23`, `crates/knot-tool/README.md:19` — both link "source on GitHub" to `github.com/aichbindas/sme_platform/blob/main/nocturne-docs/...` | N/A (docs) | fix | **N** | The repo's own README says the carve target is `aichbindas/knot`, but the "source on GitHub" link two lines above still points at the old monorepo (`sme_platform`). First-impression inconsistency for a repo whose whole pitch this wave is "stop making readers do wave/carve archaeology." |
| A14 | Pervasive dead `../../../` cross-repo links | A | (found during this pass) | 18 occurrences across `README.md`, every `crates/*/README.md`, `crates/*/CHANGELOG.md` — enumerated in `docs/doc-hygiene-inventory.md` §Dead links | N/A (docs) | fix | **N** | Verified by direct filesystem check from each citing file's own directory: every one of `../../../docs/multisig/*`, `../../../docs/versioning.md`, `../../../deployments/testnet.json`, `../../../prediction-market/*`, `../../../references/repos/multisig-contract`, `../../../nocturne-lab/` resolves to a path that does not exist in this standalone worktree. These are monorepo-relative links that never got rewritten for the carve. See full list and per-link disposition in the doc-hygiene inventory. |
| A15 | Runtime CLI text cites a nonexistent doc | A | (found during this pass) | `crates/knot-tool/src/chain.rs:315` (`bail!("RUSK_WALLET_PWD is not set — see references/testnet-wallet.md")`), `:329` (`.context(...)`); same path repeated in `knot-tool/README.md` Quick start (×3) | N/A | fix | **N** | Not just a docs problem — a real user running the actual binary gets an error message pointing at `references/testnet-wallet.md`, which does not exist anywhere in this repo. Needs either the real content vendored into `docs/` here, or the message rewritten to be self-contained (state the env var directly, drop the dead pointer). |
| A16 | Domain tag carries stale internal name | A | `crates/knot-encoding/README.md` §API ("Domain tag: `sme-platform.multisig.proposal.v1`") | `crates/knot-encoding/src/lib.rs:69,72-73,79-80` — all three domain tags (`DOMAIN_PROPOSAL_V1`, `DOMAIN_CHANGE_ACCOUNT_V1`, `DOMAIN_COUNCIL_RESOLVE_V2`) are prefixed `sme-platform.` | 12 unit tests cover digest correctness, none assert the string content is intentional | improve | discuss | Cosmetic, not a security defect — the byte string is opaque to signers. But it's a permanent, on-chain, cross-repo cryptographic constant (must match `prediction-market-logic`'s copy too) baking in the pre-"Knot" internal codename. Changing it is a breaking redeploy for every existing testnet account. Discussion call: freeze-and-document ("intentionally legacy, matches on-chain state") vs. plan a `v2` domain bump before any mainnet claim. |

## Kind B — missing, required for honest launch

| ID | Surface | Kind | Planned cite | Code cite | Tests/goldens | Tag | Launch? | Notes |
|---|---|---|---|---|---|---|---|---|
| B1 | Prove-only posture not surfaced at top level | B | `docs/security-model.md` §Dual posture: Coord vs Prove ("Decision: Prove is the only mode...") | N/A — this is a claim visibility gap, not a code gap | N/A | add | discuss | The load-bearing decision of this whole audit wave (Prove-only, no pure-Coord offer) lives one paragraph deep in `security-model.md`. Root `README.md` and `knot-tool/README.md` don't restate it as a top-level product claim. For a public launch, "this suite only offers cryptographic guarantees in Prove mode; `knot-tool` is convenience/UX, not an authorization gate" should be readable without opening the security model doc. |
| B2 | PM-resolve feature ships without an out-of-repo caveat | B | `docs/security-audit-2026-08-04.md` Critical section, "Mitigating factor" paragraph: "That expectation is **not independently verified in this repo**" | `crates/knot-tool/README.md` §Status "PM council resolve CLI + standalone UI" — presented as a first-class feature with no caveat | N/A | add | discuss | The tool's PM-resolve safety story leans entirely on an assumption about `prediction-market`'s contract (separate repo, out of this audit's scope) independently re-deriving quorum. That's plausible but unverified from here. Public docs for this feature should say so explicitly, not imply knot's own audit covers the PM contract too. |
| B3 | `docs/versioning.md` cited everywhere, doesn't exist | B | `crates/knot-tool/README.md:137`, `crates/knot-tool/CHANGELOG.md:4`, `crates/knot-collector/README.md:12`, `crates/knot-collector/CHANGELOG.md` (same pattern) — all cite `../../../docs/versioning.md` as the versioning policy SSOT | Checked: `docs/versioning.md` does not exist anywhere in this worktree | N/A | add | discuss | Every "Package version `0.2.0`" status line points readers to a versioning policy doc for what that claim means (semver? pinned-different-redeployed semantics? testnet-only caveats?) — and that doc isn't here. Either write a short `docs/versioning.md` (a few sentences covering the `PINNED-DIFFERENT-REDEPLOYED` convention already used by 3 crates) or drop the citation. |

## Kind C — missing, optional / defer

| ID | Surface | Kind | Planned cite | Code cite | Tests/goldens | Tag | Launch? | Notes |
|---|---|---|---|---|---|---|---|---|
| C1 | QR-code blob transfer (Topology B) | C | `crates/knot-tool/README.md:213` ("QR deferred") | Not implemented — `blob create/show/sign/aggregate/submit-agg` move JSON over "any BYO channel" today | N/A | defer | Y | Already honestly scoped as deferred in-repo. Nothing to change for launch; good example of the pattern the rest of this map should match. |
| C2 | Hardware key (Ledger BLS clear-signing) | C | `crates/knot-tool/README.md` §"Hardware keys (research only — no implementation in this plan)" | Not implemented, explicitly research-only | N/A | defer | Y | Already honestly scoped ("No follow-up work in the current suite plan"). Keep as-is. |
| C3 | Dusk Wallet Extension / `dusk_signMessage` alt signer | C | `crates/knot-tool/README.md` §"Explicitly out of scope (this pass)" | Not implemented | N/A | defer | Y | Already honestly scoped. Keep as-is. |
| C4 | Hosted / public Multisig Lab signing subdomain | C | `crates/knot-tool/README.md` §"Explicitly out of scope (this pass)" | Not implemented (self-host only; public docs site has no hosted signing) | N/A | defer | Y | Already honestly scoped. Keep as-is. |
| C5 | Registry README "Next steps" item belongs to another repo | C | `crates/knot-registry/README.md` §Next steps ("Wire `prediction-market::resolve`'s council path ... over to `verify_quorum_aggregate`") | The actionable work is in `prediction-market` (separate repo, post-carve) — knot's own `verify_quorum_aggregate` + `knot-tool`'s aggregate flow already exist and are tested (`blob_aggregate_local.rs`) | N/A | doc-move | discuss | Knot's side of this is done and tested. The remaining backlog item is entirely in `prediction-market`'s court. Leaving it phrased as knot's own "Next steps" implies unfinished knot work; should move to (or be mirrored from) `prediction-market`'s own backlog, or be reworded here as "done on our side; consumer migration is prediction-market's call." |
| C6 | Collector VPS deploy runbook | C | `crates/knot-collector/README.md` §Run ("VPS deploy (operator TODO)") | Runbook path cited (`docs/knot-collector-deploy-runbook.md`, via dead `../../../` link, see A14) does not exist in this repo | N/A | add / doc-move | discuss | "Operator TODO" is an honest framing for optional self-hosting, but the doc it points readers to for the how-to isn't here. Either write a minimal standalone runbook under this repo's own `docs/`, or rephrase to "bring your own ops; API surface documented above is the full contract" and drop the dead pointer. |

## Tests / goldens coverage map

Coverage found per crate (unit `#[test]` count via `grep -c '#\[test\]'`,
integration test file line/test counts, golden-vector files). "Gap" only
called out where a `ship`/`fix` row above depends on it.

| Crate / surface | Unit tests | Integration tests | Goldens | Gap for ship/fix rows? |
|---|---|---|---|---|
| `knot-encoding` | `lib.rs` 12, `fingerprint.rs` 5 | — | `src/layout_goldens.rs` (155 lines, byte-layout pin) | None — good coverage backing A8's canonical-preimage claim. |
| `knot-registry` | `state.rs` 0 direct (covered via contract tests) | `tests/contract.rs` 14 tests under `VM::ephemeral()` | `tests/layout_goldens.rs` (230 lines, post-Phase-B pin) | None — `verify_quorum`/`verify_quorum_aggregate`/`change_account` all exercised. |
| `knot-proposals` | `state.rs` 0 direct | `tests/contract.rs` 16 tests (incl. reentrancy/CEI, `test-target` helper crate) | `tests/layout_goldens.rs` (183 lines) | None — CEI/reentrancy and quorum re-derivation at `finalize` both under test. |
| `knot-collector` | `store.rs` 11 | `tests/http_smoke.rs` 3 `tokio::test` (HTTP API) | — | Minor: A6's row (unbounded row count) is untested, but that's intentional-by-design, not a regression risk. |
| `knot-tool` — `blob.rs` | 10 | `tests/blob_aggregate_local.rs` 1, `tests/collector_roundtrip.rs` 2 | — | None. |
| `knot-tool` — `mock_ledger.rs` | 7 | — | — | None. |
| `knot-tool` — `keystore.rs` | 1 | — | — | Thin, but keystore format is a thin wrapper over vetted crates per README; acceptable. |
| `knot-tool` — `chain.rs` | 3 | — | — | None for the tested paths; A15's dead-link message isn't a logic bug so wouldn't be caught by these tests anyway. |
| `knot-tool` — `bls.rs`, `collector_client.rs` | 0 each | Covered indirectly via `collector_roundtrip.rs` (2 tests) | — | Acceptable — thin wrappers, exercised end-to-end by the integration test. |
| `knot-tool` — **`rpc.rs`** | **0** | 0 direct (integration tests exercise `blob.rs`/`collector_client.rs`, never the axum handlers) | — | **Gap — see A10.** This is where A1/A2/A4/A5 all live. |
| `knot-tool` — `pm_resolve_types.rs`, `pm_read_types.rs` | 0 | 0 | **None** — no golden vector against `prediction-market`'s copy | **Gap — see A3.** |
| `knot-tool` — `main.rs` (CLI) | 0 | 0 direct | — | Same logic as `rpc.rs`, same gap, see A11. |

## Cross-walk confirmation

Every audit **Medium+** finding from `docs/security-audit-2026-08-04.md` is
rowed above with a linked fix leaf already queued in
`docs/superpowers/tracks/audit-2026-08-full/leaves/`:

| Audit severity | Audit item | Gap-map row | Fix leaf |
|---|---|---|---|
| Critical | PM council-resolve sign, zero membership gate | A1 | `010-signing-tool-tcb-pm-membership-gate.md` |
| High | PM-resolve submit target not cross-checked | A2 | `011-signing-tool-tcb-pm-target-crosscheck.md` |
| High | Cross-repo hand-mirrored PM ABI types, no parity test | A3 | `012-monorepo-carve-pm-abi-shared-crate.md` |
| Medium | Generic sign paths lack live membership/threshold gate | A4 | `013-signing-tool-tcb-generic-membership-gate.md` |
| Medium | PM-resolve blob threshold not cross-checked at creation | A5 | `014-signing-tool-tcb-pm-threshold-warn.md` |

No Medium+ finding is unrowed; none required a wontfix/claim-change instead
of a fix leaf (all five already have one queued from the audit wave).

## Top launch-blocking items (Launch?=N)

1. **A1** — PM council-resolve sign has no membership gate at all (Critical).
2. **A2** — PM-resolve submit target not cross-checked against live contract (High).
3. **A10** — `rpc.rs`, the file hosting every one of the above, has zero tests.
4. **A13** — front-door "source on GitHub" link points at the pre-carve monorepo, not the carve-target repo the README itself names.
5. **A14** — 18+ dead cross-repo relative links across every public README/CHANGELOG.

(A12, A15 are also Launch=N but smaller/mechanical — see table.)
