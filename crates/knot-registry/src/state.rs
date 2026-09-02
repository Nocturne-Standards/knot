#[dusk_forge::contract]
mod knot_registry {
    use alloc::collections::BTreeMap;
    use alloc::vec::Vec;

    use dusk_bytes::Serializable;
    use dusk_core::abi::{self, block_height};
    use dusk_core::signatures::bls::PublicKey as BlsPublicKey;
    use knot_encoding::{
        PENDING_KIND_CHANGE_ACCOUNT, PENDING_KIND_SET_TIMELOCK,
        cancel_pending_change_account_payload, cancel_pending_message_v1,
        cancel_pending_set_timelock_payload, change_account_message_v3, set_timelock_message_v1,
    };

    use knot_registry::call_types::{
        CancelPendingArgs, ChangeAccountArgs, CreateAccountArgs, MultisigAccountView,
        RegistryPendingChange, RegistryPendingView, SetTimelockArgs, SignatureEntry,
        VerifyQuorumAggregateArgs, VerifyQuorumArgs,
    };

    /// Soft cap on committee size — enough for operator councils; bounds
    /// O(n²) duplicate checks and per-sig verify work (audit I6).
    const MAX_COMMITTEE_MEMBERS: usize = 16;

    /// One registered account: member set, threshold, nonce, delay, and at
    /// most one pending mutation.
    struct MultisigAccount {
        members: Vec<BlsPublicKey>,
        threshold: u32,
        nonce: u64,
        timelock_blocks: u64,
        pending: Option<(RegistryPendingChange, u64)>,
    }

    pub struct MultisigRegistryState {
        accounts: BTreeMap<u64, MultisigAccount>,
        next_id: u64,
    }

    impl MultisigRegistryState {
        pub const fn new() -> Self {
            Self {
                accounts: BTreeMap::new(),
                next_id: 0,
            }
        }

        /// Anyone may register an account for their own member set — there
        /// is nothing to gate (the caller is only naming *other* keys as
        /// members, not granting themselves special power; the account is
        /// only ever useful to whoever those members' signatures belong
        /// to). Rejects an empty/duplicate member list or an out-of-range
        /// threshold — same bounds `prediction-market::validate_committee`
        /// enforces for its dispute council.
        pub fn create_account(&mut self, args: CreateAccountArgs) -> u64 {
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
                },
            );
            abi::emit("account_created", id);
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

        /// Next account id that `create_account` will hand out.
        pub fn next_account_id(&self) -> u64 {
            self.next_id
        }

        /// Pure verification primitive — see `VerifyQuorumArgs`'s doc for
        /// why replay protection is the caller's job here, not this
        /// registry's. Returns `false` (not panic) for an unknown
        /// `account_id`, matching `compliance-gate::check`'s convention of
        /// returning bool for the actual authorization outcome and
        /// reserving panics for infrastructure failures.
        pub fn verify_quorum(&self, args: VerifyQuorumArgs) -> bool {
            let Some(account) = self.accounts.get(&args.account_id) else {
                return false;
            };
            quorum_met(&account.members, account.threshold, &args.msg, &args.sigs)
        }

