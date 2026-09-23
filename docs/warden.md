# Warden

Implemented in `knot-warden` 0.1.1 and `knot-warden-encoding` 0.1.0. Testnet id `fb143b1e10288ea6527edc2e556c009d78e7a01add076ab90d6e133aec47cdcb` (tx `9b821f4a386126509aa4963dead608f63972af906b232b8916b7f0ed2f9e2952`), in front of Atlas 0.3.1 `21c653385e3b7cc8be32edd5e6df2e30b6464be867f39442171ba8ca8d089612`.

Warden is the contract an operator puts in Atlas's guardian slot when a service repoint must wait. Knot-proposals stays a generic `call_raw` executor and does not grow a delay. Atlas stays a registry: it does not call warden, and it does not delay `set_service`.

Pinned to the Atlas 0.3 ABI: method names `resolve`, `set_service`, `set_guardian`, `set_timelock`, `cancel_pending`, and the `atlas-encoding` types `Account` and `SetServiceArgs`. A later Atlas bytecode is a new `ContractId` and a new warden deployment.

## Job

Warden is the only contract that should call Atlas writes in a production deployment.

- A service repoint waits `delay_blocks` inside warden, then warden calls Atlas `set_service`.
- Replacing warden's scheduler, and changing `delay_blocks`, wait out the current `delay_blocks`.
- `set_guardian`, `set_timelock`, and `cancel_pending` are forwarded to Atlas in the same transaction. Atlas applies its own delay to the first two. Warden does not add a second wait on that path.

Two numbers, two jobs:

| Delay | Where it lives | What it covers |
|---|---|---|
| `delay_blocks` | warden | `schedule_service`, `set_delay`, `set_scheduler` |
| `timelock_blocks` | Atlas | `set_guardian`, `set_timelock` |

Atlas's delay is what a hostile warden cannot delete. Warden can call `set_guardian`, but the new guardian does not write until Atlas `execute_pending`. Until that call, warden is still the guardian and `delay_blocks` still applies to service repoints. After that call, warden's delay no longer matters. Operators alarm on Atlas `pending` for that reason.

`delay_blocks` is what a hostile scheduler cannot delete in one transaction. `set_delay(0)` waits out the current value. `execute_at` is fixed when the change is scheduled (`block_height + delay_blocks` at that call). A service scheduled under the old delay does not pick up a later delay.

## What this contract is not

- Not the committee. Membership and quorum stay in `knot-registry` and `knot-proposals`. Warden does not verify BLS signatures.
- Not a second Atlas. It does not store the directory and it does not answer `resolve` for consumers. Consumers keep baking the Atlas id.
- Not a generic proxy. There is no method that takes a function name and raw bytes and forwards them to Atlas. A closed method list is the policy. An open forwarder would let the scheduler call Atlas `set_service` with no wait.
- Not Vigil, and not a constants table. Bounds that used to be Atlas `constant(name)` belong in the service contract `resolve` returns (for Vigil, `vigil_policy`). Warden only decides when that name may point at a different id.
- Not upgradeable. A new warden is a new `ContractId`, installed with Atlas `set_guardian`, which waits `timelock_blocks`.

## Crates

Both under Apache-2.0, next to the existing suite. Each crate is its own Cargo workspace: they path-depend on a sibling checkout of atlas 0.3, and public knot CI does not check that sibling out. `make -C crates/knot-warden test` builds the contracts and runs the suite.

| Crate | Role |
|---|---|
| `knot-warden-encoding` | Call types and events. Depends on `atlas-encoding` for `Account` and `SetServiceArgs`. Layout goldens pin every public type. |
| `knot-warden` | The contract. Depends on `knot-warden-encoding`. No dependency on `knot-registry` or `knot-proposals`. |

The scheduler is an `Account`. A proposals contract is one possible `Account::Contract`. A lab key is `Account::External`. Warden does not link Knot.

## State

```text
atlas: ContractId                  // immutable after init
scheduler: Option<Account>
delay_blocks: u64
pending_services: BTreeMap<String, (ContractId, u64)>   // name -> (id, execute_at)
pending_admin: Option<(PendingAdmin, u64)>
```

`PendingAdmin` is `Delay(u64)` or `Scheduler(Account)`. One admin slot. One pending id per service name. The service map and the admin slot do not block each other: a pending delay change does not freeze the directory, and a pending repoint does not freeze a scheduler rotation.

`execute_at` for either kind is `block_height().checked_add(delay_blocks)` at schedule time. Overflow panics `delay overflow`. Comparison matches Atlas: execute is allowed when `block_height() >= execute_at`.

Before `init_warden`, writes panic `warden not initialized: call init_warden first`. Reads return `None` or `0`.

## Authorization

The scheduler is the only writer, except `init_warden` and the permissionless executes.

