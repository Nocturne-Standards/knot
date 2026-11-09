# nocturne-knot-encoding

Package `nocturne-knot-encoding`, Rust library `knot_encoding`.

Canonical signing preimages and digests for the knot multisig contracts, plus
the `ProposalBlob` types a signer inspects before signing. One shared
implementation means the bytes a signer approves are the bytes the contracts
verify. The crate is `no_std` with `alloc`. With default features it pulls in
only hashing crates.

## Usage

```toml
[dependencies]
nocturne-knot-encoding = "0.1"
```

Optional features:

- `call-types`: shared call argument, view and status types for the registry
  and proposals contracts (`knot_encoding::call_types`), built on `rkyv` and
  `dusk-core`.
- `contract-events`: event payload structs (`knot_encoding::events`) with
  `ContractEvent` impls. Implies `call-types`.
- `data-driver`: `serde` derives on those types. Implies `call-types`.
- `fixture-gen`: enables the `generate-event-fixtures` binary. Not for
  library users.

## API

- `proposal_digest` / `proposal_digest_v3` and `ProposalIntent` /
  `ProposalIntentV3`: Keccak-256 over a length-prefixed preimage. The v3
  digest also binds the contract id and an epoch.
- `recompute_and_verify`, `recompute_and_verify_v3`, `gate_blob_for_signing`:
  signer-side checks that a claimed digest matches the canonical fields.
  They return `GateError` on mismatch and do not trust `human_summary`.
- `change_account_digest_v3`, `set_timelock_digest_v1`,
  `cancel_pending_digest_v1`, `cancel_proposal_digest_v1`, and the matching
  `*_preimage_*` and `*_message_*` functions.
- `digest_hex`, `digest_mnemonic`, `digest_safety_number`: human-readable
  fingerprints of the full 32-byte digest for out-of-band comparison.
  Never compare a truncated digest.
- `DOMAIN_*` constants: the domain-separation tags used by each digest.

## Layout compatibility

With `call-types`, the archived (`rkyv`) layouts of the shared types are fixed
by golden tests in `src/layout_goldens.rs`. Changing a field or its order
changes the bytes the deployed contracts read, so such changes are breaking.

## Related

The contract crates `nocturne-knot-registry` and `nocturne-knot-proposals`
use this crate. The data books and Atlas are separate and not part of it.

## Status

Testnet only, not audited yet.

Source: https://github.com/Nocturne-Standards/knot

License: Apache-2.0.
