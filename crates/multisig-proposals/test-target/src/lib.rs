//! Test target for proposals `call_raw` execute path.

#![cfg_attr(target_family = "wasm", no_std)]
#![cfg(target_family = "wasm")]

#[cfg(not(feature = "contract"))]
compile_error!("Enable the 'contract' feature for WASM builds");

extern crate alloc;

#[dusk_forge::contract]
mod proposals_test_target {
    use dusk_core::abi;

    pub struct TargetState {
        value: u64,
    }

    impl TargetState {
        pub const fn new() -> Self {
            Self { value: 0 }
        }

        pub fn set_value(&mut self, v: u64) {
            self.value = v;
            abi::emit("value_set", v);
        }

        pub fn value(&self) -> u64 {
            self.value
        }
    }
}
