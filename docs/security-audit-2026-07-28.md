Point-in-time, frozen 2026-07-28. For current state see each crate's `README.md` /
`multisig/docs/security-model.md`.

# Security audit — multisig suite + pm-council-tool (2026-07-28)

Scope: all 5 `multisig/crates/*` (encoding, registry, proposals, tool, collector) +
`prediction-market/crates/pm-council-tool` (logic-interaction focus only — local
keystore-at-rest explicitly out of scope, tool is testnet-only and still in dev).

Method: 6 parallel sonnet sub-agents (one per crate + one for pm-council-tool
interactions), each read-only, briefed against `multisig/docs/security-model.md`'s
documented trust boundaries so findings are "code vs. documented invariant"
deltas, not generic linting. Highest-severity claims (pm-council-tool Critical/High,
proposals M2 hazard) independently re-verified by hand against source below —
not just relayed from sub-agent output.

## Summary — ranked by severity

| Sev | Location | Issue |
|---|---|---|
| **Critical** | `pm-council-tool/src/rpc.rs:555-591`, `main.rs` Sign path | Dispute-council membership gate is UI-only — server never checks signer PK against on-chain council before signing |
| **High** | `pm-council-tool/src/rpc.rs:606-629` | `resolve` submitted to blob's stored `pm_contract_id`, never cross-checked against live-resolved contract id — stale target can succeed silently, not just fail |
| Medium | `pm-council-tool/src/blob.rs`, `chain.rs` | No live on-chain threshold re-check at sign/submit time (stale-threshold risk docs already flag, but tool gives zero pre-emptive warning) |
| Medium | `pm-council-tool/src/chain.rs:181-191` | `live_dispute_council` treats any RPC error identically to "getter missing" → silent fallback to file wiring, invisible to UI |
| Medium | `multisig-encoding/src/lib.rs:145-146` | `u32::try_from(...).expect(...)` panics (not `Result`) on oversized `function_name`/`call_args` from untrusted-origin data |
| Medium | `multisig-registry/src/state.rs:29-32,49-64` | Unguarded `+=` on `next_id`/`nonce` (u64 wraparound, practically unreachable) |
| Low | `multisig-tool/src/rpc.rs:221` | Bearer-token compare is `==`, not constant-time |
| Low | `multisig-tool/src/keystore.rs:34` | PBKDF2-SHA256 100k rounds, below current OWASP guidance (~600k+) |
| Low | `multisig-collector/src/store.rs` | No cap on total proposal/party-roster count — aggregate disk/memory growth if reverse-proxy auth is misconfigured/skipped |
| Low | `multisig-proposals/src/state.rs:117-188` | Unbounded proposal count (permissionless propose, no pruning) |
| Low | `pm-council-tool/src/rpc.rs:139-143` | Token compare is `==`, not constant-time |
| Info | multiple | See per-crate sections — mostly "matches documented threat model, not a bug" confirmations |

No Critical/High findings in any of the 5 `multisig/` crates themselves — both
Critical/High are in `pm-council-tool`.

---

## pm-council-tool (logic interactions)

### Critical — membership hard-gate is client-side only
`rpc.rs:555-591` (`api_blob_sign`, verified by direct read): the handler pulls
the blob from the collector, gates the digest, looks up the *named local
identity* by string match, signs, and pushes the partial back. At no point does
it call `chain::live_dispute_council` / read `MultisigAccountView.members` to
confirm the identity's PK is actually a council member. The only membership
check in the whole crate is `computeOnCouncil()` in
`static/pm-council-app.js:29-33` — pure client JS that only disables buttons.
Any local process that can reach `127.0.0.1:8879` with the injected
`X-Pm-Council-Tool-Token` (i.e. anything running as the same local user) can
`curl` `/api/blob/sign` with any local identity name and get a validly-signed
partial for a non-member. The identical gap exists in the CLI `Sign` path
(`main.rs:346-409`).

