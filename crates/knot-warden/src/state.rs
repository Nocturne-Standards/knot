// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Nocturne Standards

#[dusk_forge::contract(events = [
    SchedulerSet,
    DelaySet,
    ServiceScheduled,
    ServiceExecuted,
    ServiceCancelled,
    AdminScheduled,
    AdminCancelled,
    AtlasSet,
    GuardianForwarded,
    TimelockForwarded,
    AtlasCancelForwarded,
])]
mod warden {
    use alloc::collections::BTreeMap;
    use alloc::string::String;

    use dusk_bytes::Serializable;
    use dusk_core::abi::{self, ContractId, block_height};
    use dusk_core::signatures::bls::PublicKey as BlsPublicKey;

    use knot_warden_encoding::call_types::{
        Account, AtlasPendingView, InitWardenArgs, PendingAdmin, PendingAdminView,
        PendingServiceView, SetServiceArgs,
    };
    use knot_warden_encoding::events::{
        AdminCancelled, AdminScheduled, AtlasCancelForwarded, AtlasSet, DelaySet,
        GuardianForwarded, SchedulerSet, ServiceCancelled, ServiceExecuted, ServiceScheduled,
        TimelockForwarded,
    };

    /// Delaying guardian for one Atlas deployment. Never upgraded.
    pub struct WardenState {
        atlas: Option<ContractId>,
        scheduler: Option<Account>,
        delay_blocks: u64,
        pending_services: BTreeMap<String, (ContractId, u64)>,
        pending_admin: Option<(PendingAdmin, u64)>,
    }

    impl WardenState {
        /// 90 days of 10-second Dusk blocks. Same cap as Atlas `timelock_blocks`.
        const MAX_DELAY_BLOCKS: u64 = 777_600;

        /// Atlas `set_service` rejects longer names. A queued repoint Atlas
        /// would refuse can never execute, so reject it when scheduling.
        const MAX_NAME_BYTES: usize = 64;

        pub const fn new() -> Self {
            Self {
                atlas: None,
                scheduler: None,
                delay_blocks: 0,
                pending_services: BTreeMap::new(),
                pending_admin: None,
            }
        }

        /// Deploy-owner bootstrap. Sets the Atlas id, the scheduler, and the
        /// initial delay. Call once, after deploy. The delay applies
        /// immediately because no scheduler exists yet.
        ///
        /// Not named `init`: Piecrust reserves that export for the deploy
        /// constructor and rejects later calls to it.
        pub fn init_warden(&mut self, args: InitWardenArgs) {
            Self::require_deploy_owner();
            if self.scheduler.is_some() {
                panic!("warden already initialized");
            }
            Self::require_delay(args.delay_blocks);
            Self::require_usable_account(&args.scheduler, "scheduler", args.atlas);
            self.atlas = Some(args.atlas);
            self.scheduler = Some(args.scheduler.clone());
            self.delay_blocks = args.delay_blocks;
            abi::emit("atlas_set", AtlasSet { atlas: args.atlas });
            abi::emit(
                "scheduler_set",
                SchedulerSet {
                    scheduler: args.scheduler,
                },
            );
            abi::emit(
                "delay_set",
                DelaySet {
                    blocks: args.delay_blocks,
                },
            );
        }

        pub fn atlas(&self) -> Option<ContractId> {
            self.atlas
        }

        pub fn scheduler(&self) -> Option<Account> {
            self.scheduler.clone()
        }

        pub fn delay_blocks(&self) -> u64 {
            self.delay_blocks
        }

        pub fn pending_service(&self, name: String) -> Option<PendingServiceView> {
            self.pending_services
                .get(&name)
                .map(|(id, execute_at)| PendingServiceView {
                    name,
                    id: *id,
                    execute_at: *execute_at,
                })
        }

        pub fn pending_admin(&self) -> Option<PendingAdminView> {
            self.pending_admin
                .as_ref()
                .map(|(change, execute_at)| PendingAdminView {
                    change: change.clone(),
                    execute_at: *execute_at,
                })
        }

        /// One pending repoint for `name`. Delay `0` still emits
        /// `service_scheduled`, then applies in this call.
        pub fn schedule_service(&mut self, args: SetServiceArgs) {
            self.require_scheduler();
            if args.name.is_empty() {
                panic!("empty service name");
            }
            if args.name.len() > Self::MAX_NAME_BYTES {
                panic!("service name longer than 64 bytes");
            }
            if args.id == ContractId::from_bytes([0u8; 32]) {
                panic!("service id zero");
            }
            let current: Option<ContractId> = abi::call(self.atlas_id(), "resolve", &args.name)
                .expect("warden: atlas resolve failed");
            if current == Some(args.id) {
                panic!("service unchanged");
            }
            if self.pending_services.contains_key(&args.name) {
                panic!(
                    "a pending service change already exists for this name; cancel or execute it first"
                );
            }
            let execute_at = self.execute_at();
            self.pending_services
                .insert(args.name.clone(), (args.id, execute_at));
            abi::emit(
                "service_scheduled",
                ServiceScheduled {
                    name: args.name.clone(),
                    id: args.id,
                    execute_at,
                },
            );
            if self.delay_blocks == 0 {
                self.execute_service(args.name);
            }
        }

