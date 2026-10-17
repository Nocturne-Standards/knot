# Changelog

All notable changes to the Knot workspace are documented here.

Format follows [Keep a Changelog](https://keepachangelog.com/). Crate-level
details live in each crate's `CHANGELOG.md`. Semver policy:
[`docs/versioning.md`](docs/versioning.md).

## [Unreleased]

### Changed

- `knot-warden` 0.2.0 and `knot-warden-encoding` 0.2.0: `set_guardian` waits `delay_blocks` before it is forwarded to Atlas. Delay cap is 777,600 blocks. Not deployed. 0.1.1 remains the testnet id below.

### Added

- `knot-warden` 0.1.1 and `knot-warden-encoding` 0.1.0: delaying guardian in front of Atlas. Own workspaces; need a sibling atlas 0.3 checkout. Spec: `docs/warden.md`. 0.1.1 treats the genesis transfer contract as a direct account caller, so a Moonlight wallet can `init_warden`. Testnet current `113159d33377e7778d552cc48f40318c44643ca06970e2482b41b276a4d9cdb3`. First deploy `fb143b1e10288ea6527edc2e556c009d78e7a01add076ab90d6e133aec47cdcb`.

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
