#[dusk_forge::contract]
mod knot_proposals {
    use alloc::collections::BTreeMap;
    use alloc::string::String;
    use alloc::vec::Vec;

    use dusk_bytes::Serializable;
    use dusk_core::abi::{self, ContractId, block_height, chain_id};
    use dusk_core::signatures::bls::{PublicKey as BlsPublicKey, Signature as BlsSignature};

    use knot_encoding::proposal_digest_v3;
    use knot_proposals::call_types::{
        ApproveArgs, MultisigAccountView, ProposalStatus, ProposalView, ProposeArgs,
        SignatureEntry, VerifyQuorumArgs,
    };

    const MAX_FUNCTION_NAME_LEN: usize = 64;
    const MAX_CALL_ARGS_LEN: usize = 4096;
    const MAX_PROPOSAL_TTL: u64 = 100_000;
    const MAX_PRUNE_BATCH: u32 = 128;

    struct DigestRecord {
        proposal_id: u64,
        deadline: u64,
        epoch: u64,
        consumed: bool,
    }

    struct Proposal {
        registry_account_id: u64,
        nonce: u64,
        epoch: u64,
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
        epoch: u64,
        tombstone: bool,
        proposal_ttl: u64,
        by_digest: BTreeMap<[u8; 32], DigestRecord>,
        proposals: BTreeMap<u64, Proposal>,
        next_id: u64,
    }

    impl MultisigProposalsState {
        pub const fn new() -> Self {
            Self {
                registry: None,
                epoch: 0,
                tombstone: false,
                proposal_ttl: 1000,
                by_digest: BTreeMap::new(),
                proposals: BTreeMap::new(),
                next_id: 0,
            }
        }

        fn require_owner() {
            let sender = abi::public_sender();
            let owner = abi::self_owner();
            if sender != Some(owner) {
                panic!("Only the contract owner may configure knot-proposals");
            }
        }

        fn require_registry(&self) -> ContractId {
            self.registry
                .expect("knot-proposals not initialized: call init_registry first")
        }

        /// Owner-only. Points this contract at a deployed `knot-registry` and
        /// bumps `epoch` so prior proposals are unreachable (O(1)).
        pub fn init_registry(&mut self, registry: ContractId) {
            Self::require_owner();
            self.epoch = self.epoch.checked_add(1).expect("epoch overflow");
            self.registry = Some(registry);
            abi::emit("registry_set", ());
        }

        /// Owner-only ceiling on proposal deadlines.
        pub fn set_proposal_ttl(&mut self, blocks: u64) {
            Self::require_owner();
            if blocks == 0 || blocks > MAX_PROPOSAL_TTL {
                panic!("proposal_ttl out of range");
            }
            self.proposal_ttl = blocks;
        }

        /// Owner-only. When `true`, successful finalize marks `Tombstoned`
        /// instead of `Executed`. Does **not** invalidate open proposals.
        pub fn set_tombstone(&mut self, tombstone: bool) {
            Self::require_owner();
            self.tombstone = tombstone;
        }

        pub fn epoch(&self) -> u64 {
            self.epoch
        }

        pub fn proposal_ttl(&self) -> u64 {
            self.proposal_ttl
        }

        /// Open a structured proposal. Digest = §2.12 v3 Keccak; `nonce` is a
        /// caller-supplied uniquifier (not a monotonic counter).
        pub fn propose(&mut self, args: ProposeArgs) -> u64 {
            let _registry = self.require_registry();
            if args.function_name.len() > MAX_FUNCTION_NAME_LEN {
                panic!("function_name too long");
            }
            if args.call_args.len() > MAX_CALL_ARGS_LEN {
                panic!("call_args too long");
            }
            if self.proposal_ttl == 0 {
                panic!("proposal_ttl not configured");
            }
            if args.deadline == 0 {
                panic!("proposal deadline must be non-zero");
            }

            let now = block_height();
            let max_deadline = now.checked_add(self.proposal_ttl).expect("ttl overflow");
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
                self.epoch,
                args.registry_account_id,
                args.nonce,
                &args.target.to_bytes(),
                args.function_name.as_bytes(),
                &args.call_args,
                deadline,
            )
            .expect("propose caps keep function_name/call_args within u32");

            if let Some(rec) = self.by_digest.get(&digest) {
                if rec.consumed {
                    panic!("proposal digest already executed");
                }
                if rec.epoch != self.epoch {
                    panic!("proposal digest belongs to a retired epoch");
                }
                match self.proposals.get(&rec.proposal_id).map(|p| p.status) {
                    Some(ProposalStatus::Open) => return rec.proposal_id,
                    _ => panic!("proposal digest already used"),
                }
            }

