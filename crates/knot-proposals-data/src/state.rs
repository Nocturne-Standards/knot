#[dusk_forge::contract]
mod knot_proposals_data {
    use alloc::collections::BTreeMap;
    use alloc::string::String;
    use alloc::vec::Vec;

    use dusk_core::abi::{self, block_height, chain_id, ContractId};
    use dusk_core::signatures::bls::{PublicKey as BlsPublicKey, Signature as BlsSignature};
    use knot_encoding::call_types::{
        DigestView, OpenProposal, ProposalStatus, ProposalView, ProposalsConfig,
    };
    use knot_encoding::proposal_digest_v3;

    include!(concat!(env!("OUT_DIR"), "/atlas_pin.rs"));

    const ATLAS_ID: ContractId = ContractId::from_bytes(ATLAS_ID_BYTES);
    const SERVICE: &str = "knot-proposals";
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
        execute_at: u64,
        auth_generation: u64,
    }

    pub struct ProposalsDataState {
        registry: Option<ContractId>,
        epoch: u64,
        tombstone: bool,
        proposal_ttl: u64,
        authorized_account: Option<u64>,
        auth_generation: u64,
        by_digest: BTreeMap<[u8; 32], DigestRecord>,
        proposals: BTreeMap<u64, Proposal>,
        next_id: u64,
        proposal_cursor: u64,
        digest_cursor: [u8; 32],
    }

    impl ProposalsDataState {
        pub const fn new() -> Self {
            Self {
                registry: None,
                epoch: 0,
                tombstone: false,
                proposal_ttl: 1000,
                authorized_account: None,
                auth_generation: 0,
                by_digest: BTreeMap::new(),
                proposals: BTreeMap::new(),
                next_id: 0,
                proposal_cursor: 0,
                digest_cursor: [0u8; 32],
            }
        }

        pub fn config(&self) -> ProposalsConfig {
            ProposalsConfig {
                registry: self.registry,
                epoch: self.epoch,
                tombstone: self.tombstone,
                proposal_ttl: self.proposal_ttl,
                authorized_account: self.authorized_account,
                auth_generation: self.auth_generation,
            }
        }

        pub fn set_registry(&mut self, registry: ContractId) -> u64 {
            self.require_logic_caller();
            self.epoch = self.epoch.checked_add(1).expect("epoch overflow");
            self.registry = Some(registry);
            self.authorized_account = None;
            self.epoch
        }

        pub fn set_proposal_ttl(&mut self, blocks: u64) {
            self.require_logic_caller();
            if blocks == 0 || blocks > MAX_PROPOSAL_TTL {
                panic!("proposal_ttl out of range");
            }
            self.proposal_ttl = blocks;
        }

        pub fn set_tombstone(&mut self, tombstone: bool) {
            self.require_logic_caller();
            self.tombstone = tombstone;
        }

        pub fn set_authorized_account(&mut self, account_id: u64) {
            self.require_logic_caller();
            self.authorized_account = Some(account_id);
            self.auth_generation = self
                .auth_generation
                .checked_add(1)
                .expect("auth_generation overflow");
        }

        pub fn digest(&self, key: [u8; 32]) -> Option<DigestView> {
            self.by_digest.get(&key).map(|rec| DigestView {
                proposal_id: rec.proposal_id,
                deadline: rec.deadline,
                epoch: rec.epoch,
                consumed: rec.consumed,
            })
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
                execute_at: p.execute_at,
                auth_generation: p.auth_generation,
            })
        }

        pub fn next_proposal_id(&self) -> u64 {
            self.next_id
        }

        pub fn open_proposal(&mut self, args: OpenProposal) -> u64 {
            self.require_logic_caller();
            self.require_authorized(args.registry_account_id);
            if args.epoch != self.epoch {
                panic!("proposal digest belongs to a retired epoch");
            }
            if self.by_digest.contains_key(&args.signed_digest) {
                panic!("proposal digest already used");
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
                    deadline: args.deadline,
                    signed_digest: args.signed_digest,
                    approvals: Vec::new(),
                    approval_sigs: Vec::new(),
                    status: ProposalStatus::Open,
                    execute_at: 0,
                    auth_generation: self.auth_generation,
                },
            );
            self.by_digest.insert(
                args.signed_digest,
                DigestRecord {
                    proposal_id: id,
                    deadline: args.deadline,
                    epoch: self.epoch,
                    consumed: false,
                },
            );
            id
        }

        pub fn push_approval(
            &mut self,
            proposal_id: u64,
            signer: BlsPublicKey,
            signature: BlsSignature,
        ) {
            self.require_logic_caller();
            let proposal = self
                .proposals
                .get_mut(&proposal_id)
                .unwrap_or_else(|| panic!("no such proposal"));
            if proposal.status != ProposalStatus::Open {
                panic!("proposal is not open");
            }
            if proposal.approvals.contains(&signer) {
                panic!("signer has already approved this proposal");
            }
            proposal.approvals.push(signer);
            proposal.approval_sigs.push(signature);
        }

        pub fn queue(&mut self, proposal_id: u64, execute_at: u64) {
            self.require_logic_caller();
            let authorized = self
                .authorized_account
                .expect("authorized account not set");
            let digest = {
                let proposal = self
                    .proposals
                    .get(&proposal_id)
                    .unwrap_or_else(|| panic!("no such proposal"));
                if proposal.registry_account_id != authorized {
                    panic!("proposal account is not the authorized registry account");
                }
                require_generation(self.auth_generation, proposal.auth_generation);
                require_issued_by_caller(proposal);
                if proposal.status != ProposalStatus::Open {
                    panic!("proposal is not open");
                }
                if proposal.epoch != self.epoch {
                    panic!("proposal belongs to a retired epoch");
                }
                if proposal.deadline != 0 && execute_at > proposal.deadline {
                    panic!("proposal delay exceeds deadline");
                }
                proposal.signed_digest
            };
            let proposal = self.proposals.get_mut(&proposal_id).unwrap();
            proposal.status = ProposalStatus::Queued;
            proposal.execute_at = execute_at;
            if let Some(rec) = self.by_digest.get_mut(&digest) {
                rec.consumed = true;
            }
        }

        pub fn commit_executed(&mut self, proposal_id: u64) {
            self.require_logic_caller();
            let epoch = self.epoch;
            let tombstone = self.tombstone;
            let authorized = self
                .authorized_account
                .expect("authorized account not set");
            let now = block_height();
            let digest = {
                let proposal = self
                    .proposals
                    .get(&proposal_id)
                    .unwrap_or_else(|| panic!("no such proposal"));
                if proposal.registry_account_id != authorized {
                    panic!("proposal account is not the authorized registry account");
                }
                require_generation(self.auth_generation, proposal.auth_generation);
                require_issued_by_caller(proposal);
                if proposal.epoch != epoch {
                    panic!("proposal belongs to a retired epoch");
                }
                if proposal.deadline != 0 && now > proposal.deadline {
                    panic!("proposal deadline passed");
                }
                match proposal.status {
                    ProposalStatus::Open => {}
                    ProposalStatus::Queued => {
                        if now < proposal.execute_at {
                            panic!("timelock not elapsed");
                        }
                    }
                    _ => panic!("proposal is not open"),
                }
                proposal.signed_digest
            };
            let proposal = self.proposals.get_mut(&proposal_id).unwrap();
            proposal.status = if tombstone {
                ProposalStatus::Tombstoned
            } else {
                ProposalStatus::Executed
            };
            if let Some(rec) = self.by_digest.get_mut(&digest) {
                rec.consumed = true;
            }
        }

        pub fn commit_cancelled(&mut self, proposal_id: u64) {
            self.require_logic_caller();
            let digest = {
                let proposal = self
                    .proposals
                    .get(&proposal_id)
                    .unwrap_or_else(|| panic!("no such proposal"));
                if proposal.status != ProposalStatus::Queued {
                    panic!("proposal is not queued");
                }
                if proposal.epoch != self.epoch {
                    panic!("proposal belongs to a retired epoch");
                }
                proposal.signed_digest
            };
            let proposal = self.proposals.get_mut(&proposal_id).unwrap();
            proposal.status = ProposalStatus::Cancelled;
            if let Some(rec) = self.by_digest.get_mut(&digest) {
                rec.consumed = true;
            }
        }

        /// `limit == 0` examines nothing. Each map examines at most
        /// `min(limit, MAX_PRUNE_BATCH)` records, continuing from a cursor.
        /// Consumed digests stay until `deadline`.
        pub fn prune(&mut self, limit: u32) -> u32 {
            self.require_logic_caller();
            if limit == 0 {
                return 0;
            }
            let batch = limit.min(MAX_PRUNE_BATCH);
            let now = block_height();
            let removed = self.examine_proposals(batch, now);
            self.examine_digests(batch, now);
            removed
        }

        fn examine_proposals(&mut self, budget: u32, now: u64) -> u32 {
            let mut removed = 0u32;
            let mut left = budget;
            let mut origin: Option<u64> = None;
            while left > 0 {
                let cursor = self.proposal_cursor;
                let id = self
                    .proposals
                    .range(cursor..)
                    .next()
                    .map(|(id, _)| *id)
                    .or_else(|| self.proposals.range(..cursor).next().map(|(id, _)| *id));
                let Some(id) = id else {
                    break;
                };
                if origin == Some(id) {
                    break;
                }
                if origin.is_none() {
                    origin = Some(id);
                }
                left -= 1;
                let drop = {
                    let proposal = self.proposals.get(&id).expect("proposal id");
                    let retired = proposal.epoch != self.epoch;
                    let expired = proposal.deadline < now;
                    let keep_queued = proposal.status == ProposalStatus::Queued && !expired;
                    let terminal = matches!(
                        proposal.status,
                        ProposalStatus::Executed
                            | ProposalStatus::Tombstoned
                            | ProposalStatus::Cancelled
                    );
                    (terminal || retired || expired) && !keep_queued
                };
                if drop {
                    self.proposals.remove(&id);
                    removed += 1;
                }
                self.proposal_cursor = id.saturating_add(1);
            }
            removed
        }

        fn examine_digests(&mut self, budget: u32, now: u64) {
            let mut left = budget;
            let mut origin: Option<[u8; 32]> = None;
            while left > 0 {
                let cursor = self.digest_cursor;
                let key = self
                    .by_digest
                    .range(cursor..)
                    .next()
                    .map(|(key, _)| *key)
                    .or_else(|| self.by_digest.range(..cursor).next().map(|(key, _)| *key));
                let Some(key) = key else {
                    break;
                };
                if origin == Some(key) {
                    break;
                }
                if origin.is_none() {
                    origin = Some(key);
                }
                left -= 1;
                let expired = self
                    .by_digest
                    .get(&key)
                    .map(|rec| rec.deadline < now)
                    .unwrap_or(false);
                if expired {
                    self.by_digest.remove(&key);
                }
                self.digest_cursor = self
                    .by_digest
                    .range((core::ops::Bound::Excluded(key), core::ops::Bound::Unbounded))
                    .next()
                    .map(|(next, _)| *next)
                    .unwrap_or([0u8; 32]);
            }
        }

        fn require_authorized(&self, account_id: u64) {
            match self.authorized_account {
                Some(id) if id == account_id => {}
                _ => panic!("proposal account is not the authorized registry account"),
            }
        }

        fn require_logic_caller(&self) {
            let name = String::from(SERVICE);
            let logic: Option<ContractId> = abi::call(ATLAS_ID, "resolve", &name)
                .expect("atlas resolve(knot-proposals) failed");
            let logic = logic.unwrap_or_else(|| panic!("atlas has no knot-proposals service"));
            if abi::caller() != Some(logic) {
                panic!("caller is not the knot-proposals logic contract");
            }
        }
    }

    fn require_generation(current: u64, stamped: u64) {
        if stamped != current {
            panic!("proposal belongs to a retired account binding");
        }
    }

    /// Digest must have been signed for the logic contract that is calling.
    fn require_issued_by_caller(proposal: &Proposal) {
        let logic = abi::caller().expect("direct call");
        let digest = proposal_digest_v3(
            u64::from(chain_id()),
            &logic.to_bytes(),
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
}
