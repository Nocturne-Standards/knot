#[dusk_forge::contract(events = [
    DataSet,
    RegistrySet,
    ProposalTtlSet,
    TombstoneSet,
    AuthorizedAccountSet,
    ProposalCreated,
    ProposalApproved,
    ProposalFinalized,
    ProposalQueued,
    ProposalCancelled,
    Pruned,
])]
mod knot_proposals {
    use alloc::vec::Vec;

    use dusk_core::abi::{self, ContractId, block_height, chain_id};
    use dusk_core::signatures::bls::PublicKey as BlsPublicKey;

    use knot_encoding::events::{
        AuthorizedAccountSet, DataSet, ProposalApproved, ProposalCancelled, ProposalCreated,
        ProposalFinalized, ProposalQueued, ProposalTtlSet, Pruned, RegistrySet, TombstoneSet,
    };
    use knot_encoding::{cancel_proposal_message_v1, proposal_digest_v3};
    use knot_proposals::call_types::{
        ApproveArgs, CancelProposalArgs, DigestView, MultisigAccountView, OpenProposal,
        ProposalStatus, ProposalView, ProposalsConfig, ProposeArgs, PruneReport, SignatureEntry,
        VerifyQuorumArgs,
    };

    const MAX_FUNCTION_NAME_LEN: usize = 64;
    const MAX_CALL_ARGS_LEN: usize = 4096;
    const MAX_PROPOSAL_TTL: u64 = 100_000;

    /// API for the proposal book on `knot-proposals-data`.
    /// `call_raw` stays here so the target sees this contract as `caller`.
    pub struct MultisigProposalsState {
        /// Book contract. Not cached from Atlas.
        data: Option<ContractId>,
    }

    impl MultisigProposalsState {
        pub const fn new() -> Self {
            Self { data: None }
        }

        /// Owner-only, direct account call. Points this contract at its book.
        /// One-shot: the same id may be retried; a different book panics.
        pub fn init_data(&mut self, data: ContractId) {
            Self::require_direct_owner();
            match self.data {
                Some(current) if current == data => {}
                Some(_) => panic!("knot-proposals data already set"),
                None => {
                    self.data = Some(data);
                    abi::emit("data_set", DataSet { data });
                }
            }
        }

        /// Owner-only, direct account call. Bumps epoch and clears the
        /// authorized account so the binding is the new registry pair.
        pub fn init_registry(&mut self, registry: ContractId) {
            Self::require_direct_owner();
            let epoch: u64 = abi::call(self.data_id(), "set_registry", &registry)
                .expect("knot-proposals-data set_registry failed");
            abi::emit("registry_set", RegistrySet { registry, epoch });
        }

        pub fn set_proposal_ttl(&mut self, blocks: u64) {
            Self::require_direct_owner();
            if blocks == 0 || blocks > MAX_PROPOSAL_TTL {
                panic!("proposal_ttl out of range");
            }
            let _: () = abi::call(self.data_id(), "set_proposal_ttl", &blocks)
                .expect("knot-proposals-data set_proposal_ttl failed");
            abi::emit("proposal_ttl_set", ProposalTtlSet { blocks });
        }

        pub fn set_tombstone(&mut self, tombstone: bool) {
            Self::require_direct_owner();
            let _: () = abi::call(self.data_id(), "set_tombstone", &tombstone)
                .expect("knot-proposals-data set_tombstone failed");
            abi::emit("tombstone_set", TombstoneSet { tombstone });
        }

        /// Owner-only, direct account call. Only this registry account may
        /// `propose`, `finalize`, or `execute` through this contract.
        /// Each call bumps `auth_generation`, so open and queued proposals
        /// from the previous binding cannot run, including after a switch back.
        pub fn set_authorized_account(&mut self, account_id: u64) {
            Self::require_direct_owner();
            let _: () = abi::call(self.data_id(), "set_authorized_account", &account_id)
                .expect("knot-proposals-data set_authorized_account failed");
            let auth_generation = self.book().auth_generation;
            abi::emit(
                "authorized_account_set",
                AuthorizedAccountSet {
                    account_id,
                    auth_generation,
                },
            );
        }

        pub fn epoch(&self) -> u64 {
            self.book().epoch
        }

        pub fn proposal_ttl(&self) -> u64 {
            self.book().proposal_ttl
        }

