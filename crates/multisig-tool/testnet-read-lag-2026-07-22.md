# multisig-registry testnet investigation — 2026-07-22

Point-in-time, frozen 2026-07-22 (evening wrap-up). **Corrected conclusions:**
the two “open testnet bugs” below were **client mistakes** — (1) hex-encoded
RUES request bodies (wrong `u64` id → genuine `Option::None`); (2) tool
used `sign_insecure` to match `VM::ephemeral()` while live testnet requires
secure `sign()`. For current state see [README.md](README.md).

## Resolved (supersedes “open / escalate upstream” below)

| Filed as | Actual cause | Fix |
|---|---|---|
| Problem 1: `account` always `not found` | Hex-encoded RUES body treated as raw rkyv | [`src/chain.rs`](src/chain.rs) raw `octet-stream` |
| Problem 2: `change_account` quorum never met | Insecure BLS on post-Aegis testnet | [`src/bls.rs`](src/bls.rs) `sign` / `sign_multisig` |
| Problem 3: free-read `verify_quorum*` → 500 | RUES + `abi::verify_bls` query path | Documented caveat only |

Do not escalate Problem 1/2 as node bugs. Historical narrative retained below
for the investigation trail.

---

Two independent problems were found in the same investigation session,
against the same fresh deployment of `multisig-registry`
(contract id `17af5a3b05f09990bc4d7e275225550e1ed2d49cbc1defeca4e7fa609789964d`,
deployed 2026-07-22T12:59:10Z):

1. **`account` reads are stuck reporting `not found` for state that
   demonstrably exists.**
2. **`change_account`'s quorum check fails for every signer/account
   combination tested, including maximally simple, guaranteed-correct
   cases.**

They were investigated together because the second was only discoverable
by working around the first, but they are separate bugs with separate
likely causes. Both are still open.

---

## Problem 1: `account` query stuck on `not found`

### Symptom

`account query <id>` returns `not found` for every account created against
this deployment — 20 retries over ~200s immediately after creation, and
still `not found` on a recheck 1h20m later. No recovery observed at any
point.

### What was ruled out, in order

| # | Hypothesis | Test | Result |
|---|---|---|---|
| 1 | Writes aren't actually landing | `create_account` txs propagate, wallet balance debited gas, both visible **confirmed** on the testnet explorer | ❌ ruled out — writes land |
| 2 | Client-side decode bug in `multisig-tool` | Read `src/chain.rs`/`main.rs`: response decodes to a genuine `Option::None`, not a swallowed parse error | ❌ ruled out — not a client bug |
| 3 | Testnet node / RUES endpoint down entirely | Queried `security-token.name()` (settled since 2026-07-13) over the identical RUES path — returned `"Security Token"` correctly | ❌ ruled out — node reads work in general |
| 4 | Universal "fresh writes take a while to become readable" | Wrote `security-token.set_spender` for a throwaway address, polled `is_spender_allowed` | ❌ ruled out — resolved `true` on the **first** check, ~10s later |
| 5 | Our own HTTP stack (`reqwest`) mishandling the request | Captured the exact request bytes `multisig-tool` sends, replayed with plain `curl` — different process, different HTTP client entirely | ❌ ruled out — identical response, independent of our stack |

### The decisive test: does the account exist in real execution state?

`multisig-registry` has only one read function (`account`); there's no
second, independent query to cross-check against. But `change_account` is
diagnostic in a different way: its on-chain logic panics with one specific
message if the account is truly absent
(`"no such multisig account"`, from `.get(&id).unwrap_or_else(...)`), and a
**different** message if the account exists but the quorum signatures don't
check out (`"change_account: quorum not met by current members"`).

Its CLI wrapper normally reads current state first (to fetch the account's
`nonce`) — blocked by the same broken read. Worked around this, for
diagnostic runs only, by hardcoding `nonce=0` locally (true by construction
for a never-yet-changed account) instead of fetching it, so the transaction
could still be built and submitted. Each time, the code was reverted
immediately after and confirmed clean via `git diff` before moving to the
next test.

**Result:** every `change_account` submission against every account tested
(three separate accounts, see Problem 2) hit `"quorum not met by current
members"` — never `"no such multisig account"`. That message is only
reachable after `self.accounts.get(&account_id)` already returned
`Some(account)` inside real on-chain execution.

### Conclusion — Problem 1

**The accounts genuinely exist in committed chain state.** The bug is
isolated to exactly one thing: the free-read path
(`POST /on/contracts:<id>/account`) for this specific contract's `account`
function. Not the node in general, not the RUES protocol, not our client,
not the contract's actual committed state, and not a universal
write-then-read latency property of this testnet.

### Status

Rechecked at 18:27, ~1h20m after the original writes — still `not found`
on every index tested. This is well past the "minutes" window a
previously-documented caveat in `README.md` described for this kind of
read recovering. No longer plausible as ordinary transient lag; treat as
stuck pending upstream investigation, not something to keep waiting out.

### Recommended next step

Escalate as a testnet/node bug report. The repro is tight and fully
independent of our code:
- Contract: `multisig-registry`, function `account`, `POST
  /on/contracts:<id>/account` with a `u64` account id.
