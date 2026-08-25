# Changelog

All notable changes to `knot-registry` are documented here.
Format: [Keep a Changelog](https://keepachangelog.com/). Versioning: [docs/versioning.md](../../docs/versioning.md).

## [Unreleased]

### Changed

- **Breaking:** `change_account` moved to a v3 signing domain binding
  `chain_id`, the registry's own instance id, and an explicit member count.
  v2 `change_account` signatures are burned — redeploy before v3
  `knot-proposals`. Crate version has not been bumped for this change yet.
- Crate renamed from `multisig-registry` to `knot-registry` (mechanical:
  package name, paths, docs). Pin JSON keys and `nocturne.knot.multisig.*`
  domain tags are unchanged. Keystore default path also moves to
  `~/.knot` (one-release legacy fallback in `knot-tool`).

## [0.1.6] — 2026-08-04

### Changed

- Contract pins now read through the shared `nocturne-deployments` reader
  crate instead of a bespoke local loader. Redeployed after a domain-tag
  bump — see [`versioning.md`](../../docs/versioning.md)
  (`PINNED-DIFFERENT-REDEPLOYED`).

## [0.1.5] — 2026-08-03

### Changed

- `call_raw` call-type encodings now come from `knot-encoding` instead of a
  local copy, removing a duplicated layout definition.
- Layer-E digest layouts pinned as `repr(C)`; golden layout tests updated.
  Redeployed.

## [0.1.4] — 2026-07-29

### Changed

- Redeploy record tied to a council contract redeploy. No registry code
  change.

## [0.1.3] — 2026-07-24

### Changed

- Redeploy record for the prediction-market council-resolve cutover. No
  registry code change.

## [0.1.2] — 2026-07-24

### Added

- Initial release: on-chain BLS M-of-N quorum registry.

### Changed

- `change_account` digest and committee-size cap single-sourced instead of
  duplicated across call sites.
