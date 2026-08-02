# multisig-encoding

Canonical §4a proposal signing preimage + §4b `ProposalBlob` types for the
multisig suite, plus shared layer-E call types behind a default-off feature.
Shared by `multisig-proposals`, `multisig-tool`, and tests so the signed bytes
cannot drift.

## Status

- **v0.1.0** — M0: `proposal_preimage` / `proposal_digest` /
  `recompute_and_verify`, `ProposalIntent` / `ProposalBlob` structs.
- **M3 fingerprint (2026-07-23)** — `digest_hex` / `digest_mnemonic` (BIP39
  24-word over full 32-byte digest) / `digest_safety_number` for out-of-band
  co-signer compare. Never truncate.
- **Layer E (`call-types`, 2026-08-02 / spec 26)** — default-off feature holding
  `SignatureEntry`, `VerifyQuorumArgs`, `MultisigAccountView` (rkyv ABI; no
  `repr(C)` yet — that is spec 23b). Serde derives behind `data-driver`.
  Both contracts re-export from `call_types` (`f7f4c1b` registry /
  `823ca2f` proposals); IDENTICAL carry.
- Spec: [`docs/multisig/multisig-suite-and-atlas-implementation-plan.md`](../../../docs/multisig/multisig-suite-and-atlas-implementation-plan.md) §4 / M3;
  call types: [`26-multisig-shared-call-types.md`](../../../docs/superpowers/specs/2026-07-31-shared-code/26-multisig-shared-call-types.md).
- Host `rlib` (`no_std` + `alloc`); path-dep from contracts and the tool.
  Default features stay free of `dusk-core` / `rkyv`.
- Live testnet deploy of consuming contracts: deferred (lab / `VM::ephemeral()` first).

## API

- `proposal_digest(...)` / `ProposalIntent::digest()` — full 32-byte Keccak256
  of the length-prefixed preimage (never truncate).
- `recompute_and_verify(intent, claimed)` — signer-side anti-blind-signing check.
- `gate_blob_for_signing(blob)` — same check for a §4b blob; ignores
  `human_summary` for trust (display must use canonical fields).
- `digest_mnemonic(digest)` / `digest_hex` / `digest_safety_number` — full-hash
  out-of-band fingerprints (M3).
- Domain tag: `sme-platform.multisig.proposal.v1`.
- `--features call-types` — `call_types::{SignatureEntry, VerifyQuorumArgs,
  MultisigAccountView}`.

## Build / test

```bash
cargo test
cargo build --features call-types
```