- The queried state is confirmed to exist by three independent signals:
  the testnet explorer, gas debited from the wallet, and
  `change_account`'s own execution reaching its quorum-check panic branch.
- An identical write→read pattern against a different, long-settled
  contract (`security-token`) resolved correctly in ~10 seconds over the
  same RUES path, same node.

---

## Problem 2: `change_account`'s quorum check never succeeds

### How this was found

Discovered as a side effect of the Problem 1 diagnostics above — every
`change_account` submission failed quorum, even in cases engineered to be
unambiguously correct. This is a materially different bug from Problem 1:
it's about whether a real, valid quorum of signatures is ever accepted by
the contract's execution logic, not about whether state can be read back.

### Tests run (all via the same temporary `nonce=0` bypass, reverted after each)

| Account | Members (as created) | Threshold | Signers used | Signing scheme | Result |
|---|---|---|---|---|---|
| index 1 | alice, bob | 2 | alice, bob | insecure (v1, default) | `quorum not met` |
| index 0 | *(pre-existing, from an earlier session)* | ? | alice, bob | insecure | `quorum not met` |
| index 2 | dave (sole member) | 1 | dave | insecure | `quorum not met` |
| index 2 | dave (sole member) | 1 | dave | **secure v2** (`sign`/`sign_multisig`) | `quorum not met` |
| index 3 | carol (sole member) | 1 | carol | insecure | `quorum not met` |

**Every single test failed identically**, regardless of account, identity,
or signing scheme.

### Hypotheses tested and ruled out

