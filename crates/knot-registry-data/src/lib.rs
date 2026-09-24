//! Account book for `knot-registry`.
//!
//! Quorum checks, events, and signing domains live on `knot-registry`.
//! Every mutating method here requires
//! `abi::caller() == atlas.resolve("knot-registry")`.

#![cfg_attr(target_family = "wasm", no_std)]
#![cfg(target_family = "wasm")]

#[cfg(not(feature = "contract"))]
compile_error!("Enable the 'contract' feature for WASM builds");

extern crate alloc;

#[cfg(target_family = "wasm")]
mod state;
