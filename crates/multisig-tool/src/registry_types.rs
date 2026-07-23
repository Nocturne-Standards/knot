//! Pulls in `multisig-registry`'s own call-argument types directly (same
//! `#[path = ...]` convention every host-side contract test in this repo
//! already uses, e.g. `multisig-registry/tests/contract.rs`,
//! `compliance-gate/src/lib.rs`'s `license_types` inclusion) — rather than a
//! duplicated/hand-copied struct set. Since this tool constructs the exact
//! rkyv bytes the deployed contract expects (no data-driver JSON round-trip
//! — see README.md), the wire format must match byte-for-byte; including the
//! real source file guarantees that far more robustly than keeping a copy in
//! sync by hand.

#[path = "../../multisig-registry/src/call_types.rs"]
pub mod call_types;