`Account::External`: `public_sender` equals the key, and the call is direct. A direct account call has `caller` unset (a bare session call) or equal to the genesis transfer contract. Every Moonlight transaction enters through that contract's `spend_and_execute`. Any other caller is a contract using the key, and is not the scheduler.

`Account::Contract`: `caller` equals the id. `public_sender` is ignored. The contract decides which users may reach it. Knot-proposals qualifies because `finalize` `call_raw`s the target, so `caller` is the proposals id.

Uninitialized scheduler checks panic with the not-initialized string above. After init:

- `External scheduler: direct call required`
- `Only the warden scheduler may perform this action`
- `Contract scheduler: caller must be the scheduler contract`

`execute_service` and `execute_admin` do not check the scheduler. The scheduler authorized the change when it scheduled it. Anyone may pay to apply it, including after the old key is gone. The new scheduler is not authority until `execute_admin`.

`cancel_service` and `cancel_admin` stay with the current scheduler, including after `execute_at`. Cancel and execute in the same block are ordered by the transaction. A scheduler who still holds authority can abort a change; the delay does not remove that.

## Methods

The bootstrap method is not named `init`. Piecrust treats an exported `init` as the deploy constructor, calls it during deploy, and rejects every later call.

| Method | Authority | Effect |
|---|---|---|
| `init_warden(InitWardenArgs)` | deploy owner, direct call, once | Sets `atlas`, `scheduler`, `delay_blocks` |
| `schedule_service(SetServiceArgs)` | scheduler | One pending repoint for that name |
| `execute_service(String)` | permissionless after `execute_at` | Calls Atlas `set_service` |
| `cancel_service(String)` | scheduler | Drops that name's pending repoint |
| `set_delay(u64)` | scheduler | Schedules a new `delay_blocks` |
| `set_scheduler(Account)` | scheduler | Schedules a new scheduler |
| `execute_admin()` | permissionless after `execute_at` | Applies the admin slot |
| `cancel_admin()` | scheduler | Drops the admin slot |
| `set_guardian(Account)` | scheduler | Forwards to Atlas in this transaction |
| `set_timelock(u64)` | scheduler | Forwards to Atlas in this transaction |
| `cancel_atlas_pending()` | scheduler | Forwards to Atlas `cancel_pending` |
| `atlas`, `scheduler`, `delay_blocks`, `pending_service(name)`, `pending_admin` | permissionless | Reads |

`InitWardenArgs` is `atlas: ContractId`, `scheduler: Account`, `delay_blocks: u64`.

Deploy owner matches Atlas: a direct account call and `public_sender == self_owner()`. Otherwise panic `Only the deploy owner may init_warden`. A second init panics `warden already initialized`. The Atlas id cannot be changed afterward. A different Atlas is a different warden.

### Service repoint

`schedule_service` panics `empty service name` when `name` is empty.

It calls Atlas `resolve(name)`. If the returned id equals `args.id`, it panics `service unchanged` and does not take a slot. `None` is a real change: the name is new.

If that name already has a pending entry, it panics `a pending service change already exists for this name; cancel or execute it first`. A different name may be pending at the same time. Replacing a pending id requires `cancel_service` first.

`execute_service` panics `no pending service` when the name is absent, and `delay not elapsed` when `block_height() < execute_at`. It removes the entry, then calls Atlas `set_service` with the stored name and id. An Atlas panic reverts the transaction, so the entry remains. Clearing first is so a later Atlas that called back would not see the entry still pending.

`cancel_service` panics `no pending service` when the name is absent.

Delay `0` still writes the pending entry and emits `service_scheduled`, then calls `execute_service` in the same transaction.

### Admin slot

`set_delay` panics `delay unchanged` when the value equals the current `delay_blocks`. `set_scheduler` panics `scheduler unchanged` when the account equals the current scheduler. Neither call takes the slot in that case.

If an admin change is already stored, either call panics `a pending admin change already exists; cancel or execute it first`.

`execute_admin` panics `no pending admin` or `delay not elapsed`. It clears the slot, then applies `delay_blocks` or `scheduler`.

`cancel_admin` panics `no pending admin`.

Delay `0` emits `admin_scheduled`, then applies in the same transaction. Production `init_warden` passes the delay it means to keep. A deployment left at `0` has no warden delay until a `set_delay` is executed, and that first raise from `0` applies in the same call.

A service scheduled while an admin change is pending uses the delay already stored on the service entry, not the pending delay.

### Forwards

`set_guardian`, `set_timelock`, and `cancel_atlas_pending` require the scheduler, then `abi::call` Atlas. They do not read or write warden's pending maps. They do not wait `delay_blocks`.

Atlas's own panics propagate: `guardian unchanged`, `timelock unchanged`, `no pending change`, `a pending change already exists; cancel or execute it first`. Warden does not catch them.

