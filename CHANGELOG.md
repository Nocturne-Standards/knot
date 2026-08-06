# Changelog

All notable changes to the Knot workspace are documented here.

Format follows [Keep a Changelog](https://keepachangelog.com/). Crate-level
details live in each crate's `CHANGELOG.md`. Semver policy:
[`docs/versioning.md`](docs/versioning.md).

## [Unreleased]

### Added

- Public repo hygiene: `SECURITY.md`, `CONTRIBUTING.md`, GitHub templates,
  `docs/design-notes.md`.
- Optional `deployments-crate` feature on `knot-tool` — default build loads
  contract pins from local JSON without a private git dependency.

### Changed

- Public-facing docs: internal spec references replaced with inlined substance.
- CI: `cargo fmt --check` and `cargo clippy -D warnings` on the workspace.
