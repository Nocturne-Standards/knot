*Point-in-time, frozen 2026-07-28. For the underlying findings see
`multisig/docs/security-audit-2026-07-28.md`; for current per-crate state see
each crate's own `README.md` and `multisig/docs/security-model.md`.*

# Security audit 2026-07-28 — fix plan

All findings re-verified against current source before planning. Every
cited file:line matched the audit's description except one nit under #6.

## Critical

### 1. Server-side membership gate missing (`pm-council-tool`)
**Fix**: `chain.rs` already has `live_dispute_council() -> Result<Option<(ContractId, u64)>>`,
and `api_council` (`rpc.rs:464-495`) already chains it into a
`chain::query::<Option<MultisigAccountView>>("account", ...)` call to get
`.members`. Factor that two-step lookup into a reusable
`chain::live_council_members() -> Result<Vec<BlsPublicKey>>` and call it in
`api_blob_sign` (`rpc.rs:555-591`) right after resolving `identity.pk`,
before `blob::add_partial` — return `403` ("signer PK is not a member of
the live dispute council") on no-match. Apply the same check in CLI
`Cmd::Sign` (`main.rs:346-409`, both the `--id` and `--file` branches)
before `blob::add_partial`, via one shared `blob::require_council_member(pk, &members)`
helper so RPC and CLI can't drift apart.
**Blast radius**: `rpc.rs` (`api_blob_sign`), `main.rs` (`Cmd::Sign`),
`chain.rs` (new helper), `blob.rs` (shared check). Off-chain tooling only —
rebuild + restart the local tool, no contract/redeploy impact.
**Test plan**: unit-test the new `blob::require_council_member` helper
directly (member PK passes, non-member rejected). If `rpc.rs` handlers are
mockable, add an integration test stubbing `live_dispute_council`/
`MultisigAccountView` to assert `api_blob_sign` 403s for a non-member.
**Risk**: if `live_dispute_council` itself is on the stale-fallback path
(#4), the "live" member set used here may be stale, so a real new member
could be wrongly rejected right after an on-chain `change_account`. Fail
closed is still correct, but log both facts so it's diagnosable.

## High

### 2. Submit target not cross-checked against live contract (`pm-council-tool`)
**Fix**: in `api_blob_submit` (`rpc.rs:606-629`), before calling
`chain::submit_call_to_contract_id`, call `chain::live_pm_contract_id()`
(already used at `rpc.rs:465`) and compare case-insensitively against
`file_blob.intent.pm_contract_id`. On mismatch return `409` naming both ids
("blob targets stale PM contract {stale}, live is {live}") rather than
submitting — hard-fail per the audit's own framing, not warn-only.
**Blast radius**: `rpc.rs` (`api_blob_submit` only). Off-chain, no contract
change.
**Test plan**: test with a `BlobFile.pm_contract_id` deliberately differing
from a stubbed `live_pm_contract_id()`; assert the error returns before
`submit_call_to_contract_id` runs.
**Risk**: `live_pm_contract_id()` has its own env→Atlas→file fallback chain
and is `OnceCell`-cached per process; a stale cache entry from early
process start could hard-fail a submit that's actually targeting the
correct contract. Log both ids on mismatch so "blob is stale" is
distinguishable from "live resolution is stale."

## Medium

### 3. No live threshold re-check before sign/submit (`pm-council-tool`)
**Fix**: `Cmd::Status` already fetches `MultisigAccountView` and checks
`.members`; extend it to also compare `.threshold` vs `file_blob.threshold`
and warn on mismatch. For sign/submit, add a non-blocking
`chain::warn_if_threshold_stale(&file_blob)` call (reusing the same
`MultisigAccountView` query) right before signing/submitting — log/return a
warning, not a hard error, since on-chain verify remains the actual
correctness boundary.
**Blast radius**: `blob.rs`/`chain.rs` (new helper), `rpc.rs` (`api_blob_sign`,
`api_blob_submit`), `main.rs` (`Cmd::Status`, `Cmd::Sign`). Off-chain only.
**Test plan**: unit test the compare helper (mismatch → warns, match →
silent).
**Risk**: same staleness caveat as #2 — keep as a warning, not a hard
block, so it doesn't compound with #2's stricter failure.

### 4. RPC-error vs. getter-missing collapsed in `live_dispute_council` (`pm-council-tool`)
**Fix**: in `chain.rs:181-191`, only fall back to
`dispute_council_from_deployments()` when the error genuinely indicates
"method not found" on this WASM; propagate other errors (network/5xx/decode
failure) as `Err` instead of silently swallowing them. Add a
`source: "live" | "fallback"` field to `CouncilView` (`rpc.rs:464-495`) so
`static/pm-council-app.js` can render a visible banner instead of relying
on server-only `eprintln!`.
**Blast radius**: `chain.rs` (`live_dispute_council`), `rpc.rs`
(`CouncilView`), `pm-council-app.js` (small UI change). Off-chain only.
**Test plan**: stub a "method not found" error (falls back silently) vs. a
network error (propagates / marks `source: fallback` with a reason).
**Risk**: reliably distinguishing "method missing" from "network error"
depends on what signal the RPC client actually exposes today — check that
first. If a clean split isn't available, threading the existing error
string into `CouncilView` (without a typed enum) still satisfies "not
invisible to the UI."

### 5. `u32::try_from(...).expect(...)` panics (`multisig-encoding`)
**Fix**: `proposal_preimage` (`lib.rs:136-169`) and its callers
`ProposalIntent::digest`/`preimage_bytes` (`lib.rs:107-133`) currently
return infallibly. Change `proposal_preimage` to return
`Result<Vec<u8>, EncodingError>` (add a minimal `EncodingError` if none
exists) with a `FieldTooLarge { field, len }` variant, replacing both
`.expect()`s at `lib.rs:145-146`. This is a breaking signature change.
**Correction from my own re-verification (the plan's original blast-radius
list was wrong):** the real caller graph, by actual grep of
`proposal_digest`/`proposal_preimage`/`ProposalIntent::digest`/
`preimage_bytes`, is:
- **`multisig-proposals`** — calls `proposal_digest(...)` directly from
  contract code, on-chain, in `state.rs:145` (inside `propose`). This is
  the one real on-chain caller.
- **`multisig-tool`** — `blob.rs` and `mock_ledger.rs` call
  `ProposalIntent::digest()`/`preimage_bytes()`. Off-chain only.
- **NOT `multisig-registry`** — it calls a different function,
  `change_account_message`, unaffected by this change.
- **NOT `multisig-collector`** — no `multisig_encoding` dependency at all
  (confirmed: no import anywhere in its `src/`).
- **NOT `pm-council-tool`** — only uses the fingerprint display helpers
  (`digest_hex`/`digest_mnemonic`/`digest_safety_number` in `blob.rs:200-207`),
  which don't call `proposal_preimage` and aren't touched by this fix.
**Blast radius**: `multisig-encoding/src/lib.rs`, plus call-site updates in
`multisig-proposals/src/state.rs:145` and `multisig-tool/src/{blob.rs,mock_ledger.rs}`.
Grep both call sites before considering this done (root CLAUDE.md's
change-propagation rule) — no other crate needs touching.
**Test plan**: regression-test that in-bounds inputs still produce
identical digest bytes after the signature change; unit-test the new error
path via the isolated length check (constructing an actual 4 GiB payload
isn't practical).
**Risk**: encoded bytes/digest are unchanged for all in-bounds inputs, so
on-chain behavior is identical — but because `multisig-proposals` calls
`proposal_digest` directly from contract code, landing this **does**
require `make wasm && make test` in `multisig-proposals/` and, since it's
already deployed (testnet v0.3.0 per the suite README), **a testnet
redeploy** to actually ship. `multisig-registry` is unaffected and needs no
rebuild for this specific finding.

### 6. Unguarded `+=` on `next_id`/`nonce` (`multisig-registry`)
**Verification note**: audit cites `state.rs:29-32,49-64`. The struct
fields (29-32) and `next_id += 1` in `create_account` (line 53) are within
that range. `nonce += 1` is actually in `change_account` at **line 222**,
outside the cited range — same bug, slightly off line pointer.
**Fix**: `self.next_id += 1` → `self.next_id = self.next_id.checked_add(1).expect("next_id overflow")`;
`account.nonce += 1` → `account.nonce = account.nonce.checked_add(1).expect("nonce overflow")`.
Panic-on-overflow matches this codebase's existing idiom (`multisig-proposals`
already uses `checked_add(...).expect(...)` for `deadline`).
**Blast radius**: `multisig-registry/src/state.rs`, two one-line changes.
**On-chain contract** — requires `make wasm && make test` in
`multisig-registry/`.
**Test plan**: existing suite as regression; add a test constructing
`nonce: u64::MAX` directly (if the harness allows) asserting panic instead
of wraparound — wraparound is the actual exploitable replay risk.
**Risk**: negligible; only changes behavior at the u64 boundary.

## Low — dispositions

### 7. Constant-time token comparison (`multisig-tool/src/rpc.rs:221`, `pm-council-tool/src/rpc.rs:139-143`)
**Fix now**, batched together — identical fix in two crates. Both
`require_token` middlewares do `v == state.token`. Neither crate depends on
`subtle` today. Add `subtle = "2"` to both `Cargo.toml`s and replace the
comparison with `ConstantTimeEq::ct_eq(v.as_bytes(), state.token.as_bytes()).into()`,
guarding length mismatch first (only leaking length, not content, is
acceptable).
**Blast radius**: one `require_token` fn + one dep per crate. Off-chain.
**Test plan**: existing auth tests (right/wrong token → pass/401) should
pass unchanged; timing itself isn't unit-testable, rely on review that
`ct_eq` preconditions are met.
**Risk**: none — identical pass/fail semantics.

### 8. PBKDF2-SHA256 100k rounds (`multisig-tool/src/keystore.rs:34`)
**Fix now** — in scope (audit's out-of-scope note is for
`pm-council-tool`'s keystore, not `multisig-tool`'s). Bump `PBKDF2_ROUNDS`
(line 34) from `100_000` to `600_000` (current OWASP floor). This breaks
decryption of existing `identities.dat` files encrypted under the old
round count. Two options: (a) store the round count in the file header,
defaulting missing values to `100_000`, and re-save transparently on next
password entry; (b) just bump the constant and require existing stores to
be recreated (reasonable given "testnet-only, still in dev"). Flag the
choice to the user before implementing — don't assume (b).
**Blast radius**: `keystore.rs` only.
**Test plan**: round-trip test with new constant; if (b), assert a clear
"re-create your identity store" error rather than silent garbage decrypt.
**Risk**: (b) locks out existing local stores without migration — the error
message must be explicit.

### 9. No cap on total proposal/party-roster row count (`multisig-collector/src/store.rs`)
**Defer.** Per-proposal caps already exist (`MAX_PARTIALS=32`,
`MAX_NOTE_CHARS=512`, `MAX_BODY_BYTES=64KiB`); a global row cap is
resource-exhaustion hardening for a misconfigured-reverse-proxy scenario
the docs already call the operator's responsibility, not a correctness
fix. Sketch for later: `MAX_TOTAL_PROPOSALS`/`MAX_TOTAL_PARTY_ROWS`
constants, checked via `COUNT(*)` in `create_proposal` (`store.rs:120`) and
`upsert_party_member` (`store.rs:247`) before insert.

### 10. Unbounded proposal count, no pruning (`multisig-proposals/src/state.rs:117-188`)
**Defer**, per the audit's own framing — per-field sizes are capped
already; pruning a permissionless-propose design raises unresolved
questions (who prunes, what happens to in-flight approvals) better handled
as a lifecycle/fee design discussion, not a standalone patch.

### 11. Unbounded total account count (`multisig-registry`)
**Won't-fix** — confirmed intentional (permissionless `create_account`,
audit itself labels this a documented tradeoff). No action.

### 12. `usize` capacity-sum overflow on 32-bit targets (`multisig-encoding/src/lib.rs:148-158`)
**Defer.** Only reachable on 32-bit targets with `overflow-checks=true`;
inputs are already bounded elsewhere (`MAX_FUNCTION_NAME_LEN`/
`MAX_CALL_ARGS_LEN` in `multisig-proposals`) so practical overflow can't
occur on this workspace's actual targets (`wasm32-unknown-unknown`,
x86_64/aarch64 hosts). Cheap to fold `checked_add` in alongside #5 if
already touching this function; not worth a standalone change.

