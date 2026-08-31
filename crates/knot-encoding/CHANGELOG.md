# Changelog

All notable changes to `knot-encoding` are documented here.
Format: [Keep a Changelog](https://keepachangelog.com/). Versioning: [docs/versioning.md](../../docs/versioning.md).

## [Unreleased]

### Changed

- Crate renamed from `multisig-encoding` to `knot-encoding` (mechanical:
  package name, paths, docs). Pin JSON keys and `nocturne.knot.multisig.*`
  domain tags are unchanged.

## [0.1.2] — 2026-08-03

### Changed

- Layer-E digest layouts pinned as `repr(C)` ahead of a coordinated
  `knot-registry` / `knot-proposals` redeploy.

## [0.1.1] — 2026-08-02

### Added

- Shared `call_raw` call-type encodings behind the `call-types` feature, with
  their own golden layout fixtures (previously verified only by the
  consuming crate).

## [0.1.0] — 2026-07-24

### Added

- Initial release: canonical proposal preimage, blob helpers, and digest
  fingerprints.
