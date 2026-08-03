# multisig-proposals

On-chain **propose → approve → finalize+execute** (`abi::call_raw`) using
[`multisig-registry`](../multisig-registry/) for membership/threshold.
Signed bytes are the §4a digest from [`multisig-encoding`](../multisig-encoding/).

## Status

- **v0.3.2** — Spec 23b Phase B `repr(C)` pin; measured **DIFFERENT**
  (`ProposeArgs` / `MultisigAccountView`; IDENTICAL on `ApproveArgs` /
  `ProposalView` / `ProposalStatus`). Status: **PINNED-DIFFERENT-REDEPLOYED**.
  Prior live pin: **v0.3.1** (2026-07-28). Fieldless `ProposalStatus` keeps
  `#[repr(u8)]` only (`archive_attr(repr(C))` rejected by rustc).
- **v0.3.1** — CEI `finalize` (status + nonce before `call_raw`); `tombstone: bool`
  (default `false` → `Executed`); propose caps (`function_name` ≤ 64,
  `call_args` ≤ 4096); reject past deadlines at propose; `init_chain_id` wipes
  open proposals; require `init_chain_id` before propose; fallible
  `proposal_digest` / `EncodingError` (audit 2026-07-28 #5).
- Lab AC: `make test` green under `VM::ephemeral()` (includes `test-target`
  execute / reentrancy / failed-`call_raw` paths).
- **Testnet:** **v0.3.2** live at
  `cc3dec84edce7a685bf5799a671dba7d927b6b9214e501108f6fa9fa382749b6`
  (2026-08-03 23b cutover); re-`init_registry` / `init_chain_id` against
  registry v0.1.5 when wiring (may stay deferred / unwired). See monorepo
  `deployments/testnet.json`.
- Unit tests use `sign_insecure` (PreFork). Live clients must use secure `sign`.
- Spec 26 source-carry paragraph cleared by this redeploy (R7).

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

When redeploying, bump `deployments/testnet.json` and re-`init_registry` /
`init_chain_id` (and `set_tombstone` if you want `Tombstoned` on success).
