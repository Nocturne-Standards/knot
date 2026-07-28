#[dusk_forge::contract]
mod multisig_proposals {
    use alloc::collections::BTreeMap;
    use alloc::string::String;
    use alloc::vec::Vec;

    use dusk_core::abi::{self, block_height, ContractId};
    use dusk_core::signatures::bls::{PublicKey as BlsPublicKey, Signature as BlsSignature};

    use multisig_encoding::proposal_digest;
    use multisig_proposals::call_types::{
        ApproveArgs, MultisigAccountView, ProposalStatus, ProposalView, ProposeArgs,
        SignatureEntry, VerifyQuorumArgs,
    };

    const MAX_FUNCTION_NAME_LEN: usize = 64;
    const MAX_CALL_ARGS_LEN: usize = 4096;

    struct Proposal {
        registry_account_id: u64,
        chain_id: u64,
        nonce: u64,
        target: ContractId,
        function_name: String,
        call_args: Vec<u8>,
        deadline: u64,
        signed_digest: [u8; 32],
        approvals: Vec<BlsPublicKey>,
        approval_sigs: Vec<BlsSignature>,
        status: ProposalStatus,
    }

    pub struct MultisigProposalsState {
        registry: Option<ContractId>,
        /// Network binding folded into §4a digests (configurable).
        chain_id: u64,
        /// When true, successful finalize marks `Tombstoned`; else `Executed`.
        tombstone: bool,
        /// Default proposal deadline offset from propose height; 0 = require explicit deadline.
        proposal_ttl: u64,
        /// Per-committee monotonic nonce (option A).
        committee_nonces: BTreeMap<u64, u64>,
        /// Digest → proposal id for merge / tombstone lookup.
        by_digest: BTreeMap<[u8; 32], u64>,
        proposals: BTreeMap<u64, Proposal>,
        next_id: u64,
    }

    impl MultisigProposalsState {
        pub const fn new() -> Self {
            Self {
                registry: None,
                chain_id: 0,
                tombstone: false,
                proposal_ttl: 1000,
                committee_nonces: BTreeMap::new(),
                by_digest: BTreeMap::new(),
                proposals: BTreeMap::new(),
                next_id: 0,
            }
        }

        fn require_owner() {
            let sender = abi::public_sender();
            let owner = abi::self_owner();
            if sender != Some(owner) {
                panic!("Only the contract owner may configure multisig-proposals");
            }
        }

        fn require_registry(&self) -> ContractId {
            self.registry
                .expect("multisig-proposals not initialized: call init_registry first")
        }

        /// Owner-only. Points this contract at a deployed `multisig-registry`.
        pub fn init_registry(&mut self, registry: ContractId) {
            Self::require_owner();
            self.wipe_open_proposals();
            self.registry = Some(registry);
            abi::emit("registry_set", ());
        }

        /// Owner-only. Bind §4a digests to a chain id. Wipes open proposals
        /// (digest domain changes).
        pub fn init_chain_id(&mut self, chain_id: u64) {
            Self::require_owner();
            self.wipe_open_proposals();
            self.chain_id = chain_id;
            abi::emit("chain_id_set", chain_id);
        }

        /// Owner-only knobs.
        pub fn set_proposal_ttl(&mut self, blocks: u64) {
            Self::require_owner();
            self.wipe_open_proposals();
            self.proposal_ttl = blocks;
        }

        /// Owner-only. When `true`, successful finalize marks `Tombstoned`
        /// instead of `Executed`. Does **not** wipe open proposals.
        pub fn set_tombstone(&mut self, tombstone: bool) {
            Self::require_owner();
            self.tombstone = tombstone;
        }

        pub fn chain_id(&self) -> u64 {
            self.chain_id
        }

        pub fn committee_nonce(&self, committee_id: u64) -> u64 {
            self.committee_nonces.get(&committee_id).copied().unwrap_or(0)
        }

        /// Open a structured proposal. Digest = §4a Keccak; nonce = current
        /// per-committee counter (not bumped until finalize effects).
        pub fn propose(&mut self, args: ProposeArgs) -> u64 {
            let _ = self.require_registry();
            if self.chain_id == 0 {
                panic!("call init_chain_id before propose");
            }
            if args.function_name.len() > MAX_FUNCTION_NAME_LEN {
                panic!("function_name exceeds max length");
            }
            if args.call_args.len() > MAX_CALL_ARGS_LEN {
                panic!("call_args exceeds max length");
            }

            let nonce = self.committee_nonce(args.registry_account_id);
            let deadline = if args.deadline == 0 {
                if self.proposal_ttl == 0 {
                    0
                } else {
                    block_height()
                        .checked_add(self.proposal_ttl)
                        .expect("deadline overflow")
                }
            } else {
                args.deadline
            };
            if deadline != 0 && deadline <= block_height() {
                panic!("proposal deadline is in the past");
            }

            let digest = proposal_digest(
                self.chain_id,
                args.registry_account_id,
                nonce,
                &args.target.to_bytes(),
                args.function_name.as_bytes(),
                &args.call_args,
                deadline,
            )
            .expect("propose caps keep function_name/call_args within u32");

            // Identical open digest merges into existing open proposal.
            if let Some(&existing_id) = self.by_digest.get(&digest) {
                if let Some(p) = self.proposals.get(&existing_id) {
                    if p.status == ProposalStatus::Open {
                        return existing_id;
                    }
                    if p.status == ProposalStatus::Tombstoned {
                        panic!("proposal digest is tombstoned");
                    }
                }
            }

            let id = self.next_id;
            self.next_id += 1;
            self.proposals.insert(
                id,
                Proposal {
                    registry_account_id: args.registry_account_id,
                    chain_id: self.chain_id,
                    nonce,
                    target: args.target,
                    function_name: args.function_name,
                    call_args: args.call_args,
                    deadline,
                    signed_digest: digest,
                    approvals: Vec::new(),
                    approval_sigs: Vec::new(),
                    status: ProposalStatus::Open,
                },
            );
            self.by_digest.insert(digest, id);
            abi::emit("proposal_created", id);
            id
        }