        /// Same question as `verify_quorum` — does `msg` have a valid
        /// quorum of member signatures? — checked with one native
        /// `abi::verify_bls_multisig` pairing check instead of one
        /// `abi::verify_bls` per signer. See `VerifyQuorumAggregateArgs`'s
        /// doc for what the caller must have already assembled off-chain.
        /// Returns `false` (never panics) for an unknown account, a
        /// `signer_keys` set smaller than the threshold, a set containing a
        /// duplicate or non-member key, or an aggregate signature that
        /// doesn't verify — same bool convention as `verify_quorum`.
        pub fn verify_quorum_aggregate(&self, args: VerifyQuorumAggregateArgs) -> bool {
            let Some(account) = self.accounts.get(&args.account_id) else {
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

        /// Replaces `account_id`'s member set / threshold, authorized by a
        /// quorum of the account's *current* members signing over
        /// `knot_encoding::change_account_message_v3`.
        /// Bumps the nonce at schedule so a captured quorum signature can't
        /// be replayed. Delay 0 applies in this call; otherwise wait for
        /// `execute_pending`.
        pub fn change_account(&mut self, args: ChangeAccountArgs) {
            validate_committee(&args.new_members, args.new_threshold);

            let account = self
                .accounts
                .get(&args.account_id)
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

            self.schedule(
                args.account_id,
                RegistryPendingChange::ChangeAccount {
                    new_members: args.new_members,
                    new_threshold: args.new_threshold,
                },
            );
        }

        /// Change this account's delay. Same nonce / pending slot as
        /// `change_account`. Raising from 0 applies immediately.
        pub fn set_timelock(&mut self, args: SetTimelockArgs) {
            let account = self
                .accounts
                .get(&args.account_id)
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
            self.schedule(
                args.account_id,
                RegistryPendingChange::SetTimelock(args.blocks),
            );
        }

        /// Immediate cancel of this account's pending, authorized by current
        /// members over a digest bound to this pending payload.
        pub fn cancel_pending(&mut self, args: CancelPendingArgs) {
            let account = self
                .accounts
                .get(&args.account_id)
                .unwrap_or_else(|| panic!("no such multisig account"));
            let (change, execute_at) = account
                .pending
                .as_ref()
                .unwrap_or_else(|| panic!("no pending change"));
            let (kind, payload) = pending_kind_and_payload(change);
            let msg = cancel_pending_message_v1(
                u64::from(abi::chain_id()),
                &abi::self_id().to_bytes(),
                args.account_id,
                *execute_at,
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
            let account = self.accounts.get_mut(&args.account_id).unwrap();
            account.pending = None;
            abi::emit("pending_cancelled", args.account_id);
        }

        /// Permissionless apply after `execute_at`.
        pub fn execute_pending(&mut self, account_id: u64) {
            let account = self
                .accounts
                .get(&account_id)
                .unwrap_or_else(|| panic!("no such multisig account"));
            let (change, execute_at) = account
                .pending
                .clone()
                .unwrap_or_else(|| panic!("no pending change"));
            if block_height() < execute_at {
                panic!("timelock not elapsed");
            }
            let account = self.accounts.get_mut(&account_id).unwrap();
            account.pending = None;
            match change {
                RegistryPendingChange::ChangeAccount {
                    new_members,
                    new_threshold,
                } => {
                    account.members = new_members;
                    account.threshold = new_threshold;
                    abi::emit("account_changed", account_id);
                }
                RegistryPendingChange::SetTimelock(blocks) => {
                    account.timelock_blocks = blocks;
                    abi::emit("timelock_set", account_id);
                }
            }
        }

        fn schedule(&mut self, account_id: u64, change: RegistryPendingChange) {
            let delay;
            {
                let account = self.accounts.get_mut(&account_id).unwrap();
                if account.pending.is_some() {
                    panic!("a pending change already exists; cancel or execute it first");
                }
                account.nonce = account.nonce.checked_add(1).expect("nonce overflow");
                let execute_at = block_height()
                    .checked_add(account.timelock_blocks)
                    .expect("timelock overflow");
                delay = account.timelock_blocks;
                account.pending = Some((change, execute_at));
                if delay != 0 {
                    abi::emit("pending_scheduled", (account_id, execute_at));
                    return;
                }
            }
            self.execute_pending(account_id);
        }
    }

    /// Same bounds as prediction-market's council validation, plus a hard
    /// cap of [`MAX_COMMITTEE_MEMBERS`] members.
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

    /// Counts distinct valid signatures from `members` over `msg`, ignoring
    /// entries whose `signer` isn't a member or repeats an earlier entry
    /// (so one member's signature can't be counted twice toward quorum).
    fn quorum_met(
        members: &[BlsPublicKey],
        threshold: u32,
        msg: &[u8],
        sigs: &[SignatureEntry],
    ) -> bool {
        let (_matched, verified) = quorum_counts(members, msg, sigs);
        verified >= threshold
    }

    /// Returns `(member_matches, sigs_ok)` — how many sig entries named a
    /// real member, and how many distinct members passed `verify_bls`.
    /// Early-rejects when `sigs.len() > members.len()` (audit I6).
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
