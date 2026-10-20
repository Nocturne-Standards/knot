# Changelog

All notable changes to the Knot workspace are documented here.

Format follows [Keep a Changelog](https://keepachangelog.com/). Crate-level
details live in each crate's `CHANGELOG.md`. Semver policy:
[`docs/versioning.md`](docs/versioning.md).

## [Unreleased]

### Security

- `rkyv` moves from `=0.7.39` to `=0.7.46` in every crate and lockfile (fixes RUSTSEC-2026-0001, `Rc`/`Arc` `from_value` on OOM). The `knot-encoding` layout goldens (35), `knot-proposals` (45 tests), `knot-registry` (41), `knot-warden` (27) and `knot-warden-encoding` pass unchanged: the archived layouts are byte-identical. RUSTSEC-2026-0235 has no 0.7 fix and stays accepted (no `Rc`/`Arc` archives in Knot types). `knot-event-decoder` also moves its registry `dusk-core` dev-dependency to `=1.7.0` to match `knot-warden-encoding`.
- Known, not from this change: `knot-event-decoder`'s `conformance_emit_topics` fails on `main` because `knot-registry` emits `evm_bootstrap_sent` and the decoder has no arm for it. That workspace is not covered by public CI.

### Added

- `knot-warden` 0.2.1: two-step handover for the scheduler (`accept_scheduler`, `pending_admin_accepted`) and `accept_atlas_guardian`, matching Atlas 0.4.1's `accept_guardian`. 31 contract tests.

### Fixed

- `knot-warden` 0.2.1: `set_guardian` rejects states where the forward to Atlas would fail (warden not the Atlas guardian, Atlas pending change, zero contract id), `set_timelock` is refused while a guardian change is queued, and `schedule_service` rejects names over 64 bytes and the zero id. 0.2.0 could leave its single admin slot occupied. Builds on dusk-core 1.7.0.

### Changed

- `knot-warden` 0.2.0 and `knot-warden-encoding` 0.2.0: `set_guardian` waits `delay_blocks` before it is forwarded to Atlas. Delay cap is 777,600 blocks. Not deployed. 0.1.1 remains the testnet id below.

### Added

- `knot-event-decoder` 0.1.0: dispatch named registry, proposals, and warden events by contract, topic, and version.

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