Impact is bounded by whatever the on-chain `resolve` verification does with a
non-member partial (out of this crate's scope), but the tool is documented as
providing this gate itself ("Propose and Sign & submit stay disabled until the
selected local signing identity's PK matches a member") and it doesn't, on the
server. **Fix: check membership server-side in `api_blob_sign` (and the CLI
Sign path) before calling `blob::add_partial`, not just in the UI.**

### High — submit target not cross-checked against live contract
`rpc.rs:606-629` (`api_blob_submit`, verified by direct read): resolves
`pm_contract_id` straight from `file_blob.intent.pm_contract_id` (baked in at
blob-creation time) and calls `chain::submit_call_to_contract_id` with it —
never compared against `chain::live_pm_contract_id()`, which the crate already
uses elsewhere (`chain.rs:127-170`, `rpc.rs:465`). If the PM contract is
redeployed between blob creation and submit, the tool submits `resolve` to the
stale/orphaned contract id and can report success against dead state — a
*silent wrong-target success*, not the safe on-chain verify-failure the docs
imply ("wrong submit target ... fails verify"). **Fix: fetch
`live_pm_contract_id()` and compare/warn (or hard-fail) before submit.**

### Medium — no live threshold re-check before sign/submit
`blob.rs` `gate_blob_for_signing` only checks the blob's internal
self-consistency, never re-reads the account's current on-chain threshold.
`Cmd::Status` does fetch `MultisigAccountView` but only cross-checks *signers*
against `.members`, never `.threshold` against `file_blob.threshold` — a free,
missed early-warning. Presumed eventual failure mode is a safe on-chain verify
failure (matches docs), but users get no warning before burning a signing
round on a stale blob.

### Medium — RPC-error and getter-missing collapsed into one silent fallback
`chain.rs:181-191` `live_dispute_council`: any `Err` (network blip, transient
5xx, decode failure) is handled identically to "the getter doesn't exist on
this WASM" — falls back to the file-based `deployments.json` wiring record,
logged only via `eprintln!`. `/api/council` (`rpc.rs:464-495`) surfaces no
signal to the UI that displayed data may be a stale fallback rather than a
live read.

### Verified clean
- Collector trust boundary: every pull path calls `blob::gate_blob_for_signing`,
  which recomputes the digest from canonical fields and rejects any
  collector-supplied digest mismatch — collector content never trusted
  authoritative.
- `human_summary` confirmed display-only, excluded from
  `council_resolve_digest` inputs.
- `gateway_client.rs`: URL is operator/env-configured, not attacker-reachable
  from `/api/*` input — no SSRF. Failures are genuinely soft-fail.
- No panics/unwraps on external/network input; hex decoding uses checked
  `try_into()`.

### Low
- `rpc.rs:139-143` — token compare not constant-time (loopback-only, random
  32-byte token — low real risk, but it's the sole `/api/*` gate so worth
  fixing alongside the Critical above while touching this file).

---

## multisig-proposals

No Critical/High. Two items the audit specifically targeted as highest-risk
both checked out safe **by construction**, independently re-verified:

- **Digest recompute at finalize**: digest is bound once at `propose`; every
  field feeding it is immutable post-creation (no edit path exists), and any
  owner action that changes chain-domain config (`init_chain_id` etc.) first
  calls `wipe_open_proposals()`. No "signature doesn't cover what executes" gap.
- **M2 self-target hazard** (docs warn `call_raw` targets must independently
  check `abi::caller() == proposals`, and that proposing the contract's own
  owner-gated methods as a target is dangerous): verified directly —
  `require_owner()` (`state.rs:63-69`) checks `abi::public_sender() ==
  Some(abi::self_owner())`, i.e. the original transaction signer, **not**
  `abi::caller()`. Since `call_raw` only ever satisfies a `caller()`-based
  check, this contract's own 4 owner-gated methods are structurally immune to
  the M2 bypass — a non-owner can waste storage proposing a self-target, but
  it can never finalize into privilege escalation. Docs' blanket warning is
  correct for third-party targets; doesn't apply to this contract's own
  methods given the `public_sender`-based check. Worth a one-line doc
  footnote so a future refactor to `caller()`-based checks doesn't silently
  reopen this.
- Nonce (per-committee, re-validated fresh at finalize, incremented once,
  CEI-ordered before `call_raw`), deadline (block-height-based, checked at
  propose/approve/finalize), and approve/finalize state machine (no
  double-approve, no double-finalize, reentrancy blocked by pre-external-call
  status flip) all confirmed correct, each backed by an existing test
  (`finalize_reentrancy_runs_target_once`, `propose_rejects_past_deadline`,
  `change_account_requires_quorum_and_bumps_nonce_preventing_replay`).
- No two-`ContractId`-in-one-argument codegen-bug pattern present.

**Low**: `state.rs:117-188` — permissionless `propose`, no fee/bond, no
pruning of expired-never-finalized proposals (only owner-config-change events
tombstone, they don't reclaim storage). Per-field sizes are capped
(`call_args` 4096B, `function_name` 64B) but proposal *count* isn't. Also
unguarded `+=`/`+1` on `next_id`/nonce (u64, practically unreachable).

---

## multisig-registry

No Critical/High/Medium beyond one Medium overflow note. Access control,
`change_account` nonce/replay handling, `next_account_id` allocation (docs'
"race" warning is about cross-transaction ordering, not internal state
corruption — code and docs consistent, not contradictory as initially
suspected), and quorum/threshold counting (dedup verified, no double-counting,
bounded by `MAX_COMMITTEE_MEMBERS=16`) all confirmed correct and
test-covered. No `abi::self_owner()`/`owner()`/`public_sender()` usage
anywhere in this crate, so the 96-byte-BLS-pubkey and moonlight-metadata
pitfalls from root CLAUDE.md don't apply. No two-`ContractId`-argument pattern.

**Medium**: `state.rs:29-32,49-64` — unguarded `+=` on `next_id`/`nonce`
(u64 wraparound, ~2^64 calls to matter — flagged for hardening, not urgency).

**Low**: unbounded total account count (permissionless `create_account` by
design — matches documented intent, flagged as tradeoff not bug).

---

## multisig-tool

No Critical/High/Medium beyond token-compare and KDF notes below. All 8
security-model claims this audit set out to verify were independently
confirmed true in code, not just docs: mainnet lockout is a hardcoded `const`
with no override; browser never receives secret key material (grepped every
handler); loopback binding is enforced pre-`TcpListener::bind` with no
flag/env bypass (and unlike the collector, `multisig-tool` has **no**
non-loopback escape hatch at all); bearer token applied via one shared
`route_layer` over the whole `/api/*` router with no route opting out, 32
bytes from `OsRng`, never logged; `MULTISIG_TOOL_ALLOW_ENV_PWD` gate requires
both env vars, no single-var fallback; digest recompute-before-sign is
genuine (`recompute_and_verify` always returns the *recomputed* digest, never
the caller-claimed one, on both CLI and RPC paths); AES-256-GCM keystore uses
fresh random salt+nonce every save, key material zeroized, file mode `0o600`.

**Medium**:
- `rpc.rs:221` — bearer-token compare is plain `==`, not constant-time.
- `keystore.rs:34` — PBKDF2-HMAC-SHA256 100,000 rounds; current OWASP
  guidance is ~600k+ for PBKDF2-SHA256. Relevant if protecting against
  realistic offline brute-force of a stolen `identities.dat`.

---

## multisig-collector

Explicitly untrusted-by-design (docs: "assume the collector can lie, omit,
reorder, append junk"). Verified it never exceeds that posture: no BLS/digest
verification anywhere (no `dusk_core` dep at all), only hex/length validation;
zero decisions affecting on-chain outcomes. Non-loopback bind gate
(`assert_bind_allowed`) is unconditional, unbypassable, unit-tested.
Party-roster is genuinely upsert-only — a `remove_party_member` fn exists in
`store.rs:298` but is dead code, no route calls it (confirmed by an existing
test asserting the delete route is gone). Same-`signer_pk` replace is
correctly scoped per-proposal (SQL `WHERE id = ?1`, no cross-proposal
contamination) — and confirmed **wide open by design**: any caller who knows
a signer's public PK can overwrite that signer's slot with no
caller-to-key binding at all. This is the documented tradeoff, but the docs'
"junk" framing undersells that it also allows *targeted* blanking of a known
signer's slot, not just random noise — still safe under the documented
invariant that on-chain quorum re-verifies everything, but worth naming
explicitly in the doc rather than leaving it implicit.

**Low**: `store.rs` — per-proposal caps exist (`MAX_PARTIALS=32`,
`MAX_NOTE_CHARS=512`, `MAX_BODY_BYTES=64KiB`) but no cap on total distinct
proposal/party rows — unauthenticated caller (if the reverse-proxy auth layer
is skipped/misconfigured) can grow the SQLite file unboundedly. Matches the
doc's own admission it's "an unauthenticated relay by design," but the binary
has no self-defense if the intended auth layer isn't there.

**Info**: dead `remove_party_member` fn — harmless, candidate for removal.
Licensing (AGPL-3.0-only + LICENSING.md) consistent with docs.

---

## multisig-encoding

No Critical/High. Canonical encoding correctness (length-prefixed
variable-length fields, tested adversarially against field-shifting
collisions), domain separation (every digest fn has a distinct versioned
tag), integer endianness (consistently LE, matches `_le64`/`_le32`
convention), and fingerprint collision-resistance (operates over the full
32-byte digest, no truncation) all confirmed correct.

**Medium**: `lib.rs:145-146` — `u32::try_from(...).expect(...)` panics rather
than returning `Result` if `function_name`/`call_args` exceed `u32::MAX`
bytes. `ProposalIntent` fields can originate from network-transported blob
data (collector), so a caller building an intent from untrusted input without
pre-validating length could crash the process. Impractical over the wire
(~4GiB payload) but worth converting to `Result`/documented precondition.

**Low**:
- `lib.rs:148-158` — `usize` capacity sum with no overflow guard; matters
  only on 32-bit targets under `overflow-checks=true`, not memory-unsafe.
- `fingerprint.rs:34,52-56` — wordlist-length invariant (2048 words) backed
  only by `debug_assert_eq!` (compiled out in release); if the bundled
  wordlist asset is ever edited, release build panics on OOB index instead of
  failing at build/init time. Not attacker-controlled today.

**Doc gaps noted** (not code bugs): no documented max length for
`function_name`/`call_args`; `target_contract_id`'s role as the
anti-cross-target-replay field isn't called out in-crate; `member_pks`
ordering requirement for `change_account_digest` is doc-stated but not
enforced/canonicalized.

---

## Prioritized action list

1. **pm-council-tool**: add server-side council-membership check in
   `api_blob_sign` / CLI Sign path before signing (Critical).
2. **pm-council-tool**: cross-check `pm_contract_id` against
   `live_pm_contract_id()` before submit, fail loud on mismatch (High).
3. **pm-council-tool**: surface `live_dispute_council` fallback-vs-live state
   to the UI instead of `eprintln!`-only; add a threshold staleness check at
   sign/submit (Medium, both).
4. Constant-time token comparison in `multisig-tool/src/rpc.rs:221` and
   `pm-council-tool/src/rpc.rs:139-143` (Low, cheap fix, do alongside #1
   since it's the same file).
5. `multisig-encoding`: convert the two length-prefix `expect()`s to
   `Result` (Medium).
6. Everything else in the table is Low/Info — batch whenever convenient, none
   block current testnet-only usage.
