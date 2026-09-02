// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Leon Frenzel

//! In-memory mock ledger for website-demo `DEMO_MODE=mock`.
//! No chain / contract calls — accounts, proposals, and digests only.

use std::collections::BTreeMap;

use dusk_bytes::Serializable;
use dusk_core::signatures::bls::PublicKey as BlsPublicKey;
use knot_encoding::call_types::{MultisigAccountView, RegistryPendingChange, RegistryPendingView};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DemoMode {
    Mock,
    Testnet,
}

impl DemoMode {
    /// Require explicit `DEMO_MODE=mock` or `DEMO_MODE=testnet` (R6).
    pub fn from_env() -> Result<Self, String> {
        let raw = std::env::var("DEMO_MODE").map_err(|_| {
            "DEMO_MODE is required — set DEMO_MODE=mock (in-process ledger) or \
             DEMO_MODE=testnet (live chain) before serve"
                .to_string()
        })?;
        match raw.to_ascii_lowercase().as_str() {
            "mock" => Ok(DemoMode::Mock),
            "testnet" => Ok(DemoMode::Testnet),
            other => Err(format!(
                "DEMO_MODE={other:?} is invalid — set DEMO_MODE=mock or DEMO_MODE=testnet"
            )),
        }
    }

    /// Refuse CLI `change-account --nonce` unless the dev latch is set (R8).
    pub fn change_account_nonce_override_allowed() -> bool {
        std::env::var("KNOT_ALLOW_CHANGE_ACCOUNT_NONCE").as_deref() == Ok("1")
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            DemoMode::Mock => "mock",
            DemoMode::Testnet => "testnet",
        }
    }

    /// Loud serve banner suffix — reflects actual mode risk (R6).
    pub fn serve_banner_label(&self) -> &'static str {
        match self {
            DemoMode::Mock => "in-process mock ledger (no chain writes)",
            DemoMode::Testnet => "TESTNET ONLY (live chain writes)",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MockProposalStatus {
    Open,
    Finalized,
    Queued,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MockAccount {
    pub threshold: u32,
    pub nonce: u64,
    pub members: Vec<[u8; 96]>,
    pub timelock_blocks: u64,
    pub pending_execute_at: u64,
    pub pending_timelock: Option<u64>,
}

impl MockAccount {
    pub fn to_account_view(&self) -> MultisigAccountView {
        let members = self
            .members
            .iter()
            .map(|bytes| {
                BlsPublicKey::from_bytes(bytes).expect("mock ledger stores valid BLS public keys")
            })
            .collect();
        let pending = if self.pending_execute_at == 0 {
            None
        } else {
            Some(RegistryPendingView {
                change: RegistryPendingChange::SetTimelock(
                    self.pending_timelock.unwrap_or(self.timelock_blocks),
                ),
                execute_at: self.pending_execute_at,
            })
        };
        MultisigAccountView {
            members,
            threshold: self.threshold,
            nonce: self.nonce,
            timelock_blocks: self.timelock_blocks,
            pending,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MockAccountMeta {
    pub nonce: u64,
    pub threshold: u32,
    pub member_count: u32,
    pub timelock_blocks: u64,
    pub pending_execute_at: u64,
}

/// Stable mock proposals contract id for v3 digests (Lab-only).
pub const MOCK_PROPOSALS_SELF_ID: [u8; 32] = [0xB3; 32];
/// Stable mock registry contract id for v3 `change_account` digests (Lab-only).
pub const MOCK_REGISTRY_SELF_ID: [u8; 32] = [0xA1; 32];
/// Chain id baked into mock digests (matches live testnet `init_chain_id`).
pub const MOCK_CHAIN_ID: u64 = 2;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MockProposal {
    pub id: u64,
    pub status: MockProposalStatus,
    pub registry_account_id: u64,
    pub epoch: u64,
    pub nonce: u64,
    pub target: [u8; 32],
    pub function_name: String,
    pub call_args: Vec<u8>,
    pub deadline: u64,
    pub digest: [u8; 32],
    pub approvals: Vec<[u8; 96]>,
    pub execute_at: u64,
}

pub struct MockLedger {
    accounts: BTreeMap<u64, MockAccount>,
    proposals: BTreeMap<u64, MockProposal>,
    next_account_id: u64,
    next_proposal_id: u64,
    epoch: u64,
}

impl Default for MockLedger {
    fn default() -> Self {
        Self::new()
    }
}

impl MockLedger {
    pub fn new() -> Self {
        Self {
            accounts: BTreeMap::new(),
            proposals: BTreeMap::new(),
            next_account_id: 0,
            next_proposal_id: 0,
            epoch: 0,
        }
    }

    pub fn next_account_id(&self) -> u64 {
        self.next_account_id
    }

    pub fn create_account(
        &mut self,
        members: Vec<[u8; 96]>,
        threshold: u32,
    ) -> Result<u64, String> {
        if members.is_empty() {
            return Err("members must be non-empty".into());
        }
        if threshold == 0 || threshold as usize > members.len() {
            return Err("threshold must be between 1 and committee size".into());
        }
        // Reject duplicate member keys (production validate_committee does too).
        for i in 0..members.len() {
            for j in (i + 1)..members.len() {
                if members[i] == members[j] {
                    return Err("duplicate member public key".into());
                }
            }
        }

        let id = self.next_account_id;
        self.next_account_id += 1;
        self.accounts.insert(
            id,
            MockAccount {
                threshold,
                nonce: 0,
                members,
                timelock_blocks: 0,
                pending_execute_at: 0,
                pending_timelock: None,
            },
        );
        Ok(id)
    }

    pub fn account(&self, id: u64) -> Option<MockAccount> {
        self.accounts.get(&id).cloned()
    }

    pub fn account_meta(&self, id: u64) -> Option<MockAccountMeta> {
        self.accounts.get(&id).map(|a| MockAccountMeta {
            nonce: a.nonce,
            threshold: a.threshold,
            member_count: a.members.len() as u32,
            timelock_blocks: a.timelock_blocks,
            pending_execute_at: a.pending_execute_at,
        })
    }

    pub fn next_proposal_id(&self) -> u64 {
        self.next_proposal_id
    }

    /// `target`: 32-byte contract id; `function_name` + `call_args` as on proposals API.
    /// `nonce` is the caller uniquifier (v3), not the registry account nonce.
    pub fn create_proposal(
        &mut self,
        registry_account_id: u64,
        target: [u8; 32],
        function_name: String,
        call_args: Vec<u8>,
        deadline: u64,
        nonce: u64,
    ) -> Result<u64, String> {
        let _account = self
            .accounts
            .get(&registry_account_id)
            .ok_or_else(|| format!("unknown registry account {registry_account_id}"))?;

        let digest = knot_encoding::ProposalIntentV3 {
            chain_id: MOCK_CHAIN_ID,
            self_id: MOCK_PROPOSALS_SELF_ID,
            epoch: self.epoch,
            committee_id: registry_account_id,
            nonce,
            target_contract_id: target,
            function_name: function_name.clone(),
            call_args: call_args.clone(),
            deadline,
        }
        .digest()
        .expect("mock intent in bounds");

        let id = self.next_proposal_id;
        self.next_proposal_id += 1;
        self.proposals.insert(
            id,
            MockProposal {
                id,
                status: MockProposalStatus::Open,
                registry_account_id,
                epoch: self.epoch,
                nonce,
                target,
                function_name,
                call_args,
                deadline,
                digest,
                approvals: Vec::new(),
                execute_at: 0,
            },
        );
        Ok(id)
    }

    pub fn proposal(&self, id: u64) -> Option<MockProposal> {
        self.proposals.get(&id).cloned()
    }

    /// Record approval by member pk bytes; enforce Open + member + not duplicate.
    pub fn approve(&mut self, id: u64, member_pk: [u8; 96]) -> Result<(), String> {
        let proposal = self
            .proposals
            .get(&id)
            .ok_or_else(|| format!("no such proposal {id}"))?;
        if proposal.status != MockProposalStatus::Open {
            return Err("proposal is not open".into());
        }
        let account = self
            .accounts
            .get(&proposal.registry_account_id)
            .ok_or_else(|| "unknown registry account for proposal".to_string())?;
        if !account.members.contains(&member_pk) {
            return Err("signer is not a member of the proposal's registry account".into());
        }
        if proposal.approvals.contains(&member_pk) {
            return Err("signer has already approved this proposal".into());
        }

        let proposal = self.proposals.get_mut(&id).expect("proposal exists");
        proposal.approvals.push(member_pk);
        Ok(())
    }

    /// Mark Finalized when approvals_len >= account.threshold; do not call real contracts.
    pub fn finalize(&mut self, id: u64) -> Result<(), String> {
        let proposal = self
            .proposals
            .get(&id)
            .ok_or_else(|| format!("no such proposal {id}"))?
            .clone();
        if proposal.status != MockProposalStatus::Open {
            return Err("proposal is not open".into());
        }

        let account = self
            .accounts
            .get(&proposal.registry_account_id)
            .ok_or_else(|| "unknown registry account for proposal".to_string())?;
        if (proposal.approvals.len() as u32) < account.threshold {
            return Err(format!(
                "finalize: quorum not met (approvals={}, threshold={})",
                proposal.approvals.len(),
                account.threshold
            ));
        }

        let delay = account.timelock_blocks;
        let proposal = self.proposals.get_mut(&id).expect("proposal exists");
        if delay == 0 {
            proposal.status = MockProposalStatus::Finalized;
        } else {
            proposal.status = MockProposalStatus::Queued;
            proposal.execute_at = delay;
        }
        Ok(())
    }

    pub fn execute(&mut self, id: u64) -> Result<(), String> {
        let proposal = self
            .proposals
            .get_mut(&id)
            .ok_or_else(|| format!("no such proposal {id}"))?;
        if proposal.status != MockProposalStatus::Queued {
            return Err("proposal is not queued".into());
        }
        proposal.status = MockProposalStatus::Finalized;
        Ok(())
    }

    pub fn cancel_proposal(&mut self, id: u64) -> Result<(), String> {
        let proposal = self
            .proposals
            .get_mut(&id)
            .ok_or_else(|| format!("no such proposal {id}"))?;
        if proposal.status != MockProposalStatus::Queued {
            return Err("proposal is not queued".into());
        }
        proposal.status = MockProposalStatus::Cancelled;
        Ok(())
    }

    pub fn set_timelock(&mut self, id: u64, blocks: u64) -> Result<(), String> {
        let account = self
            .accounts
            .get_mut(&id)
            .ok_or_else(|| format!("unknown registry account {id}"))?;
        if account.timelock_blocks == 0 {
            account.timelock_blocks = blocks;
            account.nonce += 1;
            return Ok(());
        }
        account.pending_timelock = Some(blocks);
        account.pending_execute_at = account.timelock_blocks;
        account.nonce += 1;
        Ok(())
    }

    pub fn execute_pending(&mut self, id: u64) -> Result<(), String> {
        let account = self
            .accounts
            .get_mut(&id)
            .ok_or_else(|| format!("unknown registry account {id}"))?;
        if account.pending_execute_at == 0 {
            return Err("no pending change".into());
        }
        if let Some(blocks) = account.pending_timelock.take() {
            account.timelock_blocks = blocks;
        }
        account.pending_execute_at = 0;
        Ok(())
    }

    pub fn cancel_pending(&mut self, id: u64) -> Result<(), String> {
        let account = self
            .accounts
            .get_mut(&id)
            .ok_or_else(|| format!("unknown registry account {id}"))?;
        if account.pending_execute_at == 0 {
            return Err("no pending change".into());
        }
        account.pending_timelock = None;
        account.pending_execute_at = 0;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member(byte: u8) -> [u8; 96] {
        [byte; 96]
    }

    fn members_2of3() -> Vec<[u8; 96]> {
        vec![member(1), member(2), member(3)]
    }

    #[test]
    fn create_2_of_3_account() {
        let mut ledger = MockLedger::new();
        assert_eq!(ledger.next_account_id(), 0);
        let id = ledger
            .create_account(members_2of3(), 2)
            .expect("create 2-of-3");
        assert_eq!(id, 0);
        assert_eq!(ledger.next_account_id(), 1);

        let acct = ledger.account(id).expect("account exists");
        assert_eq!(acct.threshold, 2);
        assert_eq!(acct.nonce, 0);
        assert_eq!(acct.members.len(), 3);

        let meta = ledger.account_meta(id).expect("meta exists");
        assert_eq!(meta.threshold, 2);
        assert_eq!(meta.nonce, 0);
        assert_eq!(meta.member_count, 3);
    }

    #[test]
    fn create_proposal_computes_production_digest() {
        let mut ledger = MockLedger::new();
        let account_id = ledger
            .create_account(members_2of3(), 2)
            .expect("create account");

        let target = [0xab; 32];
        let function_name = "set_value".to_string();
        let call_args = vec![1, 2, 3, 4];
        let deadline = 999u64;
        let nonce = 0u64;

        let proposal_id = ledger
            .create_proposal(
                account_id,
                target,
                function_name.clone(),
                call_args.clone(),
                deadline,
                nonce,
            )
            .expect("create proposal");
        assert_eq!(proposal_id, 0);
        assert_eq!(ledger.next_proposal_id(), 1);

        let p = ledger.proposal(proposal_id).expect("proposal exists");
        assert_eq!(p.status, MockProposalStatus::Open);
        assert_eq!(p.registry_account_id, account_id);
        assert_eq!(p.epoch, 0);
        assert_eq!(p.nonce, nonce);
        assert_eq!(p.target, target);
        assert_eq!(p.function_name, function_name);
        assert_eq!(p.call_args, call_args);
        assert_eq!(p.deadline, deadline);
        assert!(p.approvals.is_empty());

        let expected = knot_encoding::ProposalIntentV3 {
            chain_id: MOCK_CHAIN_ID,
            self_id: MOCK_PROPOSALS_SELF_ID,
            epoch: 0,
            committee_id: account_id,
            nonce,
            target_contract_id: target,
            function_name,
            call_args,
            deadline,
        }
        .digest()
        .expect("mock intent in bounds");
        assert_eq!(p.digest, expected);
    }

    #[test]
    fn approve_rejects_non_member() {
        let mut ledger = MockLedger::new();
        let account_id = ledger
            .create_account(members_2of3(), 2)
            .expect("create account");
        let proposal_id = ledger
            .create_proposal(account_id, [0; 32], "noop".into(), vec![], 0, 0)
            .expect("create proposal");

        let err = ledger
            .approve(proposal_id, member(9))
            .expect_err("non-member must fail");
        assert!(
            err.to_ascii_lowercase().contains("member"),
            "error should mention member: {err}"
        );
    }

    #[test]
    fn two_approvals_then_finalize_succeeds() {
        let mut ledger = MockLedger::new();
        let account_id = ledger
            .create_account(members_2of3(), 2)
            .expect("create account");
        let proposal_id = ledger
            .create_proposal(account_id, [7; 32], "set_value".into(), vec![42], 100, 0)
            .expect("create proposal");

        ledger
            .approve(proposal_id, member(1))
            .expect("first approval");
        ledger
            .approve(proposal_id, member(2))
            .expect("second approval");
        ledger.finalize(proposal_id).expect("finalize at threshold");

        let p = ledger.proposal(proposal_id).expect("proposal");
        assert_eq!(p.status, MockProposalStatus::Finalized);
        assert_eq!(p.approvals.len(), 2);

        let acct = ledger.account(account_id).expect("account");
        assert_eq!(acct.nonce, 0, "v3 finalize does not bump committee nonce");
    }

    #[test]
    fn finalize_before_threshold_fails() {
        let mut ledger = MockLedger::new();
        let account_id = ledger
            .create_account(members_2of3(), 2)
            .expect("create account");
        let proposal_id = ledger
            .create_proposal(account_id, [0; 32], "noop".into(), vec![], 0, 0)
            .expect("create proposal");

        ledger
            .approve(proposal_id, member(1))
            .expect("one approval");
        let err = ledger
            .finalize(proposal_id)
            .expect_err("finalize below threshold must fail");
        assert!(
            err.to_ascii_lowercase().contains("threshold")
                || err.to_ascii_lowercase().contains("quorum")
                || err.to_ascii_lowercase().contains("approval"),
            "error should mention threshold/quorum: {err}"
        );

        let p = ledger.proposal(proposal_id).expect("proposal");
        assert_eq!(p.status, MockProposalStatus::Open);
    }

    #[test]
    fn demo_mode_from_env_requires_explicit_value() {
        let key = "DEMO_MODE";
        let prev = std::env::var(key).ok();

        unsafe {
            std::env::remove_var(key);
        }
        let err = DemoMode::from_env().expect_err("unset DEMO_MODE must refuse");
        assert!(
            err.contains("DEMO_MODE is required"),
            "unexpected error: {err}"
        );

        unsafe {
            std::env::set_var(key, "bogus");
        }
        let err = DemoMode::from_env().expect_err("unknown DEMO_MODE must refuse");
        assert!(err.contains("invalid"), "unexpected error: {err}");

        unsafe {
            std::env::set_var(key, "mock");
        }
        assert_eq!(DemoMode::from_env().expect("mock"), DemoMode::Mock);

        unsafe {
            std::env::set_var(key, "TESTNET");
        }
        assert_eq!(DemoMode::from_env().expect("testnet"), DemoMode::Testnet);

        unsafe {
            match prev {
                Some(v) => std::env::set_var(key, v),
                None => std::env::remove_var(key),
            }
        }
    }

    #[test]
    fn change_account_nonce_latch_refuses_by_default() {
        let key = "KNOT_ALLOW_CHANGE_ACCOUNT_NONCE";
        let prev = std::env::var(key).ok();
        unsafe {
            std::env::remove_var(key);
        }
        assert!(
            !DemoMode::change_account_nonce_override_allowed(),
            "nonce override must be refused without latch"
        );
        unsafe {
            std::env::set_var(key, "1");
        }
        assert!(DemoMode::change_account_nonce_override_allowed());
        unsafe {
            match prev {
                Some(v) => std::env::set_var(key, v),
                None => std::env::remove_var(key),
            }
        }
    }

    #[test]
    fn demo_mode_as_str_for_setup_status() {
        assert_eq!(DemoMode::Mock.as_str(), "mock");
        assert_eq!(DemoMode::Testnet.as_str(), "testnet");
        assert!(
            DemoMode::Mock.serve_banner_label().contains("mock ledger"),
            "mock banner should not say TESTNET ONLY"
        );
        assert!(
            DemoMode::Testnet
                .serve_banner_label()
                .contains("TESTNET ONLY"),
            "testnet banner must warn about live writes"
        );

        #[derive(serde::Serialize)]
        struct SetupStatusSlice<'a> {
            demo_mode: &'a str,
        }
        let mock_json = serde_json::to_value(SetupStatusSlice {
            demo_mode: DemoMode::Mock.as_str(),
        })
        .expect("serialize");
        assert_eq!(mock_json["demo_mode"], "mock");
        let testnet_json = serde_json::to_value(SetupStatusSlice {
            demo_mode: DemoMode::Testnet.as_str(),
        })
        .expect("serialize");
        assert_eq!(testnet_json["demo_mode"], "testnet");
    }

    /// Exercises the mock RPC approve/finalize path: real BLS sign of digest,
    /// membership via `MockLedger::approve`, finalize → synthetic tx hash shape.
    #[test]
    fn signed_approve_finalize_path_for_rpc() {
        use dusk_bytes::Serializable;
        use dusk_core::signatures::bls::{PublicKey, SecretKey};
        use rand::rngs::OsRng;

        let sk1 = SecretKey::random(&mut OsRng);
        let sk2 = SecretKey::random(&mut OsRng);
        let sk3 = SecretKey::random(&mut OsRng);
        let pk1 = PublicKey::from(&sk1);
        let pk2 = PublicKey::from(&sk2);
        let pk3 = PublicKey::from(&sk3);
        let members = vec![pk1.to_bytes(), pk2.to_bytes(), pk3.to_bytes()];

        let mut ledger = MockLedger::new();
        let account_id = ledger.create_account(members, 2).expect("create 2-of-3");
        let proposal_id = ledger
            .create_proposal(
                account_id,
                [0x11; 32],
                "set_value".into(),
                vec![7, 8],
                1_000,
                0,
            )
            .expect("create proposal");
        let digest = ledger.proposal(proposal_id).expect("proposal").digest;

        // Same secure sign path as `bls::sign` / rpc mock approve.
        let _sig1 = sk1.sign(&digest);
        ledger
            .approve(proposal_id, pk1.to_bytes())
            .expect("approve member 1");
        let _sig2 = sk2.sign(&digest);
        ledger
            .approve(proposal_id, pk2.to_bytes())
            .expect("approve member 2");

        ledger.finalize(proposal_id).expect("finalize at threshold");
        let p = ledger.proposal(proposal_id).expect("proposal");
        assert_eq!(p.status, MockProposalStatus::Finalized);
        assert_eq!(p.approvals.len(), 2);

        // RPC returns this synthetic hash shape (no chain submit).
        let tx_hash = format!("mock-finalize-{proposal_id}");
        assert_eq!(tx_hash, "mock-finalize-0");
        assert_eq!(
            ledger.account(account_id).expect("account").nonce,
            0,
            "v3 finalize does not bump committee nonce"
        );
    }

    #[test]
    fn delay_zero_finalize_still_immediate() {
        let mut ledger = MockLedger::new();
        let account_id = ledger
            .create_account(members_2of3(), 2)
            .expect("create account");
        assert_eq!(ledger.account(account_id).unwrap().timelock_blocks, 0);
        let proposal_id = ledger
            .create_proposal(account_id, [0; 32], "noop".into(), vec![], 10, 0)
            .expect("create");
        ledger.approve(proposal_id, member(1)).unwrap();
        ledger.approve(proposal_id, member(2)).unwrap();
        ledger.finalize(proposal_id).unwrap();
        assert_eq!(
            ledger.proposal(proposal_id).unwrap().status,
            MockProposalStatus::Finalized
        );
    }

    #[test]
    fn delay_queues_until_execute() {
        let mut ledger = MockLedger::new();
        let account_id = ledger
            .create_account(members_2of3(), 2)
            .expect("create account");
        ledger.set_timelock(account_id, 5).unwrap();
        assert_eq!(ledger.account(account_id).unwrap().timelock_blocks, 5);
        let proposal_id = ledger
            .create_proposal(account_id, [0; 32], "noop".into(), vec![], 10, 0)
            .expect("create");
        ledger.approve(proposal_id, member(1)).unwrap();
        ledger.approve(proposal_id, member(2)).unwrap();
        ledger.finalize(proposal_id).unwrap();
        let p = ledger.proposal(proposal_id).unwrap();
        assert_eq!(p.status, MockProposalStatus::Queued);
        assert_eq!(p.execute_at, 5);
        ledger.execute(proposal_id).unwrap();
        assert_eq!(
            ledger.proposal(proposal_id).unwrap().status,
            MockProposalStatus::Finalized
        );
    }

    #[test]
    fn set_timelock_shorten_is_pending_until_execute() {
        let mut ledger = MockLedger::new();
        let account_id = ledger
            .create_account(members_2of3(), 2)
            .expect("create account");
        ledger.set_timelock(account_id, 5).unwrap();
        ledger.set_timelock(account_id, 1).unwrap();
        let acct = ledger.account(account_id).unwrap();
        assert_eq!(acct.timelock_blocks, 5);
        assert_eq!(acct.pending_execute_at, 5);
        assert_eq!(acct.pending_timelock, Some(1));
        ledger.execute_pending(account_id).unwrap();
        let acct = ledger.account(account_id).unwrap();
        assert_eq!(acct.timelock_blocks, 1);
        assert_eq!(acct.pending_execute_at, 0);
    }
}
