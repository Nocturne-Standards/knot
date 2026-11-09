//! The proposals contract's call-argument types, re-exported from `knot-encoding`
//! (same reason as `registry_types`: one definition, byte-identical wire format).

pub mod call_types {
    #[allow(unused_imports)]
    pub use knot_encoding::call_types::{
        ApproveArgs, CancelProposalArgs, DigestView, MultisigAccountView, OpenProposal,
        ProposalStatus, ProposalView, ProposalsConfig, ProposeArgs, PruneReport, SignatureEntry,
        VerifyQuorumArgs,
    };
}
