# knot-proposals

On-chain **propose → approve → finalize** (`abi::call_raw`), with an optional
queue when the registry account's `timelock_blocks` is > 0.
[`knot-registry`](../knot-registry/) for membership/threshold.
Signed bytes are the v3 digest from [`knot-encoding`](../knot-encoding/).

## Book

`knot-proposals` is the API and the `call_raw` caller. The rows live on
`knot-proposals-data`. Atlas service `knot-proposals`. `init_data` wires the
book once; the same id may be retried, a different book panics. A new logic
contract may `init_data` onto that book, but `finalize` and `execute` recompute
the v3 digest and reject proposals signed for the previous contract. Those
rows stay until cancel or prune.
`set_authorized_account` binds this executor to one registry account and bumps
`auth_generation`. Open and queued proposals from the previous binding cannot
run, including after a switch back to that account. Owner configuration is a
direct account call. `prune(0)` is a no-op. Each prune examines a bounded
prefix of the proposal map and of the digest map. Consumed digests stay until
`deadline`.

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
| `init_data` | Owner, direct call; one-shot book contract (same id may be retried) |
| `init_registry` | Owner, direct call; bumps `epoch` and clears the authorized account |
| `set_authorized_account` | Owner, direct call; bumps `auth_generation`, so the previous binding's open and queued proposals cannot run |
| `set_proposal_ttl` | Owner; ceiling only, no wipe (`> 0`, `≤ MAX_PROPOSAL_TTL`) |
| `set_tombstone(bool)` | Owner; no invalidation |
| `propose(ProposeArgs) -> id` | Explicit non-zero `deadline`; caller `nonce`; merge identical open digests |
| `approve` / `finalize` | BLS over digest; delay 0: CEI then `call_raw`; else queue |
| `execute` | Permissionless after `execute_at` |
| `cancel` | Immediate; current-member quorum over cancel digest |
| `prune(limit) -> count` | Permissionless payload reclamation; keeps `Queued` until deadline |
| `epoch` / `proposal_ttl` / `proposal` / `status` | Reads |

## Deploy order

1. Deploy **registry data**, then **registry logic**. Atlas `knot-registry` names the logic id. `init_data`.
2. Deploy **proposals data**, then **proposals logic**. Atlas `knot-proposals` names the logic id. `init_data`, `init_registry(registry_logic_id)`, `set_authorized_account`.
3. Re-create councils / re-sign intents. Prior monolith ids stay history. The book was not funded.

## Build / test

```bash
cd ../knot-registry && make wasm
cd ../knot-proposals && make test
```
