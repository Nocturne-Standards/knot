# nocturne-knot-tool

A local signing CLI and web Lab for the Knot multisig suite. It exercises the
`knot-registry` and `knot-proposals` contracts on the Dusk testnet: create
accounts, run quorum flows, and take a proposal through create, approve,
finalize and execute across several people.

The package is `nocturne-knot-tool`, the binary is `knot-tool`, and the
library is `knot_tool`. The CLI is scriptable; the same binary serves a local
web UI with `knot-tool serve`.

Status: testnet only, not audited yet. Do not use it with mainnet keys or funds.

Source: <https://github.com/Nocturne-Standards/knot>

## Install

```bash
cargo install nocturne-knot-tool
```

Or build from a checkout of the repository:

```bash
cargo build -p nocturne-knot-tool --release
```

Chain writes shell out to the `rusk-wallet` CLI, which must be on `PATH`, with
its keystore password in `RUSK_WALLET_PWD`. Reads go straight to the node and
need no wallet.

## Configuration

The repository holds no contract ids. Set the ids of the contracts you run
against:

```bash
export KNOT_REGISTRY_ID=<64 hex chars>
export KNOT_PROPOSALS_ID=<64 hex chars>
```

A command that needs a contract fails with a message naming the missing
variable.

The testnet node URL and chain id come from Dusk's published network data,
vendored in `data/dusk-networks/`. Every chain call targets testnet. Set
`KNOT_CHAIN_ID` to override the chain id that proposal digests bind to.

Other environment variables:

| Variable | Purpose |
|---|---|
| `RUSK_WALLET_PWD` | Unlocks `rusk-wallet` for gas-paying chain writes |
| `KNOT_PWD` with `KNOT_ALLOW_ENV_PWD=1` | Keystore password for scripting. `KNOT_PWD` alone is refused. |
| `KNOT_STORE` | Keystore path (same as the global `--store` flag) |
| `KNOT_COLLECTOR_URL`, `KNOT_COLLECTOR_USER`, `KNOT_COLLECTOR_PASSWORD` | Optional relay for sharing proposals (HTTP Basic Auth). Credentials come only from the environment, so they stay out of shell history. |
| `DEMO_MODE` | `mock` or `testnet`. Required by `serve`. |

## Quick start

```bash
# Create the local identity store (prompts for a password twice).
knot-tool init --name alice

# Start the web Lab. DEMO_MODE=mock uses an in-process ledger, no chain.
DEMO_MODE=mock knot-tool serve --bind 127.0.0.1:8877
```

Open the bootstrap URL that `serve` prints (`http://127.0.0.1:8877/?code=...`).
Running `init` against an existing store only unlocks it and prints a summary;
it never overwrites identities. From a checkout you can use
`cargo run -p nocturne-knot-tool -- <args>` instead of the installed binary.

## CLI

Commands that sign or submit print a preview with the digest fingerprint
first. Re-run with `--confirm` to sign and send.

```bash
knot-tool identity new alice
knot-tool identity list
knot-tool identity export alice               # prints the public key
knot-tool identity import-pk bob <base58-or-hex-pk>   # member that cannot sign here
knot-tool change-password                     # new password is always prompted

knot-tool account create --member alice --member bob --threshold 2
knot-tool account query 0
knot-tool account set-timelock --account 0 --blocks 5 --signer alice --signer bob --confirm
knot-tool account execute-pending --account 0
knot-tool account cancel-pending --account 0 --signer alice --signer bob --confirm

knot-tool change-account submit --account 0 \
  --new-member alice --new-member carol --new-threshold 2 \
  --signer alice --signer bob --confirm

# Sign a 32-byte digest your application already domain-separated.
knot-tool sign-digest --signer alice --digest <64-hex> --domain MYAPP/ACTION/v1 --confirm
```

The `quorum` and `quorum-agg` subcommands (`submit`, `check`, `diagnose`) sign
an arbitrary UTF-8 message. They are a lab demo, not a canonical intent: use
proposals for real authorizations.

### Proposals