- **Wrong account index (testing against an account whose real members
  don't match the signers used).** Ruled out for indices 2 and 3: both were
  created fresh, immediately before testing, with a single known member
  and threshold 1 — no ambiguity about who the members are.
- **Stale/corrupted local identity (`dave` predates this session's
  keystore, from before it was overwritten by an earlier web-UI test run
  under a different password).** Ruled out by repeating the test with
  `carol`, an identity created fresh in this exact session with no prior
  history — same failure.
- **Wrong BLS signing scheme (pre-hardfork `sign_insecure` vs. the
  post-hardfork secure `sign`, per the real concern documented in
  `references/dusk-native/dusk-vm-issue-1-ephemeral-hardfork-policy-unreachable.md`
  — real nodes derive hardfork policy from actual chain height, which may
  have moved past `PreFork` since that caveat was written).** An earlier
  attempt to flip `bls.rs` to `sign()` looked identical in the panic
  string alone and was wrongly ruled out. **Reopened and confirmed
  2026-07-22 evening** with panic counters (`member_matches=1, sigs_ok=0`
  under insecure) and a successful secure `change_account` on the 0.1.1
  redeploy — see Follow-up section. The false negative was matching the
  `VM::ephemeral()` test suite's `_insecure` workaround in the live tool.
- **Mismatched argument encoding between `multisig-tool` and the contract
  (`ChangeAccountArgs`, `SignatureEntry`, etc.).** Ruled out structurally:
  `multisig-tool/src/registry_types.rs` includes
  `multisig-registry/src/call_types.rs` directly via `#[path = ...]` — it is
  literally the same source file compiled into both the contract and the
  client, not a hand-copied duplicate. The struct layouts cannot diverge.
- **Mismatched message-hash construction (`change_account_message` in
  `bls.rs` vs. the contract's private `change_message` in
  `multisig-registry/src/state.rs`).** This one *is* a hand-maintained
  duplicate (the contract's version is intentionally private). Compared
  line-by-line: same domain tag (`"sme-platform.multisig-registry.change_
  account.v1"`), same field order (account id → nonce → each member's
  compressed bytes → threshold), same `to_le_bytes`/`to_bytes` encoding.
  No divergence found.

### What's left standing

The remaining, unconfirmed hypothesis is narrower and harder to verify
without more tooling: `quorum_met`'s member-matching step
(`members.contains(&entry.signer)`) happens *before* signature
verification — if a signer's `PublicKey` value doesn't byte-for-byte match
what's actually stored in the account's on-chain `members` list (e.g. due
to some asymmetry in how `PublicKey` round-trips through rkyv between the
value used at `create_account` time and the value compared against at
`change_account` time), every signer would be silently skipped
(`continue`), producing exactly the observed symptom — "no valid
signatures counted" — without any individual signature actually being
checked as wrong. This has not been directly confirmed: doing so would
require either working `account` reads (circular — that's Problem 1) or a
lower-level raw dump of the contract's on-chain byte state, which wasn't
attempted this session.

### Conclusion — Problem 2

**Status: open, unresolved.** Four independent test configurations, across
three accounts, three identities, and two signing schemes, all failed the
same way. Index confusion, keystore corruption, hardfork/signing-scheme
mismatch, and struct-encoding mismatch have all been directly ruled out.
The leading remaining candidate is a `PublicKey` round-trip/comparison
mismatch inside `quorum_met`, but this is inference from elimination, not a
confirmed root cause.

**Practical impact:** `change_account` cannot currently be considered
working end-to-end against this deployment. This also means `README.md`'s
previous "confirmed empirically" language about the signing scheme was
overstated — it only ever confirmed transaction *propagation*, not that
`verify_quorum`/`verify_quorum_aggregate`'s actual boolean result was ever
`true` on real chain (their result isn't observable: no event emitted, no
return value surfaced by a plain transaction submission). It's possible
`verify_quorum`/`verify_quorum_aggregate` share this same underlying issue
and have never actually returned `true` on testnet either — this hasn't
been checked and is a natural next step.

### Recommended next steps

- Build a minimal raw-state inspection path (bypassing `dusk_core::abi`
  entirely) to dump the actual bytes `multisig-registry` has stored for
  a `members` entry, and compare byte-for-byte against the locally-held
  `PublicKey` used to sign — the most direct way to confirm or refute the
  round-trip-mismatch hypothesis.
- Alternatively/additionally, write a `VM::ephemeral()` test in
  `multisig-registry`'s own test suite that mirrors this exact scenario
  (create account, sign `change_account`'s message, submit) to see whether
  it reproduces in the deterministic in-memory VM — if it does, this is a
  contract or shared-crate bug, not anything testnet-specific, and much
  cheaper to iterate on there than against real testnet transactions.
- Do **not** treat `change_account` (or, by extension, `verify_quorum`/
  `verify_quorum_aggregate`'s actual pass/fail correctness) as confirmed
  working until one of the above lands.

---

## Session bookkeeping

- All diagnostic code edits (in `multisig-tool/src/main.rs`, `src/bls.rs`,
  `src/chain.rs`) were temporary, reverted immediately after each test, and
  confirmed clean via `git diff` before moving on. No diagnostic code
  shipped.
- Test accounts created this session (all real testnet transactions, real
  gas spent): index 1 (alice+bob, threshold 2), index 2 (dave, threshold
  1), index 3 (carol, threshold 1). All three exist in real chain state per
  the Problem 1 findings above, none currently readable via `account`.
- `deployments/testnet.json`'s `security-token` entry gained one
  additional `set_spender` call (the Problem 1 control test), recorded
  under its normal wiring-history mechanism.

---

## Follow-up hunt — 2026-07-22 evening

Reproduced and extended against the same deployment
(`17af5a3b…964d`). Tooling additions kept in-tree: `quorum check` /
`quorum-agg check` (free-read bool) and `change-account --nonce` bypass.

| Probe | Result |
|---|---|
| `VM::ephemeral()` suite | 11/11 still green (+2 wire probes) |
| `security-token.name` RUES control | OK (`"Security Token"`) |
| `account` free-read ids 0–6 | always `None` / tool `not found` |
| Raw RUES body for `account` | exactly 32 zero bytes |
| rkyv `Option<MultisigAccountView>::None` | **also exactly 32 zero bytes** — so the node is returning a genuine `None`, not garbage |
| `create_account` (carol 1-of-1, alice+bob 2-of-2) | txs propagate; ids **4** and **5** exist per `change_account` panic branch |
| `change_account --nonce 0` carol@4 (her own 1-of-1) | `quorum not met` (insecure **and** secure v2) |
| `change_account --nonce 0` alice+bob@5 | `quorum not met` |
| Free-read `verify_quorum` / `verify_quorum_aggregate` | **HTTP 500** on every id (new finding vs submit-tx path) |
| Host-side PK rkyv round-trip + `Vec::contains` | passes — pure rkyv mismatch not reproduced off-chain |
| `serve` RPC | token gate 401/200 OK; `/api/account/4` → `null` (same Problem 1) |

### Sharpened conclusions

1. **Problem 1 is a true free-read `None`**, not a client decode artifact.
   Execution state still has the accounts (`change_account` reaches quorum
   panic, never `no such multisig account`). On the 0.1.1 redeploy,
   `next_account_id` *is* visible over RUES (advances after create) while
   `account` / `account_meta` / `member_key_bytes` still return `None` —
   state is partially readable; lookups that touch the members map are not.
2. **Problem 2 was the BLS signing scheme** — not a PublicKey `contains`
   mismatch. Redeploy `diagnose`/panic counters on insecure signatures:
   `member_matches=1, sigs_ok=0`. Switching `multisig-tool` to post-Aegis
   secure `sign()` made `change_account` succeed (nonce bumped; stale
   nonce-0 replay then fails verify as expected). Documented earlier in
   `references/dusk-native/dusk-vm-issue-1-ephemeral-hardfork-policy-unreachable.md`
   and proven live for RFQ in `rfq-settlement/README.md`; the earlier
   "secure also failed" row in this note was a false negative (tool still
   on `_insecure`, or wrong account). **`VM::ephemeral()` tests must keep
   `_insecure`; the live tool must not.**
3. **Problem 3:** free-read paths that invoke `abi::verify_bls` /
   `verify_bls_multisig` (and `diagnose_quorum`) return node **HTTP 500**,
   so the observable bool/counters for those still need the
   `change_account` panic string (or a future non-verify diagnostic).
