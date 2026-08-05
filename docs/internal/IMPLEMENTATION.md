# Knot — implementation truth

**Written against `276b19e`** (residual audit HEAD; settle pass began at `46a64b4`).
Settled-gap + §10 phasing + §11 residual findings 2026-08-05. §2.5 chain_id
ephemeral probe confirmed 2026-08-05 (leaf #2).

**This file is authoritative.** Where it disagrees with `AUDIT-2026-08-05.md`,
this file wins — the audit was written against `2fb3c94` and is frozen evidence,
not instructions. See `README.md` in this directory for the precedence rule.

Status key: **LOCKED** — decided, specified, ready to implement ·
**OPEN** — discussed but not yet specified; do not implement ·
**DEFERRED** — decided to postpone; listed so it cannot vanish.

As of 2026-08-05 every item in §1–§5 is LOCKED. §8 is the decision log.
§10 is execution phasing (no leaves until that section is accepted).

---

## 0. How to use this document

§1–§3 are implementable as written, as are **§4.2 (collector) and §4.3 (registry)**,
which are marked **LOCKED**. §8 is the decision log. §9 is product scope. **§10 is
execution phasing** — accept it before cutting leaves.

**Read the per-item status marker, not the section number.** Anything marked **LOCKED**
is specified and ready. Anything marked **OPEN** is agreed in direction but unspecified
and *will* change — do not implement from it. **DEFERRED** means decided to postpone
and must stay on the checklist.

Verified facts an implementer can rely on (checked at `7e58d4c`):

| Claim | Evidence |
|---|---|
| `abi::self_id()` exists | `dusk-core` 1.6.0 `src/abi.rs:63`, re-export from `piecrust_uplink`, feature `abi` |
| `abi::chain_id() -> u8` exists, panics if metadata unset | `src/abi.rs:187-194` |
| `abi::keccak256(Vec<u8>) -> [u8;32]` is a host query | `src/abi.rs:153` |
| `abi::caller()`, `abi::callstack()` exist | `src/abi.rs:62` |
| Domain tags are already `nocturne.knot.*.v2` | `crates/knot-encoding/src/lib.rs:69,72` |
| `council_resolve` is fully removed from encoding | `grep -c council_resolve` = 0 |
| `finalize` has **no** caller/membership check | `crates/knot-proposals/src/state.rs`, whole fn |
| Collector SQL is fully parameterized | `store.rs` — all queries are literals with bound params |
| Lab escapes HTML at every string sink (`escapeHtml` / `textContent`) | `app.js` — §11 R9 (2026-08-05) |
| Tool API token compares in constant time | `rpc.rs:158-166`, `ct_eq` |
| `rusk-wallet` is invoked with argument arrays, no shell | `chain.rs:290-299` |

---

## 1. Blockers — must clear before the repo is public

### B1 — credential in README and git history · LOCKED

`crates/knot-tool/README.md:38`

```
export KNOT_COLLECTOR_PASSWORD='navHam-cemnib-4pytja'
```

Paired with `KNOT_COLLECTOR_USER=demo` for `collector.nocturne-standards.org`.
In history since 2026-07-24.

1. Leonidas rotates the credential on the collector host. **Assume it is public.**
2. Replace with `...` in the README.
3. History is handled by the launch squash (§5.4) — no `filter-repo` needed.

### B2 — testnet wallet password · LOCKED

`crates/knot-tool/README.md:30` — `RUSK_WALLET_PWD=sme-platform-testnet-dev`.
Also `:33` and `:358` — `KNOT_PWD='local-dev-only'`.

Accepted risk until launch. Replace all three with `...`.

### B3 — local paths and internal tooling · LOCKED

`.nocturne-agent-kit-adopted` contains `/Users/leonidas/dev/aichbindas/nocturne-agent-kit`
twice. `.cursor/mcp.json` contains paths to `nocturne-mcp-gates` and
`nocturne-mcp-tracks`. Both tracked. Removal list in §5.1.

### B4 — internal documents · LOCKED

**Decision: publish no internal document.** No plans, audits, specs, tracks, or
fix-plans. `docs/` on `7e58d4c` holds **46 tracked files**, most of which are
internal. Full disposition in §5.1.

### B5 — private git dependency blocks public builds · LOCKED

`crates/knot-tool/Cargo.toml:45`

```toml
nocturne-deployments = { git = "https://github.com/aichbindas/nocturne-deployments", rev = "d40ab40…" }
```

The repo's own CI notes it needs *"a PAT with contents:read on private
aichbindas/nocturne-deployments."* Nobody outside the org can `cargo build`
`knot-tool` — the fetch fails before compilation.

**Decision: make `nocturne-deployments` public, AND make the dependency optional
(off by default).**

Publishing alone fixes the build. Two caveats drove the second half:

- **Git dependencies cannot be published to crates.io.** Crates are `publish = false`
  today, so this does not bite yet, but a git dep forecloses `cargo install knot-tool`
  permanently.
- **Rev-pinned git deps get no semver and no dependabot.** The pin must be bumped by
  hand on every redeploy that changes a contract ID.

What the dependency actually is matters here: used in exactly **one** function
(`chain.rs:39`) to load a JSON file of contract IDs. It is *deployment data, not
logic*, and data that changes on every redeploy. The better mechanism already exists —
the `NOCTURNE_DEPLOYMENTS` env var and a sibling-path fallback (`chain.rs:38-47`).

Target: default build fetches nothing and reads pins via `NOCTURNE_DEPLOYMENTS`;
`--features deployments-crate` opts into the crate for internal convenience. This
decouples "can strangers build Knot" from "does the pin repo still exist".

---

## 2. Contracts — `knot-proposals` v3 and the digest redesign

Resolves H1, H2, M1, M2, M3, L1, L2, L13. Contracts are immutable: this is a
**redeploy**, and these changes land together or not at all.

### 2.1 Why the v2 rename did not fix H1/H2

`feat/launch-form-knot` renamed the domains to `nocturne.knot.*.v2`. That fixed the
branding leak and retired every v1 signature. It did **not** change *which fields
are hashed*. Two deployments of the v2 code still produce identical digests for
identical intents.

**Renaming the tag versions the format. Adding `self_id` scopes the instance.**
Different problems. A domain tag is a compile-time constant, identical in every
deployment, so it can distinguish *message kinds* but never *contract instances*.
Instance scoping requires a runtime value in the hashed fields.

Flat preimage chosen over EIP-712-style nested separators — cryptographically
equivalent, smaller diff against the existing streaming concatenation. Note the
choice in public docs; reviewers from Ethereum will expect the nested form.

### 2.2 H1 — proposal digest, concretely

Committee 7 at nonce 5 authorizes `set_fee(100)` on target T, deadline 1000. Alice
and Bob sign digest D on proposals contract `P_old`.

You redeploy to `P_new` (done three times: v0.3.0 → v0.3.1 → v0.3.2). Committee 7
starts at nonce 0. After five ordinary finalizes it reaches nonce 5. An attacker —
`propose` is permissionless — calls `propose` with the same target, function, args
and deadline. The contract computes the **same digest D**. They then call `approve`
twice with Alice's and Bob's harvested signatures, which are public: in the old
contract's state, in the collector, in any blob file. Quorum met. Neither member acted.

The deadline bounds the window — which is exactly what **M1 removes**, since
deadlines are attacker-controlled and uncapped. H1 and M1 compose.

Simpler variant, no waiting: two proposals contracts live simultaneously against one
registry make signatures fungible between them.

### 2.3 H2 — change_account digest, concretely

Verified at `7e58d4c`, `crates/knot-encoding/src/lib.rs:81-99`:

```
DOMAIN_CHANGE_ACCOUNT_V2 || account_id || nonce || pk₀..pkₙ || threshold
```

No `chain_id`, no registry identity.

Testnet registry account **0**, nonce **0**, alice + partner, threshold 2 — live
today. Deploy the mainnet registry; its first `create_account` is account **0**,
nonce **0**. Same members in, and a `change_account` quorum signature harvested from
testnet applies on mainnet **immediately, with no waiting**, rewriting the mainnet
committee. Fix this first.

### 2.4 L13 — the missing member count

`change_account_digest` concatenates member keys with no count prefix. **This is not
currently a bug**: every field is fixed-width (96-byte keys, 4-byte threshold
suffix), so the parse is unique and no collision exists.

The problem is that the safety rests on an invariant living *outside* the function.
Add one variable-length field later — a label, a varint threshold, a shorter key
encoding — and the format becomes ambiguous with no test failing. Since v3 is being
cut anyway, add `member_count` and retire the fragility.

### 2.5 chain_id must be derived · LOCKED

Replace the `chain_id` state field and `init_chain_id` with `abi::chain_id()`.
Returns `u8`; widen with `u64::from(...)` to keep the preimage field at 8 bytes.

Two consequences:

- **`init_chain_id` disappears**, removing one of three owner-only entry points to
  `wipe_open_proposals()`.
- **`abi::chain_id()` panics if the host has no `CHAIN_ID` metadata.**
  **Verified 2026-08-05 (leaf #2):** under the repo's test harness
  (`VM::ephemeral()` then `genesis_session(chain_id)`), `abi::chain_id()` returns
  the genesis chain id — `dusk-vm` 1.6.0 `genesis_session` inserts
  `Metadata::CHAIN_ID` (`vm/src/lib.rs:261-272`). Panics only when metadata is
  absent (e.g. bare `VM::ephemeral()` without `genesis_session`). **No test shim
  required.** Probe: `knot-proposals/tests/contract.rs::
  abi_chain_id_available_under_ephemeral_vm` (test-target `chain_id()` →
  `abi::chain_id()`).

### 2.6 M3 — the nonce does two jobs, and one of them is wrong

```rust
let nonce = self.committee_nonce(args.registry_account_id);          // propose
if proposal.nonce != current_nonce { panic!("proposal nonce is stale"); }  // finalize
self.committee_nonces.insert(committee, current_nonce + 1);          // bump
```

Job one, replay protection: necessary. Job two, **global serialization per
committee**: accidental and harmful.

**Worked example.** Committee 7 at nonce 0. Alice proposes P1 "pay contractor A 100",
Bob proposes P2 "pay contractor B 50", Carol proposes P3 "upgrade config" — three
distinct legitimate decisions, all at nonce 0, all signed by five members each.

P2 finalizes. `committee_nonces[7] = 1`. P1 and P3 now have `nonce == 0` against
`current_nonce == 1` and panic **permanently** — the nonce is monotonic. Fifteen
signatures are worthless. Re-proposing computes a different digest (the nonce
differs), so **every member must re-sign from scratch**, and the original P1 and P3
sit `Open` in state forever.

Net effect: a council can only ever have one decision in flight, and every landed
decision invalidates all pending ones.

**Fix: replay protection comes from digest single-use, not a global counter.**

- `by_digest` becomes permanent and authoritative — any digest ever used is rejected
  while its deadline still allows re-propose; `consumed` blocks forever until prune
  after expiry (then deadline-in-past rejects anyway).
- The finalize-time nonce equality check is **removed**.
- `committee_nonces` and the `committee_nonce` ABI go away with v3.
- The field in `ProposeArgs` / the blob intent is a **caller-supplied uniquifier**
  (still named `nonce` on the wire for layout continuity). It is **not** the
  registry account nonce used by `change_account`.

**Two nonces, two jobs · LOCKED (2026-08-05)**

| | Registry account nonce | Proposal uniquifier (`ProposeArgs.nonce`) |
|---|---|---|
| Where | `knot-registry` / `knot-registry` account | Proposal digest + blob intent |
| Job | Anti-replay for `change_account` | Distinguish parallel / re-opened identical intents |
| Who sets | Contract bumps on successful change | Coordinator / blob author |

**Uniquifier rules**

- Identical full digest (including uniquifier) while Open → **merge** (return existing
  id). Not a funds attack: merge requires every hashed field to match, so the
  authorization is the same decision. At worst a submit race / id attribution.
- Same uniquifier + different args/deadline/… → different digest → independent.
- After `consumed`, same digest panics; re-do the same call → **new uniquifier**.
- Chain tracks **digests**, not “used uniquifier values.” No on-chain used-nonce set.
- Tool: default **CSPRNG `u64`** at blob-create / proposal-create; `--nonce N`
  override for tests / deliberate values. No off-chain used-nonce DB required for
  safety (optional UX history only).
- Public docs: prefer **uniquifier** or “proposal nonce (caller-chosen)” so it does
  not collide with registry account nonce in operator heads.

### 2.7 The `propose` gap — must land in the same commit

```rust
if p.status == Open       { return existing_id; }
if p.status == Tombstoned { panic!("tombstoned"); }
// Executed FALLS THROUGH -> new proposal under the same digest, by_digest overwritten
```

Currently **unreachable**, because the moving nonce means an executed digest can
never be recomputed. **Removing the nonce (§2.6) makes it live and exploitable** —
an executed proposal becomes re-proposable and its harvested signatures replay.

The `consumed` flag (§2.9) closes it. §2.6 and §2.7 are one change, not two.

### 2.8 M1 — deadline and `proposal_ttl` · LOCKED

Today `proposal_ttl` is only a fallback for `args.deadline == 0`; an explicit deadline
has no ceiling. `u64::MAX` yields a permanent proposal, and `propose` is permissionless.

The deadline is the **only** natural expiry in the system. Uncapped means a permanent
state entry, a permanent signature-collection window, and an unbounded H1 replay window.

**Deadline rules**

- `proposal_ttl` is a **ceiling**, not a default deadline.
- **`deadline == 0` is forbidden** — panic at propose. No “use ttl as default” branch.
  Callers always pass an explicit deadline in `(block_height(), block_height() + ttl]`.
- Reject `deadline < block_height()` (**L2**, `<` not `<=`, consistent with
  `approve`/`finalize` which use `block_height() > deadline`; `deadline == height`
  is the last valid block).
- Reject `deadline > block_height() + proposal_ttl`.

**`proposal_ttl` invariant**

- Always `proposal_ttl > 0`. Deploy default remains **1000**.
- `set_proposal_ttl(0)` panics. Propose also panics if `proposal_ttl == 0`
  (belt-and-suspenders; should be unreachable).
- `set_proposal_ttl` does **not** wipe open proposals (deadlines are baked into digests).
- **`MAX_PROPOSAL_TTL`**: hard cap on what the owner may set (constant, reasonable
  default at implement — suggested **100_000** blocks; must be `≥` deploy default).
  `set_proposal_ttl(n)` panics if `n == 0 || n > MAX_PROPOSAL_TTL`. Stops an owner
  setting `u64::MAX` and re-opening an unbounded H1 window.

### 2.9 M2 — epoch counter and pruning

**State vs history.** Every `propose` transaction is permanently in the blocks;
pruning cannot erase it. Contract *state* is what the contract can read and act on.
Pruning means the contract can no longer act on a proposal — nothing is lost archivally.

**What may be pruned.** The payload (`target`, `call_args`, `approvals`,
`approval_sigs` — up to ~6 KB) is dead once terminal. **`by_digest` is not**: it is
what stops a spent digest being re-proposed and its signatures replayed. Deleting it
reintroduces H1 by hand.

**Why the state is nonetheless bounded.** The digest commits to `deadline`, and
`propose` rejects `deadline < block_height()`. So once a deadline passes, that digest
can never be validly re-proposed and remembering it is pointless. With M1's ceiling,
both maps are bounded by a rolling TTL window. **M1 is what makes pruning sound.**

**Epoch counter.** One `u64` folded into the digest; `epoch += 1` invalidates
everything in O(1), replacing `wipe_open_proposals()` (O(n), can exceed gas and
permanently brick all three owner-only setters). Because `approve`/`finalize` use the
*stored* digest rather than recomputing, the epoch must be checked explicitly.

**Epoch is not garbage collection** — it makes proposals unreachable, not absent.
Storage reclamation needs a separate permissionless paginated `prune(limit)`.

**The wipe surface mostly evaporates:**

| Caller | Disposition |
|---|---|
| `init_chain_id` | Gone — derived (§2.5) |
| `set_proposal_ttl` | **Never needed a wipe.** Each deadline is baked into its own digest; changing the ceiling cannot affect an existing proposal. Delete the wipe call. |
| `init_registry` | Genuinely needs invalidation → `self.epoch += 1` |

`wipe_open_proposals()` is **deleted outright**, and the brick risk with it.

**No archive contract.** Every write is already in transaction history. An archive
contract would add a cross-contract call to every write, roughly doubling gas, and
merely relocate unbounded growth. Instead **make the event stream self-sufficient**
(§2.12) — that is the archive, at zero on-chain cost.

### 2.10 Target state

```rust
pub struct MultisigProposalsState {
    registry: Option<ContractId>,
    epoch: u64,
    tombstone: bool,
    proposal_ttl: u64,          // ceiling, must be > 0
    by_digest: BTreeMap<[u8; 32], DigestRecord>,
    proposals: BTreeMap<u64, Proposal>,
    next_id: u64,
}

/// Survives payload pruning. ~25 bytes vs ~6 KB.
struct DigestRecord {
    proposal_id: u64,
    deadline: u64,
    epoch: u64,
    consumed: bool,             // true once finalize ran; blocks re-propose forever
}

struct Proposal {
    registry_account_id: u64,
    nonce: u64,                 // caller-supplied uniquifier, NOT a counter
    epoch: u64,
    target: ContractId,
    function_name: String,
    call_args: Vec<u8>,
    deadline: u64,
    signed_digest: [u8; 32],
    approvals: Vec<BlsPublicKey>,
    approval_sigs: Vec<BlsSignature>,
    status: ProposalStatus,
}
```

Removed: `chain_id`, `committee_nonces`, `Proposal::chain_id`.
`ProposeArgs` gains `nonce: u64`.

### 2.11 Methods

```rust
pub fn propose(&mut self, args: ProposeArgs) -> u64 {
    let _registry = self.require_registry();
    if args.function_name.len() > MAX_FUNCTION_NAME_LEN { panic!("function_name too long"); }
    if args.call_args.len() > MAX_CALL_ARGS_LEN { panic!("call_args too long"); }
    if self.proposal_ttl == 0 { panic!("proposal_ttl not configured"); }
    if args.deadline == 0 { panic!("proposal deadline must be non-zero"); }       // §2.8

    let now = block_height();
    let max_deadline = now.checked_add(self.proposal_ttl).expect("ttl overflow");
    let deadline = args.deadline;
    if deadline < now          { panic!("proposal deadline is in the past"); }   // L2
    if deadline > max_deadline { panic!("proposal deadline exceeds max TTL"); }  // M1

    let digest = proposal_digest_v3(
        u64::from(abi::chain_id()),
        &abi::self_id().to_bytes(),
        self.epoch,
        args.registry_account_id,
        args.nonce,
        &args.target.to_bytes(),
        args.function_name.as_bytes(),
        &args.call_args,
        deadline,
    ).expect("caps keep fields within u32");

    if let Some(rec) = self.by_digest.get(&digest) {
        if rec.consumed        { panic!("proposal digest already executed"); }      // §2.7
        if rec.epoch != self.epoch { panic!("proposal digest belongs to a retired epoch"); }
        match self.proposals.get(&rec.proposal_id).map(|p| p.status) {
            Some(ProposalStatus::Open) => return rec.proposal_id,                   // merge
            _ => panic!("proposal digest already used"),
        }
    }

    let id = self.next_id;
    self.next_id = self.next_id.checked_add(1).expect("next_id overflow");           // L1
    /* insert Proposal { .., epoch: self.epoch, .. } */
    self.by_digest.insert(digest, DigestRecord {
        proposal_id: id, deadline, epoch: self.epoch, consumed: false,
    });
    abi::emit("proposal_created", (id, digest, args.registry_account_id, deadline));
    id
}
```

`approve` — unchanged except add, after the status check:
`if proposal.epoch != self.epoch { panic!("proposal belongs to a retired epoch"); }`

`finalize` —

```rust
-   let current_nonce = self.committee_nonce(proposal.registry_account_id);
-   if proposal.nonce != current_nonce { panic!("proposal nonce is stale"); }
+   if proposal.epoch != self.epoch { panic!("proposal belongs to a retired epoch"); }
...
-   self.committee_nonces.insert(committee, current_nonce + 1);
+   if let Some(rec) = self.by_digest.get_mut(&digest) { rec.consumed = true; }
```

Add before `call_raw`, using the now-confirmed `abi::self_id()`:

```rust
if target == abi::self_id() { panic!("finalize: target must not be this contract"); }
```

CEI ordering and terminal-status-before-`call_raw` stay as they are — already correct.

`prune(limit: u32) -> u32` — new, permissionless, bounded by
`MAX_PRUNE_BATCH = 128`. Removes payloads whose status is terminal, or whose epoch is
retired, or whose deadline has passed; removes `by_digest` records **only** where
`deadline < block_height()`.

**Invariant: a `consumed` record whose deadline has not passed must be retained.**
Dropping it early permits replay.

`set_proposal_ttl(blocks: u64)` — owner-only; no wipe:

```rust
if blocks == 0 || blocks > MAX_PROPOSAL_TTL { panic!("proposal_ttl out of range"); }
self.proposal_ttl = blocks;
```

### 2.12 Digests

```
DOMAIN_PROPOSAL_V3 = b"nocturne.knot.multisig.proposal.v3"
  || chain_id:u64_le          <- abi::chain_id(), widened from u8
  || self_id:[u8;32]          <- abi::self_id()
  || epoch:u64_le
  || committee_id:u64_le
  || nonce:u64_le             <- caller-supplied
  || target:[u8;32]
  || fn_len:u32_le  || function_name
  || args_len:u32_le || call_args
  || deadline:u64_le

DOMAIN_CHANGE_ACCOUNT_V3 = b"nocturne.knot.multisig-registry.change_account.v3"
  || chain_id:u64_le          <- NEW
  || self_id:[u8;32]          <- NEW, the registry contract
  || account_id:u64_le
  || nonce:u64_le
  || member_count:u32_le      <- NEW (L13)
  || pk₀..pkₙ
  || threshold:u32_le
```

Both bump to **v3**. v2 is already deployed; changing fields without a bump would
make v2 signatures mis-verify silently instead of failing loudly.

### 2.13 Events — the archive · LOCKED

```rust
abi::emit("proposal_created",   (id, digest, committee_id, deadline));
abi::emit("proposal_approved",  (id, digest, signer_pk_bytes));
abi::emit("proposal_finalized", (id, digest, committee_id, target, function_name));
abi::emit("pruned",             count);
```

Today only an id is emitted, so an indexer that missed `propose` cannot reconstruct a
pruned record. With the above, events alone are a complete archive.

**Consumer (option 3) · LOCKED**

- Knot **emits** only. Decode arms live in the existing
  `sme_platform/rusk-experiments/event-decoder` crate (SSOT for chain-gateway /
  Nocturne indexing). Do **not** grow a parallel decoder inside knot.
- **No historical dual-decode** for pre-v3 Knot / `multisig-*` shapes. Repo is still
  private; nobody depends on old emits. Clean break. Dual-decode / fallback arms are
  for **after** a public shape has shipped and must keep decoding — not for disposable
  testnet scrap.
- **DEFERRED:** extract `event-decoder` → standalone `nocturne-event-decoder` so
  consumers need not path-depend on `sme_platform`. Named so it cannot vanish; not on
  the critical path for v3.

### 2.14 Migration order

1. **Confirm `abi::chain_id()` under `VM::ephemeral()`.** Done (leaf #2,
   2026-08-05) — works via `genesis_session(chain_id)`; no shim.
2. Encoding: add v3 preimages; keep v2 one release for tooling only if still useful
   internally — public launch burns v2 anyway.
3. Contracts: state and methods above. Re-run layout goldens — `ProposeArgs` gains
   uniquifier field.
4. Tool: supply uniquifier (CSPRNG / `--nonce`); drop derived `chain_id` from blob
   intents where host-derived.
5. Deploy registry v3, then proposals v3 pointing at it.
6. **Treat every v2 signature as burned.** Re-create councils; do not migrate state.
   No compatibility theater for prior private deployments.

### 2.15 Tests

| Case | Expect |
|---|---|
| Same intent, two proposals contracts | different digests (H1) |
| Same intent, two chain ids | different digests (H1) |
| Three parallel proposals, one finalizes | other two still finalizable (M3) |
| Re-propose an executed digest | panic `already executed` (§2.7) |
| `deadline == block_height()` | accepted at propose, approve, finalize (L2) |
| `deadline = now + ttl + 1` | panic `exceeds max TTL` (M1) |
| `args.deadline == 0` | panic `must be non-zero` |
| `set_proposal_ttl(0)` | panic |
| `set_proposal_ttl(MAX+1)` | panic |
| Epoch bump | old proposals unapprovable and unfinalizable |
| `prune` with unexpired consumed digest | record retained |
| `prune` past deadline, then re-propose | rejected, deadline in past |
| `finalize` targeting self | panic |
| 10k proposals then `init_registry` | succeeds, O(1) |
| Identical Open propose twice | merge, same id |

---

## 3. Keystore v2

Resolves M4, M5, M6, M7, L4, L5, L6. Ships incrementally; M6 and M7 together.
All of `keystore.rs` is byte-identical between `2fb3c94` and `7e58d4c`, so the
round-one findings stand unmodified.

Current: `[salt:16][nonce:12][AES-256-GCM ct]`, plaintext JSON
`[{"name":..,"sk_hex":..,"pk_hex":..}]`, PBKDF2-HMAC-SHA256 600 000 rounds.

**Scope note (decided 2026-08-05):** this keystore is *test tooling*. In production
each identity holds its own key in its own wallet. Seed-derived multi-identity
recovery was considered and **rejected** — do not implement. The README must state
plainly that this is not production key custody.

### 3.1 M4 — permissions

`fs::write` creates at `0o666 & !umask` (typically `0o644`), then chmods to `0o600` —
secret keys are world-readable in between. `create_dir_all` similarly makes
`~/.knot-tool/` at `0o755`.

```rust
fs::DirBuilder::new().recursive(true).mode(0o700).create(parent)?;

let mut f = fs::OpenOptions::new()
    .write(true).create(true).truncate(true)
    .mode(0o600)                     // applied at creation — no window
    .open(&tmp)?;
```

Also refuse to load an over-permissive store, as OpenSSH does:

```rust
let mode = fs::metadata(path)?.permissions().mode() & 0o777;
if mode & 0o077 != 0 { bail!("identity store is group/world accessible (mode {mode:o}) — chmod 600"); }
```

`.mode()` is Unix-only — gate on `#[cfg(unix)]`.

### 3.2 M5 — atomic write

`tmp + rename` gives **atomicity** (never a torn file) but not **durability**. Four
steps, and step 4 is the commonly omitted one:

```rust
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let dir = path.parent().context("store path has no parent")?;
    let tmp = path.with_extension("tmp");          // same dir => same filesystem

    let mut f = fs::OpenOptions::new()
        .write(true).create(true).truncate(true).mode(0o600).open(&tmp)?;
    f.write_all(bytes)?;
    f.sync_all()?;                                  // 2. flush contents
    full_fsync(&f)?;
    drop(f);

    fs::rename(&tmp, path)?;                        // 3. atomic only on same fs

    let dfd = fs::File::open(dir)?;                 // 4. fsync the DIRECTORY
    dfd.sync_all()?;
    full_fsync(&dfd)?;
    Ok(())
}
```

**Why the directory fsync.** `rename()` updates directory metadata. Without fsyncing
the directory, a crash can leave *neither* name pointing at the new data — the classic
"empty file after power loss".

**macOS.** `fsync()` returns once data reaches the drive, not once the drive has
flushed its own write cache. Rust's `sync_all()` maps to `fsync`. Real durability
needs `F_FULLFSYNC`:

```rust
#[cfg(target_os = "macos")]
fn full_fsync(f: &fs::File) -> std::io::Result<()> {
    use std::os::unix::io::AsRawFd;
    if unsafe { libc::fcntl(f.as_raw_fd(), libc::F_FULLFSYNC) } == -1 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}
#[cfg(not(target_os = "macos"))]
fn full_fsync(_: &fs::File) -> std::io::Result<()> { Ok(()) }
```

Clean up a stale `.tmp` on load; never treat it as a store.

**Atomicity is not recovery.** Also ship a rotating backup (move the existing file to
`identities.dat.bak` before each save) and an explicit `identity export` / `import`
command documented in the README.

### 3.3 M6 + M7 — zeroization and format, one change

**M6 cannot be fully fixed on the current format.** Wrapping `plaintext` in
`Zeroizing` helps, but `serde_json` has already allocated a `String` per `sk_hex` and
may have reallocated internal buffers, leaving copies Rust cannot reach. Perfect
zeroization through serde is not achievable — the fix is to stop putting secrets in JSON.

```
offset  len  field
0       8    magic         "KNOTKS\x00\x02"
8       1    version       2
9       1    kdf_id        1 = PBKDF2-HMAC-SHA256, 2 = Argon2id
10      4    kdf_p1        PBKDF2: rounds | Argon2: m_cost (KiB)
14      4    kdf_p2        PBKDF2: 0      | Argon2: t_cost
18      1    kdf_p3        PBKDF2: 0      | Argon2: p_cost
19      1    salt_len
20      N    salt
20+N    12   nonce
32+N    ..   AES-256-GCM ciphertext || tag
```

Header bytes `0..20+N` are passed as **AES-GCM associated data**. Precisely what this
buys: any tampering already breaks decryption, because every header field feeds the
KDF and a changed field yields a different key. AAD's value is that tampering fails as
an explicit tag mismatch rather than a confusing "wrong password", and it future-proofs
against a field that does not feed the KDF.

Plaintext becomes fixed-layout binary — no JSON, no hex:

```
count:u32_le
repeat count:
  name_len:u16_le || name:utf8
  has_sk:u8
  if has_sk: sk:[u8;32]
  pk:[u8;96]
```

The only secret in memory is then a `[u8;32]`, which `Zeroizing` covers completely.

```rust
let key       = Zeroizing::new(derive_key(password, salt, &params));   // L4
let plaintext = Zeroizing::new(cipher.decrypt(nonce, payload)?);
```

`Zeroizing` wipes on `Drop`, so **error paths are covered structurally** — that is L4,
fixed by construction rather than by remembering to call `.zeroize()` after each `?`.
Add `#[derive(ZeroizeOnDrop)]` to any struct holding `sk` bytes.

**KDF.** Keep `kdf_id = 1` for reading existing files; write `kdf_id = 2`
(**Argon2id**, m = 64 MiB, t = 3, p = 4) for new stores. 600k PBKDF2-HMAC-SHA256 meets
the OWASP floor but is weak against GPU/ASIC; memory-hardness is the material gain.

**Migration.** v1 files never begin with `KNOTKS`, so detection is unambiguous:
decrypt with the hardcoded 600k, immediately re-save as v2. The current error message
instructing users to **delete their keystore** after a rounds change is then
permanently obsolete.

### 3.4 L5 — `parse_pk`

`t.chars().all(is_ascii_hexdigit)` routes a base58 key composed only of `[0-9a-fA-F]`
to the hex path, failing with a misleading error.

Length alone would fix it — 96 bytes is 192 hex chars versus ~131 base58, ranges
cannot overlap — but try-both is barely longer and gives a better error:

```rust
pub fn parse_pk(s: &str) -> Result<BlsPublicKey> {
    let t = s.trim();
    let hex_err = match pk_from_hex(t)    { Ok(pk) => return Ok(pk), Err(e) => e };
    let b58_err = match pk_from_base58(t) { Ok(pk) => return Ok(pk), Err(e) => e };
    bail!("not a valid BLS public key.\n  as hex:    {hex_err}\n  as base58: {b58_err}")
}
```

**Also replace `trim_start_matches("0x")` with `strip_prefix("0x").unwrap_or(s)`** —
`trim_start_matches` strips *repeated* prefixes, so `0x0x0xAB…` is currently accepted.
Confirmed at `7e58d4c` in `keystore.rs:79` and `blob.rs:103,111,124,176,190`.

### 3.5 L6 — `default_path()`

`std::env::var("HOME").expect("HOME must be set")` panics. Return `Result`, honour a
`KNOT_STORE` override, and use the `directories` crate so it works on Windows
and follows XDG on Linux. This **changes the default path** — keep the old location as
a read fallback for one release and log when it is used.

### 3.6 Order and tests

| # | Item | Why here |
|---|---|---|
| 1 | L4 `Zeroizing` | One line, structural, fixes error paths immediately |
| 2 | M4 permissions | Small, closes a live exposure |
| 3 | M5 atomic write + `.bak` | Prevents key loss; do before format churn |
| 4 | M7 + M6 format v2 | Together — the format change is what makes zeroization real |
| 5 | L5, L6 | Cosmetic by comparison |

| Case | Expect |
|---|---|
| Save, inspect mode | `0o600`; parent `0o700` |
| Load a `0o644` store | refuse with chmod hint |
| Kill between write and rename | old store intact and loadable |
| Stale `.tmp` present | ignored and cleaned |
| v1 file | loads, silently upgrades to v2 |
| v2 with flipped header byte | AEAD tag failure, not "wrong password" |
| 96-byte key, hex and base58 | both parse to the same key |
| `0x0x` prefix | rejected |
| No `HOME` | error, not panic |

---

## 4. Tool, collector and registry

All clusters here are **LOCKED** as of 2026-08-05 and implementable. (This section was
originally the "agreed but unspecified" bucket; it was specified across the
2026-08-05 walkthrough. Per-item markers are authoritative.)

### 4.1 Tool — blob handling

Both re-verified at `7e58d4c`; `blob.rs` changed only via the PM peel.

- **M8 — blob `threshold` is unauthenticated but used as a guard · LOCKED.**

  `threshold` is **not** in the proposal preimage — `gate_blob_for_signing` recomputes
  from intent fields only. Yet `aggregate_partials` (`blob.rs:309`) enforces
  `partials.len() >= blob.threshold` and bails with "REFUSING", and
  `print_canonical_intent` (`blob.rs:272`) displays it among fields that *are*
  authenticated. A hostile blob sets `threshold: 1` and the guard passes with one
  signature.

  Not a funds risk — on-chain `finalize` re-reads the real threshold from the registry
  and re-verifies via `verify_quorum`. The problem is a guard that looks like a
  security control and is not one.

  **Decision: fetch the real threshold from the registry before aggregating.** The
  tool already has chain reads (`chain.rs` queries `account`), and `submit-agg` talks
  to the chain anyway.

  **Rejected: binding threshold into the digest.** Threshold is a property of the
  *registry account*, not the proposal, and it changes via `change_account`. A signed
  copy would be overridden by the live registry value at `finalize`, which is more
  confusing than not having it. (Note the removed PM path *did* bind threshold —
  correct there, because `council_resolve` had no registry re-check behind it.)

  **Offline fallback:** aggregation may happen air-gapped, where no fetch is possible.
  Then label it honestly rather than implying verification —
  `"partials 1 below blob-declared threshold 2 (unverified locally; the chain enforces
  the real threshold)"`. Never print "REFUSING" for an unverified value.
- **M9 — no local verification before aggregating.** `aggregate_partials` deserializes
  and sums without verifying any partial. Per `VerifyQuorumAggregateArgs`' own doc, a
  single bad partial invalidates the whole aggregate. **Agreed: verify each partial
  locally, drop and name invalid ones.**
- **L7 — `bls::aggregate` panics on empty input.** Agreed: return `Result`.
- **L8 — blob writes non-atomic.** Agreed: reuse the §3.2 helper.
- **L14 — `Result<_, ()>` in `recompute_and_verify` / `gate_blob_for_signing`**
  (`encoding/src/lib.rs:243,257`). Callers cannot distinguish digest mismatch from
  field-too-large; `blob.rs` hardcodes a message that is wrong in the overflow case.
  Agreed: typed errors.

### 4.2 Collector · LOCKED

All collector sources are byte-identical between `2fb3c94` and `7e58d4c`, so the
round-one findings stand unmodified.

**Threat model correction.** The collector enforces **no authentication of its own** —
`KNOT_COLLECTOR_USER`/`PASSWORD` are credentials the *client* sends, and the Rust
code never verifies them. Enforcement is entirely the operator's nginx htpasswd.

Basic Auth is a client-presents-credential scheme, so **every co-signer must hold the
credential** — `crates/knot-tool/README.md` instructs exactly this ("give them the
three `KNOT_COLLECTOR_*` values"). Today that is a **single shared `demo` user**,
which yields no attribution (logs name `demo`, not a person), no revocation short of
rotating for everyone, and a blast radius of the whole council.

So the realistic attacker for everything below is **any council member** — precisely
the adversary M-of-N exists to tolerate. These findings are in-scope, not exotic.

**Note:** no auth scheme substitutes for C1/M10 regardless of how credentials are
issued — a legitimate council member holds valid credentials by construction, so
authentication can only ever tell you *who* acted, never prevent it. The protocol
fixes are the control.

**Deployment invariant:** because the collector enforces nothing itself, its security
rests entirely on the proxy being in front. `KNOT_COLLECTOR_ALLOW_NON_LOOPBACK`
is what keeps that true. If anyone sets the escape hatch and binds `0.0.0.0`, the API
is open to the internet with **no authentication at all**. State this in the README.

#### C1 — digest squatting (was: "the store doc comment") · LOCKED

`store.rs:41-43` claims *"`id` = hash of `signed_digest`, this can only happen on a
hash collision."* **Factually wrong** — `digest_to_id` (`dto.rs:201`) strips `0x` and
lowercases. There is no hash; the id *is* the digest.

So `Conflict` does not mean "two digests collided" (ignorable). It means **a different
intent was submitted under this exact digest** — and the collector never checks that an
intent produces its claimed digest, so any pair is accepted.

Attack:

1. Attacker learns digest `D` (shared out-of-band for fingerprint comparison).
2. Attacker POSTs first: `{intent: EVIL, signed_digest: D}` → stored under id `D`.
3. Alice POSTs her real `{intent: I, signed_digest: D}` → identity differs →
   `Conflict` → **409, her genuine proposal is rejected**.
4. Bob pulls `D`, gets `EVIL`, his tool recomputes and **refuses to sign**.

Step 4 holds — no wrong signature is ever produced, because `gate_blob_for_signing`
recomputes from canonical intent fields. **That gate is load-bearing.** But the id is
permanently squatted: Alice cannot publish, Bob cannot sign. Unauthenticated DoS.

The wrong comment is *why nobody would notice*: it frames a live tamper attempt as
impossible, so the code 409s with no log, no metric, and an attack is indistinguishable
from a typo.

**Fix, three parts:**

1. Correct the comment.
2. Log `Conflict` at `warn` with id and both intents — it is the clearest signal that
   someone is probing the relay.
3. **Recompute the digest from the intent at create time and reject mismatches.**

**Alternatives rejected** (all four were worked through):

| Alternative | Why not |
|---|---|
| Server-assigned ids | Destroys content-addressing — signers can no longer derive the same id independently, which is the collector's point |
| Multiple bodies per digest | Needs a per-digest cap; with a cap the attacker fills every slot first and Alice still cannot publish. Eviction policy also attackable. Strictly worse. |
| Require a valid partial with `create` | Attacker signs `D` with their own key; collector cannot check membership (it is on-chain). Squatting survives unless the digest is also recomputed — then redundant. |
| Key by `H(intent)` | Collapses into (3) — the digest already is a hash of the intent fields |

**Costs, accepted:**

- **Version coupling** (the real one) — the collector must understand the preimage
  format; v3 requires a collector upgrade. Dispatch on `version` and **reject unknown
  versions** rather than relaying unverified. Operator-controlled deployment ordering.

  **Accepted 2026-08-05 (Leonidas):** nothing depends on Knot yet, there is no
  production deployment, and testnet state is disposable. Lockstep upgrades cost
  nothing today and the window to take this cost for free closes at launch.
- **DTO duplication becomes a benefit** — `ProposalsIntentDto` hand-duplicates the
  preimage fields today, which is already a drift hazard. Reuse `ProposalIntent` from
  `knot-encoding` and delete the duplicate.
- **"Dumb pipe" purity** — already gone; the collector normalizes hex, caps lengths,
  enforces `MAX_PARTIALS`, clears caller partials. Hashing is more of the same.
- **Licensing** — collector AGPL-3.0-only, `knot-encoding` Apache-2.0. One-way
  compatible, no issue.
- **Dependency weight** — `knot-encoding` `default = []` pulls only `tiny-keccak` and
  `sha2`. **No `dusk-core`, no `rkyv`, no BLS.** Exactly what the feature gating exists
  for; this does not touch the "never holds keys" property.
- **Implementation hazard to watch** — the DTO holds hex *strings*; recomputation needs
  bytes. Any mismatch in casing, `0x` handling, or padding turns a valid proposal into
  a 400. Must use a single canonical parse **shared with the tool**, never a
  reimplementation. Most likely place to ship a bug.

#### M10 — unauthenticated partial overwrite · LOCKED

Last-write-wins by `signer_pk` (`store.rs`, `append_partial`), no auth. Anyone with the
shared credential can clobber a valid signature.

**M9 does not fix this.** Local verification makes a poisoned partial *detectable and
attributable* instead of silently breaking the aggregate — a real improvement — but the
valid signature is still gone, leaving the group below threshold until that member
re-signs. An attacker can overwrite partials as fast as they are added.

**Fix: verify partials at the collector before storing.** The collector already holds
`signed_digest`; a partial arrives with `signer_pk` and `sig`, so
`verify_bls(digest, pk, sig)` is fully computable. Only valid signatures ever land, and
overwrites become harmless — replacing a valid signature with another valid signature
is a no-op in effect.

**On the "no `dusk_core`" principle — restated · LOCKED (2026-08-05).**

Older docs used “no `dusk_core`” as shorthand for “collector cannot forge.” That
conflates dependency with capability. **Forging needs secret keys.** Verification is
a public operation. M10/M12 **do** take a `dusk-core` (or equivalent) verify path.

**Rewrite every README / module doc that says the collector has no `dusk_core`.**
Replacement guarantee:

> The collector never holds secret keys, never signs, and never submits on-chain
> transactions. It may verify public BLS signatures and recompute digests so it
> cannot be used as an unauthenticated griefing relay.

Applying the old rule to verification is what created M10. Cost of verify: one
pairing per POST (~1–2 ms), same work the chain does.

Rejected: **first-write-wins** is actively worse — an attacker pre-squats each
`signer_pk` slot with garbage before the real member signs, permanently blocking them.

C1 stays on `knot-encoding` only (no `dusk-core`). M10/M12 may land together once
`dusk-core` is accepted. Keep the two decisions distinct in sequencing.

M9 ships regardless — the tool must never trust the collector.

#### M11 — unbounded rows and an unbounded list endpoint · LOCKED

**There is no `LIMIT` anywhere in `store.rs`** (verified). `list_proposals` selects
every row and parses each `body_json` in full:

```sql
SELECT id, digest, body_json, created_at FROM proposals ORDER BY created_at DESC, id ASC
```

So `GET /v1/proposals` is unbounded in both response size and CPU. This is not a slow
disk-fill — it is a **self-inflicted DoS that arrives with ordinary use**, attacker or
not. `MAX_PARTIALS = 32` and `MAX_BODY_BYTES = 64 KiB` do not bound row count.

Fix: pagination (`?limit`/`?offset`, capped), row caps for both tables, and a TTL
sweep. `proposals.created_at` already exists, so expiry is straightforward.

#### M12 — party roster impersonation · LOCKED

`POST /v1/party` accepts `{name, pk, note}` with no proof of key possession — anyone
can register a real member's key under any name, or their own key under a trusted name.

**Fix: require a signature over a domain-separated, length-prefixed preimage** — not
raw concatenation, or `("ab","cd")` and `("abc","d")` collide, the same field-shifting
bug `proposal_preimage` already guards against:

```
"nocturne.knot.collector.party.v1" || name_len:u32_le || name || pk[96]
```

Verify with `verify_bls` against the claimed `pk`. Same `dusk-core` dependency as M10 —
land together.

#### L9–L12 · LOCKED

- **L9** — `append_partial` (`api.rs:157`) skips `validate_proposal_id`, unlike
  `get_proposal` (`:141`). Fails closed via 404; fix the inconsistency.
- **L10** — `e.to_string()` on SQLite errors leaks schema and paths. Log detail, return
  a generic message. **SQL injection: none — every query is a literal with bound
  `params![]` (verified across the whole file).**
- **L11** — loopback guard is string-prefix matching (`lib.rs:56`). Parse `SocketAddr`
  and test `ip().is_loopback()`. Fails closed today, so not a hole.
- **L12** — no graceful shutdown (`main.rs:38`), and **`PRAGMA synchronous` is never
  set** (`store.rs:70` sets only `journal_mode = WAL`). WAL defaults to
  `synchronous = NORMAL`, which **can lose recently committed transactions** on power
  loss — not corruption, but silent loss of the last partials.

  Set `synchronous = FULL`, plus `PRAGMA fullfsync = 1` for macOS (same `F_FULLFSYNC`
  issue as §3.2). **No meaningful cost here:** write volume is a handful of humans, and
  every write already serializes behind the single `Mutex<Connection>` — the mutex is
  the throughput ceiling, not fsync.

  Add `.with_graceful_shutdown(...)`. Per Leonidas, widened repo-wide: **audit every
  write path for mid-write loss**, not just the collector.

#### Sequencing

| # | Item | Dependency |
|---|---|---|
| 1 | L9, L10, L11, L12 | none — independent, ship first |
| 2 | M11 pagination + caps | none |
| 3 | C1 digest recomputation | `knot-encoding` (no `dusk-core`) |
| 4 | M10 + M12 signature verification | `dusk-core` |
| 5 | M9 in the tool | independent; ship regardless |

Docs: `lib.rs`, `api.rs` and README (~line 56) all state the collector never verifies
signatures or recomputes the digest. All three become false and must be rewritten —
a strengthening, but a stated guarantee people may have read.

### 4.3 Registry

- **L3 — diagnostic methods as permanent public ABI · LOCKED: move off-chain.**

  The decisive argument is not gas: **a WASM contract cannot be feature-flagged at
  runtime.** A cargo feature means two artifacts — a debug WASM and a production
  WASM — so the mainnet contract would not be the bytecode tested on testnet, plus a
  new way to deploy the wrong one. Unacceptable for an immutable contract.

  `diagnose_quorum` needs no chain support at all: it is pure computation over data
  already publicly readable. `account()` returns members and threshold; the caller
  already holds `msg` and the sigs.

  | Method | Disposition |
  |---|---|
  | `diagnose_quorum` | **Delete.** Reimplement in `knot-tool` from `account()` + local BLS verify. Same output, no gas, no ABI surface, and it can print more than a contract return value can carry. |
  | `account_meta` | **Delete** — strict subset of `account()`. Keep only if the RUES free-read failure when returning `BlsPublicKey` is still real; testable. |
  | `member_key_bytes` | **Delete** — also a subset of `account()`. |
  | `next_account_id` | **Keep** — trivial, useful, no verification work. |

- **`verify_quorum` caller-side replay.** Documented in `call_types.rs`, but any
  integrating contract calling it with a nonce-free `msg` gets replayable
  authorization. Agreed: promote from doc comment to a prominent warning in the
  registry README and `security-model.md`.
- **Rogue-key / proof-of-possession · RESOLVED — no action.**

  Verified in `bls12_381-bls` 0.6.0, `src/keys/secret.rs:124-137`:

  ```rust
  pub fn sign_multisig(&self, pk: &PublicKey, msg: &[u8]) -> MultisigSignature {
      let mut sig = self.sign(msg);
      // Turn signature into its modified construction,
      // which provides protection against rogue-key attacks.
      let t = h1(pk);
      sig.0 = (sig.0 * t).into();
      MultisigSignature(sig.0)
  }
  ```

  Each signature is scaled by `t = h1(pk)` — the BDN distinct-exponents
  construction, and the reason `sign_multisig` takes `pk`. The classic rogue key
  `pk_evil = pk_target − Σ(other pks)` fails because the aggregate is `Σ tᵢ·pkᵢ`
  with `tᵢ = h1(pkᵢ)`; choosing `pk_evil` changes `t_evil`, which is circular.

  **Registering bare public keys with no proof of possession is therefore safe.**
  Both paths are sound for different reasons: `approve`/`verify_quorum` use plain
  `sign()` + `verify_bls` per signer with no aggregation, so rogue keys are
  irrelevant; `verify_quorum_aggregate` uses the scaled construction.

  Only action: **state this in `security-model.md`** — reviewers will ask.
- **`abi::keccak256` is a host query** (`src/abi.rs:153`) while both contracts hash
  with in-contract `tiny-keccak` compiled to wasm. Likely cheaper. Not yet measured.
- **`abi::caller()` / `abi::callstack()`** are available for an explicit reentrancy
  guard beyond the §2.11 self-target check.
- **blst** — Leonidas asked whether it would be faster than dusk's BLS for wasm.
  Not yet investigated.

### 4.4 Documented behaviour, no code change

`finalize` has **no caller or membership check** — anyone may call it once quorum is
collected (verified at `7e58d4c`). This is correct: authorization lives in the
signatures, not the caller, and it lets a funded relayer pay gas while council members
hold no DUSK. The surface is coherent — `propose` permissionless, `approve` requires
membership plus a valid signature, `finalize` permissionless. **It is not stated
anywhere in `security-model.md` and should be**, since every integrator will ask.

---

## 5. Repo hygiene

### 5.1 Files to remove from the public repo · LOCKED

Keep all of these locally; gitignore and `git rm --cached`.

| Path | Reason |
|---|---|
| `.nocturne-agent-kit-adopted` | `/Users/leonidas/…` ×2, names an unreleased repo |
| `.cursor/mcp.json` | absolute paths to `nocturne-mcp-gates`, `nocturne-mcp-tracks` |
| `.cursor/agents/*.md`, `.cursor/rules/*.mdc` | internal agent workflow |
| `.pituitary/pituitary.toml` | internal index config; enumerates the private doc taxonomy |
| `AGENTS.md` | `NOCTURNE_AGENT_KIT`, `claim.py acquire`, `.worktrees/<track>` |
| `docs/superpowers/**` | plans, specs, tracks, leaves — all internal |
| `docs/security-audit-2026-07-28*.md`, `docs/security-audit-2026-08-04.md` | internal audits |
| `docs/internal/**` | including this file |
| `docs/launch-form-knot.md`, `docs/launch-gap-map-2026-08.md`, `docs/doc-hygiene-inventory.md` | launch planning |

`.github/workflows/` **stays** — needed.

Keep public: `docs/security-model.md`, `docs/versioning.md`,
`crates/knot-proposals/test-target/`, `crates/knot-tool/static/mock-ledger.js`.

**Add `docs/design-notes.md` (public)** — rationale for choices a reader would
otherwise take for oversights, and which integrators will otherwise ask about. Five
entries, all decided in this round:

1. **No on-chain diagnostics** (§4.3 L3) — a WASM contract cannot be feature-flagged
   at runtime; two artifacts would mean mainnet is not the bytecode tested on testnet.
   Diagnostics are pure computation over already-readable data, so they live in the
   tool. *Also record this in the registry `CHANGELOG.md` as the reason for the
   removal, or someone re-adds `diagnose_quorum` the next time testnet reads misbehave.*
2. **Flat preimage, not EIP-712 nesting** (§2.1) — cryptographically equivalent;
   reviewers arriving from Ethereum will expect the nested form and should not have to
   guess.
3. **`finalize` is permissionless** (§4.4) — authorization lives in the signatures, not
   the caller; lets a funded relayer pay gas while members hold no DUSK.
4. **No proof of possession required** (§4.3) — `sign_multisig` scales by `t = h1(pk)`
   (BDN construction), which defeats rogue-key attacks by design.
5. **What the collector can and cannot do to you** (§4.2) — it can withhold, reorder,
   squat ids, and (until C1/M10 land) corrupt partials. It can never forge a signature
   or induce one over the wrong intent, because `gate_blob_for_signing` recomputes the
   digest from canonical intent fields. **That gate is the load-bearing control** and
   should be named as such.

Never track: `.worktrees/`, `target/`, `.env`.

`docs/` holds **46 tracked files** at `7e58d4c`; on this list roughly 6 remain public.

### 5.2 Files to add · LOCKED

`SECURITY.md`, `CONTRIBUTING.md`, root `CHANGELOG.md`, root `NOTICE`,
`.github/PULL_REQUEST_TEMPLATE.md`, `.github/ISSUE_TEMPLATE/`,
`.github/dependabot.yml`. CI additions: `cargo clippy -- -D warnings`,
`cargo fmt --check`, `cargo deny check`.

**Why NOTICE:** Apache-2.0 §4(d) requires redistributing NOTICE content if the work
has one. Three crates ship a `NOTICE`; the workspace root does not — that is an
inconsistency, not an option.

**CODE_OF_CONDUCT: not for now** (decided 2026-08-05). Pure convention; can be added
later without cost.

### 5.3 Prose to rewrite · LOCKED

Counts at `7e58d4c`, tracked files excluding `docs/`:

| Pattern | Files |
|---|---|
| `§[0-9]` | 20 |
| `Wave\|Track\|Phase N` | 10 |
| `Spec N` | 10 |
| `docs/superpowers` | 3 |
| `references/` | 2 |
| `sme-platform` | 2 |
| `rusk-experiments` | 2 |
| `rfq-settlement` | 2 |
| `atlas/` | 1 |
| `TODO`/`FIXME` | 1 |

Vocabulary: `§4a` → **proposal preimage**; `§4b` → **proposal blob**;
Layer E → **shared call types**; `M1/M2/M3`, `Spec N`, `Wave N`, `Track N`, `Phase N`,
`R4/R7`, `audit I6/I8/I9/I10` → delete or describe the change.

Also locked: **remove every `sme-platform` mention**; **no ETA talk and no framing
subjective to our development** in public docs.

Substantive content must be **inlined, not deleted** — e.g. `bls.rs`'s explanation of
why `sign()` and not `sign_insecure()` cites a private VM writeup; the reasoning is
valuable and belongs in the public doc.

### 5.4 Launch · LOCKED

**Squash the entire history into a single commit.** No history at all at launch. This
disposes of B1/B2 in history. Push the real history to a private backup remote
first if wanted (`git push private --all` / full ref backup).

Enable GitHub secret scanning **and push protection** before the first public push.

### 5.5 Private tier · LOCKED

Adopt **`knot-internal`**, a separate private repo checked out as a *sibling*
(`~/dev/aichbindas/knot-internal`), not a gitignored subdirectory and not a submodule.
A gitignored directory is one `git add -f` from publication; a submodule leaks the
private repo's name via `.gitmodules`. Same pattern for all future public projects.

Rationale and the four-tier model: `PUBLIC-REPO-STANDARD.md`.

### 5.6 Secrets inventory · LOCKED

Every project keeps one durable list of sensitive values and env vars.
`.env.example` (tracked, placeholder values) exists at the repo root; `.env` (real
values) is gitignored. Enumerated from source at `7e58d4c`: `RUSK_WALLET_PWD`,
`KNOT_PWD`, `KNOT_ALLOW_ENV_PWD`, `KNOT_COLLECTOR_URL`,
`KNOT_COLLECTOR_USER`, `KNOT_COLLECTOR_PASSWORD`, `KNOT_COLLECTOR_BIND`,
`KNOT_COLLECTOR_DB`, `KNOT_COLLECTOR_ALLOW_NON_LOOPBACK`, `DEMO_MODE`.

### 5.7 Crate renaming · LOCKED — do it

Product is Knot; crate dirs still said `multisig-*` while binary/env already said
`knot` / `KNOT_*`. Finish the mechanical surface rename before v3 semantics.

**Sequencing: a pure mechanical rename commit with zero behaviour change, landed
BEFORE the v3 contract work.** Rename, verify tests pass, commit. v3 then lands on
final names. Mixing a repo-wide rename with semantic changes makes review impossible,
and v3 is the change that most needs reviewing.

Scope — wider than crate names:

| Surface | From | To |
|---|---|---|
| Crates + directories | `multisig-{encoding,registry,proposals,tool,collector}` | `knot-*` |
| Module paths | `multisig_encoding::` etc. | `knot_encoding::` etc. |
| Binary | `multisig-tool` (interim `knot-tool`) | `knot-tool` |
| Env vars | `MULTISIG_*` / `MULTISIG_COLLECTOR_*` (interim `KNOT_*`) | `KNOT_*`, `KNOT_COLLECTOR_*` |
| Keystore dir | `~/.multisig-tool/` (interim `~/.knot-tool/`) | `~/.knot/` |
| HTTP header | `X-Multisig-Tool-Token` (interim `X-Knot-Token`) | `X-Knot-Token` |
| WASM artifacts | `multisig_*.wasm` | `knot_*.wasm` — Makefiles + `include_bytes!` in contract tests |
| `#[path]` includes | `registry_types.rs`, `proposals_types.rs` | follow the crate move |
| Docs | root `README.md` crate table, all crate READMEs, `docs/versioning.md` | |

**Not renamed:** signing domain tags stay `nocturne.knot.multisig.*` — crypto domains.
Pin JSON keys (`"multisig-registry"`, `"multisig-proposals"` in `chain.rs` json_key)
are external deployment data; rename only with a paired pin-repo update.

Migration details:

- Keystore path change folds into the L6 `default_path()` fix (§3.5): read the old
  location as a fallback for one release and log when it is used.
- Env var change: accept the old names for one release with a deprecation warning.
  After the launch squash there are no external users, so this is courtesy only.

---

## 6. CI notes — tooling

**Implemented** (see `TOOLING-AUDIT-LESSONS.md`): kit `bin/check-*.sh`, Knot
`scripts/` + CI `hygiene` job, MCP `audit_doc_stamp` / `leak_scan` /
`release_readiness`. Org secret scanning + push protection remain manual.

### 6.1 SHA stamping

**Every audit or design document must carry the commit SHA it was written against.**
This single rule would have prevented a full detour in this engagement: round one was
written against `2fb3c94` while the release candidate was 26 commits ahead on an
unmerged branch, and two findings were already fixed there.

Implement as a CI check: any file under `docs/internal/` matching `AUDIT-*` or
`DESIGN-*` must contain a 7+ hex SHA in its first 10 lines, and that SHA must resolve
in the repo.

### 6.2 Never trust remote-tracking refs

Related failure in the same detour: `origin/main` looked authoritative while 26 commits
stale, because it had not been fetched since Aug 4 12:02. Any release-readiness tooling
must begin with `git fetch --prune` and hard-fail unless `HEAD == origin/HEAD`.

### 6.3 Version drift

Live at `7e58d4c` — the README crate table disagrees with `Cargo.toml`:

| Crate | Cargo.toml | README | |
|---|---|---|---|
| `knot-encoding` | 0.1.2 | 0.1.2 | ok |
| `knot-registry` | **0.1.6** | **0.1.5** | drift |
| `knot-proposals` | **0.3.3** | **0.3.2** | drift |
| `knot-tool` | 0.2.0 | 0.2.0 | ok |
| `knot-collector` | 0.2.0 | 0.2.0 | ok |

Answers the "how do we avoid version drift" question: **do not hand-maintain the
table.** Either generate it from `cargo metadata` at build time, or add a CI check
parsing both and failing on mismatch. The latter is a few lines and needs no template
machinery. `docs/versioning.md` already declares `Cargo.toml` the source of truth — CI
should enforce what the policy already says.

### 6.4 Leak gate

A `scripts/check-public-surface.sh` failing on the §5.3 patterns, plus absolute local
paths, escaping relative links (`](../../../`), unresolved markers, and
credential-shaped strings. Draft in `PUBLIC-REPO-STANDARD.md` §5. Pair with
**gitleaks** — the regex catches the shape of the leaked password but is not a
substitute for a real scanner.

---

## 7. Corrections to the round-one audit

`AUDIT-2026-08-05.md` was written against `2fb3c94`. These entries are wrong at
`7e58d4c` and the audit is annotated accordingly:

| Audit entry | Correction |
|---|---|
| Domain tags are `sme-platform.*` | Already `nocturne.knot.*.v2` |
| "Domain tags cannot be changed without invalidating deployments" | **Wrong.** Nobody depends on Knot; freely changeable. Claim withdrawn. |
| PM constant `DOMAIN_COUNCIL_RESOLVE_V2` should move to pm | Already peeled out; `grep -c` = 0 |
| Version drift: encoding Cargo 0.1.2 vs README 0.1.1 | Fixed for encoding; **new drift** in registry and proposals (§6.3) |
| M3 fix: "document it, or auto-tombstone siblings" | Superseded — remove the nonce serialization entirely (§2.6) |
| M2 fix: "purge or epoch, pick one" | Superseded — epoch **and** prune, plus delete `wipe_open_proposals` (§2.9) |
| M5 fix: "tmp + rename" | Insufficient — needs directory fsync and `F_FULLFSYNC` (§3.2) |
| L2: "pick one comparison" | Decided: `<` (§2.8) |
| RELEASE-CLEANUP: "publishing the audit is a strong trust signal" | Rejected — publish no internal document (§1 B4) |
| DESIGN-keystore: seed derivation "worth reconsidering" | Rejected — test tooling only (§3) |

---

## 8. Decision log and remaining open items

| Item | Resolution |
|---|---|
| B5 private git dep | Publish the repo **and** make the dep optional (§1 B5) |
| L3 diagnostics | Off-chain; delete three methods, keep `next_account_id` (§4.3) |
| Crate renaming | Do it, as a mechanical commit before v3 (§5.7); pin JSON keys stay `multisig-*` until paired pin-repo update |
| CODE_OF_CONDUCT | Not for now (§5.2) |
| Rogue-key | Verified safe; documentation only (§4.3) |
| `deadline == 0` | Forbidden (§2.8) |
| `proposal_ttl` / `MAX_PROPOSAL_TTL` | Always `> 0`; set rejects 0 and `> MAX`; ceiling only (§2.8) |
| Proposal uniquifier | Caller-supplied; CSPRNG + `--nonce`; merge identical Open OK; track digests not nonces (§2.6) |
| Events + decoder | Rich emits; arms in existing `event-decoder`; no pre-v3 fallbacks; extract `nocturne-event-decoder` DEFERRED (§2.13) |
| Collector `dusk-core` | Accepted for M10/M12 verify; rewrite “no dusk_core” docs (§4.2) |
| Residual code audit | Done §11 at `b1d883d` — R1/R2 HIGH (Lab token HTML; multi-key sign sans confirm); see §11 |
| Human ops (B1 rotate, publish pins, `knot-internal`, org scanning, squash) | Explicit checklist; **deferred** until public launch unless needed for continued private work (§10) |

Still open / deprioritised:

1. **blst** — deprioritised (§4.3); contracts do no BLS in wasm.
2. **§9.3 signer UI scope** — agreed direction, packaging undecided.
3. **§9.4 `call_args` decoding** — required, design unspecified; separate product track.

---

## 9. Product scope and framing

Decided 2026-08-05. Not security findings — these are about what Knot *is* and how it
is presented. They change documentation and roadmap, not the audit.

### 9.1 The collector is per-council infrastructure, not a hosted service · LOCKED

Current docs imply a service Leonidas runs that councils connect to
(`collector.nocturne-standards.org`). That framing has consequences nobody wants:
Leonidas becomes infrastructure for other people's governance, can see every council's
proposals, owns their availability, and one credential spans unrelated groups.

**Per-council inverts all of it.** The credential is shared among people already on the
same council — exactly the set who would see those proposals anyway. No cross-council
exposure. No dependency on the Nocturne org.

It also makes the licensing coherent. AGPL-3.0-only is a *self-hosting* license; its
network clause targets running the software as a service. Per-council deployment is the
canonical AGPL case, and it gives the commercial-license offer in `LICENSING.md` a
clear meaning. Under the hosted-service framing the AGPL is doing something much vaguer.

**This does not resolve C1 or M10.** A hostile member can still squat ids and clobber
partials inside their own council — the M-of-N adversary. It scopes the blast radius
from "everyone using Knot" to "one council", and makes the shared credential
defensible rather than odd. See §4.2.

**Docs to rewrite** (verified at `7e58d4c`):

| Location | Current | Change |
|---|---|---|
| `crates/knot-tool/README.md:36` | `KNOT_COLLECTOR_URL=https://collector.nocturne-standards.org` | placeholder; no Nocturne-hosted default |
| `crates/knot-tool/README.md:52` | "shared relay" | council-operated relay |
| `crates/knot-tool/README.md:54` | "Share the collector with co-signers: give them the three values" | the coordinator deploys it for their council |
| `crates/knot-collector/README.md:92` | "VPS deploy (operator TODO): bring your own ops (TLS, auth, SQLite backup)" | a real deployment story — see below |

If councils are expected to self-host, *"bring your own ops"* is not a deployment
story. Target: `docker compose up` with automatic TLS (Caddy/Traefik sidecar), not a
homework list.

**Keep the no-collector path documented.** `blob sign --file … --signer … --out … --confirm`
runs fully offline — no collector, no chain, no credentials, no env vars (verified in
`main.rs`, `BlobCmd::Sign` local mode). Some councils will prefer Signal or email, and
that path needs zero infrastructure. The collector is optional convenience.

### 9.2 Coordinator / signer role split · LOCKED

Not three features — one architecture seen from three angles.

| Role | Needs | Does |
|---|---|---|
| **Coordinator** | chain access, gas, env vars, ops | deploys the collector, creates proposals, aggregates, submits |
| **Signer** | keystore + a way to receive a blob | reviews and approves. No collector setup, no chain, no gas, no configuration |

Per-council deployment does **not** mean every member deploys something — one person
does it once. The signer surface is already minimal in the CLI; it is simply not
presented as a distinct mode.

### 9.3 Signer UI · AGREED, scope open

Two tiers, and they are different projects:

- **Convenient** — load blob, show canonical intent, show fingerprint, unlock keystore,
  approve. Small; everything needed already exists.
- **Meaningful** — the above plus §9.4 decoding. This is where the security value is.

Leonidas wants the signer UI and considers it low-cost. Packaging as `.dmg`/`.exe` was
**not** committed to — "we don't necessarily need it" — technical users can run a
binary. Revisit if non-technical councils become real.

### 9.4 `call_args` decoding · LOCKED as required, unspecified

**The gap this closes:** the anti-blind-signing gate solves *"shown X, signed Y"*. It
does nothing about *"shown Y, did not understand Y, signed Y."*

A signer sees `call_args: 0x0001af…` — rkyv-encoded bytes. Even a technical member
cannot meaningfully review that. The gate guarantees the digest matches the displayed
intent, defeating a lying collector; it gives the human no idea what they are
authorising. Without decoding, "review the canonical intent" is theatre.

Needs a registry of known target contracts and function signatures rendering
`call_args` into human-readable form — what Safe and Etherscan do. A real feature, not
a UI detail.

**It does not weaken the trust model.** A decoder is advisory in exactly the way
`human_summary` already is: the digest stays authoritative, so a lying decoder can
induce a wrong *belief*, never a wrong *signature*. It slots in without disturbing
anything.

### 9.5 Fingerprint UX · LOCKED

The out-of-band comparison is the one control that cannot be automated — automating it
removes the protection rather than improving it. So in the signer UI the fingerprint
must be the **visually dominant element**, with explicit language
(*"compare this with a co-signer by phone before approving"*), not one line of output
among many. Possibly gated behind a checkbox before Approve enables. Imperfect —
people click boxes — but it is the only thing standing between a signer and implicit
trust in the proposer.

### 9.6 Private-network deployment · FUTURE, unscheduled

Binding the collector to a Tailscale/WireGuard network removes public exposure and the
need for htpasswd entirely — identity moves to the network layer. Attractive at council
scale. Leonidas: explore later. Not scoped.

### 9.7 For `security-model.md`

Two things belong in the public trust model:

1. **Unusability collapses M-of-N.** If a member cannot operate the tool, the practical
   outcome is not that they abstain — it is that a technical member "helps" and ends up
   holding their key. A 3-of-5 silently becomes 1-of-5 while every cryptographic check
   still reports success. This is the argument for §9.3 and §9.4.
2. **The gate's precise scope** — it prevents signing over a substituted intent; it
   does not make the intent comprehensible. State the boundary plainly.

---

## 10. Execution phasing · LOCKED (process)

Dependency order for the public-ready track. Calendar dates optional later; leaves
cut only after this section is accepted. Product §9 (signer UI / `call_args`) is a
**separate** track.

| Phase | What | Depends on | Notes |
|---|---|---|---|
| **0** | Spec sync | — | Done (`9c4ff8b` / `b1d883d`) |
| **1** | Residual audit | 0 | Done — findings §11; dispositions locked |
| **1b** | Lab/RPC hardening (§11 R1–R4,R6–R9,R12) | 1 | Leaf **#14**; parallel OK with #3; before sharing Lab |
| **2** | Mechanical rename `multisig-*` → `knot-*` | 1 | Zero behaviour change; pin JSON keys stay `multisig-*` until paired pin update |
| **3a** | Confirm `abi::chain_id()` under `VM::ephemeral()` | 2 | **Hard gate** for contract work; shim if unset |
| **3b** | Encoding digests v3 | 3a | |
| **3c** | Registry + proposals contracts v3 | 3b | Rich events; redeploy; burn v2 |
| **4a** | Tool: uniquifier, blobs, M8/M9, L7/L8/L14 **+ R5, R11** | 3b (3c for live pins) | Leaf **#6** |
| **4b** | Keystore v2 | 2 | Parallel with 4a |
| **5** | Collector: L9–L12, M11, C1, M10/M12 **+ R10** | 3b for C1; dusk-core for M10/M12 | Leaf **#8**; rewrite “no dusk_core” docs |
| **6** | Registry diagnostics off-chain | 3c | Delete on-chain methods; tool reimplement |
| **7** | `event-decoder` Knot arms | 3c | In `sme_platform` crate; knot only emits |
| **8** | Public hygiene + B5 optional deployments dep | 2+ | Prose gate, templates, design-notes |
| **9** | Launch ops checklist | Ready to go public | Rotate/scrub creds, publish pins repo, `knot-internal`, squash, org secret scanning — **deferred** from coding phases |
| **∞** | Extract `nocturne-event-decoder` | After 7, when needed | Named DEFERRED |
| **∞** | Product §9 | Independent | Signer UI / `call_args` decode |

**Human ops (phase 9) — explicit, not sprint blockers for private continued work**

- [ ] B1 rotate collector htpasswd (no funds at risk today; before any public or shared use)
- [ ] B2 scrub README placeholders
- [ ] Publish `nocturne-deployments` and/or ship B5 optional feature
- [ ] Create sibling `knot-internal`
- [ ] Org secret scanning + push protection
- [ ] History squash + private backup remote if wanted
- [ ] Unset `ALLOW_PRIVATE_TIER` in CI

**Out of scope for this track:** bending for pre-v3 private deployment compatibility;
`blst`; CODE_OF_CONDUCT.

---

## 11. Residual host-surface audit · LOCKED (findings)

**Written against `b1d883d`.** Full read of previously unaudited surfaces
(2026-08-05). Amendments belong here — not a second frozen audit-as-authority.

**Scope actually read:** `knot-tool` `rpc.rs`, `main.rs`, `chain.rs`,
`collector_client.rs`, `membership.rs`, `bls.rs`, `mock_ledger.rs`, `static/`;
`knot-collector` `store.rs`, `dto.rs`, `api.rs` (re-verify). Note: tool has
**no** `store.rs`/`dto.rs` — those live only in the collector (README list was
imprecise).

Already LOCKED elsewhere (C1, M4–M12, L9–L12, keystore, …) — not re-listed unless
status changed.

### 11.1 New findings · dispositions LOCKED (2026-08-05)

| ID | Sev | Problem (short) | Disposition |
|---|---|---|---|
| **R1** | HIGH | Token in unauthenticated `GET /` HTML | **OTP → HttpOnly session cookie** (`SameSite=Strict`, localhost). HTML never holds secret. Loopback bind required (R12). |
| **R2** | HIGH | Quorum / change-account multi-key sign, no confirm | Same preview+confirm UX as proposal approve; prefer one signer per `serve` call; CLI twins. |
| **R3** | MED | "confirmed" on mere propagate | Relabel `submitted`/`propagated` until block inclusion. |
| **R4** | MED | Raw wallet log / `e.to_string()` to browser | **Error-code schema** at RPC boundary; fixed messages; raw log stderr-only (same pattern as collector L10). |
| **R5** | MED | Basic Auth to any collector URL | Allowlist loopback or `https://` only. |
| **R6** | MED | `DEMO_MODE` defaults mock | Require explicit `DEMO_MODE`; refuse ambiguous; loud banner. |
| **R7** | MED | `__TOKEN__` → silent frontend mock | Fail closed if session/bootstrap missing. |
| **R8** | MED | `--nonce` bypasses account free-read | **Refuse by default**; dev-only latch for diagnostics. |
| **R9** | LOW | `escapeHtml` not every sink | Escape all interpolations; fix claim. |
| **R10** | LOW | Party `name` uncapped | Cap like `MAX_NOTE_CHARS`. |
| **R11** | LOW | Client skips proposal-id hex check | Validate on client. |
| **R12** | LOW | Bind string-prefix | `SocketAddr` + `is_loopback()`. |

**Lab vs collector:** R1 cookie session is Lab-only (`serve`). Collector stays untrusted relay + proxy auth + C1/M10/M12; do not import Lab cookie design there. API header was never the villain — embedding the secret in HTML was.

### 11.2 Verified OK (spot-checks)

- `/api/*` auth compare with `ct_eq`; fonts allowlisted; no shell for browser open or
  `rusk-wallet` (argv arrays); `RUSK_WALLET_PWD` env not argv.
- Proposal approve + blob sign: confirm + digest recompute gate.
- Membership fail-closed before quorum/change/approve sign.
- Collector SQL fully parameterized; create clears caller partials.
- Locked collector issues C1/M10–M12/L9–L12 **still open** — leaf `#8`.

### 11.3 Track wiring · LOCKED

| Leaf | Findings / work |
|---|---|
| **#14 `lab-rpc-hardening`** (new) | R1, R2, R3, R4, R6, R7, R8, R9, R12 |
| **#6 `tool-uniquifier-blobs`** | Planned M8/M9/L7/L8/L14/uniquifier **+ R5, R11** |
| **#8 `collector-hardening`** | Planned C1/M10–M12/L9–L12/M11 **+ R10** |
| **#7 keystore** | Unchanged by §11 |
| **#3–#5 contracts** | Unchanged by §11 |

Phase order: after #1 (done), **#14 may run parallel with #3 rename** (prefer after rename if both in flight to avoid path churn). Does not block encoding/contracts v3 if Lab stays private until #14 lands.
