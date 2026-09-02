# knot-proposals

On-chain **propose → approve → finalize** (`abi::call_raw`), with an optional
queue when the registry account's `timelock_blocks` is > 0.
[`knot-registry`](../knot-registry/) for membership/threshold.
Signed bytes are the v3 digest from [`knot-encoding`](../knot-encoding/).

## Status

- **Timelock** — **PINNED-DIFFERENT-REDEPLOYED** `ProposalView.execute_at` and
  `ProposalStatus::{Queued,Cancelled}`. Signing digest v3 unchanged. Delay is
  read from the registry account at `finalize`.
  `abi::chain_id()` + `abi::self_id()` in digests, `consumed` digest records,
  permissionless `prune`, rich events. **Burns v2** — redeploy fresh;
  no state migration.
- Prior **v0.3.x** testnet pins are obsolete after v3 cutover.

## Functions

| Method | Notes |
|--------|--------|
| `init_registry` | Owner; bumps `epoch` (invalidates prior proposals, O(1)) |
| `set_proposal_ttl` | Owner; ceiling only, no wipe (`> 0`, `≤ MAX_PROPOSAL_TTL`) |
| `set_tombstone(bool)` | Owner; no invalidation |
| `propose(ProposeArgs) -> id` | Explicit non-zero `deadline`; caller `nonce`; merge identical open digests |
| `approve` / `finalize` | BLS over digest; delay 0: CEI then `call_raw`; else queue |
| `execute` | Permissionless after `execute_at` |
| `cancel` | Immediate; current-member quorum over cancel digest |
| `prune(limit) -> count` | Permissionless payload reclamation; keeps `Queued` until deadline |
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
