//! Test target for proposals `call_raw` execute path (incl. reentrancy / failure).

#![cfg_attr(target_family = "wasm", no_std)]
#![cfg(target_family = "wasm")]

#[cfg(not(feature = "contract"))]
compile_error!("Enable the 'contract' feature for WASM builds");

extern crate alloc;

#[dusk_forge::contract]
mod proposals_test_target {
    use dusk_core::abi::{self, ContractId};

    pub struct TargetState {
        value: u64,
        /// How many times a side-effecting method ran (reentrancy probe).
        hit_count: u64,
        proposals: Option<ContractId>,
        reenter_proposal_id: u64,
    }

    impl TargetState {
        pub const fn new() -> Self {
            Self {
                value: 0,
                hit_count: 0,
                proposals: None,
                reenter_proposal_id: 0,
            }
        }

        pub fn set_value(&mut self, v: u64) {
            self.value = v;
            abi::emit("value_set", v);
        }

        /// Store proposals id + proposal id so `set_value_reenter_finalize` can
        /// call back into `finalize` on the same proposal (reentrancy test).
        pub fn configure_reenter(&mut self, proposals: ContractId, proposal_id: u64) {
            self.proposals = Some(proposals);
            self.reenter_proposal_id = proposal_id;
        }

        /// Side-effect once, then attempt a nested `finalize` on the configured
        /// proposal. Nested failure is ignored so the outer `call_raw` can still
        /// succeed when CEI has already consumed the proposal.
        pub fn set_value_reenter_finalize(&mut self, v: u64) {
            self.value = v;
            self.hit_count = self.hit_count.saturating_add(1);
            abi::emit("value_set", v);
            if self.hit_count == 1 {
                if let Some(proposals) = self.proposals {
                    let _ = abi::call::<u64, ()>(
                        proposals,
                        "finalize",
                        &self.reenter_proposal_id,
                    );
                }
            }
        }

        /// Intentional panic — used to assert failed `call_raw` reverts finalize.
        pub fn fail_set(&mut self, _v: u64) {
            panic!("test-target: intentional call_raw failure");
        }

        pub fn value(&self) -> u64 {
            self.value
        }

        pub fn hit_count(&self) -> u64 {
            self.hit_count
        }

        /// Host-metadata probe for phase-3a `abi::chain_id` gate.
        pub fn chain_id(&self) -> u8 {
            abi::chain_id()
        }
    }
}
