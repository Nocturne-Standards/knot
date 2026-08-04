Point-in-time, frozen 2026-08-04. For current state see each crate's `README.md` /
`docs/security-model.md`.

# Security audit — multisig suite (knot), full re-pass + signing-tool TCB elevation (2026-08-04)

Scope: all 5 `crates/*` (encoding, registry, proposals, tool, collector),
monorepo-carve invariants (Cargo.toml/Cargo.lock, licensing), and the
Coord-vs-Prove dual-mode question. Posture D: attack surface is every host
that gates digests, aggregates partials, or submits — primary TCB is
`multisig-tool`; the collector is untrusted by design.

Method: read-only, briefed against `docs/security-model.md`'s documented
trust boundaries and cross-checked line-by-line against
`docs/security-audit-2026-07-28.md` (frozen point-in-time — re-proven from
current source, not relayed). Every Critical/High below was independently
re-verified by direct code read (file:line cited). `multisig-tool` got a full
deep pass per posture D (elevate signing-tool TCB): every RPC handler and CLI
subcommand that can produce a signature or submit a transaction was read.

## Summary — ranked by severity

| Sev | Location | Issue |
|---|---|---|
| **Critical** | `multisig-tool/src/rpc.rs:1919-1986`, `src/main.rs` PmResolveCmd::Sign | PM council-resolve signing has **no membership gate at all** — not server-side, not even UI-only (worse than the July-28 `pm-council-tool` precedent, which at least disabled a button client-side) |
| **High** | `multisig-tool/src/rpc.rs:1988-2011`, `src/main.rs:1367-1386` | PM-resolve submit target (`pm_contract_id`) taken verbatim from the untrusted collector blob, never cross-checked against a live-fetched PM deployment id — stale/redeployed contract → silent wrong-target submit |
| **High** | `multisig-tool/src/pm_resolve_types.rs`, `src/pm_read_types.rs` | Cross-**repo** hand-mirrored rkyv ABI types (`ResolveArgs`/`CouncilSigEntry`/`MarketInfo`) for `prediction-market`, "kept in sync by hand" — no compiler enforcement, no golden-vector test, violates repo's shared-code rule; PM lives in a separate repo post-carve so drift is now silent |
| Medium | `multisig-tool/src/rpc.rs` (`api_proposal_approve`, `api_quorum_submit`, `api_quorum_agg_submit`, `api_change_account_submit`) + CLI equivalents | No live on-chain membership/threshold pre-check before any generic sign path either — mitigated by robust on-chain re-verification in this repo's own contracts (see Prove-mode confirmation below), but still a TCB-hygiene gap the posture-D brief calls out |
| Medium | `multisig-tool/src/rpc.rs:1693-1731` (`api_pm_resolve_init`) | Blob `threshold` is client-supplied with no live cross-check against the registry account's current threshold at creation time; `registry_warn` only checks member PKs, never threshold — PM-flow analogue of July-28's pm-council-tool "no live threshold re-check" Medium |
| Low | `multisig-collector/src/store.rs` | No cap on total proposal/party row count (unchanged from July-28) |
| Low | `multisig-registry/src/state.rs`, `multisig-proposals/src/state.rs` | Unbounded account/proposal count (permissionless by design, unchanged) |
| Low/Info | `multisig-encoding/src/lib.rs` | `change_account_digest`/`council_resolve_digest` member-pk ordering is doc-stated, not canonicalized/enforced (unchanged doc gap) |
| **Resolved since 2026-07-28** | `multisig-tool/src/keystore.rs:34` | PBKDF2-SHA256 rounds 100k → **600k** — fixed |
| **Resolved since 2026-07-28** | `multisig-tool/src/rpc.rs:219-231` | Bearer-token compare `==` → **`subtle::ConstantTimeEq`** — fixed |
| **Resolved since 2026-07-28** | `multisig-encoding/src/lib.rs:145-146` | `u32::try_from(...).expect(...)` panic → **`checked_u32_len` returning `Result`** — fixed |
| **Resolved since 2026-07-28** | `multisig-registry/src/state.rs` (`next_id`/`nonce`) | Unguarded `+=` → **`checked_add(...).expect(...)`** (hard panic instead of silent wraparound) — fixed |
| **Resolved since 2026-07-28** | `multisig-encoding/src/fingerprint.rs:34` | Wordlist-length invariant `debug_assert_eq!` (compiled out in release) → **`assert_eq!`** (always checked) — fixed |

