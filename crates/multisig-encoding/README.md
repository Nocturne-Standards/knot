# multisig-encoding

Canonical §4a proposal signing preimage + §4b `ProposalBlob` types for the
multisig suite, plus shared layer-E call types behind a default-off feature.
Shared by `multisig-proposals`, `multisig-tool`, and tests so the signed bytes
cannot drift.

## Status

- **v0.1.2** — Spec 23b Phase B: `#[archive_attr(repr(C))]` on struct Archive
  types in `call_types` (fieldless `ProposalStatus` stays `#[repr(u8)]` only).
  Measured **DIFFERENT** on `MultisigAccountView` (and several registry /
  proposals local args); consumers redeployed — see registry/proposals READMEs.
  Status: **PINNED-DIFFERENT-REDEPLOYED** (layout owner; contracts carry the
  on-chain cutover).
- **v0.1.0** — M0: `proposal_preimage` / `proposal_digest` /
  `recompute_and_verify`, `ProposalIntent` / `ProposalBlob` structs.
- **M3 fingerprint (2026-07-23)** — `digest_hex` / `digest_mnemonic` (BIP39
  24-word over full 32-byte digest) / `digest_safety_number` for out-of-band
  co-signer compare. Never truncate.
- **Layer E (`call-types`, 2026-08-02)** — default-off feature holding
  shared registry/proposals ABI types. Serde derives behind `data-driver`.
  Both contracts re-export from `call_types`.
- Host `rlib` (`no_std` + `alloc`); path-dep from contracts and the tool.
  Default features stay free of `dusk-core` / `rkyv`.

## API

- `proposal_digest(...)` / `ProposalIntent::digest()` — full 32-byte Keccak256
  of the length-prefixed preimage (never truncate).
- `recompute_and_verify(intent, claimed)` — signer-side anti-blind-signing check.
- `gate_blob_for_signing(blob)` — same check for a §4b blob; ignores
  `human_summary` for trust (display must use canonical fields).
- `digest_mnemonic(digest)` / `digest_hex` / `digest_safety_number` — full-hash
  out-of-band fingerprints (M3).
- Domain tags: `nocturne.knot.multisig.proposal.v2` (proposal preimage),
  `nocturne.knot.multisig-registry.change_account.v2` (registry quorum).
- `--features call-types` — `call_types::{SignatureEntry, VerifyQuorumArgs,
  MultisigAccountView}`.

## Build / test

```bash
cargo test
cargo build --features call-types
```
