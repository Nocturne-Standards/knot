#[dusk_forge::contract(events = [
    DataSet,
    AccountCreated,
    PendingScheduled,
    AccountChanged,
    TimelockSet,
    PendingCancelled,
])]
mod knot_registry {
    use alloc::vec::Vec;

    use dusk_bytes::Serializable;
    use dusk_core::abi::{self, ContractId};
    use dusk_core::signatures::bls::PublicKey as BlsPublicKey;
    use knot_encoding::{
        PENDING_KIND_CHANGE_ACCOUNT, PENDING_KIND_SET_TIMELOCK, bootstrap_min_gas,
        cancel_pending_change_account_payload, cancel_pending_message_v1,
        cancel_pending_set_timelock_payload, change_account_message_v3, encode_bootstrap_calldata,
        evm_bootstrap_message_v1, set_timelock_message_v1,
    };

    use knot_encoding::events::{
        AccountChanged, AccountCreated, DataSet, PendingCancelled, PendingScheduled, TimelockSet,
    };
    use knot_registry::call_types::{
        CancelPendingArgs, ChangeAccountArgs, CreateAccountArgs, MultisigAccountView,
        PublishBootstrapRootArgs, RegistryBookEffect, RegistryPendingChange, SetTimelockArgs,
        SignatureEntry, VerifyQuorumAggregateArgs, VerifyQuorumArgs,
    };

    const MAX_COMMITTEE_MEMBERS: usize = 16;

    /// API for the account book on `knot-registry-data`.
    pub struct MultisigRegistryState {
        /// Book contract. Not cached from Atlas: this contract is the id
        /// Atlas resolves, and data checks `abi::caller()` against that.
        data: Option<ContractId>,
        /// L1 `CrossDomainMessenger` for the EVM root bootstrap.
        l1_messenger: Option<ContractId>,
        /// `KnotEvmRoot` on DuskEVM (20 bytes, EVM wire shape).
        evm_root_receiver: Option<[u8; 20]>,
    }

    impl MultisigRegistryState {
        pub const fn new() -> Self {
            Self {
                data: None,
                l1_messenger: None,
                evm_root_receiver: None,
            }
        }

        /// Owner-only, direct account call. Points this contract at its book.
        /// One-shot: the same id may be retried; a different book panics.
        pub fn init_data(&mut self, data: ContractId) {
            require_direct_owner();
            match self.data {
                Some(current) if current == data => {}
                Some(_) => panic!("knot-registry data already set"),
                None => {
                    self.data = Some(data);
                    abi::emit("data_set", DataSet { data });
                }
            }
        }

        pub fn create_account(&mut self, args: CreateAccountArgs) -> u64 {
            validate_committee(&args.members, args.threshold);
            let id: u64 = abi::call(self.data_id(), "create_account", &args)
                .expect("knot-registry-data create_account failed");
            abi::emit(
                "account_created",
                AccountCreated {
                    id,
                    members: args.members,
                    threshold: args.threshold,
                    timelock_blocks: 0,
                    nonce: 0,
                },
            );
            id
        }

        pub fn account(&self, id: u64) -> Option<MultisigAccountView> {
            abi::call(self.data_id(), "account", &id).expect("knot-registry-data account failed")
        }

        pub fn next_account_id(&self) -> u64 {
            abi::call(self.data_id(), "next_account_id", &())
                .expect("knot-registry-data next_account_id failed")
        }

        pub fn verify_quorum(&self, args: VerifyQuorumArgs) -> bool {
            let Some(account) = self.account(args.account_id) else {
                return false;
            };
            quorum_met(&account.members, account.threshold, &args.msg, &args.sigs)
        }

        pub fn verify_quorum_aggregate(&self, args: VerifyQuorumAggregateArgs) -> bool {
            let Some(account) = self.account(args.account_id) else {
                return false;
            };
            if args.signer_keys.len() > account.members.len() {
                return false;
            }
            if args.signer_keys.len() < account.threshold as usize {
                return false;
            }
            if has_duplicates(&args.signer_keys) {
                return false;
            }
            if !args
                .signer_keys
                .iter()
                .all(|key| account.members.contains(key))
            {
                return false;
            }
            abi::verify_bls_multisig(args.msg, args.signer_keys, args.aggregate_sig)
        }

        pub fn change_account(&mut self, args: ChangeAccountArgs) {
            validate_committee(&args.new_members, args.new_threshold);
            let account = self
                .account(args.account_id)
                .unwrap_or_else(|| panic!("no such multisig account"));
            let member_pks: Vec<[u8; 96]> =
                args.new_members.iter().map(|pk| pk.to_bytes()).collect();
            let msg = change_account_message_v3(
                u64::from(abi::chain_id()),
                &abi::self_id().to_bytes(),
                args.account_id,
                account.nonce,
                &member_pks,
                args.new_threshold,
            )
            .expect("change_account member set within encoding caps");
            require_quorum(
                &account.members,
                account.threshold,
                &msg,
                &args.sigs,
                "change_account",
            );
            let effect: RegistryBookEffect = abi::call(
                self.data_id(),
                "schedule",
                &(
                    args.account_id,
                    RegistryPendingChange::ChangeAccount {
                        new_members: args.new_members,
                        new_threshold: args.new_threshold,
                    },
                ),
            )
            .expect("knot-registry-data schedule failed");
            emit_effect(args.account_id, effect);
        }

