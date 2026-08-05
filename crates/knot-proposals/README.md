# knot-proposals

On-chain **propose → approve → finalize+execute** (`abi::call_raw`) using
[`knot-registry`](../knot-registry/) for membership/threshold.
Signed bytes are the §2.12 v3 digest from [`knot-encoding`](../knot-encoding/).

## Status

- **v3** — epoch counter, caller-supplied proposal uniquifier (`ProposeArgs.nonce`),
  `abi::chain_id()` + `abi::self_id()` in digests, `consumed` digest records,
  permissionless `prune`, rich events (§2.13). **Burns v2** — redeploy fresh;
  no state migration.
- Prior **v0.3.x** testnet pins are obsolete after v3 cutover.

## Functions

| Method | Notes |
|--------|--------|
| `init_registry` | Owner; bumps `epoch` (invalidates prior proposals, O(1)) |
| `set_proposal_ttl` | Owner; ceiling only, no wipe (`> 0`, `≤ MAX_PROPOSAL_TTL`) |
| `set_tombstone(bool)` | Owner; no invalidation |
| `propose(ProposeArgs) -> id` | Explicit non-zero `deadline`; caller `nonce`; merge identical open digests |
| `approve` / `finalize` | BLS over digest; CEI before `call_raw`; blocks self-target finalize |
| `prune(limit) -> count` | Permissionless payload reclamation |
| `epoch` / `proposal_ttl` / `proposal` / `status` | Reads |

## Deploy order (v3)

1. Deploy **registry v3** (new `change_account` digest domain).
2. Deploy **proposals v3**, then `init_registry(registry_id)`.
3. Re-create councils / re-sign all intents — **v2 signatures are burned**.

## Build / test

```bash
cd ../knot-registry && make wasm
cd ../knot-proposals && make test
```