        pub fn approve(&mut self, args: ApproveArgs) {
            let registry = self.require_registry();

            let proposal = self
                .proposals
                .get_mut(&args.proposal_id)
                .unwrap_or_else(|| panic!("no such proposal"));
            if proposal.status != ProposalStatus::Open {
                panic!("proposal is not open");
            }
            if proposal.deadline != 0 && block_height() > proposal.deadline {
                panic!("proposal deadline passed");
            }

            let view: Option<MultisigAccountView> =
                abi::call(registry, "account", &proposal.registry_account_id)
                    .expect("cross-contract call to multisig-registry account failed");
            let view = view.unwrap_or_else(|| panic!("unknown multisig-registry account"));

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

            proposal.approvals.push(args.signer);
            proposal.approval_sigs.push(args.signature);
            abi::emit("proposal_approved", args.proposal_id);
        }

        pub fn proposal(&self, id: u64) -> Option<ProposalView> {
            self.proposals.get(&id).map(|p| ProposalView {
                registry_account_id: p.registry_account_id,
                chain_id: p.chain_id,
                nonce: p.nonce,
                target: p.target,
                function_name: p.function_name.clone(),
                call_args: p.call_args.clone(),
                deadline: p.deadline,
                signed_digest: p.signed_digest,
                approvals: p.approvals.clone(),
                approval_sigs: p.approval_sigs.clone(),
                status: p.status,
            })
        }

        pub fn status(&self, id: u64) -> Option<ProposalStatus> {
            self.proposals.get(&id).map(|p| p.status)
        }

        pub fn next_proposal_id(&self) -> u64 {
            self.next_id
        }

        /// At threshold: verify quorum, then CEI — mark terminal status, bump
        /// committee nonce, emit — **then** `call_raw` the target.
        ///
        /// A failed `call_raw` still panics (tx reverts), so the proposal stays
        /// `Open` and the nonce is unchanged for retry.
        pub fn finalize(&mut self, proposal_id: u64) {
            let registry = self.require_registry();

            let proposal = self
                .proposals
                .get(&proposal_id)
                .unwrap_or_else(|| panic!("no such proposal"));
            if proposal.status != ProposalStatus::Open {
                panic!("proposal is not open");
            }
            if proposal.deadline != 0 && block_height() > proposal.deadline {
                panic!("proposal deadline passed");
            }
            // Nonce must still match current committee nonce (serialization).
            let current_nonce = self.committee_nonce(proposal.registry_account_id);
            if proposal.nonce != current_nonce {
                panic!("proposal nonce is stale");
            }

            let view: Option<MultisigAccountView> =
                abi::call(registry, "account", &proposal.registry_account_id)
                    .expect("cross-contract call to multisig-registry account failed");
            let view = view.unwrap_or_else(|| panic!("unknown multisig-registry account"));

            if (proposal.approvals.len() as u32) < view.threshold {
                panic!(
                    "finalize: quorum not met (approvals={}, threshold={})",
                    proposal.approvals.len(),
                    view.threshold
                );
            }

            let sigs: Vec<SignatureEntry> = proposal
                .approvals
                .iter()
                .zip(proposal.approval_sigs.iter())
                .map(|(signer, signature)| SignatureEntry {
                    signer: *signer,
                    signature: *signature,
                })
                .collect();
            let quorum_args = VerifyQuorumArgs {
                account_id: proposal.registry_account_id,
                msg: proposal.signed_digest.to_vec(),
                sigs,
            };
            let ok: bool = abi::call(registry, "verify_quorum", &quorum_args)
                .expect("cross-contract call to multisig-registry verify_quorum failed");
            if !ok {
                panic!("finalize: registry verify_quorum rejected collected approvals");
            }

            let target = proposal.target;
            let fn_name = proposal.function_name.clone();
            let call_args = proposal.call_args.clone();
            let committee = proposal.registry_account_id;

            // Effects first (CEI): consume proposal before external call.
            let proposal = self.proposals.get_mut(&proposal_id).unwrap();
            proposal.status = if self.tombstone {
                ProposalStatus::Tombstoned
            } else {
                ProposalStatus::Executed
            };
            self.committee_nonces.insert(committee, current_nonce + 1);
            abi::emit("proposal_finalized", proposal_id);

            // Interaction last.
            let _ = abi::call_raw(target, &fn_name, &call_args)
                .expect("finalize: call_raw to target failed");
        }

        fn wipe_open_proposals(&mut self) {
            let open_ids: Vec<u64> = self
                .proposals
                .iter()
                .filter(|(_, p)| p.status == ProposalStatus::Open)
                .map(|(id, _)| *id)
                .collect();
            for id in open_ids {
                if let Some(p) = self.proposals.get_mut(&id) {
                    let digest = p.signed_digest;
                    p.status = ProposalStatus::Tombstoned;
                    self.by_digest.remove(&digest);
                }
            }
            abi::emit("open_proposals_wiped", ());
        }
    }
}
