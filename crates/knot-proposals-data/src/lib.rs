//! Proposal book for `knot-proposals`.
//!
//! Who may propose, quorum, events, and `call_raw` live on `knot-proposals`.
//! Every mutating method here requires
//! `abi::caller() == atlas.resolve("knot-proposals")`.

#![cfg_attr(target_family = "wasm", no_std)]
#![cfg(target_family = "wasm")]

#[cfg(not(feature = "contract"))]
compile_error!("Enable the 'contract' feature for WASM builds");

extern crate alloc;

#[cfg(target_family = "wasm")]
mod state;
