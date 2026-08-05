# knot-tool

Local signing tool + web UI for exercising [`knot-registry`](../knot-registry/README.md)
and [`knot-proposals`](../knot-proposals/README.md) against the real
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

Cold-start: `cargo run -p knot-tool -- serve --bind 127.0.0.1:8877` from repo root.

Everything below is lab **Status**, env, and maintainer run notes. Dense on purpose.

## Quick start

From this repo root:

```bash
export RUSK_WALLET_PWD=...   # rusk-wallet keystore password (testnet)
# Optional scripting — both required, or omit both and type the keystore password:
export KNOT_ALLOW_ENV_PWD=1
export KNOT_PWD=...                      # unlocks default identity store (see Security model)

# Shared collector (if you use one) — password is the nginx htpasswd you handed out:
export KNOT_COLLECTOR_URL=https://collector.nocturne-standards.org
export KNOT_COLLECTOR_USER=demo                # collector htpasswd user (not WEN metadata user)
export KNOT_COLLECTOR_PASSWORD=...

cargo run -p knot-tool -- serve --bind 127.0.0.1:8877
# or: cargo run -p knot-tool -- serve --bind 127.0.0.1:8877
```

Set `DEMO_MODE=mock` or `DEMO_MODE=testnet`, then open the printed bootstrap URL
(`http://127.0.0.1:8877/?code=…`) — one-shot OTP sets an HttpOnly session cookie;
the secret is never embedded in HTML. Re-running `init` against an existing store
only unlocks + summarizes — it does not wipe identities.

| Env | Purpose |
|---|---|
| `RUSK_WALLET_PWD` | Gas-paying `rusk-wallet` unlock (chain writes) |
| `KNOT_PWD` + `KNOT_ALLOW_ENV_PWD=1` | Local identity keystore; refuse env pwd without the latch |
| `KNOT_COLLECTOR_*` | Optional HTTP Basic Auth client → shared relay |

**Share the collector with co-signers:** give them the three `KNOT_COLLECTOR_*`
values (out of band). They run the same Quick start on their laptop — keys never
leave their machine. Ops detail (htpasswd, nginx, participant checklist): bring
your own reverse-proxy wiring — the collector HTTP API in
`crates/knot-collector/README.md` is the contract.

**PM dispute council:** use wen
[`pm-council-tool`](https://github.com/aichbindas/wen) (product UX for council
resolve). Knot knot-tool covers registry, proposals, and generic Lab only.

## Scope

