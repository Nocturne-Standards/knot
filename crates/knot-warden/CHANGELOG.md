# Changelog

All notable changes to `knot-warden` are documented here.
Format: [Keep a Changelog](https://keepachangelog.com/). Versioning: [docs/versioning.md](../../docs/versioning.md).

## [Unreleased]

### Added

- `init_warden` emits `atlas_set`. `set_guardian`, `set_timelock`, and
  `cancel_atlas_pending` emit the forwarded value. They still do not wait
  `delay_blocks`. A deployment that wants these payloads is a new contract id.

## [0.1.1]

### Fixed

- A Moonlight transaction's `caller` is the genesis transfer contract. `init_warden` and `Account::External` treat that caller as a direct account call. 0.1.0 testnet id `d61dfc3f7cebf46ca03376093f15e719505ce7d69a3f23a154112dc72f42aee5` cannot be initialized from a wallet.

### Deploy

- Testnet id `fb143b1e10288ea6527edc2e556c009d78e7a01add076ab90d6e133aec47cdcb` (tx `9b821f4a386126509aa4963dead608f63972af906b232b8916b7f0ed2f9e2952`). `init_warden` points at Atlas 0.3.1 `21c653385e3b7cc8be32edd5e6df2e30b6464be867f39442171ba8ca8d089612`, scheduler the deploy wallet, `delay_blocks` 2.

## [0.1.0]

### Added

- Delaying guardian contract. `delay_blocks` covers `schedule_service`, `set_delay`, and `set_scheduler`. `set_guardian`, `set_timelock`, and `cancel_atlas_pending` forward to Atlas in the same transaction. Not deployed.
