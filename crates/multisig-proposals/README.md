# multisig-proposals

On-chain **propose → approve → finalize+execute** (`abi::call_raw`) using
[`multisig-registry`](../multisig-registry/) for membership/threshold.
Signed bytes are the §4a digest from [`multisig-encoding`](../multisig-encoding/).

## Status

- **v0.3.0** — CEI `finalize` (status + nonce before `call_raw`); `tombstone: bool`
  (default `false` → `Executed`); propose caps (`function_name` ≤ 64,
  `call_args` ≤ 4096); reject past deadlines at propose; `init_chain_id` wipes
  open proposals; require `init_chain_id` before propose.
- Bytecode changed for `EncodingError` / fallible `proposal_digest` (audit
  2026-07-28 #5); **testnet redeploy still pending** (local wasm/tests only).
- Lab AC: `make test` green under `VM::ephemeral()` (includes `test-target`
  execute / reentrancy / failed-`call_raw` paths).
- **Testnet:** **v0.3.0** live (2026-07-24 cutover); `init_registry` →
  registry v0.1.3, `init_chain_id` = `2`. See
  `../../../deployments/testnet.json`.
- Unit tests use `sign_insecure` (PreFork). Live clients must use secure `sign`.

## Functions

| Method | Notes |
|--------|--------|
| `init_registry` / `init_chain_id` | Owner; both wipe open proposals |
| `set_proposal_ttl` | Owner; wipes open proposals |
| `set_tombstone(bool)` | Owner; **no** wipe. `true` → finalize marks `Tombstoned` |
| `propose(ProposeArgs) -> id` | Needs `chain_id`; caps + past-deadline reject; identical open digests merge |
| `approve(ApproveArgs)` | BLS over 32-byte digest |
| `finalize(id)` | Quorum → mark terminal + bump nonce + emit → `call_raw` |
| `committee_nonce` / `proposal` / `status` | Reads |

## Finalize / failed execute

`finalize` applies effects **before** `call_raw` so a reentrant target cannot
double-execute. If `call_raw` fails, the host still panics and the **whole
transaction reverts** — the proposal stays `Open` and the committee nonce is
unchanged, so operators can retry. Failed execute does **not** consume the
proposal (differs from some Dusk examples that mark executed regardless).

## Build / test

```bash
cd ../multisig-registry && make wasm
cd ../multisig-proposals && make test
```

## Deploy

When redeploying v0.3.0, bump `deployments/testnet.json` and re-`init_registry` /
`init_chain_id` (and `set_tombstone` if you want `Tombstoned` on success).