        /// Apply a pending repoint. Permissionless after `execute_at`.
        /// The entry is removed before the Atlas call.
        pub fn execute_service(&mut self, name: String) {
            self.require_initialized();
            let (id, execute_at) = self
                .pending_services
                .get(&name)
                .copied()
                .unwrap_or_else(|| panic!("no pending service"));
            if block_height() < execute_at {
                panic!("delay not elapsed");
            }
            self.pending_services.remove(&name);
            abi::emit(
                "service_executed",
                ServiceExecuted {
                    name: name.clone(),
                    id,
                },
            );
            let args = SetServiceArgs { name, id };
            let _: () = abi::call(self.atlas_id(), "set_service", &args)
                .expect("warden: atlas set_service failed");
        }

        pub fn cancel_service(&mut self, name: String) {
            self.require_scheduler();
            let (id, _) = self
                .pending_services
                .remove(&name)
                .unwrap_or_else(|| panic!("no pending service"));
            abi::emit("service_cancelled", ServiceCancelled { name, id });
        }

        /// Wait out the current delay. Raising and lowering both wait, so
        /// the scheduler cannot zero the delay and repoint in one transaction.
        pub fn set_delay(&mut self, blocks: u64) {
            self.require_scheduler();
            Self::require_delay(blocks);
            if self.delay_blocks == blocks {
                panic!("delay unchanged");
            }
            self.schedule_admin(PendingAdmin::Delay(blocks));
        }

        pub fn set_scheduler(&mut self, next: Account) {
            self.require_scheduler();
            Self::require_usable_account(&next, "scheduler", self.atlas_id());
            if self.scheduler.as_ref() == Some(&next) {
                panic!("scheduler unchanged");
            }
            self.schedule_admin(PendingAdmin::Scheduler(next));
        }

        /// Apply the admin slot. Permissionless after `execute_at`.
        pub fn execute_admin(&mut self) {
            self.require_initialized();
            let (change, execute_at) = self
                .pending_admin
                .clone()
                .unwrap_or_else(|| panic!("no pending admin"));
            if block_height() < execute_at {
                panic!("delay not elapsed");
            }
            self.pending_admin = None;
            match change {
                PendingAdmin::Delay(blocks) => {
                    self.delay_blocks = blocks;
                    abi::emit("delay_set", DelaySet { blocks });
                }
                PendingAdmin::Scheduler(scheduler) => {
                    self.scheduler = Some(scheduler.clone());
                    abi::emit("scheduler_set", SchedulerSet { scheduler });
                }
                PendingAdmin::AtlasGuardian(guardian) => {
                    let _: () = abi::call(self.atlas_id(), "set_guardian", &guardian)
                        .expect("warden: atlas set_guardian failed");
                    abi::emit("guardian_forwarded", GuardianForwarded { guardian });
                }
            }
        }

        pub fn cancel_admin(&mut self) {
            self.require_scheduler();
            let (change, _) = self
                .pending_admin
                .take()
                .unwrap_or_else(|| panic!("no pending admin"));
            abi::emit("admin_cancelled", AdminCancelled { change });
        }

        /// Queue Atlas `set_guardian` for `delay_blocks`. `execute_admin`
        /// forwards it. The directory then waits its own timelock.
        ///
        /// The forward panics, and the panic rolls back the slot clear, so it
        /// would leave the single admin slot occupied, if the warden is not
        /// Atlas's guardian, Atlas holds a pending change, or the guardian is
        /// the zero contract id. All three are rejected here, and
        /// `set_timelock` cannot open an Atlas change while this one is queued,
        /// so the forward has no other way to fail.
        pub fn set_guardian(&mut self, next: Account) {
            self.require_scheduler();
            Self::require_usable_account(&next, "guardian", self.atlas_id());
            let current: Option<Account> =
                abi::call(self.atlas_id(), "guardian", &()).expect("warden: atlas guardian failed");
            if current != Some(Account::Contract(abi::self_id())) {
                panic!("warden is not the atlas guardian");
            }
            if current.as_ref() == Some(&next) {
                panic!("guardian unchanged");
            }
            let atlas_pending: Option<AtlasPendingView> =
                abi::call(self.atlas_id(), "pending", &()).expect("warden: atlas pending failed");
            if atlas_pending.is_some() {
                panic!("atlas has a pending change; cancel or execute it first");
            }
            self.schedule_admin(PendingAdmin::AtlasGuardian(next));
        }

