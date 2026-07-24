#[dusk_forge::contract]
mod multisig_registry {
    use alloc::collections::BTreeMap;
    use alloc::vec::Vec;

    use dusk_bytes::Serializable;
    use dusk_core::abi;
    use dusk_core::signatures::bls::PublicKey as BlsPublicKey;
    use multisig_encoding::change_account_message;

    use multisig_registry::call_types::{
        AccountMeta, ChangeAccountArgs, CreateAccountArgs, DiagnoseQuorumResult,
        MultisigAccountView, SignatureEntry, VerifyQuorumAggregateArgs, VerifyQuorumArgs,
    };

    /// Soft cap on committee size — enough for operator councils; bounds
    /// O(n²) duplicate checks and per-sig verify work (audit I6).
    const MAX_COMMITTEE_MEMBERS: usize = 16;

    /// One registered account: its member set, the number of distinct
    /// member signatures required to satisfy a quorum, and a nonce used
    /// only by `change_account` (see that method's doc).
    struct MultisigAccount {
        members: Vec<BlsPublicKey>,
        threshold: u32,
        nonce: u64,
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
            self.next_id += 1;
            self.accounts.insert(
                id,
                MultisigAccount {
                    members: args.members,
                    threshold: args.threshold,
                    nonce: 0,
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
            })
        }

        /// Same lookup as `account`, but returns only scalars — no
        /// `BlsPublicKey` values on the wire. Diagnostic for the testnet
        /// free-read path that always returns `None` from `account`.
        pub fn account_meta(&self, id: u64) -> Option<AccountMeta> {
            self.accounts.get(&id).map(|a| AccountMeta {
                threshold: a.threshold,
                nonce: a.nonce,
                members_len: a.members.len() as u32,
            })
        }

        /// Raw compressed member public keys (96 bytes each). Diagnostic for
        /// comparing on-chain membership against an off-chain keystore.
        pub fn member_key_bytes(&self, id: u64) -> Option<Vec<Vec<u8>>> {
            self.accounts.get(&id).map(|a| {
                a.members
                    .iter()
                    .map(|pk| pk.to_bytes().to_vec())
                    .collect()
            })
        }

        /// Next account id that `create_account` will hand out. Free-read
        /// probe for whether *any* of this contract's state is visible to
        /// RUES queries (should be >0 after successful creates).
        pub fn next_account_id(&self) -> u64 {
            self.next_id
        }

        /// Breaks `quorum_met` into observable counters + dumps member key
        /// bytes. Thin wrapper over [`quorum_counts`] plus a member-pk dump.
        /// Used on testnet where free-read `verify_quorum` returns HTTP 500
        /// and `change_account` only surfaces a single panic string.
        pub fn diagnose_quorum(&self, args: VerifyQuorumArgs) -> DiagnoseQuorumResult {
            let Some(account) = self.accounts.get(&args.account_id) else {
                return DiagnoseQuorumResult {
                    exists: false,
                    threshold: 0,
                    members_len: 0,
                    member_matches: 0,
                    sigs_ok: 0,
                    member_pk_bytes: Vec::new(),
                };
            };

            let member_pk_bytes: Vec<Vec<u8>> = account
                .members
                .iter()
                .map(|pk| pk.to_bytes().to_vec())
                .collect();
            let (member_matches, sigs_ok) =
                quorum_counts(&account.members, &args.msg, &args.sigs);

            DiagnoseQuorumResult {
                exists: true,
                threshold: account.threshold,
                members_len: account.members.len() as u32,
                member_matches,
                sigs_ok,
                member_pk_bytes,
            }
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
        /// doesn't verify — mirrors `verify_quorum`'s bool convention.
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
        /// `multisig_encoding::change_account_message(...)`.
        /// Bumps the nonce on success so a captured quorum signature can't
        /// be replayed against a later change.
        pub fn change_account(&mut self, args: ChangeAccountArgs) {
            validate_committee(&args.new_members, args.new_threshold);

            let account = self
                .accounts
                .get(&args.account_id)
                .unwrap_or_else(|| panic!("no such multisig account"));

            let member_pks: Vec<[u8; 96]> = args
                .new_members
                .iter()
                .map(|pk| pk.to_bytes())
                .collect();
            let msg = change_account_message(
                args.account_id,
                account.nonce,
                &member_pks,
                args.new_threshold,
            );
            let (matched, verified) =
                quorum_counts(&account.members, &msg, &args.sigs);
            if verified < account.threshold {
                panic!(
                    "change_account: quorum not met by current members \
                     (members={}, threshold={}, member_matches={}, sigs_ok={})",
                    account.members.len(),
                    account.threshold,
                    matched,
                    verified
                );
            }

            let account = self.accounts.get_mut(&args.account_id).unwrap();
            account.members = args.new_members;
            account.threshold = args.new_threshold;
            account.nonce += 1;
            abi::emit("account_changed", args.account_id);
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
    fn quorum_counts(
        members: &[BlsPublicKey],
        msg: &[u8],
        sigs: &[SignatureEntry],
    ) -> (u32, u32) {
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
}