No Critical/High findings survive in `multisig-registry`, `multisig-proposals`,
or `multisig-collector` — all three independently re-verified clean and
consistent with docs (see per-crate sections). All Critical/High findings
this wave are in `multisig-tool`, the designated primary TCB under posture D.

---

## Critical — PM council-resolve sign path has zero membership gate

**Location:** `crates/multisig-tool/src/rpc.rs:1919-1986` (`api_pm_resolve_sign`);
identical gap in CLI `PmResolveCmd::Sign` (`src/main.rs`, same crate); UI at
`crates/multisig-tool/static/pm-resolve-app.js` (`sign()`, lines 294-320) and
`pm-resolve-app.js` overall — grepped for any membership/council check
(`member`, `isMember`, `onCouncil`, etc.): **none found**.

**Invariant broken:** the tool is the designated last-mile TCB for signing
(posture D). Before it hands a valid BLS partial signature to the untrusted
collector, it should confirm the chosen local identity's PK is actually a
current member of `registry_account_id` — the same invariant the July-28
audit found broken (Critical) in the sibling `pm-council-tool` crate. Here
it's worse: `pm-council-tool` at least had a client-side-only check
(`computeOnCouncil()` disabling a button); this crate's `pm-resolve-app.js`
has no membership-aware UI logic whatsoever — the Sign button is gated only
by a generic "I confirmed the preview" checkbox (`sign()` line 301,
`confirm.checked`).

**Evidence:** `api_pm_resolve_sign` (rpc.rs:1919): pulls the blob from the
collector → `gate_pm_blob_for_signing` (digest-integrity only) → optional
`expect_digest` match → looks up the named local identity → signs → pushes
the partial. No call to `chain::query::<Option<MultisigAccountView>>("account", ...)`
or any other membership check anywhere in this function. Compare to
`status_out_from_file` (rpc.rs:1791-1856), which *does* fetch the registry
account and computes `registry_warn` — but that function backs the read-only
`/status` endpoint, not `/sign`; its warning is display-only and is never
consulted by the sign handler. Any process on the same loopback host that can
present the bearer token (any process running as the same local user — see
`require_token`, rpc.rs:213) can request a valid, collector-pushed partial
signature for any locally-held identity name, member or not.

