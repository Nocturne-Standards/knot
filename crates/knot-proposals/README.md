# nocturne-knot-proposals

Package `nocturne-knot-proposals`, Rust library `knot_proposals`.

Source of the knot proposals smart contract for Dusk: an on-chain
propose, approve, finalize flow for multisig-authorised calls. Members sign a
digest defined by
[`nocturne-knot-encoding`](https://crates.io/crates/nocturne-knot-encoding),
membership and threshold come from `nocturne-knot-registry`, and the final
call is made with `call_raw`. If the registry account has a timelock, finalize
queues the proposal and `execute` runs it later.

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
build tooling. Shared argument and view types live in
`nocturne-knot-encoding` (feature `call-types`).

Features: `contract` (contract build), `data-driver` (data-driver build),
`data-driver-js` (data-driver with `alloc`, for JS use).

## Contract methods

- Owner configuration (direct account calls): `init_data`, `init_registry`,
  `set_authorized_account`, `set_proposal_ttl`, `set_tombstone`.
- `propose`: create a proposal with a non-zero deadline and a caller nonce.
- `approve`: add a BLS signature over the digest.
- `finalize`: check the quorum, then call the target or queue it.
- `execute`: run a queued proposal after its delay. Anyone may call it.
- `cancel`: cancel with a quorum of current members.
- `prune(limit)`: permissionless storage cleanup.
- `epoch`, `proposal_ttl`, `proposal`, `status`, `next_proposal_id`: reads.

State lives in a separate data book contract, wired once with `init_data`.
The data books and the Atlas service registry are separate from this crate.

## Layout compatibility

Archived layouts of the shared types are pinned by golden tests in `tests/layout_goldens.rs`. Changing
them breaks deployed clients.

## Status

Testnet only, not audited yet.

Source: https://github.com/Nocturne-Standards/knot

License: Apache-2.0.