A proposal names a target contract, a function and its arguments. Each
approver recomputes the digest locally and the tool refuses to sign if it does
not match.

```bash
knot-tool proposal create --account 0 --target <32-byte-hex> \
  --function <name> [--args-hex <rkyv-hex>] [--deadline <block>]
knot-tool proposal approve --id 0 --signer alice            # preview
knot-tool proposal approve --id 0 --signer alice --confirm  # sign and submit
knot-tool proposal status --id 0
knot-tool proposal finalize --id 0
knot-tool proposal execute --id 0
knot-tool proposal cancel --id 0 --signer alice --signer bob --confirm
```

Finalize once approvals reach the threshold. With a delay of 0 the call runs
at once; with a delay above 0 it queues until `proposal execute`.

### Signing over a file or relay

`blob` moves a JSON proposal between machines over any channel you choose:

```bash
knot-tool blob create --out proposal.json --committee-id 0 --threshold 2 \
  --target <32-byte-hex> --function <name> --args-hex <hex>
knot-tool blob show proposal.json
knot-tool blob sign --file proposal.json --signer alice --out proposal.json --confirm
knot-tool blob aggregate proposal.json
knot-tool blob submit-agg --file proposal.json --account 0
```

With the `KNOT_COLLECTOR_*` variables set you can use a `knot-collector` relay
instead of passing files: `blob push --file`, `blob sign --id <id> --signer
<name> --confirm`, `blob pull --id <id> --out <file>`. The relay never holds a
secret key. `party signup --name <n> --pk <pk>` and `party list` use the same
relay's roster.

`blob show`, `blob fingerprint` and `proposal approve` print the full digest in
hex, 24 fingerprint words and a safety number. Compare them with your
co-signers on a second channel before signing.

### DuskEVM

Commands for a council to publish its member set to a `KnotEvmRoot` contract
on DuskEVM and sign actions that `KnotEvmRoot.checkAction` accepts. Partial
signatures travel in one JSON file of public data. Each member signs on their
own machine with their own `--store`.

```bash
knot-tool --store <store> evm bootstrap-sign --account 0 \
  --receiver 0x<KnotEvmRoot> --signer alice --file bootstrap.json --confirm
knot-tool evm bootstrap-submit --file bootstrap.json --confirm

knot-tool --store <store> evm action-sign --account 0 --root 0x<KnotEvmRoot> \
  --rotation-nonce 0 --consumer 0x<caller> --digest 0x<32 bytes> \
  --signer alice --file action.json --confirm
knot-tool evm action-args --file action.json   # prints the cast send arguments
```

`--evm-chain` defaults to the DuskEVM testnet chain id from the vendored
network data. The tool refuses to add a signature to a file from a different
round, from a non-member, or a second one from the same member.

## Web Lab

`knot-tool serve [--bind 127.0.0.1:8877]` serves a local UI and RPC. The
default path is a guided proposals walkthrough; advanced panels sit in a
developer drawer. In `mock` mode some advanced panels return 501; use
`DEMO_MODE=testnet` for those.

## Security model

This is test tooling, not production key custody. In production each identity
should keep its key in its own wallet. The local store exists so developers can
exercise multisig flows. Do not treat it as a hardened wallet.

- Secret keys live in process memory or in the local keystore file. Signing
  happens in the server process; the web UI never receives a secret key.
- The keystore is AES-256-GCM, with the key derived by Argon2id. Older
  PBKDF2-based stores are re-saved in the current format on load. Saves are
  atomic, keep a `.bak` copy, and use file mode `0600` in a `0700` directory on
  Unix. A store with looser permissions is refused. The default location is the
  platform data directory.
- `KNOT_PWD` is honored only with `KNOT_ALLOW_ENV_PWD=1`.
- `serve` binds loopback addresses only. It prints a one-shot bootstrap URL
  that sets an HttpOnly, `SameSite=Strict` session cookie; `/api/*` requests
  without it get 401.
- External commands run with argument arrays, not shell strings.

## License

Apache-2.0. See `LICENSE`.
