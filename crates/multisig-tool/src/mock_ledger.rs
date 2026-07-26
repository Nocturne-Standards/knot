// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Leon Frenzel

//! In-memory mock ledger for website-demo `DEMO_MODE=mock`.
//! No chain / contract calls — accounts, proposals, and digests only.

use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DemoMode {
    Mock,
    Testnet,
}

impl DemoMode {
    pub fn from_env() -> Self {
        match std::env::var("DEMO_MODE")
            .unwrap_or_else(|_| "mock".into())
            .to_ascii_lowercase()
            .as_str()
        {
            "testnet" => DemoMode::Testnet,
            _ => DemoMode::Mock,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            DemoMode::Mock => "mock",
            DemoMode::Testnet => "testnet",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MockProposalStatus {
    Open,
    Finalized,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MockAccount {
    pub threshold: u32,
    pub nonce: u64,
    pub members: Vec<[u8; 96]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MockAccountMeta {
    pub nonce: u64,
    pub threshold: u32,
    pub member_count: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MockProposal {
    pub id: u64,
    pub status: MockProposalStatus,
    pub registry_account_id: u64,
    pub chain_id: u64,
    pub nonce: u64,
    pub target: [u8; 32],
    pub function_name: String,
    pub call_args: Vec<u8>,
    pub deadline: u64,
    pub digest: [u8; 32],
    pub approvals: Vec<[u8; 96]>,
}

pub struct MockLedger {
    accounts: BTreeMap<u64, MockAccount>,
    proposals: BTreeMap<u64, MockProposal>,
    next_account_id: u64,
    next_proposal_id: u64,
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
        })
    }

    pub fn next_proposal_id(&self) -> u64 {
        self.next_proposal_id
    }

    /// `target`: 32-byte contract id; `function_name` + `call_args` as on proposals API.
    pub fn create_proposal(
        &mut self,
        registry_account_id: u64,
        target: [u8; 32],
        function_name: String,
        call_args: Vec<u8>,
        deadline: u64,
        chain_id: u64,
    ) -> Result<u64, String> {
        let account = self
            .accounts
            .get(&registry_account_id)
            .ok_or_else(|| format!("unknown registry account {registry_account_id}"))?;
        let nonce = account.nonce;

        let digest = multisig_encoding::ProposalIntent {
            chain_id,
            committee_id: registry_account_id,
            nonce,
            target_contract_id: target,
            function_name: function_name.clone(),
            call_args: call_args.clone(),
            deadline,
        }
        .digest();

        let id = self.next_proposal_id;
        self.next_proposal_id += 1;
        self.proposals.insert(
            id,
            MockProposal {
                id,
                status: MockProposalStatus::Open,
                registry_account_id,
                chain_id,
                nonce,
                target,
                function_name,
                call_args,
                deadline,
                digest,
                approvals: Vec::new(),
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
        if !account.members.iter().any(|m| *m == member_pk) {
            return Err("signer is not a member of the proposal's registry account".into());
        }
        if proposal.approvals.iter().any(|a| *a == member_pk) {
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
            .get_mut(&proposal.registry_account_id)
            .ok_or_else(|| "unknown registry account for proposal".to_string())?;
        if proposal.nonce != account.nonce {
            return Err("proposal nonce is stale".into());
        }
        if (proposal.approvals.len() as u32) < account.threshold {
            return Err(format!(
                "finalize: quorum not met (approvals={}, threshold={})",
                proposal.approvals.len(),
                account.threshold
            ));
        }

        account.nonce = account
            .nonce
            .checked_add(1)
            .ok_or_else(|| "committee nonce overflow".to_string())?;

        let proposal = self.proposals.get_mut(&id).expect("proposal exists");
        proposal.status = MockProposalStatus::Finalized;
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
        let chain_id = 2u64;

        let proposal_id = ledger
            .create_proposal(
                account_id,
                target,
                function_name.clone(),
                call_args.clone(),
                deadline,
                chain_id,
            )
            .expect("create proposal");
        assert_eq!(proposal_id, 0);
        assert_eq!(ledger.next_proposal_id(), 1);

        let p = ledger.proposal(proposal_id).expect("proposal exists");
        assert_eq!(p.status, MockProposalStatus::Open);
        assert_eq!(p.registry_account_id, account_id);
        assert_eq!(p.chain_id, chain_id);
        assert_eq!(p.nonce, 0);
        assert_eq!(p.target, target);
        assert_eq!(p.function_name, function_name);
        assert_eq!(p.call_args, call_args);
        assert_eq!(p.deadline, deadline);
        assert!(p.approvals.is_empty());

        let expected = multisig_encoding::ProposalIntent {
            chain_id,
            committee_id: account_id,
            nonce: 0,
            target_contract_id: target,
            function_name,
            call_args,
            deadline,
        }
        .digest();
        assert_eq!(p.digest, expected);
    }

    #[test]
    fn approve_rejects_non_member() {
        let mut ledger = MockLedger::new();
        let account_id = ledger
            .create_account(members_2of3(), 2)
            .expect("create account");
        let proposal_id = ledger
            .create_proposal(
                account_id,
                [0; 32],
                "noop".into(),
                vec![],
                0,
                2,
            )
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
            .create_proposal(
                account_id,
                [7; 32],
                "set_value".into(),
                vec![42],
                100,
                2,
            )
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
        assert_eq!(acct.nonce, 1, "committee nonce bumps on finalize");
    }

    #[test]
    fn finalize_before_threshold_fails() {
        let mut ledger = MockLedger::new();
        let account_id = ledger
            .create_account(members_2of3(), 2)
            .expect("create account");
        let proposal_id = ledger
            .create_proposal(
                account_id,
                [0; 32],
                "noop".into(),
                vec![],
                0,
                2,
            )
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
}