        /// Reject an account that cannot act, or that anyone can be.
        ///
        /// A contract account is matched against `abi::caller()`. The zero id
        /// never matches, so it locks the slot. A genesis-style id
        /// (`[n, 0, ..., 0]`) is a protocol contract: the transfer contract is
        /// the caller of every Moonlight transaction, so naming it makes every
        /// account the scheduler or guardian. The warden's own id never
        /// matches. A key is rejected unless it decodes to a valid curve point;
        /// the archived type does not check that. `label` names the slot.
        fn require_usable_account(account: &Account, label: &str, atlas: ContractId) {
            match account {
                Account::Contract(id) => {
                    if *id == ContractId::from_bytes([0u8; 32]) {
                        panic!("{label} contract id zero");
                    }
                    if id.to_bytes()[1..].iter().all(|b| *b == 0) {
                        panic!("{label} contract id reserved");
                    }
                    if *id == abi::self_id() {
                        panic!("{label} cannot be the warden itself");
                    }
                    // Atlas rejects its own id as a guardian, and never calls
                    // the warden as scheduler. Match Atlas exactly so a queued
                    // guardian change cannot fail when it is forwarded.
                    if *id == atlas {
                        panic!("{label} cannot be atlas");
                    }
                }
                Account::External(key) => {
                    if BlsPublicKey::from_bytes(&key.to_bytes()).is_err() {
                        panic!("{label} key invalid");
                    }
                }
            }
        }

        fn require_delay(blocks: u64) {
            if blocks > Self::MAX_DELAY_BLOCKS {
                panic!("delay above maximum");
            }
        }

        /// Forward Atlas `set_timelock` in this transaction.
        pub fn set_timelock(&mut self, blocks: u64) {
            self.require_scheduler();
            if matches!(
                self.pending_admin,
                Some((PendingAdmin::AtlasGuardian(_), _))
            ) {
                panic!("an atlas guardian change is queued; cancel it first");
            }
            let _: () = abi::call(self.atlas_id(), "set_timelock", &blocks)
                .expect("warden: atlas set_timelock failed");
            abi::emit("timelock_forwarded", TimelockForwarded { blocks });
        }

        /// Forward Atlas `cancel_pending` in this transaction.
        pub fn cancel_atlas_pending(&mut self) {
            self.require_scheduler();
            let _: () = abi::call(self.atlas_id(), "cancel_pending", &())
                .expect("warden: atlas cancel_pending failed");
            abi::emit("atlas_cancel_forwarded", AtlasCancelForwarded);
        }

        fn schedule_admin(&mut self, change: PendingAdmin) {
            if self.pending_admin.is_some() {
                panic!("a pending admin change already exists; cancel or execute it first");
            }
            let execute_at = self.execute_at();
            let scheduled = AdminScheduled {
                change: change.clone(),
                execute_at,
            };
            self.pending_admin = Some((change, execute_at));
            abi::emit("admin_scheduled", scheduled);
            if self.delay_blocks == 0 {
                self.execute_admin();
            }
        }

        fn execute_at(&self) -> u64 {
            block_height()
                .checked_add(self.delay_blocks)
                .expect("delay overflow")
        }

        fn atlas_id(&self) -> ContractId {
            self.atlas
                .unwrap_or_else(|| panic!("warden not initialized: call init_warden first"))
        }

        /// Genesis transfer contract (`dusk_core::transfer::TRANSFER_CONTRACT`).
        /// Every Moonlight transaction enters through `spend_and_execute`, so
        /// this id is the caller of a direct wallet call.
        fn transfer_contract_id() -> ContractId {
            let mut bytes = [0u8; 32];
            bytes[0] = 1;
            ContractId::from_bytes(bytes)
        }

        /// Bare session call (`caller` unset) or a Moonlight transaction
        /// (caller is the transfer contract). Any other caller is a contract
        /// using the sender's key.
        fn is_direct_account_call() -> bool {
            match abi::caller() {
                None => true,
                Some(id) => id == Self::transfer_contract_id(),
            }
        }

        /// Deploy-time owner, direct account call only, for `init_warden`.
        fn require_deploy_owner() {
            if !Self::is_direct_account_call() {
                panic!("Only the deploy owner may init_warden");
            }
            let sender = abi::public_sender();
            let owner = abi::self_owner();
            if sender != Some(owner) {
                panic!("Only the deploy owner may init_warden");
            }
        }

        fn require_initialized(&self) {
            if self.scheduler.is_none() {
                panic!("warden not initialized: call init_warden first");
            }
        }

        /// External: direct call from the key. Contract: `caller` equals the id.
        fn require_scheduler(&self) {
            self.require_initialized();
            match self.scheduler.as_ref().expect("scheduler") {
                Account::External(key) => {
                    if !Self::is_direct_account_call() {
                        panic!("External scheduler: direct call required");
                    }
                    if abi::public_sender() != Some(*key) {
                        panic!("Only the warden scheduler may perform this action");
                    }
                }
                Account::Contract(id) => {
                    if abi::caller() != Some(*id) {
                        panic!("Contract scheduler: caller must be the scheduler contract");
                    }
                }
            }
        }
    }
}