            let id = self.next_id;
            self.next_id = self.next_id.checked_add(1).expect("next_id overflow");
            self.proposals.insert(
                id,
                Proposal {
                    registry_account_id: args.registry_account_id,
                    nonce: args.nonce,
                    epoch: self.epoch,
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
            self.by_digest.insert(
                digest,
                DigestRecord {
                    proposal_id: id,
                    deadline,
                    epoch: self.epoch,
                    consumed: false,
                },
            );
            abi::emit(
                "proposal_created",
                (id, digest, args.registry_account_id, deadline),
            );
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
            if proposal.epoch != self.epoch {
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

            let digest = proposal.signed_digest;
            proposal.approvals.push(args.signer);
            proposal.approval_sigs.push(args.signature);
            abi::emit(
                "proposal_approved",
                (args.proposal_id, digest, args.signer.to_bytes()),
            );
        }

        pub fn proposal(&self, id: u64) -> Option<ProposalView> {
            self.proposals.get(&id).map(|p| ProposalView {
                registry_account_id: p.registry_account_id,
                epoch: p.epoch,
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

        /// At threshold: verify quorum, then CEI — mark terminal status, mark
        /// digest consumed, emit — **then** `call_raw` the target.
        pub fn finalize(&mut self, proposal_id: u64) {
            let registry = self.require_registry();

            let proposal = self
                .proposals
                .get(&proposal_id)
                .unwrap_or_else(|| panic!("no such proposal"));
            if proposal.status != ProposalStatus::Open {
                panic!("proposal is not open");
            }
            if proposal.epoch != self.epoch {
                panic!("proposal belongs to a retired epoch");
            }
            if proposal.deadline != 0 && block_height() > proposal.deadline {
                panic!("proposal deadline passed");
            }

            let view: Option<MultisigAccountView> =
                abi::call(registry, "account", &proposal.registry_account_id)
                    .expect("cross-contract call to knot-registry account failed");
            let view = view.unwrap_or_else(|| panic!("unknown knot-registry account"));

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
                .expect("cross-contract call to knot-registry verify_quorum failed");
            if !ok {
                panic!("finalize: registry verify_quorum rejected collected approvals");
            }

            let digest = proposal.signed_digest;
            let target = proposal.target;
            let fn_name = proposal.function_name.clone();
            let call_args = proposal.call_args.clone();
            let committee = proposal.registry_account_id;

            if target == abi::self_id() {
                panic!("finalize: target must not be this contract");
            }

            // Effects first (CEI): consume proposal before external call.
            let proposal = self.proposals.get_mut(&proposal_id).unwrap();
            proposal.status = if self.tombstone {
                ProposalStatus::Tombstoned
            } else {
                ProposalStatus::Executed
            };
            if let Some(rec) = self.by_digest.get_mut(&digest) {
                rec.consumed = true;
            }
            abi::emit(
                "proposal_finalized",
                (proposal_id, digest, committee, target, fn_name.clone()),
            );

            // Interaction last.
            let _ = abi::call_raw(target, &fn_name, &call_args)
                .expect("finalize: call_raw to target failed");
        }

        /// Permissionless storage reclamation. Removes prunable proposal payloads
        /// and expired `by_digest` entries (bounded batch).
        pub fn prune(&mut self, limit: u32) -> u32 {
            let batch = limit.min(MAX_PRUNE_BATCH);
            let now = block_height();
            let mut pruned = 0u32;

            let mut remove_ids: Vec<u64> = Vec::new();
            for (&id, proposal) in self.proposals.iter() {
                if pruned >= batch {
                    break;
                }
                let terminal = proposal.status != ProposalStatus::Open;
                let retired = proposal.epoch != self.epoch;
                let expired = proposal.deadline < now;
                if terminal || retired || expired {
                    remove_ids.push(id);
                    pruned += 1;
                }
            }
            for id in remove_ids {
                self.proposals.remove(&id);
            }

            let mut remove_digests: Vec<[u8; 32]> = Vec::new();
            for (&digest, rec) in self.by_digest.iter() {
                if rec.deadline < now {
                    remove_digests.push(digest);
                }
            }
            for digest in remove_digests {
                self.by_digest.remove(&digest);
            }

            if pruned > 0 {
                abi::emit("pruned", pruned);
            }
            pruned
        }
    }
}
