# Changelog

All notable changes to `multisig-collector` are documented here.
Format: [Keep a Changelog](https://keepachangelog.com/). Versioning: [docs/versioning.md](../../../docs/versioning.md).

## [0.2.0] — 2026-07-24

### Added

- `kind` on proposal blobs / summaries: `proposals` | `pm_council_resolve` (prediction-market council resolve intents without `chain_id`).
- `GET /v1/health` includes `"version"` (`CARGO_PKG_VERSION`) so VPS redeploys are checkable with one curl.

### Changed

- Intent deserialize is kind-gated (PM shape is not forced through the treasury `chain_id` schema).

## [0.1.0] — 2026-07-23

### Added

- Initial proposals + partials + party-finder API (pre-versioning baseline).
