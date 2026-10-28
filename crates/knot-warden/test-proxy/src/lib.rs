// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Nocturne Standards

//! Test-only proxy: cross-calls warden so `abi::caller()` is this contract.

#![cfg_attr(target_family = "wasm", no_std)]
#![cfg(target_family = "wasm")]

#[cfg(not(feature = "contract"))]
compile_error!("Enable the 'contract' feature for WASM builds");

extern crate alloc;

#[dusk_forge::contract]
mod warden_test_proxy {
    use alloc::string::String;
    use alloc::vec::Vec;

    use dusk_core::abi::{self, ContractId};

    use knot_warden_encoding::call_types::{Account, InitWardenArgs, SetServiceArgs};

    pub struct ProxyState;

    impl ProxyState {
        pub const fn new() -> Self {
            Self
        }

        pub fn call_init_warden(&self, warden: ContractId, args: InitWardenArgs) {
            let _: () = abi::call(warden, "init_warden", &args).expect("proxy init_warden");
        }

        pub fn call_schedule_service(&self, warden: ContractId, name: String, id: ContractId) {
            let args = SetServiceArgs { name, id };
            let _: () =
                abi::call(warden, "schedule_service", &args).expect("proxy schedule_service");
        }

        pub fn call_cancel_service(&self, warden: ContractId, name: String) {
            let _: () = abi::call(warden, "cancel_service", &name).expect("proxy cancel_service");
        }

        pub fn call_accept_scheduler(&self, warden: ContractId) {
            let _: () = abi::call(warden, "accept_scheduler", &()).expect("proxy accept_scheduler");
        }

        /// Hop through `path`, then `call_raw` `target`. Deployed at both
        /// the transfer contract id and `PROXY_ID`, a path of
        /// `[PROXY_ID, TRANSFER_CONTRACT]` reproduces the stack of the
        /// transfer contract's `contract_to_contract` relay: the target sees
        /// the transfer contract as `caller` with two more frames below it.
        pub fn relay(
            &self,
            path: Vec<ContractId>,
            target: ContractId,
            fn_name: String,
            args: Vec<u8>,
        ) {
            match path.split_first() {
                None => {
                    abi::call_raw(target, &fn_name, &args).expect("proxy relay target");
                }
                Some((next, rest)) => {
                    let _: () = abi::call(*next, "relay", &(rest.to_vec(), target, fn_name, args))
                        .expect("proxy relay hop");
                }
            }
        }

        pub fn call_set_guardian(&self, warden: ContractId, next: Account) {
            let _: () = abi::call(warden, "set_guardian", &next).expect("proxy set_guardian");
        }
    }
}