This split is deliberate. The kernel delay exists because a contract that can both authorize and replace itself can delete its own delay. Putting warden's `delay_blocks` in front of `set_guardian` would stack two waits on one rotation and would still not be the delay the kernel is for. The wait on replacing warden is Atlas `timelock_blocks`, passed to `init_guardian`.

While an Atlas guardian rotation is pending, warden remains the caller Atlas accepts, and `schedule_service` still waits `delay_blocks`. Atlas does not freeze the directory during `set_guardian`. Warden is what keeps a repoint from landing in that window.

## Events

Typed rkyv payloads, `contract-events` feature, topic string equal to the emit name. Payloads carry the whole value so an indexer does not need a follow-up read. `init_warden` emits `scheduler_set` and `delay_set`.

| Topic | When |
|---|---|
| `scheduler_set` | `init_warden`, `execute_admin(Scheduler)` including the zero-delay path |
| `delay_set` | `init_warden`, `execute_admin(Delay)` including the zero-delay path |
| `service_scheduled` | `schedule_service`, including the zero-delay path, before apply |
| `service_executed` | `execute_service`, after the local entry is taken and before the Atlas call returns |
| `service_cancelled` | `cancel_service` |
| `admin_scheduled` | `set_delay` / `set_scheduler`, including the zero-delay path, before apply |
| `admin_cancelled` | `cancel_admin` |

Atlas still emits `service_updated` when the directory actually changes, and `pending_scheduled` / `guardian_set` / `timelock_set` / `pending_cancelled` for the forwarded calls. Indexers that care about the directory subscribe to Atlas. Indexers that care about the wait subscribe to warden.

`PendingAdmin` is a data-carrying enum. It is not `repr(C)`. The pin is the layout golden, same rule as Atlas `Account`.

## Wiring

Bootstrap, after the contracts are deployed:

1. Init the knot registry and proposals the way those contracts already require.
2. Deploy owner calls `init_warden` with the Atlas id, `Account::Contract(proposals_id)`, and the service delay. Direct call.
3. Atlas deploy owner calls `init_guardian` with `Account::Contract(warden_id)` and the kernel delay. Direct call.

A lab can skip proposals: `init_warden` with `Account::External(operator)` and `delay_blocks` `0`, and `init_guardian` with the warden id and `timelock_blocks` `0`.

A service move through knot, once this is deployed:

1. `propose` on knot-proposals. `target` is the warden id, not the Atlas id. `function_name` is `schedule_service`. `call_args` is the rkyv encoding of `SetServiceArgs`. `deadline` is the knot expiry ceiling. It is not a minimum wait and it does not replace `delay_blocks`.
2. Members approve. Anyone `finalize`s. That `call_raw` only schedules.
3. After `execute_at`, anyone calls `execute_service`. That transaction is the one that calls Atlas `set_service`.

A guardian rotation through knot uses `function_name` `set_guardian` and `target` warden. `finalize` reaches Atlas in that same transaction. Atlas then waits `timelock_blocks`. Anyone calls Atlas `execute_pending`. Cancelling that rotation is a proposal whose function is `cancel_atlas_pending`.

Pointing Atlas's guardian straight at the proposals contract still works. `finalize` then calls `set_service` immediately. That wiring has quorum and no service delay. Warden is the deployment that wants the delay.

## Vigil

Vigil stays pinned to Atlas 0.2.0 until its `constant(name)` reads move. This spec does not change Vigil.

On an Atlas 0.3 pin, `resolve("vigil_policy")` is still how the vault finds the policy contract. A repoint of that name waits `delay_blocks` only when warden is the Atlas guardian and the proposal targets `schedule_service`. Vigil's withdrawal delay and veto stay in the vault. They are not this delay.

## Acceptance

An implementation is wrong if any of these fail:

- Deploy does not call `init_warden`. A later `init_warden` from the deploy owner works once. A second call panics. A non-owner and an inter-contract call panic.
- An external scheduler is rejected when `caller` is `Some`, even if `public_sender` is the key.
- A contract scheduler is accepted only when `caller` is that id.
- `schedule_service` of the current Atlas id panics and leaves no pending entry. An empty name panics. A second schedule for the same name panics. A second name may be pending beside the first.
- `execute_service` before `execute_at` panics. At `execute_at`, a caller who is not the scheduler applies it, and Atlas `resolve` returns the new id.
- `cancel_service` by the scheduler removes the entry. A non-scheduler cannot cancel.
- Delay `0` emits `service_scheduled` and `service_executed` and updates Atlas in one call.
- `set_delay` to the current value does not take the admin slot. A real `set_delay` waits. `schedule_service` during that wait stores `execute_at` from the old delay.
- `set_delay(0)` does not apply before the current delay has elapsed.
- `set_scheduler` follows the same slot, unchanged check, and wait.
- `set_guardian` from the scheduler sets Atlas `pending` in that transaction and does not write warden's maps. A non-scheduler cannot forward.
- Writes before `init_warden` panic. The public method list has no `init`.