        pub fn propose(&mut self, args: ProposeArgs) -> u64 {
            let book = self.book();
            if book.registry.is_none() {
                panic!("knot-proposals not initialized: call init_registry first");
            }
            require_authorized(&book, args.registry_account_id);
            if args.function_name.len() > MAX_FUNCTION_NAME_LEN {
                panic!("function_name too long");
            }
            if args.call_args.len() > MAX_CALL_ARGS_LEN {
                panic!("call_args too long");
            }
            if book.proposal_ttl == 0 {
                panic!("proposal_ttl not configured");
            }
            if args.deadline == 0 {
                panic!("proposal deadline must be non-zero");
            }

            let now = block_height();
            let max_deadline = now.checked_add(book.proposal_ttl).expect("ttl overflow");
            let deadline = args.deadline;
            if deadline < now {
                panic!("proposal deadline is in the past");
            }
            if deadline > max_deadline {
                panic!("proposal deadline exceeds max TTL");
            }

            let digest = proposal_digest_v3(
                u64::from(chain_id()),
                &abi::self_id().to_bytes(),
                book.epoch,
                args.registry_account_id,
                args.nonce,
                &args.target.to_bytes(),
                args.function_name.as_bytes(),
                &args.call_args,
                deadline,
            )
            .expect("propose caps keep function_name/call_args within u32");

            if let Some(rec) = self.digest(digest) {
                if rec.consumed {
                    panic!("proposal digest already executed");
                }
                if rec.epoch != book.epoch {
                    panic!("proposal digest belongs to a retired epoch");
                }
                match self.proposal(rec.proposal_id).map(|p| p.status) {
                    Some(ProposalStatus::Open) => return rec.proposal_id,
                    _ => panic!("proposal digest already used"),
                }
            }

            let id: u64 = abi::call(
                self.data_id(),
                "open_proposal",
                &OpenProposal {
                    registry_account_id: args.registry_account_id,
                    nonce: args.nonce,
                    epoch: book.epoch,
                    target: args.target,
                    function_name: args.function_name.clone(),
                    call_args: args.call_args.clone(),
                    deadline,
                    signed_digest: digest,
                },
            )
            .expect("knot-proposals-data open_proposal failed");
            abi::emit(
                "proposal_created",
                ProposalCreated {
                    proposal_id: id,
                    signed_digest: digest,
                    registry_account_id: args.registry_account_id,
                    deadline,
                    epoch: book.epoch,
                    nonce: args.nonce,
                    auth_generation: book.auth_generation,
                    target: args.target,
                    function_name: args.function_name,
                    call_args: args.call_args,
                },
            );
            id
        }

        pub fn approve(&mut self, args: ApproveArgs) {
            let book = self.book();
            let registry = book
                .registry
                .expect("knot-proposals not initialized: call init_registry first");
            let proposal = self
                .proposal(args.proposal_id)
                .unwrap_or_else(|| panic!("no such proposal"));
            if proposal.status != ProposalStatus::Open {
                panic!("proposal is not open");
            }
            if proposal.epoch != book.epoch {
                panic!("proposal belongs to a retired epoch");
            }
            if proposal.deadline != 0 && block_height() > proposal.deadline {
                panic!("proposal deadline passed");
            }

            let view: Option<MultisigAccountView> =
                abi::call(registry, "account", &proposal.registry_account_id)
                    .expect("cross-contract call to knot-registry account failed");
            let view = view.unwrap_or_else(|| panic!("unknown knot-registry account"));
            if !view.members.contains(&args.signer) {
                panic!("signer is not a member of the proposal's registry account");
            }
            if proposal.approvals.contains(&args.signer) {
                panic!("signer has already approved this proposal");
            }
            let msg = proposal.signed_digest.to_vec();
            if !abi::verify_bls(msg, args.signer, args.signature) {
                panic!("invalid BLS signature over proposal digest");
            }
            let _: () = abi::call(
                self.data_id(),
                "push_approval",
                &(args.proposal_id, args.signer, args.signature),
            )
            .expect("knot-proposals-data push_approval failed");
            abi::emit(
                "proposal_approved",
                ProposalApproved {
                    proposal_id: args.proposal_id,
                    signed_digest: proposal.signed_digest,
                    signer: args.signer,
                    signature: args.signature,
                },
            );
        }

        pub fn proposal(&self, id: u64) -> Option<ProposalView> {
            abi::call(self.data_id(), "proposal", &id).expect("knot-proposals-data proposal failed")
        }

        pub fn status(&self, id: u64) -> Option<ProposalStatus> {
            self.proposal(id).map(|p| p.status)
        }

        pub fn next_proposal_id(&self) -> u64 {
            abi::call(self.data_id(), "next_proposal_id", &())
                .expect("knot-proposals-data next_proposal_id failed")
        }

