# knot-encoding

Canonical proposal signing preimage + `ProposalBlob` types for the multisig
suite, plus shared call types behind a default-off feature. Shared by
`knot-proposals`, `knot-tool`, and tests so the signed bytes cannot drift.

## Status

- **v0.1.2** — `#[archive_attr(repr(C))]` on struct Archive types in
  `call_types` (fieldless `ProposalStatus` stays `#[repr(u8)]` only).
  Measured **DIFFERENT** on `MultisigAccountView` (and several registry /
  proposals local args); consumers redeployed — see registry/proposals READMEs.
- **v0.1.0** — `proposal_preimage` / `proposal_digest` /
  `recompute_and_verify`, `ProposalIntent` / `ProposalBlob` structs.
- **M3 fingerprint (2026-07-23)** — `digest_hex` / `digest_mnemonic` (BIP39
  24-word over full 32-byte digest) / `digest_safety_number` for out-of-band
  co-signer compare. Never truncate.
- **Call types (`call-types`, 2026-08-02)** — default-off feature holding
  shared registry/proposals ABI types. Serde derives behind `data-driver`.
  Both contracts re-export from `call_types`.
- Host `rlib` (`no_std` + `alloc`); path-dep from contracts and the tool.
  Default features stay free of `dusk-core` / `rkyv`.

## API

- `proposal_digest(...)` / `ProposalIntent::digest()` — full 32-byte Keccak256
  of the length-prefixed preimage (never truncate).
- `recompute_and_verify(intent, claimed)` / `recompute_and_verify_v3` —
  signer-side anti-blind-signing check; returns typed [`GateError`].
- `gate_blob_for_signing(blob)` — same check for a proposal blob; ignores
  `human_summary` for trust (display must use canonical fields).
- `digest_mnemonic(digest)` / `digest_hex` / `digest_safety_number` — full-hash
  out-of-band fingerprints (M3).
- Domain tags: `nocturne.knot.multisig.proposal.v2` (proposal preimage),
  `nocturne.knot.multisig-registry.change_account.v2` (registry quorum).
  **v3**: `proposal_digest_v3` / `ProposalIntentV3` bind `self_id` +
  `epoch`; `change_account_digest_v3` binds `chain_id`, registry `self_id`, and
  explicit `member_count`.
- `--features call-types` — `call_types::{SignatureEntry, VerifyQuorumArgs,
  MultisigAccountView}`.

## Build / test

```bash
cargo test
cargo build --features call-types
```
