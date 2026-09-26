# Events, and where the delay lives

Knot branch `feat/data-logic-split`. Atlas checked out at
`feat/guardian-kernel` `a20ad3f` (0.3.1 guardian kernel). Event payloads
below are implemented. The kernel delay is unchanged.

Books do not emit. Logic emits, after the book write commits and before
`call_raw`. Payloads are named structs in `knot-encoding` (registry and
proposals) or `knot-warden-encoding` (warden), with layout goldens. Tuples
and `()` go away. A payload carries the new value, so an indexer does not
reread state.

Registry and proposals are not on testnet in this split, so their event
shape can change before the cutover. Warden `fb143b1e…` is live. A new
event payload means a new warden `ContractId`. That redeploy is accepted.
It still goes in through Atlas `set_guardian`.

## Atlas delay

0.2 delayed `set_service` inside Atlas, and it also had constants, roles,
`gov_mode`, and a proposals id. 0.3.1 removed those. `set_service` is
immediate. The kernel delay that is still in the code is only
`set_guardian` and `set_timelock`. They share one slot.
`execute_at = block_height + timelock_blocks`. `cancel_pending` is
immediate. `execute_pending` is anyone, after `execute_at`.

Warden already owns the delay that Atlas dropped:

| Call | Atlas 0.3.1 | Warden today |
|---|---|---|
| `set_service` | Guardian, immediate. Emits `service_updated { name, id }` | `schedule_service` waits `delay_blocks`, then `execute_service` calls Atlas `set_service` |
| `set_guardian` | Guardian, waits `timelock_blocks`. Emits `pending_scheduled`, then `guardian_set` | `set_guardian` forwards in this transaction. Warden emits nothing |
| `set_timelock` | Guardian, waits the current delay. Emits `pending_scheduled`, then `timelock_set` | `set_timelock` forwards in this transaction. Warden emits nothing |
| `cancel_pending` | Guardian, immediate | `cancel_atlas_pending` forwards. Warden emits nothing |
| `set_delay`, `set_scheduler` | Not Atlas methods | Warden waits its own `delay_blocks` |

`docs/warden.md` matches this table. Atlas delays guardian replacement and
changes to that delay. A service repoint waits in warden, then Atlas writes
it immediately.

## Assumptions these decisions keep

Fuller events are safe to ship, including a warden redeploy. Cutting the
kernel delay is not.

`set_service` is immediate in Atlas 0.3.1. That cut holds. The guardian
contract is supposed to wait before it calls `set_service`. Warden does.
Atlas does not need a second clock for a directory write.

`set_guardian` and `set_timelock` stay delayed inside Atlas, on the one
shared slot. Warden keeps forwarding them in the same transaction.
`delay_blocks` stays on `schedule_service`, `set_delay`, and `set_scheduler`.
It does not move in front of `set_guardian`.

The reason is the one in Atlas design notes, section 2, and in
`docs/warden.md`. The guardian is a contract that is allowed to authorize a
change and to replace itself. If the wait lived only in that contract, the
installed bytecode could call Atlas `set_guardian` immediately and skip its
own `delay_blocks`. A kernel delay on the replacement does not consult
warden's storage, so the installed bytecode cannot skip it. Zeroing it
is `set_timelock(0)`, which waits the current kernel value and occupies
the one slot, so the rotation cannot share that transaction. After the
zero is applied, the next `set_guardian` is immediate. That is the same
patient-attacker bound `set_delay(0)` would give. The difference is who
is trusted to keep the clock. With the wait only in warden, that contract
can call Atlas `set_guardian` in whatever transaction it chooses, and once
`set_delay(0)` has executed, service repoints are immediate in that same
moment.

`cancel_pending` stays immediate on Atlas, and the warden forward stays
immediate. The guardian who still holds the slot can abort a rotation.

