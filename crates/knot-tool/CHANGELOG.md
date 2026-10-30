# Changelog

All notable changes to `knot-tool` are documented here.
Format: [Keep a Changelog](https://keepachangelog.com/). Versioning: [docs/versioning.md](../../docs/versioning.md).

## [Unreleased]

### Added

- CLI/RPC: `account set-timelock` / `execute-pending` / `cancel-pending`,
  `proposal execute` / `cancel`. Mock ledger shows queued + eta.
- CLI: `evm bootstrap-sign` / `bootstrap-submit` / `action-sign` / `action-args`
  for `KnotEvmRoot` on DuskEVM, one signer per store, partials in a public JSON
  file. Testnet only, not audited yet.

### Fixed

- `deployments-crate` feature: the `nocturne-deployments` git dependency moves
  off rev `5811377` (gone from the public repo) to tag
  `nocturne-deployments-v0.2.0`, and reads the pins-v2 `Catalog`
  (`knot/<name>/testnet`, tier `own` then `public`; `NOCTURNE_PINS` or walk-up)
  instead of the v1 `load_from`.
- Default build loads both pin shapes: the flat `testnet.json` and the
  enveloped `duskds/testnet.json` (`nocturne.pins.v1`, `contracts`,
  `aliases`). `NOCTURNE_DEPLOYMENTS` may name either file or the pins dir
  (`duskds/testnet.json` first).

### Changed

- Registry diagnostics (`diagnose_quorum`, `account_meta`, `member_key_bytes`) moved
  off-chain: `knot-tool` derives them from `account()` + local BLS verify. On-chain
  ABI keeps only `next_account_id` among scalar probes.

## [0.2.0] — 2026-07-24

### Added

- `pm_council_resolve` blob kind + `pm-resolve` UI/CLI (init/push/sign/aggregate against collector).
- `knot-tool --version` (clap from `CARGO_PKG_VERSION`).

### Changed

- Collector client lockstep with collector `0.2.0` wire (`kind` on summaries/bodies).

## [0.1.0] — 2026-07-23

### Added

- Initial registry/proposals tooling + collector Topology B (pre-versioning baseline).
