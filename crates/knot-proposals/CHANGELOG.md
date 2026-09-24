# Changelog

All notable changes to `knot-proposals` are documented here.
Format: [Keep a Changelog](https://keepachangelog.com/). Versioning: [docs/versioning.md](../../docs/versioning.md).

## [Unreleased]

### Added

- Split into `knot-proposals` (logic) and `knot-proposals-data` (book).
  `call_raw` stays on the logic contract. Logic version 0.4.0. Book version 0.1.0.
- `set_authorized_account`: only that registry account may propose, finalize,
  or execute. Rebinding blocks open and queued proposals from the previous account.
- Owner configuration (`init_data`, `init_registry`, `set_proposal_ttl`,
  `set_tombstone`, `set_authorized_account`) requires a direct account call.
- `finalize` sends only approvals that are still members.
- `prune(0)` removes nothing. Each call examines a bounded prefix of both maps.
  Consumed digests stay until their deadline.
- Queue/execute/cancel: `finalize` queues when account delay > 0; `execute`
  after `execute_at`; `cancel` immediate. **PINNED-DIFFERENT-REDEPLOYED**.

### Changed

- **Breaking:** landed the v3 contract interface — epoch counter (`init_registry`
  bumps it, invalidating prior open proposals in O(1) instead of a wipe scan),
  caller-supplied `ProposeArgs.nonce`, `abi::chain_id()` + `abi::self_id()`
  folded into the signed digest, `consumed` digest records, and a
  permissionless `prune(limit)` for payload reclamation. v2 signatures are
  burned — no state migration; redeploy registry before proposals. Crate
  version has not been bumped for this change yet.
- Crate renamed from `multisig-proposals` to `knot-proposals` (mechanical:
  package name, paths, docs). Pin JSON keys and `nocturne.knot.multisig.*`
  domain tags are unchanged.

## [0.3.3] — 2026-08-04

### Changed

- Redeploy record after a domain-tag bump (contract pins now read via the
  shared `nocturne-deployments` reader crate). No proposals code change —
  see [`versioning.md`](../../docs/versioning.md) (`PINNED-DIFFERENT-REDEPLOYED`).

## [0.3.2] — 2026-08-03

### Changed

- `call_raw` call-type encodings now come from `knot-encoding` instead of a
  local copy.
- Layer-E digest layouts pinned as `repr(C)`. Redeployed.

## [0.3.1] — 2026-07-29

### Changed

- Redeploy record tied to a council contract redeploy. No proposals code
  change.

## [0.3.0] — 2026-07-24

### Changed

- `finalize` now updates status and bumps the committee nonce **before**
  calling `call_raw` (checks-effects-interactions ordering), so a reentrant
  target cannot double-execute. If `call_raw` fails, the whole transaction
  still reverts — the proposal stays `Open` and the nonce is unchanged, so
  operators can retry; failed execute does not consume the proposal.
- `tombstone` changed from an enum to a plain `bool` (default `false`);
  `set_tombstone(bool)` no longer wipes open proposals.
- `init_chain_id` and `init_registry` now both wipe open proposals;
  `propose` requires `chain_id` to be set first.

### Added

- Propose-time caps: `function_name` ≤ 64 bytes, `call_args` ≤ 4096 bytes.
- Past-deadline proposals are rejected at `propose` time instead of only at
  `finalize`.

## [0.2.0] — 2026-07-24

### Added

- Initial release: propose → approve → finalize `call_raw` execution over a
  `knot-registry` BLS quorum.