### 13. Wordlist-length invariant only `debug_assert_eq!` (`multisig-encoding/src/fingerprint.rs:34,52-56`)
**Fix now** — cheap, closes the stated release-build OOB-panic risk.
Promote both `debug_assert_eq!` (word count at line 34, index count at line
58) to real `assert_eq!` so a release build fails loudly rather than
OOB-panicking deeper in `words[*idx]` (line 66) with a worse message. Also
add a `#[test]` asserting `wordlist().len() == 2048` against the bundled
`include_str!` asset, for CI-time detection before release.
**Blast radius**: `fingerprint.rs` only, 2-line change + 1 test.
**Test plan**: the new test itself.
**Risk**: none — behavior unchanged for the current (correct) wordlist.

### 14. Dead `remove_party_member` fn (`multisig-collector/src/store.rs:298`)
**Fix now** — confirmed dead: only called from its own tests
(`store.rs:684,687`), no `rpc.rs` route reaches it. Delete the fn and its
two test call sites (or convert them to test the underlying delete SQL
directly if that coverage is worth keeping). Confirm with the user before
deleting per repo convention (ask before removing things).
**Blast radius**: `store.rs` only.
**Test plan**: `cargo test -p multisig-collector` passes after removal.
**Risk**: none.

## Work batches

**Batch A — pm-council-tool Critical/High/Medium** (#1, #2, #3, #4, #7's
pm-council-tool half). Same crate, overlapping files (`rpc.rs`/`chain.rs`).
Off-chain — rebuild + restart the local tool only.

