# nocturne-knot-registry

Package `nocturne-knot-registry`, Rust library `knot_registry`.

Source of the knot registry smart contract for Dusk: a BLS M-of-N quorum
verification registry. It answers one question, "did enough of this account's
members sign this message?", so other contracts can reuse committee and
threshold logic. It is not a custody wallet and never holds funds.

## Usage

This crate is the contract source. It is built to WebAssembly with the
Makefile in the crate directory (see the repository), not linked as a host
library:

```bash
make wasm      # contract
make wasm-dd   # data-driver
make test
```

On non-wasm targets the library is empty. Depend on it only for its tests and
build tooling. The shared argument and view types live in
[`nocturne-knot-encoding`](https://crates.io/crates/nocturne-knot-encoding)
(feature `call-types`).

Features: `contract` (contract build), `data-driver` (data-driver build),
`data-driver-js` (data-driver with `alloc`, for JS use).

## Contract methods

- `create_account`: register a member set and threshold, returns an account id.
- `account`, `next_account_id`: reads.
- `verify_quorum`: checks that enough distinct members signed `msg`. Returns
  `false` for an unknown account or unmet quorum. It does not interpret `msg`,
  so callers must include their own nonce or context against replay.
- `verify_quorum_aggregate`: same check with one aggregated BLS multisignature.
- `change_account`: replace members and threshold, authorised by a quorum of
  the current members over a signed digest.
- `set_timelock`, `cancel_pending`, `execute_pending`: per-account delay for
  membership changes.

State lives in a separate data book contract, wired once with `init_data`.
The data books and the Atlas service registry are separate from this crate.

## Layout compatibility

Archived layouts of the argument and view types are pinned by golden tests in
`tests/layout_goldens.rs`. Changing them breaks deployed clients.

## Status

Testnet only, not audited yet.

Source: https://github.com/Nocturne-Standards/knot

License: Apache-2.0.