A warden redeployed for the new event structs is a new `ContractId`. It
becomes guardian only when Atlas `execute_pending` runs, after
`timelock_blocks`. Until then the old warden is still the guardian and
still delays service repoints. The new bytecode forwards `set_guardian`,
`set_timelock`, and `cancel_atlas_pending` the same way.

Events do not authorize. The payload names the new value. It does not
change who may call, which digest was signed, or which `auth_generation` a
proposal belongs to. `finalize`, `execute`, `queue`, and `commit_executed`
still recompute the digest and still require the generation. `approve`
still does not, and an event from `set_authorized_account` does not close
that. Tombstone still does not stop `call_raw`. `execute_pending` on Atlas
and on the registry stays permissionless after `execute_at`.

Delay 0 stays an immediate write. The applied event carries the new value.
Production `init_guardian` and `init_warden` still pass the delay they mean
to keep. A deployment left at 0 has no delay on that clock until a raise
is executed, and that first raise applies in the same call.

## Registry events

Logic only. Replace the current scalars and pairs.

| Method | Today | Payload |
|---|---|---|
| `init_data` | none | `data: ContractId` |
| `create_account` | account id | id, members, threshold. Timelock is 0. Nonce is 0 |
| `change_account` | via `emit_effect` | see below |
| `set_timelock` | via `emit_effect` | see below |
| `cancel_pending` | account id | account id, `execute_at`, and the pending change that was dropped |
| `execute_pending` | via `emit_effect` | see below |

`RegistryBookEffect` has to carry the value, not only a tag.
`Scheduled` already has `execute_at`; add the `RegistryPendingChange`.
`AccountChanged` carries the new members and threshold.
`TimelockSet` carries the new `blocks`.

| Effect | Topic | Payload |
|---|---|---|
| `Scheduled` | `pending_scheduled` | account id, `execute_at`, the pending change |
| `AccountChanged` | `account_changed` | account id, members, threshold |
| `TimelockSet` | `timelock_set` | account id, blocks |

Delay 0 applies inside the call, so that call emits `account_changed` or
`timelock_set` and does not also emit `pending_scheduled`.

## Proposals events

| Method | Today | Payload |
|---|---|---|
| `init_data` | none | `data: ContractId` |
| `init_registry` | `registry_set` with `()` | registry `ContractId`, new epoch. Authorized account is cleared |
| `set_proposal_ttl` | none | blocks |
| `set_tombstone` | none | the bool |
| `set_authorized_account` | none | account id, new `auth_generation` |
| `propose` | id, digest, account, deadline | also epoch, nonce, `auth_generation`, target, function name, call args |
| `approve` | id, digest, signer bytes | also the signature |
| `finalize` delay 0 | id, digest, account, target, function name | also call args. Topic stays `proposal_finalized` |
| `finalize` delay > 0 | id, digest, account, `execute_at` | also target, function name, call args. Topic stays `proposal_queued` |
| `execute` | same as delay-0 finalize | same full payload. Topic `proposal_finalized` |
| `cancel` | id, digest | also account id |
| `prune` | a count, and only when the count is > 0 | the proposal ids removed and the digest keys removed in this call. Emit even when a digest was removed and the proposal count is 0 |

`prune` today returns only the proposal count. The book should return both
id lists, or the logic should learn them another way, so the event is not
a bare number. One event, both lists.

## Warden events

These already carry the value: `scheduler_set`, `delay_set`,
`service_scheduled`, `service_executed`, `service_cancelled`,
`admin_scheduled`, `admin_cancelled`.

Add:

| Method | Payload |
|---|---|
| `init_warden` | also the Atlas `ContractId`, beside the scheduler and delay events it already emits |
| `set_guardian` | the `Account` forwarded |
| `set_timelock` | the `blocks` forwarded |
| `cancel_atlas_pending` | empty of warden state. Emit a topic so the forward is visible. Atlas emits the cancelled change |

These three stay forwards. Their events record the call. The wait stays on
Atlas.

## Not events

Reads. `knot-mock-atlas`. The proposals test target. A second `init_data`
of the same book id, because state does not change.