        pub fn finalize(&mut self, proposal_id: u64) {
            let book = self.book();
            let registry = book
                .registry
                .expect("knot-proposals not initialized: call init_registry first");
            let proposal = self
                .proposal(proposal_id)
                .unwrap_or_else(|| panic!("no such proposal"));
            if proposal.status != ProposalStatus::Open {
                panic!("proposal is not open");
            }
            if proposal.epoch != book.epoch {
                panic!("proposal belongs to a retired epoch");
            }
            if proposal.deadline != 0 && block_height() > proposal.deadline {
                panic!("proposal deadline passed");
            }
            require_authorized(&book, proposal.registry_account_id);
            require_generation(&book, &proposal);
            require_issued_here(&proposal);

            let view: Option<MultisigAccountView> =
                abi::call(registry, "account", &proposal.registry_account_id)
                    .expect("cross-contract call to knot-registry account failed");
            let view = view.unwrap_or_else(|| panic!("unknown knot-registry account"));
            let sigs = current_member_sigs(&proposal, &view.members);
            if (sigs.len() as u32) < view.threshold {
                panic!(
                    "finalize: quorum not met (approvals={}, threshold={})",
                    sigs.len(),
                    view.threshold
                );
            }
            let quorum_args = VerifyQuorumArgs {
                account_id: proposal.registry_account_id,
                msg: proposal.signed_digest.to_vec(),
                sigs,
            };
            let ok: bool = abi::call(registry, "verify_quorum", &quorum_args)
                .expect("cross-contract call to knot-registry verify_quorum failed");
            if !ok {
                panic!("finalize: registry verify_quorum rejected collected approvals");
            }

            self.refuse_target(proposal.target);
            let digest = proposal.signed_digest;
            let target = proposal.target;
            let fn_name = proposal.function_name.clone();
            let call_args = proposal.call_args.clone();
            let committee = proposal.registry_account_id;
            let deadline = proposal.deadline;
            let delay = view.timelock_blocks;

            if delay == 0 {
                let _: () = abi::call(self.data_id(), "commit_executed", &proposal_id)
                    .expect("knot-proposals-data commit_executed failed");
                abi::emit(
                    "proposal_finalized",
                    ProposalFinalized {
                        proposal_id,
                        signed_digest: digest,
                        registry_account_id: committee,
                        target,
                        function_name: fn_name.clone(),
                        call_args: call_args.clone(),
                    },
                );
                let _ = abi::call_raw(target, &fn_name, &call_args)
                    .expect("finalize: call_raw to target failed");
                return;
            }

            let execute_at = block_height()
                .checked_add(delay)
                .expect("timelock overflow");
            if deadline != 0 && execute_at > deadline {
                panic!("proposal delay exceeds deadline");
            }
            let _: () = abi::call(self.data_id(), "queue", &(proposal_id, execute_at))
                .expect("knot-proposals-data queue failed");
            abi::emit(
                "proposal_queued",
                ProposalQueued {
                    proposal_id,
                    signed_digest: digest,
                    registry_account_id: committee,
                    execute_at,
                    target,
                    function_name: fn_name,
                    call_args,
                },
            );
        }

        pub fn execute(&mut self, proposal_id: u64) {
            let book = self.book();
            let proposal = self
                .proposal(proposal_id)
                .unwrap_or_else(|| panic!("no such proposal"));
            if proposal.status != ProposalStatus::Queued {
                panic!("proposal is not queued");
            }
            if proposal.epoch != book.epoch {
                panic!("proposal belongs to a retired epoch");
            }
            if block_height() < proposal.execute_at {
                panic!("timelock not elapsed");
            }
            if proposal.deadline != 0 && block_height() > proposal.deadline {
                panic!("proposal deadline passed");
            }
            require_authorized(&book, proposal.registry_account_id);
            require_generation(&book, &proposal);
            require_issued_here(&proposal);
            self.refuse_target(proposal.target);

            let digest = proposal.signed_digest;
            let target = proposal.target;
            let fn_name = proposal.function_name.clone();
            let call_args = proposal.call_args.clone();
            let committee = proposal.registry_account_id;
            let _: () = abi::call(self.data_id(), "commit_executed", &proposal_id)
                .expect("knot-proposals-data commit_executed failed");
            abi::emit(
                "proposal_finalized",
                ProposalFinalized {
                    proposal_id,
                    signed_digest: digest,
                    registry_account_id: committee,
                    target,
                    function_name: fn_name.clone(),
                    call_args: call_args.clone(),
                },
            );
            let _ = abi::call_raw(target, &fn_name, &call_args)
                .expect("execute: call_raw to target failed");
        }

