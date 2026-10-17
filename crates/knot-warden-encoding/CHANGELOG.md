# Changelog

All notable changes to `knot-warden-encoding` are documented here.
Format: [Keep a Changelog](https://keepachangelog.com/). Versioning: [docs/versioning.md](../../docs/versioning.md).

## [Unreleased]

## [0.2.0]

### Changed

- `PendingAdmin::AtlasGuardian(Account)`. Existing `Delay` and `Scheduler` layouts stay pinned. Not deployed.

## [0.1.0]

### Added

- Call types and events for warden. `Account` and `SetServiceArgs` come from `atlas-encoding` 0.3. Layout goldens pin every warden-owned public type.
