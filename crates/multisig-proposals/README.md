# multisig-proposals

On-chain **propose → approve → finalize+execute** (`abi::call_raw`) using
[`multisig-registry`](../multisig-registry/) for membership/threshold.
Signed bytes are the §4a digest from [`multisig-encoding`](../multisig-encoding/).

## Status

- **v0.3.1** — CEI `finalize` (status + nonce before `call_raw`); `tombstone: bool`
  (default `false` → `Executed`); propose caps (`function_name` ≤ 64,
  `call_args` ≤ 4096); reject past deadlines at propose; `init_chain_id` wipes
  open proposals; require `init_chain_id` before propose; fallible
  `proposal_digest` / `EncodingError` (audit 2026-07-28 #5).
- Lab AC: `make test` green under `VM::ephemeral()` (includes `test-target`
  execute / reentrancy / failed-`call_raw` paths).
- **Testnet:** **v0.3.1** live (2026-07-28 cutover); `init_registry` →
  registry v0.1.4, `init_chain_id` = `2`. See
  `../../../deployments/testnet.json`.
- Unit tests use `sign_insecure` (PreFork). Live clients must use secure `sign`.

**Source divergence (Spec 26, `823ca2f` / extraction `117183c..823ca2f`):**
`SignatureEntry` / `VerifyQuorumArgs` / `MultisigAccountView` now come from
`multisig-encoding` behind the `call-types` feature. Source differs from
deployed v0.3.1 wasm, but layout-golden hex is byte-identical
(**IDENTICAL**). Carry indefinitely; no redeploy required (same Track 9 /
2026-08-01 standing decision as Wave 3 byte-identical type moves). Derives
moved unchanged (R12); no derive edits in the adoption commits.

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
