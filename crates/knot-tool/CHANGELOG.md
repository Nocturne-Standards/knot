# Changelog

All notable changes to `knot-tool` are documented here.
Format: [Keep a Changelog](https://keepachangelog.com/). Versioning: [docs/versioning.md](../../docs/versioning.md).

## [Unreleased]

### Added

- Local `deployments/testnet.json` pin loader (`NOCTURNE_DEPLOYMENTS` or walk-up).
  Dropped the private `nocturne-deployments` git dependency so default CI/builds
  need no org token.

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
