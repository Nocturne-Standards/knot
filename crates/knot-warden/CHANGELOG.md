# Changelog

All notable changes to `knot-warden` are documented here.
Format: [Keep a Changelog](https://keepachangelog.com/). Versioning: [docs/versioning.md](../../docs/versioning.md).

## [Unreleased]

## [0.2.1]

### Added

- Two-step handover for the scheduler. `set_scheduler` waits for the delay AND for the incoming scheduler to call `accept_scheduler` (authenticated like a scheduler call); `execute_admin` refuses an unaccepted scheduler change (`incoming scheduler has not accepted`), so a mistyped or non-acting scheduler never takes the role and the current scheduler cancels the stuck change. Delay zero no longer applies a scheduler change at once; it applies on acceptance. A delay change and the Atlas forward are unchanged. `pending_admin_accepted()` is a read-only view.
- `accept_atlas_guardian()` (scheduler only): makes the warden call Atlas `accept_guardian` when the warden is the incoming guardian of an Atlas guardian change (Atlas 0.4.1 two-step handover).

### Fixed

- `execute_admin` could fail forwarding a queued guardian change to Atlas and leave the single admin slot occupied (the panic rolls back the slot clear). Reproduced on 0.2.0: queue `set_guardian`, open an Atlas change with `set_timelock`, advance, `execute_admin` fails, `set_delay` is blocked behind the slot. `set_guardian` now panics `warden is not the atlas guardian`, `atlas has a pending change; cancel or execute it first`, or `guardian contract id zero`, and `set_timelock` panics `an atlas guardian change is queued; cancel it first` while the change is queued.
- `init_warden`, `set_scheduler` and `set_guardian` reject an account that cannot act or that anyone can be: the zero id, a genesis-style id `[n, 0, ..., 0]` (the transfer contract is the caller of every Moonlight transaction, so naming it as scheduler made every account the scheduler), the warden's own id, Atlas's id, and an `External` key whose bytes are not a valid curve point (the archived key type does not check). Before, a zero-id scheduler locked the warden for good. The guardian checks match Atlas 0.4.1 exactly so a queued change cannot fail at the forward.
- `schedule_service` rejects a name over 64 bytes and the all-zero id up front, as Atlas does at `set_service`. Before, such an entry could be queued, never execute, and sit until cancelled.

### Changed

- `dusk-core` and `dusk-vm` 1.7.0, to build against the Atlas 0.4.1 tree. Bytecode differs from 0.2.0. `knot-warden-encoding` is source-unchanged (re-exports Atlas `PendingView` as `AtlasPendingView`); call-type and event layouts are unchanged.

### Added

- `init_warden` emits `atlas_set`. `set_timelock` and `cancel_atlas_pending` emit the forwarded value in the same transaction. `set_guardian` emits `guardian_forwarded` when `execute_admin` forwards the queued change.

## [0.2.0]

### Changed

- `set_guardian` waits `delay_blocks` in the admin slot as `PendingAdmin::AtlasGuardian`, then `execute_admin` forwards it to Atlas. A guardian change waits the warden delay plus the Atlas timelock. `set_timelock` and `cancel_atlas_pending` still forward in the same transaction.
- `init_warden` and `set_delay` reject a delay above 777,600 blocks (`delay above maximum`).
- `knot-warden-encoding` 0.2.0. New `PendingAdmin` variant. Not deployed.

## [0.1.1]

### Fixed

- A Moonlight transaction's `caller` is the genesis transfer contract. `init_warden` and `Account::External` treat that caller as a direct account call. 0.1.0 testnet id `d61dfc3f7cebf46ca03376093f15e719505ce7d69a3f23a154112dc72f42aee5` cannot be initialized from a wallet.

### Deploy

- Testnet id `fb143b1e10288ea6527edc2e556c009d78e7a01add076ab90d6e133aec47cdcb` (tx `9b821f4a386126509aa4963dead608f63972af906b232b8916b7f0ed2f9e2952`). `init_warden` points at Atlas 0.3.1 `21c653385e3b7cc8be32edd5e6df2e30b6464be867f39442171ba8ca8d089612`, scheduler the deploy wallet, `delay_blocks` 2.
- Later 0.1.1 `113159d33377e7778d552cc48f40318c44643ca06970e2482b41b276a4d9cdb3` (deploy tx `ebf2d30d449860949d44aeb904b01188a90d3853a95e84ffc8a71bce88676d0a`, 2026-09-27) is the id the deployments record names current. Its `init_warden` and the Atlas `set_guardian` that installed it are not in that record.

## [0.1.0]

### Added

- Delaying guardian contract. `delay_blocks` covers `schedule_service`, `set_delay`, and `set_scheduler`. `set_guardian`, `set_timelock`, and `cancel_atlas_pending` forward to Atlas in the same transaction. Not deployed.