        pub fn set_timelock(&mut self, args: SetTimelockArgs) {
            let account = self
                .account(args.account_id)
                .unwrap_or_else(|| panic!("no such multisig account"));
            let msg = set_timelock_message_v1(
                u64::from(abi::chain_id()),
                &abi::self_id().to_bytes(),
                args.account_id,
                account.nonce,
                args.blocks,
            )
            .expect("set_timelock encoding");
            require_quorum(
                &account.members,
                account.threshold,
                &msg,
                &args.sigs,
                "set_timelock",
            );
            let effect: RegistryBookEffect = abi::call(
                self.data_id(),
                "schedule",
                &(
                    args.account_id,
                    RegistryPendingChange::SetTimelock(args.blocks),
                ),
            )
            .expect("knot-registry-data schedule failed");
            emit_effect(args.account_id, effect);
        }

        pub fn cancel_pending(&mut self, args: CancelPendingArgs) {
            let account = self
                .account(args.account_id)
                .unwrap_or_else(|| panic!("no such multisig account"));
            let pending = account
                .pending
                .clone()
                .unwrap_or_else(|| panic!("no pending change"));
            let (kind, payload) = pending_kind_and_payload(&pending.change);
            let msg = cancel_pending_message_v1(
                u64::from(abi::chain_id()),
                &abi::self_id().to_bytes(),
                args.account_id,
                pending.execute_at,
                kind,
                &payload,
            )
            .expect("cancel_pending encoding");
            require_quorum(
                &account.members,
                account.threshold,
                &msg,
                &args.sigs,
                "cancel_pending",
            );
            let _: () = abi::call(self.data_id(), "clear_pending", &args.account_id)
                .expect("knot-registry-data clear_pending failed");
            abi::emit(
                "pending_cancelled",
                PendingCancelled {
                    account_id: args.account_id,
                    execute_at: pending.execute_at,
                    change: pending.change,
                },
            );
        }

        pub fn execute_pending(&mut self, account_id: u64) {
            let effect: RegistryBookEffect =
                abi::call(self.data_id(), "execute_pending", &account_id)
                    .expect("knot-registry-data execute_pending failed");
            emit_effect(account_id, effect);
        }

        /// Owner-only, direct account call. L1 `CrossDomainMessenger` for the
        /// EVM root bootstrap. Once-only.
        pub fn init_l1_messenger_contract(&mut self, messenger: ContractId) {
            require_direct_owner();
            if self.l1_messenger.is_some() {
                panic!("l1 messenger already set");
            }
            self.l1_messenger = Some(messenger);
        }

        /// Owner-only, direct account call. `KnotEvmRoot` address on DuskEVM
        /// (20 bytes, EVM wire shape). Once-only.
        pub fn init_evm_root_receiver(&mut self, receiver: [u8; 20]) {
            require_direct_owner();
            if self.evm_root_receiver.is_some() {
                panic!("evm root receiver already set");
            }
            if receiver == [0u8; 20] {
                panic!("evm root receiver must be non-zero");
            }
            self.evm_root_receiver = Some(receiver);
        }

        /// Sends `account_id`'s current member set and threshold to
        /// `KnotEvmRoot.bootstrap` through the L1 messenger. Authorized by a
        /// quorum of the account's current members signing over
        /// `evm_bootstrap_message_v1`. The book records the nonce that was
        /// sent, so a second publish at the same nonce is refused. A later
        /// `change_account` bumps that nonce and opens one new publish.
        pub fn publish_bootstrap_root(&mut self, args: PublishBootstrapRootArgs) {
            let l1_messenger = self.l1_messenger.expect(
                "knot-registry XDM not configured: call init_l1_messenger_contract first",
            );
            let receiver = self.evm_root_receiver.expect(
                "knot-registry XDM not configured: call init_evm_root_receiver first",
            );
            let account = self
                .account(args.account_id)
                .unwrap_or_else(|| panic!("no such multisig account"));
            let published: Option<u64> = abi::call(
                self.data_id(),
                "published_bootstrap_nonce",
                &args.account_id,
            )
            .expect("knot-registry-data published_bootstrap_nonce failed");
            if published == Some(account.nonce) {
                panic!("publish_bootstrap_root: already published at this nonce");
            }

            let member_pks: Vec<[u8; 96]> =
                account.members.iter().map(|pk| pk.to_bytes()).collect();
            let published_nonce = account.nonce;
            let msg = evm_bootstrap_message_v1(
                u64::from(abi::chain_id()),
                &abi::self_id().to_bytes(),
                args.account_id,
                account.nonce,
                &receiver,
                &member_pks,
                account.threshold,
            )
            .expect("member set within encoding caps");
            require_quorum(
                &account.members,
                account.threshold,
                &msg,
                &args.sigs,
                "publish_bootstrap_root",
            );

            let payload = encode_bootstrap_calldata(
                args.account_id,
                u64::from(abi::chain_id()),
                &member_pks,
                account.threshold,
            );
            let min_gas = bootstrap_min_gas(account.members.len() as u32);
            abi::call::<([u8; 20], Vec<u8>, u32), ()>(
                l1_messenger,
                "sendMessage",
                &(receiver, payload, min_gas),
            )
            .expect("cross-contract call to L1 messenger sendMessage failed");
            let _: () = abi::call(
                self.data_id(),
                "mark_bootstrap_published",
                &(args.account_id, published_nonce),
            )
            .expect("knot-registry-data mark_bootstrap_published failed");
            abi::emit("evm_bootstrap_sent", args.account_id);
        }