**Mitigating factor (do not let this understate the fix priority):** the
PM contract's `resolve()` lives in a separate repo (`prediction-market`), out
of this audit's scope; per `docs/security-model.md`'s existing text it is
expected to independently re-derive quorum from the registry's live
threshold (Prove mode) — so a non-member partial should fail verification
on submit rather than authorize anything. That expectation is **not
independently verified in this repo** (the contract code isn't here), and
even if true, it does not excuse the tool from providing the last-mile gate
posture D asks for.

**Suggested fix leaf:** `signing-tool-tcb-pm-membership-gate` — before
`api_pm_resolve_sign` / CLI `PmResolveCmd::Sign` call `bls::sign`, fetch
`MultisigAccountView` for `intent.registry_account_id` and reject
(400/hard error) if the chosen signer's PK is not in `view.members`. Mirror
in the standalone UI (disable Sign until a membership check succeeds, not
just "confirmed" checkbox) for defense-in-depth, matching the *documented*
expectation this repo already sets for the generic proposals flow's UX.

---

## High — PM-resolve submit target not cross-checked against live contract

**Location:** `crates/multisig-tool/src/rpc.rs:1988-2011` (`api_pm_resolve_submit`);
`crates/multisig-tool/src/main.rs:1367-1386` (`PmResolveCmd::Submit`).

**Invariant broken:** "wrong submit target ... fails verify" (implied safe
failure) vs. actual behavior: a redeployed/stale PM contract id baked into
the blob at creation time is used verbatim, with no comparison against
`chain::contract_id_hex(chain::Contract::PredictionMarket)` — the exact
live-lookup helper already used by `api_deployments_pm` (rpc.rs:790-800) and
`PmResolveCmd::Deployments` (main.rs:1428+). This is the same class of bug
July-28 rated High in `pm-council-tool` (`rpc.rs:606-629` there), now present
in `multisig-tool`'s own PM-resolve feature, unfixed.

**Evidence:** `api_pm_resolve_submit` builds `args` via
`blob::build_pm_resolve_args(&file)`, then reads `pm_id =
intent.pm_contract_id.clone()` (attacker/collector-controlled input — the
blob is pulled straight from the untrusted collector) and calls
`chain::submit_call_to_contract_id(&pm_id, "resolve", &bytes)` directly.
`grep pm_contract_id` across `crates/multisig-tool/src` shows no call to
`contract_id_hex(Contract::PredictionMarket)` anywhere near the submit path.

**Suggested fix leaf:** `signing-tool-tcb-pm-target-crosscheck` — in both the
RPC and CLI submit paths, fetch `chain::contract_id_hex(chain::Contract::PredictionMarket)`
and compare (case-insensitively, hex-normalized) against `intent.pm_contract_id`
before calling `submit_call_to_contract_id`; hard-fail loudly on mismatch
(don't silently proceed — matches the fix already prescribed for
`pm-council-tool` in the July-28 action list, item 2).

---

## High — cross-repo hand-mirrored rkyv ABI types, no parity test

**Location:** `crates/multisig-tool/src/pm_resolve_types.rs` (`CouncilSigEntry`,
`ResolveArgs`); `crates/multisig-tool/src/pm_read_types.rs` (`MarketStatus`,
`MarketInfo`).

**Invariant broken:** repo's `shared-code` rule (`AGENTS.md` /
`.cursor/rules/shared-code.mdc`): "No hand-copied type mirrors... Do not
write a local copy documented as 'mirrors X byte-for-byte'" and, for
contract-to-contract/off-chain-to-contract rkyv ABI args specifically
(domain E), "the callee owns the type." Both files self-document exactly the
forbidden pattern: `pm_resolve_types.rs` says "Field order must stay in
lockstep with `prediction-market/crates/prediction-market/src/call_types.rs`";
`pm_read_types.rs` says "Kept in sync by hand." Compare with
`registry_types.rs`/`proposals_types.rs` in the same crate, which correctly
use `#[path = "../../multisig-registry/src/call_types.rs"]` same-repo file
inclusion (compiler-enforced single source of truth, zero drift risk) — the
PM types don't get that treatment because `prediction-market` is a
**separate repo** post-carve (`multisig`/knot's own Wave-7 carve, per root
`README.md`), so file-path inclusion isn't available. No golden-vector test
exists for either struct (checked: `grep -r "golden\|parity\|ResolveArgs\|CouncilSigEntry"`
across the crate's `tests/` — zero hits outside the source files themselves).

**Evidence:** `ResolveArgs`/`CouncilSigEntry` (`pm_resolve_types.rs`) feed
directly into `chain::encode()` → `submit_call_to_contract_id(pm_id, "resolve", &bytes)`
— i.e., the exact bytes sent to the live PM contract for the resolve
transaction. If `prediction-market`'s own `ResolveArgs`/`CouncilSigEntry`
gain/reorder/resize a field (plausible — it's actively developed per
`AGENTS.md`/README wave notes), this crate's copy silently drifts. Depending
on how the mismatch manifests in rkyv's fixed-offset archive format, this
could be: (a) a decode failure on the contract side (safe abort, tx reverts)
or (b) — if the two shapes happen to have compatible total size/alignment —
a semantically-misinterpreted call (e.g. `market_id`/`winning_outcome` bytes
landing in the wrong field). `MarketInfo`/`MarketStatus` drift is lower
severity (free-read/display only — wrong `yes_reserve`/`no_reserve` shown in
UI, or a hard decode error) but same root cause.

**Suggested fix leaf:** `monorepo-carve-pm-abi-shared-crate` — extract
`CouncilSigEntry`/`ResolveArgs`/`MarketStatus`/`MarketInfo` into a small
Apache-licensed shared crate (`prediction-market-types` or similar,
analogous to `achievement-nft-types`/`multisig-encoding`'s own
`call-types` feature) that both `prediction-market` and `knot`'s
`multisig-tool` depend on by version pin; until that lands, add a
golden-vector byte-layout test in `multisig-tool` (encode a fixed
`ResolveArgs`/`MarketInfo` value here, assert the hex against a constant
copied from a matching test in `prediction-market`) so drift fails CI on
whichever side lags, per the shared-code rule's cross-boundary fallback.

---

## Medium — generic sign paths also lack a live membership/threshold gate

**Location:** `crates/multisig-tool/src/rpc.rs` — `api_proposal_approve`
(1427-1571), `api_quorum_submit` (1022-1049), `api_quorum_agg_submit`
(1091-1131), `api_change_account_submit` (1183-1215); CLI equivalents in
`main.rs` (`ProposalCmd::Approve`, `QuorumCmd::Submit`, `QuorumAggCmd::Submit`,
`ChangeAccountCmd::Submit`).

**Invariant broken:** same TCB-hygiene expectation as the PM-resolve Critical
above, applied to the suite's own registry/proposals flows. `diagnose_quorum`
free-reads exist (`api_quorum_diagnose`, CLI `diagnose_local_signers` helper
at `main.rs:591-616`) and *do* surface a `member_matches`/`sigs_ok`
mismatch warning — but they are separate, opt-in commands, never invoked
automatically before a sign/submit call.

**Evidence:** none of the four handlers above call `chain::query::<Option<MultisigAccountView>>("account", ...)`
(or the mock-mode equivalent) to check the chosen signer(s)' PKs against
current membership before calling `build_sigs_locked` / `bls::sign_multisig`
and submitting.

**Why this is Medium, not Critical/High (unlike the PM-resolve finding):**
independently confirmed by direct code read that the on-chain paths these
flows feed *do* robustly re-verify, in this repo:
- `multisig-registry::verify_quorum`/`verify_quorum_aggregate`
  (`state.rs:140-183`) check membership, dedup, threshold, and
  `abi::verify_bls`/`verify_bls_multisig` before returning `true` —
  a non-member signature simply doesn't count.
- `multisig-proposals::approve` (`state.rs:191-225`) panics outright if
  `!view.members.contains(&args.signer)`, before recording the approval.
- `multisig-proposals::finalize` (`state.rs:256-326`) re-derives quorum via
  a second `verify_quorum` call over the collected approvals before
  `call_raw`.

So the failure mode for a non-member local signer today is a safe on-chain
revert / `false` return, not an authorization bypass — but it still burns a
signing round, occupies a collector partial slot (subject to
`MAX_PARTIALS=32`), and gives the operator zero warning until the chain
rejects it minutes later.

**Suggested fix leaf:** `signing-tool-tcb-generic-membership-gate` — thread
the same live-membership pre-check from the PM-resolve fix into the four
handlers above (and their CLI twins); surface `diagnose_quorum`'s counters
automatically on the approve/submit path instead of as a separate opt-in
command.

---

## Medium — PM-resolve blob threshold not cross-checked at creation

**Location:** `crates/multisig-tool/src/rpc.rs:1693-1731` (`api_pm_resolve_init`).

**Invariant broken:** analogous to July-28's pm-council-tool Medium ("no
live threshold re-check before sign/submit"). `req.threshold` is taken
directly from the API caller with no comparison against the registry
account's *current* on-chain threshold before `blob::create_pm_blob_file`
bakes it into the digest (`council_resolve_digest` includes `threshold` as
an input, per `docs/security-model.md`'s domain-separation note). A stale or
simply-wrong threshold value produces a blob that will safely fail on-chain
(digest mismatch against whatever the PM contract independently derives at
authorize time, per docs) — but the tool gives no early warning, and
`status_out_from_file`'s `registry_warn` (rpc.rs:1802-1836) only cross-checks
partial *signer PKs* against the registry member list, never `file.threshold`
against the account's live `threshold` field (which the same `MultisigAccountView`
read already has available and simply discards).

**Suggested fix leaf:** `signing-tool-tcb-pm-threshold-warn` — in
`status_out_from_file`, additionally compare `file.threshold` to
`view.threshold` and add a second `registry_warn`-style message on mismatch;
optionally warn at `api_pm_resolve_init` time too, before the digest (and
therefore the immutable blob identity) is fixed.

---

## multisig-registry — re-verified clean, three July-28 items now fixed

No Critical/High/Medium. Access control (`create_account` intentionally
permissionless, matches docs), `change_account` nonce/replay handling,
`verify_quorum`/`verify_quorum_aggregate` membership+threshold+BLS checks
(`state.rs:140-183`), and `MAX_COMMITTEE_MEMBERS=16` bound all confirmed
correct by direct read, consistent with July-28's clean bill.

**Fixed since 2026-07-28:** `next_id` (`create_account`) and `nonce`
(`change_account`) increments now use `checked_add(1).expect(...)`
(`state.rs:53-56`, `225-228`) instead of unguarded `+=` — converts the
theoretical u64-wraparound Medium into a hard, safe panic. No regression
found elsewhere in the crate.

**Low (unchanged):** unbounded total account count — permissionless
`create_account` by design, matches documented intent.

## multisig-proposals — re-verified clean

No Critical/High/Medium. `propose`/`approve`/`finalize` state machine
re-read line-by-line (`state.rs:117-326`): digest bound once at `propose`
and immutable thereafter; `approve` checks membership
(`!view.members.contains` panics) and re-verifies the BLS signature before
recording; `finalize` re-checks the *current* committee nonce (stale-nonce
guard), re-derives quorum via a second `verify_quorum` call, and follows
CEI (status flip + nonce bump + emit, *then* `call_raw`) — matches July-28's
"reentrancy blocked by pre-external-call status flip" finding, unchanged.
`require_owner()` still checks `abi::public_sender()`, not `abi::caller()`
(the M2-immunity property July-28 flagged as worth a doc footnote) — footnote
still absent from `docs/security-model.md`; low-priority doc gap, folded
into this report's Low list rather than a separate item.

**Low (unchanged):** unbounded proposal count, no pruning; per-field caps
(`call_args` 4096B via `MAX_CALL_ARGS_LEN`, `function_name` 64B via
`MAX_FUNCTION_NAME_LEN`) still present and enforced (`propose`, `state.rs:122-127`).

## multisig-collector — re-verified clean, untrusted-by-design posture intact

No `dusk_core` dependency (confirmed via `Cargo.toml` — no BLS/digest
verification capability exists in this crate at all, by construction).
Non-loopback bind gate (`assert_bind_allowed`/`assert_bind_allowed_with`,
`lib.rs:45-64`) unconditional and unit-tested. Same-`signer_pk`
last-write-wins replace confirmed scoped per-proposal
(`store.rs::append_partial`, `WHERE id = ?1`). Party roster upsert-only, no
DELETE route (test `party_delete_route_is_gone` confirms 404). `MAX_PARTIALS=32`,
`MAX_NOTE_CHARS=512`, `MAX_BODY_BYTES=64KiB` all present and enforced
(`api.rs`).

**Low (unchanged):** no cap on total distinct proposal/party row count —
matches the crate's own "unauthenticated relay by design" framing if the
reverse-proxy auth layer is skipped/misconfigured.

## multisig-encoding — re-verified clean, two July-28 items now fixed

Canonical §4a preimage (length-prefixed variable fields, adversarial
field-shifting test present: `length_prefix_rejects_field_shifting`),
domain separation (`DOMAIN_PROPOSAL_V1`/`DOMAIN_CHANGE_ACCOUNT_V1`/
`DOMAIN_COUNCIL_RESOLVE_V2`, each distinct and versioned), and fingerprint
(operates over the full 32-byte digest, no truncation) all confirmed
correct. `registry_types.rs`/`proposals_types.rs` in `multisig-tool` correctly
re-export Layer-E types from this crate's `call_types` module (no
duplication) — the July-28-era "15+ declarations of one ABI struct" anti-
pattern this repo's `shared-code.mdc` rule warns about is *not* present for
the registry/proposals ABI; it *is* present for the PM cross-repo types (see
High finding above).

