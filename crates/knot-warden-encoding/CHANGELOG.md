# Changelog

All notable changes to `knot-warden-encoding` are documented here.
Format: [Keep a Changelog](https://keepachangelog.com/). Versioning: [docs/versioning.md](../../docs/versioning.md).

## [Unreleased]

## [0.2.1]

### Added

- Re-export Atlas `PendingChange` as `AtlasPendingChange`, for warden 0.3.0's timelock check. No layout change.
- `AtlasSet`, `GuardianForwarded`, `TimelockForwarded`, `AtlasCancelForwarded`.
  Layout goldens pin them.

### Changed

- Package renamed to `nocturne-knot-warden-encoding` (the `nocturne-knot-*` crates.io pattern). `[lib] name = "knot_warden_encoding"` keeps the Rust name. Dependents keep the `knot-warden-encoding` key and add `package = "nocturne-knot-warden-encoding"`.
- `atlas-encoding` is `nocturne-atlas-encoding` 0.4 (sibling path, `package = ...`). 0.4.0 keeps every 0.3.0 layout, so `Account` and `SetServiceArgs` bytes are unchanged; the goldens pass.

## [0.2.0]

### Changed

- `PendingAdmin::AtlasGuardian(Account)`. Existing `Delay` and `Scheduler` layouts stay pinned. Not deployed.

## [0.1.0]

### Added

- Call types and events for warden. `Account` and `SetServiceArgs` come from `atlas-encoding` 0.3. Layout goldens pin every warden-owned public type.
