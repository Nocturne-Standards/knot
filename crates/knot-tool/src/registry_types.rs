//! The registry's call-argument types. The canonical definitions live in
//! `knot-encoding`; this module re-exports the set the registry contract uses, so the
//! tool builds the exact rkyv bytes the deployed contract expects with no copy to keep
//! in sync.

pub mod call_types {
    #[allow(unused_imports)]
    pub use knot_encoding::call_types::{
        CancelPendingArgs, ChangeAccountArgs, CreateAccountArgs, MultisigAccountView,
        PublishBootstrapRootArgs, RegistryBookEffect, RegistryPendingChange, RegistryPendingView,
        SetTimelockArgs, SignatureEntry, VerifyQuorumAggregateArgs, VerifyQuorumArgs,
    };
}