**Fixed since 2026-07-28:**
- `lib.rs:64-65` (`checked_u32_len`) replaces the old `u32::try_from(...).expect(...)`
  — now returns `Result<u32, EncodingError>`; `proposal_preimage`/`digest`
  propagate the error instead of panicking on oversized untrusted-origin
  `function_name`/`call_args`.
- `fingerprint.rs:34` uses `assert_eq!` (not `debug_assert_eq!`) for the
  2048-word BIP39 wordlist invariant — now checked in release builds too.

**Low/Info (unchanged doc gaps, not code bugs):** no documented max length
for `function_name`/`call_args` (the new `Result` API makes this a soft
limit at `u32::MAX`, still undocumented as a *product* limit);
`member_pks` ordering for `change_account_digest`/`council_resolve_digest`
is doc-stated but not enforced/canonicalized in the type system.

## Monorepo carve — clean, one cross-cutting exception (see High above)

`Cargo.toml`/per-crate manifests reviewed: no path-deps outside `crates/`,
no `vendor/` directory in the workspace, no private-git pins (only
`dusk-network/rusk` public tag pins). Licensing edges clean: `multisig-tool`
(Apache-2.0) depends only on Apache/MIT-class crates plus `multisig-encoding`
(Apache, path dep) — no Apache crate depends on the AGPL `multisig-collector`
(collector is a separate binary reached only over HTTP). `multisig-collector`
itself correctly has zero `dusk-core`/BLS dependency, consistent with its
AGPL-isolated, key-material-free design. The one exception to "clean" is the
PM cross-repo type mirroring flagged as a High finding above — that's a
monorepo-carve consequence (PM moved to a separate repo, breaking the
`#[path=...]` same-repo trick the registry/proposals types still use), not a
licensing or path-dep violation.