This tool owns both ends of the wire — the contract's own source
(`knot-registry/src/call_types.rs`, included directly via `#[path = ...]`,
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

**Test tooling only — not production key custody.** In production each identity
holds its own key in its own wallet. This local encrypted store exists so
developers can exercise multisig flows; do not treat it as a hardened wallet.

- Every identity's secret key lives only in this process (in memory) or the
  local encrypted keystore file (platform data dir by default, see below) —
  signing happens server-side; the web UI's JS never receives a secret key,
  only names/public keys/messages/signatures.
- Keystore v2: AES-256-GCM with header bytes as associated data; key derived via
  **Argon2id** (64 MiB, t=3, p=4) for new stores. Legacy v1 files (PBKDF2
  600k rounds, JSON plaintext) load once and are silently re-saved as v2.
  Saves are atomic (`tmp` + `rename` + directory fsync; `F_FULLFSYNC` on
  macOS), rotate a `.bak` copy, and create files at mode `0o600` / parent
  `0o700` on Unix. Over-permissive stores are refused on load.
  `identity export <name>` prints a public key for sharing;
  `identity import-pk <name> <pk>` adds a pk-only member. `KNOT_PWD` is honored
  only when `KNOT_ALLOW_ENV_PWD=1` is also set (scripting); otherwise the tool
  refuses the env password with a clear error. Not `rusk-wallet`'s wallet format
  (that's one BIP39-seed wallet — wrong shape for N independently-named
  identities); reuses vetted crates instead of inventing a new format.
- The local RPC (`serve`) binds loopback only — refuses any non-loopback
  address (see `rpc::validate_loopback_bind`). `serve` requires explicit
  `DEMO_MODE=mock` or `DEMO_MODE=testnet`. Session auth: CLI prints a
  one-shot bootstrap URL (`/?code=…`); visiting it sets an HttpOnly
  `SameSite=Strict` session cookie used for `/api/*`. No cookie → `401`.
  `X-Knot-Token` header is optional (tests/scripting only); HTML never
  embeds the session secret.
- `--network testnet` / the testnet RUES base URL are hard-coded, not
  configurable via any flag, env var, or UI control.
- All shelling-out uses argument arrays (`std::process::Command`), never a
  shell string — no injection surface from user-typed message text or names.

## Known caveats

- **Signing scheme**: uses post-hardfork secure `sign`/`sign_multisig`.
  Real testnet is past Aegis/`PreFork` and rejects `sign_insecure` —
  confirmed for RFQ (`rfq-settlement/README.md`) and for this registry
  (`member_matches=1, sigs_ok=0` under insecure; secure `change_account`
  succeeds). `VM::ephemeral()` unit tests in `knot-registry` still
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

- **Package version `0.2.0`** — see [CHANGELOG.md](CHANGELOG.md). Policy: [docs/versioning.md](../../docs/versioning.md). `knot-tool --version` prints the same string.
- **Nocturne Lab UI (2026-07-28)** — static HTML uses synced `static/lab/*`
  tokens/layout. Product override: `--you: #2b6cb0` in `static/style.css`.
- **Website demo Lab (2026-07-26)** — `serve` **requires** explicit
  `DEMO_MODE=mock` (in-process `MockLedger` for account/proposal APIs;
  approve still signs the digest with real local BLS; chain submit is skipped
  in mock only) or `DEMO_MODE=testnet` (live chain path). Unset or unknown
  values refuse start. Lab UI on **`:8877`**: cover /
  demo studio / use cases slides; default path is a **five-beat** proposals
  walkthrough (Cast → Form council → Look up → Propose & first approve →
  Threshold & finalize); advanced panels (Setup, Aggregate, Rotate, Unsafe
  UTF-8, Party finder) live in the **developer drawer**. Setup
  works in mock; Aggregate, Rotate, Unsafe UTF-8, and Party finder return 501 in mock — set `DEMO_MODE=testnet` and restart. Public
  story (no hosted signing):
  [`docs.nocturne-standards.org/v1/knot/`](https://docs.nocturne-standards.org/v1/knot/).
- **PM council resolve** — peeled to wen `pm-council-tool` (2026-08-04 public-launch P0).
  Knot no longer ships `pm-resolve` CLI/UI/RPC or `council_resolve_*` in
  `knot-encoding`. Collector may still relay `kind=pm_council_resolve` blobs
  for wen wire compatibility — see `knot-collector` README.
- **Signing preview/confirm (2026-07-24)** — proposal approve, blob sign,
  quorum submit, quorum-agg submit, and change-account submit print the
  fingerprint first; CLI requires `--confirm`; HTTP sign endpoints require
  `"confirm": true` (400 otherwise). UI: Preview → show mnemonic → confirm →
  Submit. Prefer one signer identity per `serve` process (soft note when
  multiple local signers are requested). Quorum “type any message” lab panels
  remain an **unsafe demo** (arbitrary UTF-8, not a canonical intent) — use
  the five-beat proposals path for real authorizations.
- **`init` + first-run script (2026-07-23)** — `knot-tool init [--name
  alice] [--store path]` creates the local identity store if missing
  (prompts for a new password twice, refuses on mismatch), optionally with
  a first identity; against an existing store it's an idempotent
  unlock-check + summary (never overwrites, never silently adds an
  identity a rerun's `--name` implies). `knot-tool init` / `serve`
  wraps it end to end from repo root: builds the release binary, warns if
  `rusk-wallet` isn't on `PATH` or `RUSK_WALLET_PWD` is unset, pings a
  `knot-collector`'s `/v1/health` if `KNOT_COLLECTOR_URL` is set,
  runs `init`, then prints the `serve` command (or starts it with
  `--serve`).
- **Collector client (2026-07-23)** — `blob push|pull` and `blob sign
  --collector <url> --id <id>` talk to `knot-collector` over plain HTTP
  (no `knot-collector` Cargo dependency — see `src/collector_client.rs`
  module doc). `party list|signup` drives the same server's
  party-finder roster (upsert-only; no leave/DELETE). Credentials are HTTP Basic Auth from
  `KNOT_COLLECTOR_URL`/`_USER`/`_PASSWORD` env vars (no `--user`/
  `--password` flags, so a password never lands in shell history). The
  collector never sees a secret key or an unsigned digest it could forge —
  every signer still gates+recomputes the §4a digest locally before signing.
  Local AC: `cargo test --test collector_roundtrip` (spawns the real
  `knot-collector` binary as its own process, drives a 2-of-3
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
  applies the same gate.
- Against `knot-registry` / `knot-proposals` on Dusk testnet — confirm
  contract IDs with your operator before live writes.

| Check | Result |
|---|---|
| Registry create/query/change_account | Pass |
| Quorum submit + outcome / diagnose surfacing | Pass (free-read verify still untrusted) |
| Scenario web UI (slide Lab + five-beat mock walkthrough) | Pass |
| Proposal create / approve (canonical intent) / finalize+execute | Lab green; live proposals **v0.3.1** deployed+wired 2026-07-28 |
| Adversarial digest mismatch refuse | Pass (`knot-encoding` `gate_blob_for_signing`) |
| Pk-only import + refuse as signer | Pass |
| File/BYO blob 2-of-3 → aggregate → `verify_quorum_aggregate` | Pass (local `VM::ephemeral`) |
| Out-of-band full-digest mnemonic / safety-number | Pass (`knot-encoding` fingerprint tests) |

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
cd crates/knot-tool
cargo build --release
```

### First run

```bash
cargo run -p knot-tool -- init [--name alice]
cargo run -p knot-tool -- serve [--store path] [--bind 127.0.0.1:8877]
```

Builds the release binary, warns about a missing `rusk-wallet` /
`RUSK_WALLET_PWD` (needed for chain-writing commands, not `init` itself),
optionally checks a `knot-collector`'s health (`KNOT_COLLECTOR_URL`),
then runs `init` and prints (or, with `--serve`, starts) the `serve`
command. Safe to re-run — `init` against an existing store is just an
unlock-check + summary.

### CLI

```bash
export RUSK_WALLET_PWD=...      # rusk-wallet keystore password (testnet)
# Scripting only — both required or the tool refuses the env password:
export KNOT_ALLOW_ENV_PWD=1
export KNOT_PWD=...    # or omit both to be prompted (init prompts twice on first creation)

knot-tool init --name alice   # creates the store if missing, else unlock-check + summary
knot-tool identity new alice
knot-tool identity list
knot-tool identity export alice
# On another machine / store: import a foreign member PK (cannot sign):
# knot-tool identity import-pk bob <base58-or-hex-pk>

knot-tool account create --member alice --member bob --threshold 2
knot-tool account query 0
knot-tool account meta 0
knot-tool account keys 0
knot-tool account next-id

# Lab-only unsafe demo — arbitrary UTF-8 message, not a structured intent:
knot-tool quorum submit --account 0 --msg "hello" --signer alice --signer bob  # preview
knot-tool quorum submit --account 0 --msg "hello" --signer alice --signer bob --confirm
knot-tool quorum check --account 0 --msg "hello" --signer alice --signer bob
knot-tool quorum diagnose --account 0 --msg "hello" --signer alice --signer bob
knot-tool quorum-agg submit --account 0 --msg "hello" --signer alice --signer bob  # preview
knot-tool quorum-agg submit --account 0 --msg "hello" --signer alice --signer bob --confirm

knot-tool change-account submit --account 0 \
  --new-member alice --new-member carol --new-threshold 2 \
  --signer alice --signer bob   # preview fingerprint
knot-tool change-account submit --account 0 \
  --new-member alice --new-member carol --new-threshold 2 \
  --signer alice --signer bob --confirm
# optional: --nonce N to bypass account free-read when diagnosing

# Multi-person proposals (after deploy + one-time `proposal init-registry` + `init_chain_id`)
knot-tool proposal create --account 0 \
  --target <32-byte-hex-ContractId> --function set_value --args-hex <rkyv-hex>
knot-tool proposal approve --id 0 --signer alice            # preview fingerprint
knot-tool proposal approve --id 0 --signer alice --confirm  # sign + submit
knot-tool proposal approve --id 0 --signer bob --confirm
knot-tool proposal status --id 0
knot-tool proposal finalize --id 0

# Topology B — file / BYO channel (QR deferred). Move the JSON between machines.
knot-tool blob create --out proposal.json --committee-id 0 --threshold 2 \
  --target <32-byte-hex> --function milestone_release --args-hex <hex>
knot-tool blob show proposal.json
# Machine A:
knot-tool blob sign --file proposal.json --signer alice --out proposal.json --confirm
# Machine B (after receiving the file):
knot-tool blob sign --file proposal.json --signer bob --out proposal.json --confirm
knot-tool blob aggregate proposal.json
knot-tool blob submit-agg --file proposal.json --account 0

# Topology B via knot-collector (untrusted relay — no keys held server-side).
export KNOT_COLLECTOR_URL=http://127.0.0.1:8899
export KNOT_COLLECTOR_USER=...       # optional; omit for no Basic Auth
export KNOT_COLLECTOR_PASSWORD=...   # optional
knot-tool blob push --file proposal.json           # prints the content-addressed id
knot-tool blob sign --id <id> --signer alice --confirm
knot-tool blob sign --id <id> --signer bob --out proposal.json --confirm
knot-tool blob pull --id <id> --out proposal.json
knot-tool blob aggregate proposal.json

# PM council resolve → wen pm-council-tool (not knot-tool)

knot-tool party signup --name alice --pk <base58-or-hex-pk>
knot-tool party list
# (no party leave — collector roster is upsert-only; clear the DB to remove a row)
```

Writes print `=== fn: tx included/propagated ===` or `=== fn: FAIL (contract panic) ===`
plus any `Panic: ...` line (governance counters). Quorum submit also runs a
free-read diagnose/check follow-up and warns when it looks untrusted.
`--store <path>` (global flag) or `KNOT_STORE` overrides the default keystore
location (platform data dir; legacy `~/.knot/identities.dat` read fallback).

### Web UI

```bash
knot-tool serve --bind 127.0.0.1:8877
```

Prints a loopback bootstrap URL (`http://127.0.0.1:8877/?code=…`) — open it
once to set the HttpOnly session cookie. `X-Knot-Token` is optional for
tests; the session secret is never embedded in served HTML. Or via this repo's preview convention:
`scripts/run-knot-tool-native.sh` (wired into `.claude/launch.json` as
`knot-tool`, port 8877) — uses a fixed dev password
(`KNOT_ALLOW_ENV_PWD=1` + `KNOT_PWD=...`), fine for local dev only.

The UI matches the Agent Pay demo visual language (Literata/Sora, cream/sky)
as **three slides**: cover → demo studio → use cases. Set
`DEMO_MODE=mock` or `DEMO_MODE=testnet` before `serve`. In the
studio, **Start walkthrough** runs the **five-beat** proposals path
(Cast → Form council → Look up → Propose & first approve → Threshold &
finalize) — creates alice/bob/carol, prefills fields, and advances on
success. A shared narrator updates per beat; each beat has a green
“Values to enter” card.

Advanced panels live in the **developer drawer** (not the default path):
**Setup** (keystore unlock status + collector URL from server-side
`KNOT_COLLECTOR_*` — browser never sees the Basic Auth password —
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
