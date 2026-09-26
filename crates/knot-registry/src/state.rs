#[dusk_forge::contract]
mod knot_registry {
    use alloc::vec::Vec;

    use dusk_bytes::Serializable;
    use dusk_core::abi::{self, ContractId};
    use dusk_core::signatures::bls::PublicKey as BlsPublicKey;
    use knot_encoding::{
        PENDING_KIND_CHANGE_ACCOUNT, PENDING_KIND_SET_TIMELOCK,
        cancel_pending_change_account_payload, cancel_pending_message_v1,
        cancel_pending_set_timelock_payload, change_account_message_v3, set_timelock_message_v1,
    };

    use knot_registry::call_types::{
        CancelPendingArgs, ChangeAccountArgs, CreateAccountArgs, MultisigAccountView,
        RegistryBookEffect, RegistryPendingChange, SetTimelockArgs, SignatureEntry,
        VerifyQuorumAggregateArgs, VerifyQuorumArgs,
    };

    const MAX_COMMITTEE_MEMBERS: usize = 16;

    /// API for the account book on `knot-registry-data`.
    pub struct MultisigRegistryState {
        /// Book contract. Not cached from Atlas: this contract is the id
        /// Atlas resolves, and data checks `abi::caller()` against that.
        data: Option<ContractId>,
    }

    impl MultisigRegistryState {
        pub const fn new() -> Self {
            Self { data: None }
        }

        /// Owner-only, direct account call. Points this contract at its book.
        /// One-shot: the same id may be retried; a different book panics.
        pub fn init_data(&mut self, data: ContractId) {
            require_direct_owner();
            match self.data {
                Some(current) if current == data => {}
                Some(_) => panic!("knot-registry data already set"),
                None => self.data = Some(data),
            }
        }

        pub fn create_account(&mut self, args: CreateAccountArgs) -> u64 {
            validate_committee(&args.members, args.threshold);
            let id: u64 = abi::call(self.data_id(), "create_account", &args)
                .expect("knot-registry-data create_account failed");
            abi::emit("account_created", id);
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
                .as_ref()
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
            abi::emit("pending_cancelled", args.account_id);
        }

        pub fn execute_pending(&mut self, account_id: u64) {
            let effect: RegistryBookEffect = abi::call(self.data_id(), "execute_pending", &account_id)
                .expect("knot-registry-data execute_pending failed");
            emit_effect(account_id, effect);
        }

        fn data_id(&self) -> ContractId {
            self.data
                .expect("knot-registry data not set: call init_data first")
        }
    }

    fn emit_effect(account_id: u64, effect: RegistryBookEffect) {
        match effect {
            RegistryBookEffect::Scheduled(execute_at) => {
                abi::emit("pending_scheduled", (account_id, execute_at));
            }
            RegistryBookEffect::AccountChanged => {
                abi::emit("account_changed", account_id);
            }
            RegistryBookEffect::TimelockSet => {
                abi::emit("timelock_set", account_id);
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