**Batch B — multisig-encoding** (#5's `Result` conversion, #12's
`checked_add` bundled in, #13's wordlist assert). #5 changes an API
`multisig-proposals` calls directly from contract code (verified:
`state.rs:145`) — this batch **requires `make wasm && make test` in
`multisig-proposals/` and a testnet redeploy**, plus updating the two
off-chain call sites in `multisig-tool` (`blob.rs`, `mock_ledger.rs`).
`multisig-registry` is **not** affected (it uses `change_account_message`,
a different function) and needs no change or redeploy for this batch.

**Batch C — multisig-tool constant-time compare** (#7's multisig-tool
half). Separate crate/binary from Batch A but identical fix — fine to land
in the same PR sequence as its own commit.

**Batch D — multisig-registry on-chain fix** (#6). Independent of Batch B
— confirmed `multisig-registry` does not call the function Batch B
changes, so these two batches don't need sequencing relative to each
other. **Requires `make wasm && make test` in `multisig-registry/` and a
real testnet redeploy** — treat as a real-consequence action, not
local-only.

**Batch E — deferred/low-priority cleanup** (#8 keystore rounds — needs a
migration-strategy decision first; #14 dead-fn removal — ask before
deleting). Independent, no urgency.

**Not batched**: #9, #10 (deferred, design discussion), #11 (won't-fix).

## Redeploy summary

- **Requires testnet contract redeploy**: Batch D (`multisig-registry`,
  for #6) and Batch B (`multisig-proposals`, for #5 — confirmed direct
  on-chain caller of the changed function). These two are independent of
  each other; neither blocks the other.
- **Local rebuild + restart only**: Batch A (`pm-council-tool`), Batch C
  (`multisig-tool`), Batch E, and the `multisig-tool`/`multisig-encoding`
  off-chain-only portions of Batch B (`blob.rs`, `mock_ledger.rs`,
  `fingerprint.rs`).
