#![cfg_attr(target_family = "wasm", no_std)]
#![cfg(target_family = "wasm")]

#[cfg(not(feature = "contract"))]
compile_error!("Enable the 'contract' feature for WASM builds");

extern crate alloc;

#[cfg(target_family = "wasm")]
mod state;
