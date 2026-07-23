# multisig-proposals

On-chain **propose → approve → finalize+execute** (`abi::call_raw`) using
[`multisig-registry`](../multisig-registry/) for membership/threshold.
Signed bytes are the §4a digest from [`multisig-encoding`](../multisig-encoding/).

## Status

- **v0.2.0 (M1)** — structured `ProposeArgs`, per-`committee_id` nonce (option A),
  content-hash op-id (= full `signed_digest`), proposal TTL, tombstoning,
  wipe-open-on-registry/TTL config change, **execute via `call_raw`**.
- Lab AC: `make test` green under `VM::ephemeral()` (includes `test-target`
  execute path).
- **Testnet (2026-07-23):** v0.2.0 deployed; `init_registry` → live
  `multisig-registry` v0.1.2; `init_chain_id` = `2` (DuskDS testnet). Id in
  `../deployments/testnet.json`.
- Unit tests use `sign_insecure` (PreFork). Live clients must use secure `sign`.

## Functions

| Method | Notes |
|--------|--------|
| `init_registry` / `init_chain_id` | Owner config |
| `set_proposal_ttl` / `set_tombstone_ttl` | Owner; wipes open proposals |
| `propose(ProposeArgs) -> id` | Digest recomputed on-chain; identical open digests merge |
| `approve(ApproveArgs)` | BLS over 32-byte digest |
| `finalize(id)` | Quorum check → `call_raw` → bump committee nonce → Tombstoned |
| `committee_nonce` / `proposal` / `status` | Reads |

## Build / test

```bash
cd ../multisig-registry && make wasm
cd ../multisig-proposals && make test
```

## Deploy

**Live on testnet since 2026-07-23** (see Status above and `../deployments/testnet.json`).
When redeploying, bump `deployments/testnet.json` and re-`init_registry` / `init_chain_id`.
