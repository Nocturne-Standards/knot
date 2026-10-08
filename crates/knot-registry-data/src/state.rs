#[dusk_forge::contract]
mod knot_registry_data {
    use alloc::collections::BTreeMap;
    use alloc::string::String;
    use alloc::vec::Vec;

    use dusk_core::abi::{self, ContractId, block_height};
    use dusk_core::signatures::bls::PublicKey as BlsPublicKey;
    use knot_encoding::call_types::{
        CreateAccountArgs, MultisigAccountView, RegistryBookEffect, RegistryPendingChange,
        RegistryPendingView,
    };

    include!(concat!(env!("OUT_DIR"), "/atlas_pin.rs"));

    const ATLAS_ID: ContractId = ContractId::from_bytes(ATLAS_ID_BYTES);
    const SERVICE: &str = "knot-registry";
    const MAX_COMMITTEE_MEMBERS: usize = 16;

    struct MultisigAccount {
        members: Vec<BlsPublicKey>,
        threshold: u32,
        nonce: u64,
        timelock_blocks: u64,
        pending: Option<(RegistryPendingChange, u64)>,
        /// Registry nonce last sent to `KnotEvmRoot.bootstrap`. A second
        /// publish at the same nonce is refused. `schedule` bumps `nonce`,
        /// which opens one new publish.
        published_bootstrap_nonce: Option<u64>,
    }

    pub struct RegistryDataState {
        accounts: BTreeMap<u64, MultisigAccount>,
        next_id: u64,
    }

    impl RegistryDataState {
        pub const fn new() -> Self {
            Self {
                accounts: BTreeMap::new(),
                next_id: 0,
            }
        }

        pub fn create_account(&mut self, args: CreateAccountArgs) -> u64 {
            self.require_logic_caller();
            validate_committee(&args.members, args.threshold);
            let id = self.next_id;
            self.next_id = self.next_id.checked_add(1).expect("next_id overflow");
            self.accounts.insert(
                id,
                MultisigAccount {
                    members: args.members,
                    threshold: args.threshold,
                    nonce: 0,
                    timelock_blocks: 0,
                    pending: None,
                    published_bootstrap_nonce: None,
                },
            );
            id
        }

        pub fn account(&self, id: u64) -> Option<MultisigAccountView> {
            self.accounts.get(&id).map(|a| MultisigAccountView {
                members: a.members.clone(),
                threshold: a.threshold,
                nonce: a.nonce,
                timelock_blocks: a.timelock_blocks,
                pending: a
                    .pending
                    .as_ref()
                    .map(|(change, execute_at)| RegistryPendingView {
                        change: change.clone(),
                        execute_at: *execute_at,
                    }),
            })
        }

        pub fn next_account_id(&self) -> u64 {
            self.next_id
        }

        /// Nonce last published for this account. `None` when the account
        /// is missing or has never been published.
        pub fn published_bootstrap_nonce(&self, account_id: u64) -> Option<u64> {
            self.accounts
                .get(&account_id)
                .and_then(|account| account.published_bootstrap_nonce)
        }

        /// Records that `nonce` was sent to the EVM root. Logic-only.
        /// Refuses a second mark at the account's current nonce.
        pub fn mark_bootstrap_published(&mut self, account_id: u64, nonce: u64) {
            self.require_logic_caller();
            let account = self
                .accounts
                .get_mut(&account_id)
                .unwrap_or_else(|| panic!("no such multisig account"));
            if account.published_bootstrap_nonce == Some(account.nonce) {
                panic!("publish_bootstrap_root: already published at this nonce");
            }
            if nonce != account.nonce {
                panic!("publish_bootstrap_root: nonce mismatch");
            }
            account.published_bootstrap_nonce = Some(nonce);
        }

        /// One pending slot. Delay 0 applies before return. Nonce bumps here.
        pub fn schedule(
            &mut self,
            account_id: u64,
            change: RegistryPendingChange,
        ) -> RegistryBookEffect {
            self.require_logic_caller();
            if let RegistryPendingChange::ChangeAccount {
                new_members,
                new_threshold,
            } = &change
            {
                validate_committee(new_members, *new_threshold);
            }
            let delay = {
                let account = self
                    .accounts
                    .get_mut(&account_id)
                    .unwrap_or_else(|| panic!("no such multisig account"));
                if account.pending.is_some() {
                    panic!("a pending change already exists; cancel or execute it first");
                }
                account.nonce = account.nonce.checked_add(1).expect("nonce overflow");
                let execute_at = block_height()
                    .checked_add(account.timelock_blocks)
                    .expect("timelock overflow");
                let delay = account.timelock_blocks;
                account.pending = Some((change, execute_at));
                delay
            };
            if delay != 0 {
                let (change, execute_at) = self
                    .accounts
                    .get(&account_id)
                    .unwrap()
                    .pending
                    .clone()
                    .unwrap();
                return RegistryBookEffect::Scheduled { execute_at, change };
            }
            self.apply(account_id)
        }

        pub fn clear_pending(&mut self, account_id: u64) {
            self.require_logic_caller();
            let account = self
                .accounts
                .get_mut(&account_id)
                .unwrap_or_else(|| panic!("no such multisig account"));
            if account.pending.is_none() {
                panic!("no pending change");
            }
            account.pending = None;
        }

        pub fn execute_pending(&mut self, account_id: u64) -> RegistryBookEffect {
            self.require_logic_caller();
            let account = self
                .accounts
                .get(&account_id)
                .unwrap_or_else(|| panic!("no such multisig account"));
            let (_, execute_at) = account
                .pending
                .as_ref()
                .unwrap_or_else(|| panic!("no pending change"));
            if block_height() < *execute_at {
                panic!("timelock not elapsed");
            }
            self.apply(account_id)
        }

        fn apply(&mut self, account_id: u64) -> RegistryBookEffect {
            let account = self.accounts.get_mut(&account_id).unwrap();
            let (change, _) = account
                .pending
                .take()
                .unwrap_or_else(|| panic!("no pending change"));
            match change {
                RegistryPendingChange::ChangeAccount {
                    new_members,
                    new_threshold,
                } => {
                    account.members = new_members.clone();
                    account.threshold = new_threshold;
                    RegistryBookEffect::AccountChanged {
                        members: new_members,
                        threshold: new_threshold,
                    }
                }
                RegistryPendingChange::SetTimelock(blocks) => {
                    account.timelock_blocks = blocks;
                    RegistryBookEffect::TimelockSet { blocks }
                }
            }
        }

        fn require_logic_caller(&self) {
            let name = String::from(SERVICE);
            let logic: Option<ContractId> =
                abi::call(ATLAS_ID, "resolve", &name).expect("atlas resolve(knot-registry) failed");
            let logic = logic.unwrap_or_else(|| panic!("atlas has no knot-registry service"));
            if abi::caller() != Some(logic) {
                panic!("caller is not the knot-registry logic contract");
            }
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
}