        fn data_id(&self) -> ContractId {
            self.data
                .expect("knot-registry data not set: call init_data first")
        }
    }

    fn emit_effect(account_id: u64, effect: RegistryBookEffect) {
        match effect {
            RegistryBookEffect::Scheduled { execute_at, change } => {
                abi::emit(
                    "pending_scheduled",
                    PendingScheduled {
                        account_id,
                        execute_at,
                        change,
                    },
                );
            }
            RegistryBookEffect::AccountChanged { members, threshold } => {
                abi::emit(
                    "account_changed",
                    AccountChanged {
                        account_id,
                        members,
                        threshold,
                    },
                );
            }
            RegistryBookEffect::TimelockSet { blocks } => {
                abi::emit("timelock_set", TimelockSet { account_id, blocks });
            }
        }
    }

    fn transfer_contract_id() -> ContractId {
        let mut bytes = [0u8; 32];
        bytes[0] = 1;
        ContractId::from_bytes(bytes)
    }

    fn is_direct_account_call() -> bool {
        match abi::caller() {
            None => true,
            Some(id) => id == transfer_contract_id(),
        }
    }

    fn require_direct_owner() {
        if !is_direct_account_call() {
            panic!("Only the contract owner may configure knot-registry");
        }
        let sender = abi::public_sender();
        let owner = abi::self_owner();
        if sender != Some(owner) {
            panic!("Only the contract owner may configure knot-registry");
        }
    }

    fn validate_committee(members: &[BlsPublicKey], threshold: u32) {
        if members.is_empty() {
            panic!("multisig account must have at least one member");
        }
        if members.len() > MAX_COMMITTEE_MEMBERS {
            panic!("multisig account exceeds MAX_COMMITTEE_MEMBERS");
        }
        if threshold == 0 || threshold as usize > members.len() {
            panic!("threshold must be between 1 and committee size");
        }
        if has_duplicates(members) {
            panic!("multisig account members must be distinct");
        }
    }

    fn has_duplicates(keys: &[BlsPublicKey]) -> bool {
        for i in 0..keys.len() {
            for j in (i + 1)..keys.len() {
                if keys[i] == keys[j] {
                    return true;
                }
            }
        }
        false
    }

    fn quorum_met(
        members: &[BlsPublicKey],
        threshold: u32,
        msg: &[u8],
        sigs: &[SignatureEntry],
    ) -> bool {
        let (_matched, verified) = quorum_counts(members, msg, sigs);
        verified >= threshold
    }

    fn quorum_counts(members: &[BlsPublicKey], msg: &[u8], sigs: &[SignatureEntry]) -> (u32, u32) {
        if sigs.len() > members.len() {
            return (0, 0);
        }
        let mut counted: Vec<BlsPublicKey> = Vec::new();
        let mut matched = 0u32;
        let mut verified = 0u32;
        for entry in sigs {
            if !members.contains(&entry.signer) {
                continue;
            }
            matched += 1;
            if counted.contains(&entry.signer) {
                continue;
            }
            if abi::verify_bls(msg.to_vec(), entry.signer, entry.signature) {
                counted.push(entry.signer);
                verified += 1;
            }
        }
        (matched, verified)
    }

    fn require_quorum(
        members: &[BlsPublicKey],
        threshold: u32,
        msg: &[u8],
        sigs: &[SignatureEntry],
        what: &str,
    ) {
        let (matched, verified) = quorum_counts(members, msg, sigs);
        if verified < threshold {
            panic!(
                "{what}: quorum not met by current members \
                 (members={}, threshold={}, member_matches={}, sigs_ok={})",
                members.len(),
                threshold,
                matched,
                verified
            );
        }
    }

    fn pending_kind_and_payload(change: &RegistryPendingChange) -> (u8, Vec<u8>) {
        match change {
            RegistryPendingChange::ChangeAccount {
                new_members,
                new_threshold,
            } => {
                let member_pks: Vec<[u8; 96]> =
                    new_members.iter().map(|pk| pk.to_bytes()).collect();
                let payload = cancel_pending_change_account_payload(&member_pks, *new_threshold)
                    .expect("pending payload within encoding caps");
                (PENDING_KIND_CHANGE_ACCOUNT, payload)
            }
            RegistryPendingChange::SetTimelock(blocks) => (
                PENDING_KIND_SET_TIMELOCK,
                cancel_pending_set_timelock_payload(*blocks),
            ),
        }
    }
}
