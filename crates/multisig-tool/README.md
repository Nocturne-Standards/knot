# multisig-tool

Local signing tool + web UI for exercising [`multisig-registry`](../multisig-registry/README.md)
and [`multisig-proposals`](../multisig-proposals/README.md) against the real
Dusk testnet — account creation, quorum flows, governance, and the
multi-person propose/approve/finalize path. One binary, two skins: a CLI
(scriptable/headless) and a served local web UI (`serve` subcommand).

**TESTNET ONLY.** Never use with mainnet keys or funds — see "Security model"
below.

## For newcomers

Local signing Lab + web UI for Knot (registry / proposals / collector). Product
docs name: **Knot**.

Architecture and crate interaction diagram:
[docs.nocturne-standards.org — Knot](https://docs.nocturne-standards.org/v1/knot/)
· [source on GitHub](https://github.com/aichbindas/knot)

Cold-start: `./scripts/multisig-first-run.sh --serve` from repo root.

Everything below is lab **Status**, env, and maintainer run notes. Dense on purpose.

## Quick start

From this repo root:

```bash
export RUSK_WALLET_PWD=sme-platform-testnet-dev   # rusk-wallet keystore password (testnet)
# Optional scripting — both required, or omit both and type the keystore password:
export MULTISIG_TOOL_ALLOW_ENV_PWD=1
export MULTISIG_TOOL_PWD='local-dev-only'                      # unlocks ~/.multisig-tool/identities.dat

# Shared collector (if you use one) — password is the nginx htpasswd you handed out:
export MULTISIG_COLLECTOR_URL=https://collector.nocturne-standards.org
export MULTISIG_COLLECTOR_USER=demo                # collector htpasswd user (not WEN metadata user)
export MULTISIG_COLLECTOR_PASSWORD='navHam-cemnib-4pytja'

./scripts/multisig-first-run.sh --serve
# or: cd multisig && cargo run -p multisig-tool -- serve --bind 127.0.0.1:8877
```

Open the printed `http://127.0.0.1:8877/` URL (API bearer token is printed once
and embedded in the page). Re-running `multisig-first-run.sh` / `init` against
an existing store only unlocks + summarizes — it does not wipe identities.

| Env | Purpose |
|---|---|
| `RUSK_WALLET_PWD` | Gas-paying `rusk-wallet` unlock (chain writes) |
| `MULTISIG_TOOL_PWD` + `MULTISIG_TOOL_ALLOW_ENV_PWD=1` | Local identity keystore; refuse env pwd without the latch |
| `MULTISIG_COLLECTOR_*` | Optional HTTP Basic Auth client → shared relay |

**Share the collector with co-signers:** give them the three `MULTISIG_COLLECTOR_*`
values (out of band). They run the same Quick start on their laptop — keys never
leave their machine. Ops detail (htpasswd, nginx, participant checklist): bring
your own reverse-proxy wiring — the collector HTTP API in
`crates/multisig-collector/README.md` is the contract.

**PM dispute council:** use wen
[`pm-council-tool`](https://github.com/aichbindas/wen) (product UX for council
resolve). Knot multisig-tool covers registry, proposals, and generic Lab only.

## Scope

This tool owns both ends of the wire — the contract's own source
(`multisig-registry/src/call_types.rs`, included directly via `#[path = ...]`,
see `src/registry_types.rs`) and this client — so it skips the JSON/
data-driver round-trip every other deploy/wire script in this repo uses for
*other* contracts. Args are rkyv-serialized directly in Rust; chain
submission goes two ways:

- **Writes** (`create_account`, `verify_quorum`, `verify_quorum_aggregate`,
  `change_account`): shell out to the `rusk-wallet` CLI binary, same pattern
  as `scripts/wire-contract.sh` — real testnet transaction, costs gas, needs
  `RUSK_WALLET_PWD` set (unlock password for `rusk-wallet` gas-paying writes).
- **Reads** (`account`, `account_meta`, `member_key_bytes`, `next_account_id`,
  …): a direct RUES HTTP call (`POST /on/contracts:<id>/<fn>`) — free, no
  gas, no wallet. Request and response bodies are **raw rkyv bytes** with
  `Content-Type: application/octet-stream` (and a `rusk-version` header),
  same as `agent-pay-lp`. Hex-encoding the request body is wrong: the node
  does not hex-decode it, so a hex ASCII `u64` is parsed as a huge id and
  every `account` lookup returns `None`. See `src/chain.rs`.

## Security model

- Every identity's secret key lives only in this process (in memory) or the
  local encrypted keystore file (`~/.multisig-tool/identities.dat` by
  default) — signing happens server-side; the web UI's JS never receives a
  secret key, only names/public keys/messages/signatures.
- Keystore: AES-256-GCM, key derived via PBKDF2-HMAC-SHA256 (600k rounds)
  from a password prompted at startup. Rounds are a **compile-time constant**
  (not persisted in the keystore file) — bumping rounds invalidates existing
  stores; delete `~/.multisig-tool/identities.dat` (or your `--store` path)
  and re-import identities after an upgrade. `MULTISIG_TOOL_PWD` is honored only
  when `MULTISIG_TOOL_ALLOW_ENV_PWD=1` is also set (scripting); otherwise the
  tool refuses the env password with a clear error. Not `rusk-wallet`'s
  wallet format (that's one BIP39-seed wallet — wrong shape for N
  independently-named identities); reuses the same class of vetted crates
  instead of inventing a new format.
- The local RPC (`serve`) binds `127.0.0.1` only — refuses any other bind
  address outright (see `rpc::serve`'s check). Every `/api/*` route requires
  a random bearer token generated at process start
  (`X-Multisig-Tool-Token` header) — printed once to the terminal, embedded
  into the served `index.html`. No token, no access (`401`).
- `--network testnet` / the testnet RUES base URL are hard-coded, not
  configurable via any flag, env var, or UI control.
- All shelling-out uses argument arrays (`std::process::Command`), never a
  shell string — no injection surface from user-typed message text or names.

## Known caveats

- **Signing scheme**: uses post-hardfork secure `sign`/`sign_multisig`.
  Real testnet is past Aegis/`PreFork` and rejects `sign_insecure` —
  confirmed for RFQ (`rfq-settlement/README.md`) and for this registry
  (`member_matches=1, sigs_ok=0` under insecure; secure `change_account`
  succeeds). `VM::ephemeral()` unit tests in `multisig-registry` still
  sign with `_insecure` because dusk-vm defaults host-query policy to
  `HardFork::PreFork` with no public override — ephemeral VM tests cannot
  reach post-hardfork signing policy without upstream dusk-vm support.
  Matching the test suite's `_insecure` calls in this tool is wrong for
  live testnet.
- **RUES free-reads must use raw bodies** (see Scope). An early client bug
  hex-encoded requests and looked like “stuck `account` not found” /
  upstream lag; that was not a node or contract-state failure.
- **Free-read `verify_quorum` / `verify_quorum_aggregate` / `diagnose_quorum`:**
  with raw RUES these no longer 500, but can report `false` /
  `sigs_ok=0` for secure signatures that succeed in transaction
  execution (`change_account`). Do not treat free-read verify as the
  source of truth for live correctness; use writes + account reads.

## Status

- **Package version `0.2.0`** — see [CHANGELOG.md](CHANGELOG.md). Policy: [docs/versioning.md](../../docs/versioning.md). `multisig-tool --version` prints the same string.
- **Nocturne Lab UI (2026-07-28)** — static HTML uses synced `static/lab/*`
  tokens/layout. Product override: `--you: #2b6cb0` in `static/style.css`.
- **Website demo Lab (2026-07-26)** — `DEMO_MODE=mock` (**default**) uses an
  in-process `MockLedger` for account/proposal APIs (approve still signs the
  digest with real local BLS; chain submit is skipped in mock only).
  `DEMO_MODE=testnet` keeps the live chain path. Lab UI on **`:8877`**: cover /
  demo studio / use cases slides; default path is a **five-beat** proposals
  walkthrough (Cast → Form council → Look up → Propose & first approve →
  Threshold & finalize); advanced panels (Setup, Aggregate, Rotate, Unsafe
  UTF-8, Party finder) live in the **developer drawer**. Setup
  works in mock; Aggregate, Rotate, Unsafe UTF-8, and Party finder return 501 in mock — set `DEMO_MODE=testnet` and restart. Public
  story (no hosted signing):
  [`docs.nocturne-standards.org/v1/knot/`](https://docs.nocturne-standards.org/v1/knot/).
- **PM council resolve** — peeled to wen `pm-council-tool` (2026-08-04 public-launch P0).
  Knot no longer ships `pm-resolve` CLI/UI/RPC or `council_resolve_*` in
  `multisig-encoding`. Collector may still relay `kind=pm_council_resolve` blobs
  for wen wire compatibility — see `multisig-collector` README.
- **Signing preview/confirm (2026-07-24)** — proposal approve and blob sign
  print the fingerprint first; CLI requires `--confirm`; HTTP sign endpoints
  require `"confirm": true` (400 otherwise). UI: Preview → show mnemonic →
  checkbox → Sign. Quorum “type any message” lab panels remain an **unsafe
  demo** (arbitrary UTF-8, not a canonical intent) — use the five-beat
  proposals path / PM resolve for real authorizations.
- **`init` + first-run script (2026-07-23)** — `multisig-tool init [--name
  alice] [--store path]` creates the local identity store if missing
  (prompts for a new password twice, refuses on mismatch), optionally with
  a first identity; against an existing store it's an idempotent
  unlock-check + summary (never overwrites, never silently adds an
  identity a rerun's `--name` implies). `scripts/multisig-first-run.sh`
  wraps it end to end from repo root: builds the release binary, warns if
  `rusk-wallet` isn't on `PATH` or `RUSK_WALLET_PWD` is unset, pings a
  `multisig-collector`'s `/v1/health` if `MULTISIG_COLLECTOR_URL` is set,
  runs `init`, then prints the `serve` command (or starts it with
  `--serve`).
- **Collector client (2026-07-23)** — `blob push|pull` and `blob sign
  --collector <url> --id <id>` talk to `multisig-collector` over plain HTTP
  (no `multisig-collector` Cargo dependency — see `src/collector_client.rs`
  module doc). `party list|signup` drives the same server's
  party-finder roster (upsert-only; no leave/DELETE). Credentials are HTTP Basic Auth from
  `MULTISIG_COLLECTOR_URL`/`_USER`/`_PASSWORD` env vars (no `--user`/
  `--password` flags, so a password never lands in shell history). The
  collector never sees a secret key or an unsigned digest it could forge —
  every signer still gates+recomputes the §4a digest locally before signing.
  Local AC: `cargo test --test collector_roundtrip` (spawns the real
  `multisig-collector` binary as its own process, drives a 2-of-3
  push → sign → sign → pull → aggregate over real HTTP).
- **v0.1.0 + M2 file blob (2026-07-23)** — topology B: `blob create|show|sign|
  aggregate|submit-agg` moves a JSON `ProposalBlob` over any BYO channel;
  combiner aggregates `MultisigSignature` and submits one
  `verify_quorum_aggregate`. Local AC:
  `cargo test --test blob_aggregate_local` (builds against registry WASM;
  PreFork insecure sigs). QR deferred. Collector holds no secret keys.
- **M3 fingerprint (2026-07-23)** — `blob show` / `blob fingerprint` /
  `proposal approve` print full-digest hex + 24-word BIP39 mnemonic + safety
  number for out-of-band compare. Hardware keys: research note only (below).
- **M1 intent display (2026-07-23)** — structured `proposal create`
  (target / function / args / deadline), approve recomputes §4a digest and
  prints **canonical fields first** (refuses on digest mismatch). Web UI
  mirrors the same gate.
- Against `multisig-registry` / `multisig-proposals` on Dusk testnet — confirm
  contract IDs with your operator before live writes.

| Check | Result |
|---|---|
| Registry create/query/change_account | Pass |
| Quorum submit + outcome / diagnose surfacing | Pass (free-read verify still untrusted) |
| Scenario web UI (slide Lab + five-beat mock walkthrough) | Pass |
| Proposal create / approve (canonical intent) / finalize+execute | Lab green; live proposals **v0.3.1** deployed+wired 2026-07-28 |
| Adversarial digest mismatch refuse | Pass (`multisig-encoding` `gate_blob_for_signing`) |
| Pk-only import + refuse as signer | Pass |
| File/BYO blob 2-of-3 → aggregate → `verify_quorum_aggregate` | Pass (local `VM::ephemeral`) |
| Out-of-band full-digest mnemonic / safety-number | Pass (`multisig-encoding` fingerprint tests) |

### Multi-person runbook (two machines)

1. Each person: `identity new <name>` (or import the other's pk for council creation).
2. One operator creates the registry account with everyone's PKs (mix of local + `import-pk`).
3. Anyone: `proposal create --account N --target <32-byte-hex> --function <name> [--args-hex ...] [--deadline N]`.
4. Each member on their own machine: `proposal approve --id ID --signer <me>`
   (prints fingerprint; re-run with `--confirm` to submit) — tool refuses if
   digest ≠ recomputed.
5. Anyone: `proposal finalize --id ID` when approvals ≥ threshold (executes `call_raw` on target).
6. `proposal status --id ID` should show `Executed` (or `Tombstoned`).

Same flow is in the web UI "Multi-person — on-chain proposals" panel.

### Out-of-band fingerprint (M3)

Before signing (topology A approve or topology B `blob sign`), co-signers must
compare the printed **full** digest fingerprint on a second channel (call,
Signal, in person) — not via the same machine that handed them the blob:

- `hex` — full `0x` + 64 hex chars
- `mnemonic` — 24 BIP39 English words over the full 32-byte digest
- `safety-number` — grouped decimal of all 256 bits

Never trust a short truncation alone (salt-grinding trap). Compromised
coordinator cannot forge a matching mnemonic for a different intent.

### Hardware keys (research only — no implementation in this plan)

Ledger / similar devices expose BLS12-381 primarily for Ethereum 2.0
validator paths, not as a general clear-signing surface for arbitrary
Dusk/Piecrust contract call payloads. Clear-signing limits (what the device
screen can show vs what our §4a intent needs) are the blocker, not raw key
custody. Intent rendering stays in this tool for now; hardware adoption is a
later product decision if device firmware gains usable BLS clear-signing for
our preimage shape. **No follow-up work in the current suite plan.**

### Monitoring note

Atlas authority / service-repoint changes are timelocked (Atlas is outside this
repo). Operators should alarm on the change *and* on unexpected silence. Registry `change_account` remains the membership path
with built-in nonce replay protection.

## Usage

### Build

```bash
cd multisig/crates/multisig-tool
cargo build --release
```

### First run

```bash
scripts/multisig-first-run.sh [--name alice] [--store path] [--bind 127.0.0.1:8877] [--serve]
```

Builds the release binary, warns about a missing `rusk-wallet` /
`RUSK_WALLET_PWD` (needed for chain-writing commands, not `init` itself),
optionally checks a `multisig-collector`'s health (`MULTISIG_COLLECTOR_URL`),
then runs `init` and prints (or, with `--serve`, starts) the `serve`
command. Safe to re-run — `init` against an existing store is just an
unlock-check + summary.

### CLI

```bash
export RUSK_WALLET_PWD=...      # rusk-wallet keystore password (testnet)
# Scripting only — both required or the tool refuses the env password:
export MULTISIG_TOOL_ALLOW_ENV_PWD=1
export MULTISIG_TOOL_PWD=...    # or omit both to be prompted (init prompts twice on first creation)

multisig-tool init --name alice   # creates the store if missing, else unlock-check + summary
multisig-tool identity new alice
multisig-tool identity list
multisig-tool identity export alice
# On another machine / store: import a foreign member PK (cannot sign):
# multisig-tool identity import-pk bob <base58-or-hex-pk>

multisig-tool account create --member alice --member bob --threshold 2
multisig-tool account query 0
multisig-tool account meta 0
multisig-tool account keys 0
multisig-tool account next-id

# Lab-only unsafe demo — arbitrary UTF-8 message, not a structured intent:
multisig-tool quorum submit --account 0 --msg "hello" --signer alice --signer bob
multisig-tool quorum check --account 0 --msg "hello" --signer alice --signer bob
multisig-tool quorum diagnose --account 0 --msg "hello" --signer alice --signer bob
multisig-tool quorum-agg submit --account 0 --msg "hello" --signer alice --signer bob

multisig-tool change-account submit --account 0 \
  --new-member alice --new-member carol --new-threshold 2 \
  --signer alice --signer bob   # must be a quorum of the CURRENT members
# optional: --nonce N to bypass account free-read when diagnosing

# Multi-person proposals (after deploy + one-time `proposal init-registry` + `init_chain_id`)
multisig-tool proposal create --account 0 \
  --target <32-byte-hex-ContractId> --function set_value --args-hex <rkyv-hex>
multisig-tool proposal approve --id 0 --signer alice            # preview fingerprint
multisig-tool proposal approve --id 0 --signer alice --confirm  # sign + submit
multisig-tool proposal approve --id 0 --signer bob --confirm
multisig-tool proposal status --id 0
multisig-tool proposal finalize --id 0

# Topology B — file / BYO channel (QR deferred). Move the JSON between machines.
multisig-tool blob create --out proposal.json --committee-id 0 --threshold 2 \
  --target <32-byte-hex> --function milestone_release --args-hex <hex>
multisig-tool blob show proposal.json
# Machine A:
multisig-tool blob sign --file proposal.json --signer alice --out proposal.json --confirm
# Machine B (after receiving the file):
multisig-tool blob sign --file proposal.json --signer bob --out proposal.json --confirm
multisig-tool blob aggregate proposal.json
multisig-tool blob submit-agg --file proposal.json --account 0

# Topology B via multisig-collector (untrusted relay — no keys held server-side).
export MULTISIG_COLLECTOR_URL=http://127.0.0.1:8899
export MULTISIG_COLLECTOR_USER=...       # optional; omit for no Basic Auth
export MULTISIG_COLLECTOR_PASSWORD=...   # optional
multisig-tool blob push --file proposal.json           # prints the content-addressed id
multisig-tool blob sign --id <id> --signer alice --confirm
multisig-tool blob sign --id <id> --signer bob --out proposal.json --confirm
multisig-tool blob pull --id <id> --out proposal.json
multisig-tool blob aggregate proposal.json

# PM council resolve → wen pm-council-tool (not multisig-tool)

multisig-tool party signup --name alice --pk <base58-or-hex-pk>
multisig-tool party list
# (no party leave — collector roster is upsert-only; clear the DB to remove a row)
```

Writes print `=== fn: tx included/propagated ===` or `=== fn: FAIL (contract panic) ===`
plus any `Panic: ...` line (governance counters). Quorum submit also runs a
free-read diagnose/check follow-up and warns when it looks untrusted.
`--store <path>` (global flag) overrides the default keystore location
(`~/.multisig-tool/identities.dat`).

### Web UI

```bash
multisig-tool serve --bind 127.0.0.1:8877
```

Prints a loopback URL (`http://127.0.0.1:8877/`) and the API auth header
name (`X-Multisig-Tool-Token`). The token value is **not** printed in the
URL — it is injected into the served HTML only. Or via this repo's preview convention:
`scripts/run-multisig-tool-native.sh` (wired into `.claude/launch.json` as
`multisig-tool`, port 8877) — uses a fixed dev password
(`MULTISIG_TOOL_ALLOW_ENV_PWD=1` + `MULTISIG_TOOL_PWD=local-dev-only`), fine for local dev only.

The UI matches the Agent Pay demo visual language (Literata/Sora, cream/sky)
as **three slides**: cover → demo studio → use cases. Default mode is
`DEMO_MODE=mock`; set `DEMO_MODE=testnet` for live chain writes. In the
studio, **Start walkthrough** runs the **five-beat** proposals path
(Cast → Form council → Look up → Propose & first approve → Threshold &
finalize) — creates alice/bob/carol, prefills fields, and advances on
success. A shared narrator updates per beat; each beat has a green
“Values to enter” card.

Advanced panels live in the **developer drawer** (not the default path):
**Setup** (keystore unlock status + collector URL from server-side
`MULTISIG_COLLECTOR_*` — browser never sees the Basic Auth password —
and first identity create), Aggregate verify, Rotate, Unsafe UTF-8,
**Party finder** (signs a local identity's *public* key up to the
collector roster via `/api/party`; multi-select prefills Form council;
roster is discovery only), and PM resolve. Aggregate / Rotate / Unsafe /
Party / PM resolve return 501 in mock (Setup does not).

*(Historical: the Lab used to be ten chapter tabs with Setup as the
default landing tab and Party finder as “chapter 8”; that tab strip is
gone — beats + drawer replaced it.)*

## Explicitly out of scope (this pass)

- Hosted / public Multisig Lab or signing subdomain (public docs only:
  [`docs.nocturne-standards.org/v1/knot/`](https://docs.nocturne-standards.org/v1/knot/)).
- Dusk Wallet Extension / Dusk Connect `dusk_signMessage` integration as an
  alternative signer — still unverified whether the real extension
  implements it and in what byte format.
- A polished `ratatui` TUI — plain CLI subcommands cover the
  scriptable/headless path today.