        pub fn cancel(&mut self, args: CancelProposalArgs) {
            let book = self.book();
            let registry = book
                .registry
                .expect("knot-proposals not initialized: call init_registry first");
            let proposal = self
                .proposal(args.proposal_id)
                .unwrap_or_else(|| panic!("no such proposal"));
            if proposal.status != ProposalStatus::Queued {
                panic!("proposal is not queued");
            }
            if proposal.epoch != book.epoch {
                panic!("proposal belongs to a retired epoch");
            }
            let view: Option<MultisigAccountView> =
                abi::call(registry, "account", &proposal.registry_account_id)
                    .expect("cross-contract call to knot-registry account failed");
            if view.is_none() {
                panic!("unknown knot-registry account");
            }
            let msg = cancel_proposal_message_v1(
                u64::from(chain_id()),
                &abi::self_id().to_bytes(),
                args.proposal_id,
                &proposal.signed_digest,
            )
            .expect("cancel encoding");
            let quorum_args = VerifyQuorumArgs {
                account_id: proposal.registry_account_id,
                msg,
                sigs: args.sigs,
            };
            let ok: bool = abi::call(registry, "verify_quorum", &quorum_args)
                .expect("cross-contract call to knot-registry verify_quorum failed");
            if !ok {
                panic!("cancel: registry verify_quorum rejected");
            }
            let digest = proposal.signed_digest;
            let _: () = abi::call(self.data_id(), "commit_cancelled", &args.proposal_id)
                .expect("knot-proposals-data commit_cancelled failed");
            abi::emit(
                "proposal_cancelled",
                ProposalCancelled {
                    proposal_id: args.proposal_id,
                    signed_digest: digest,
                    registry_account_id: proposal.registry_account_id,
                },
            );
        }

        pub fn prune(&mut self, limit: u32) -> u32 {
            let report: PruneReport = abi::call(self.data_id(), "prune", &limit)
                .expect("knot-proposals-data prune failed");
            let pruned = u32::try_from(report.proposal_ids.len()).expect("prune count");
            if !report.proposal_ids.is_empty() || !report.digest_keys.is_empty() {
                abi::emit(
                    "pruned",
                    Pruned {
                        proposal_ids: report.proposal_ids.clone(),
                        digest_keys: report.digest_keys.clone(),
                    },
                );
            }
            pruned
        }

        fn book(&self) -> ProposalsConfig {
            abi::call(self.data_id(), "config", &()).expect("knot-proposals-data config failed")
        }

        fn digest(&self, key: [u8; 32]) -> Option<DigestView> {
            abi::call(self.data_id(), "digest", &key).expect("knot-proposals-data digest failed")
        }

        fn data_id(&self) -> ContractId {
            self.data
                .expect("knot-proposals data not set: call init_data first")
        }

        fn refuse_target(&self, target: ContractId) {
            if target == abi::self_id() || target == self.data_id() {
                panic!("finalize: target must not be this contract");
            }
        }

        fn require_direct_owner() {
            if !is_direct_account_call() {
                panic!("Only the contract owner may configure knot-proposals");
            }
            let sender = abi::public_sender();
            let owner = abi::self_owner();
            if sender != Some(owner) {
                panic!("Only the contract owner may configure knot-proposals");
            }
        }
    }

    fn require_authorized(book: &ProposalsConfig, account_id: u64) {
        match book.authorized_account {
            Some(id) if id == account_id => {}
            _ => panic!("proposal account is not the authorized registry account"),
        }
    }

    fn require_generation(book: &ProposalsConfig, proposal: &ProposalView) {
        if proposal.auth_generation != book.auth_generation {
            panic!("proposal belongs to a retired account binding");
        }
    }

    /// Stored digest must be the v3 digest of this contract. A replacement
    /// logic contract sharing the book fails here, before `call_raw`.
    fn require_issued_here(proposal: &ProposalView) {
        let digest = proposal_digest_v3(
            u64::from(chain_id()),
            &abi::self_id().to_bytes(),
            proposal.epoch,
            proposal.registry_account_id,
            proposal.nonce,
            &proposal.target.to_bytes(),
            proposal.function_name.as_bytes(),
            &proposal.call_args,
            proposal.deadline,
        )
        .expect("proposal digest encoding");
        if digest != proposal.signed_digest {
            panic!("proposal was not issued by this contract");
        }
    }

    /// Drop signatures from keys that are no longer members. The registry
    /// rejects a signature vector longer than the current member list.
    fn current_member_sigs(
        proposal: &ProposalView,
        members: &[BlsPublicKey],
    ) -> Vec<SignatureEntry> {
        proposal
            .approvals
            .iter()
            .zip(proposal.approval_sigs.iter())
            .filter(|(signer, _)| members.contains(signer))
            .map(|(signer, signature)| SignatureEntry {
                signer: *signer,
                signature: *signature,
            })
            .collect()
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
}
