# Changelog

All notable changes to `knot-warden` are documented here.
Format: [Keep a Changelog](https://keepachangelog.com/). Versioning: [docs/versioning.md](../../docs/versioning.md).

## [Unreleased]

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
