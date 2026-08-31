# Changelog

All notable changes to the Knot workspace are documented here.

Format follows [Keep a Changelog](https://keepachangelog.com/). Crate-level
details live in each crate's `CHANGELOG.md`. Semver policy:
[`docs/versioning.md`](docs/versioning.md).

## [Unreleased]

### Added

- Public repo hygiene: `SECURITY.md`, `CONTRIBUTING.md`, GitHub templates,
  `docs/design-notes.md`.

### Changed

- Public-facing docs: internal spec references replaced with inlined substance.
- CI: `cargo fmt --check` and `cargo clippy -D warnings` on the workspace;
  public-surface gate hard-fails (no `ALLOW_PRIVATE_TIER` soften).
- Pre-squash carve: remove editor/agent local tooling from the tracked tree.
- `knot-tool`: restore optional `deployments-crate` feature; default build uses
  in-tree JSON pin loader so a fresh clone compiles without a sibling path dep.
  Optional git dep points at `Nocturne-Standards/nocturne-deployments`.