---

## Dual mode: Coord vs Prove — decision

See `docs/security-model.md`'s new "Dual posture: Coord vs Prove" section
(added this audit) for the integrator-facing writeup, and
`DECISIONS.md` for the append-only record. Short version: **Prove is the
only mode implemented on-chain in this repo today** — every write path in
`multisig-registry`/`multisig-proposals` independently re-verifies
membership, threshold, and BLS signatures against live state before
honoring a quorum claim. Pure Coord (chain trusts an off-chain decision
without its own re-check) is not implemented anywhere in this suite's own
contracts. The recommendation is to **offer Prove as the only supported mode
for this suite**, and treat `multisig-tool`'s own gating (digest recompute,
and — once the fix leaves above land — membership/threshold pre-checks) as
hygiene/UX that reduces wasted signing rounds, not as an alternate
authorization path. Any future integrator whose target contract does *not*
independently re-verify quorum (a hypothetical pure-Coord consumer) inherits
the tool's full TCB and must say so explicitly in its own docs — this
suite's docs should not be read as covering that case.

---

## Fix-leaf priority order (for a future implementation wave)

1. `signing-tool-tcb-pm-membership-gate` (Critical)
2. `signing-tool-tcb-pm-target-crosscheck` (High)
3. `monorepo-carve-pm-abi-shared-crate` (High)
4. `signing-tool-tcb-generic-membership-gate` (Medium)
5. `signing-tool-tcb-pm-threshold-warn` (Medium)
6. Everything else in the table is Low/Info — batch whenever convenient, none
   block current testnet-only usage; several July-28 Low/Medium items are
   already resolved (see table).
