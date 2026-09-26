// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Nocturne Standards

//! Archived-layout golden pins for shared multisig layer-E call types.
//!
//! Owner-crate `pub mod` — consumers (`knot-registry`, `knot-proposals`)
//! import these consts and still call `rkyv::to_bytes` at runtime (accepted
//! trade-off: no second independent recording). Do **not** re-paste hex in
//! consumer crates.
//!
//! **rkyv camp:** this crate pins `rkyv = "=0.7.39"` (same as the multisig
//! contracts). Resolved patch: `(cd crates/knot-encoding && cargo tree -p rkyv)`.
//!
//! **Layer E + `repr(C)` (2026-08-03):** structs carry
//! `#[archive_attr(repr(C))]`. Measured **DIFFERENT** on `MultisigAccountView`
//! (IDENTICAL on `SignatureEntry` / `VerifyQuorumArgs`). Constants below are
//! after-pin bytes where they moved.
//!
//! Fixed inputs: `StdRng::seed_from_u64(0xa11ce_u64)`; message bytes
//! `b"wave5-layout-golden-multisig"` for signatures.
//!
//! R9 corrupt-one-digit on **post-`repr(C)`** constants 2026-08-03:
//! `GOLDEN_MULTISIG_ACCOUNT_VIEW_HEX` final digit flipped; encoding + both
//! consumer `layout_goldens` went red; reverted; green.

/// `SignatureEntry` — signer from seed key 0, `sign_insecure(MSG)`.
/// Provenance: rustc 1.94.0 (4a4ef493e 2026-03-02); rkyv 0.7.39.
pub const GOLDEN_SIGNATURE_ENTRY_HEX: &str = concat!(
    "e3a945bd7dbd51365c255b3a7851432419f20ddb7bc948f5b60d677c5b02ff9e",
    "6255228ee75c9dd8a3bd4a86751e9b14cf501c89e69b4b2a2169c189accff3af",
    "c07b7ff80a0acfc75a4e073ee006624f722dd52ef90ae1828d8bfdcb6c1e260a",
    "ad4c44e90e1b5e5c2067d4363ee978a0db41fdba0f29829a1263e43f33f231a9",
    "dc20fc5acafc235d9c920f2772cbd716ddb84cca39704625b55a01a011e7eeae",
    "177ef0949bce380f2d64afd6038e15ff70e7aaf4d9b92e8bf4188696e1264e09",
    "0000000000000000374b44e24b396af6703685cae52d9efa06485d0954ed8303",
    "f47ff2b955438a2cc14672d519da0194c99f0af3c65a370e0df875cf13bc6853",
    "0d224df5959be5496703761533d81f2a3d7f3343a5b8927cc044a8cfb03f1867e",
    "123aeb71aba5b160000000000000000"
);

/// `VerifyQuorumArgs { account_id: 1, msg: MSG, sigs: [one entry] }`.
/// Provenance: rustc 1.94.0 (4a4ef493e 2026-03-02); rkyv 0.7.39.
pub const GOLDEN_VERIFY_QUORUM_ARGS_HEX: &str = concat!(
    "77617665352d6c61796f75742d676f6c64656e2d6d756c746973696700000000",
    "e3a945bd7dbd51365c255b3a7851432419f20ddb7bc948f5b60d677c5b02ff9e",
    "6255228ee75c9dd8a3bd4a86751e9b14cf501c89e69b4b2a2169c189accff3af",
    "c07b7ff80a0acfc75a4e073ee006624f722dd52ef90ae1828d8bfdcb6c1e260a",
    "ad4c44e90e1b5e5c2067d4363ee978a0db41fdba0f29829a1263e43f33f231a9",
    "dc20fc5acafc235d9c920f2772cbd716ddb84cca39704625b55a01a011e7eeae",
    "177ef0949bce380f2d64afd6038e15ff70e7aaf4d9b92e8bf4188696e1264e09",
    "0000000000000000374b44e24b396af6703685cae52d9efa06485d0954ed8303",
    "f47ff2b955438a2cc14672d519da0194c99f0af3c65a370e0df875cf13bc6853",
    "0d224df5959be5496703761533d81f2a3d7f3343a5b8927cc044a8cfb03f1867e",
    "123aeb71aba5b1600000000000000000100000000000000a8feffff1c000000",
    "c0feffff01000000"
);

/// `MultisigAccountView { members: [pk0, pk1], threshold: 2, nonce: 3, timelock_blocks: 0, pending: None }`.
/// Provenance: rustc 1.94.0 (4a4ef493e 2026-03-02); rkyv 0.7.39.
/// After-pin hex (`repr(C)` pin, measured DIFFERENT from the prior layout,
/// 2026-08-03). Re-recorded 2026-09-02: `timelock_blocks` + `pending` (PINNED-DIFFERENT-REDEPLOYED).
pub const GOLDEN_MULTISIG_ACCOUNT_VIEW_HEX: &str = concat!(
    "e3a945bd7dbd51365c255b3a7851432419f20ddb7bc948f5b60d677c5b02ff9e",
    "6255228ee75c9dd8a3bd4a86751e9b14cf501c89e69b4b2a2169c189accff3af",
    "c07b7ff80a0acfc75a4e073ee006624f722dd52ef90ae1828d8bfdcb6c1e260a",
    "ad4c44e90e1b5e5c2067d4363ee978a0db41fdba0f29829a1263e43f33f231a9",
    "dc20fc5acafc235d9c920f2772cbd716ddb84cca39704625b55a01a011e7eeae",
    "177ef0949bce380f2d64afd6038e15ff70e7aaf4d9b92e8bf4188696e1264e09",
    "0000000000000000e6462a07bf9af4a6126bc4d85bbe536d2fc447763ff180e",
    "261faac4b6e55cc2584d642716e7a9290e2cd8ff171f72718b6d77b23e66051",
    "0394064b51492818905f053502e57fc16013564eaab60a865355e725bcc05fc",
    "fd430913d88f43f140eb423fc1b9f5bfca7275eef2e1535532d8a405101a6d00",
    "f5b8d28252b6c3a8918eef733d241600235b1bd43fec9e36511e234afcdfe9a",
    "1eb9754a83cc21f3992f76a05fb31f425978548db4a800f7acc435f5bb0fcbcb",
    "1ff8dd71321866759507000000000000000070feffff020000000200000000000000",
    "0300000000000000000000000000000000000000000000000000000000000000",
    "00000000000000000000000000000000"
);

/// `DataSet { data: ContractId([0x0d; 32]) }`. Re-recorded 2026-09-26.
pub const GOLDEN_DATA_SET_HEX: &str =
    "0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d";
/// `AccountCreated` with seed key 0, id 7, threshold 1, timelock 0, nonce 0.
pub const GOLDEN_ACCOUNT_CREATED_HEX: &str = "e3a945bd7dbd51365c255b3a7851432419f20ddb7bc948f5b60d677c5b02ff9e6255228ee75c9dd8a3bd4a86751e9b14cf501c89e69b4b2a2169c189accff3afc07b7ff80a0acfc75a4e073ee006624f722dd52ef90ae1828d8bfdcb6c1e260aad4c44e90e1b5e5c2067d4363ee978a0db41fdba0f29829a1263e43f33f231a9dc20fc5acafc235d9c920f2772cbd716ddb84cca39704625b55a01a011e7eeae177ef0949bce380f2d64afd6038e15ff70e7aaf4d9b92e8bf4188696e1264e090000000000000000070000000000000030ffffff01000000010000000000000000000000000000000000000000000000";
/// `PendingScheduled { account 7, execute_at 100, SetTimelock(5) }`.
pub const GOLDEN_PENDING_SCHEDULED_HEX: &str =
    "0700000000000000640000000000000001000000000000000500000000000000";
/// `AccountChanged` with seed key 0, account 7, threshold 1.
pub const GOLDEN_ACCOUNT_CHANGED_HEX: &str = "e3a945bd7dbd51365c255b3a7851432419f20ddb7bc948f5b60d677c5b02ff9e6255228ee75c9dd8a3bd4a86751e9b14cf501c89e69b4b2a2169c189accff3afc07b7ff80a0acfc75a4e073ee006624f722dd52ef90ae1828d8bfdcb6c1e260aad4c44e90e1b5e5c2067d4363ee978a0db41fdba0f29829a1263e43f33f231a9dc20fc5acafc235d9c920f2772cbd716ddb84cca39704625b55a01a011e7eeae177ef0949bce380f2d64afd6038e15ff70e7aaf4d9b92e8bf4188696e1264e090000000000000000070000000000000030ffffff010000000100000000000000";
/// `TimelockSet { account 7, blocks 5 }`.
pub const GOLDEN_TIMELOCK_SET_HEX: &str = "07000000000000000500000000000000";
/// `PendingCancelled { account 7, execute_at 100, SetTimelock(5) }`.
pub const GOLDEN_PENDING_CANCELLED_HEX: &str =
    "0700000000000000640000000000000001000000000000000500000000000000";
/// `RegistrySet { registry: [0x0d; 32], epoch: 1 }`.
pub const GOLDEN_REGISTRY_SET_HEX: &str =
    "0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0100000000000000";
/// `ProposalTtlSet { blocks: 1000 }`.
pub const GOLDEN_PROPOSAL_TTL_SET_HEX: &str = "e803000000000000";
/// `TombstoneSet { tombstone: true }`.
pub const GOLDEN_TOMBSTONE_SET_HEX: &str = "01";
/// `AuthorizedAccountSet { account 7, generation 1 }`.
pub const GOLDEN_AUTHORIZED_ACCOUNT_SET_HEX: &str = "07000000000000000100000000000000";
/// `ProposalCreated` for `set_value`, args `[1, 2, 3]`, digest `0x11`.
pub const GOLDEN_PROPOSAL_CREATED_HEX: &str = "7365745f76616c756501020300000000040000000000000011111111111111111111111111111111111111111111111111111111111111110700000000000000e8030000000000000100000000000000090000000000000001000000000000000d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0900000080ffffff81ffffff03000000";
/// `ProposalApproved` over digest `0x11`, seed key 0, `sign_insecure`.
pub const GOLDEN_PROPOSAL_APPROVED_HEX: &str = "04000000000000001111111111111111111111111111111111111111111111111111111111111111e3a945bd7dbd51365c255b3a7851432419f20ddb7bc948f5b60d677c5b02ff9e6255228ee75c9dd8a3bd4a86751e9b14cf501c89e69b4b2a2169c189accff3afc07b7ff80a0acfc75a4e073ee006624f722dd52ef90ae1828d8bfdcb6c1e260aad4c44e90e1b5e5c2067d4363ee978a0db41fdba0f29829a1263e43f33f231a9dc20fc5acafc235d9c920f2772cbd716ddb84cca39704625b55a01a011e7eeae177ef0949bce380f2d64afd6038e15ff70e7aaf4d9b92e8bf4188696e1264e090000000000000000374b44e24b396af6703685cae52d9efa06485d0954ed8303f47ff2b955438a2cc14672d519da0194c99f0af3c65a370e0df875cf13bc68530d224df5959be5496703761533d81f2a3d7f3343a5b8927cc044a8cfb03f1867e123aeb71aba5b160000000000000000";
/// `ProposalFinalized` for `set_value`, args `[1, 2, 3]`.
pub const GOLDEN_PROPOSAL_FINALIZED_HEX: &str = "7365745f76616c7565010203000000000400000000000000111111111111111111111111111111111111111111111111111111111111111107000000000000000d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d09000000a0ffffffa1ffffff03000000";
/// `ProposalQueued` with `execute_at` 100.
pub const GOLDEN_PROPOSAL_QUEUED_HEX: &str = "7365745f76616c75650102030000000004000000000000001111111111111111111111111111111111111111111111111111111111111111070000000000000064000000000000000d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0900000098ffffff99ffffff03000000";
/// `ProposalCancelled { id 4, digest 0x11, account 7 }`.
pub const GOLDEN_PROPOSAL_CANCELLED_HEX: &str = "040000000000000011111111111111111111111111111111111111111111111111111111111111110700000000000000";
/// `PruneReport { proposal_ids: [4], digest_keys: [0x11; 32] }`.
pub const GOLDEN_PRUNE_REPORT_HEX: &str = "04000000000000001111111111111111111111111111111111111111111111111111111111111111d8ffffff01000000d8ffffff01000000";
/// `Pruned` carries the same two lists. Pin is recorded beside `PruneReport`.
pub const GOLDEN_PRUNED_HEX: &str = "04000000000000001111111111111111111111111111111111111111111111111111111111111111d8ffffff01000000d8ffffff01000000";
/// `RegistryBookEffect::TimelockSet { blocks: 5 }`.
pub const GOLDEN_EFFECT_TIMELOCK_HEX: &str =
    "0200000000000000050000000000000000000000000000000000000000000000";
/// `RegistryBookEffect::Scheduled { execute_at: 100, SetTimelock(5) }`.
pub const GOLDEN_EFFECT_SCHEDULED_HEX: &str =
    "0000000000000000640000000000000001000000000000000500000000000000";

#[cfg(test)]
mod tests {
    use alloc::format;
    use alloc::string::String;
    use alloc::vec;

    use dusk_core::signatures::bls::{PublicKey as BlsPublicKey, SecretKey as BlsSecretKey};
    use rand::SeedableRng;
    use rand::rngs::StdRng;
    use rkyv::Serialize;
    use rkyv::ser::serializers::AllocSerializer;

    use super::{
        GOLDEN_MULTISIG_ACCOUNT_VIEW_HEX, GOLDEN_SIGNATURE_ENTRY_HEX, GOLDEN_VERIFY_QUORUM_ARGS_HEX,
    };
    use crate::call_types::{MultisigAccountView, SignatureEntry, VerifyQuorumArgs};

    const MSG: &[u8] = b"wave5-layout-golden-multisig";

    fn archive_hex<T>(v: &T) -> String
    where
        T: Serialize<AllocSerializer<4096>>,
    {
        rkyv::to_bytes::<_, 4096>(v)
            .expect("archive")
            .as_ref()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }

    fn fixed_keys() -> [(BlsSecretKey, BlsPublicKey); 3] {
        let mut rng = StdRng::seed_from_u64(0xa11ce_u64);
        std::array::from_fn(|_| {
            let sk = BlsSecretKey::random(&mut rng);
            let pk = BlsPublicKey::from(&sk);
            (sk, pk)
        })
    }

    fn fixed_signature_entry(sk: &BlsSecretKey, pk: &BlsPublicKey) -> SignatureEntry {
        SignatureEntry {
            signer: *pk,
            // PreforkHostQuery: VM::ephemeral PreFork — dusk-vm-issue-1; live clients use sign()/sign_multisig() (F-001)
            signature: sk.sign_insecure(MSG),
        }
    }

    #[test]
    fn signature_entry_golden() {
        let keys = fixed_keys();
        let entry = fixed_signature_entry(&keys[0].0, &keys[0].1);
        assert_eq!(archive_hex(&entry), GOLDEN_SIGNATURE_ENTRY_HEX);
    }

    #[test]
    fn verify_quorum_args_golden() {
        let keys = fixed_keys();
        let args = VerifyQuorumArgs {
            account_id: 1,
            msg: MSG.to_vec(),
            sigs: vec![fixed_signature_entry(&keys[0].0, &keys[0].1)],
        };
        assert_eq!(archive_hex(&args), GOLDEN_VERIFY_QUORUM_ARGS_HEX);
    }

    #[test]
    fn multisig_account_view_golden() {
        let keys = fixed_keys();
        let view = MultisigAccountView {
            members: vec![keys[0].1, keys[1].1],
            threshold: 2,
            nonce: 3,
            timelock_blocks: 0,
            pending: None,
        };
        assert_eq!(archive_hex(&view), GOLDEN_MULTISIG_ACCOUNT_VIEW_HEX);
    }

    #[test]
    fn event_payload_goldens() {
        use super::{
            GOLDEN_ACCOUNT_CHANGED_HEX, GOLDEN_ACCOUNT_CREATED_HEX,
            GOLDEN_AUTHORIZED_ACCOUNT_SET_HEX, GOLDEN_DATA_SET_HEX, GOLDEN_EFFECT_SCHEDULED_HEX,
            GOLDEN_EFFECT_TIMELOCK_HEX, GOLDEN_PENDING_CANCELLED_HEX, GOLDEN_PENDING_SCHEDULED_HEX,
            GOLDEN_PROPOSAL_APPROVED_HEX, GOLDEN_PROPOSAL_CANCELLED_HEX,
            GOLDEN_PROPOSAL_CREATED_HEX, GOLDEN_PROPOSAL_FINALIZED_HEX, GOLDEN_PROPOSAL_QUEUED_HEX,
            GOLDEN_PROPOSAL_TTL_SET_HEX, GOLDEN_PRUNE_REPORT_HEX, GOLDEN_PRUNED_HEX,
            GOLDEN_REGISTRY_SET_HEX,
            GOLDEN_TIMELOCK_SET_HEX, GOLDEN_TOMBSTONE_SET_HEX,
        };
        use alloc::string::String;
        use dusk_core::abi::ContractId;

        use crate::call_types::{PruneReport, RegistryBookEffect, RegistryPendingChange};
        use crate::events::{
            AccountChanged, AccountCreated, AuthorizedAccountSet, DataSet, PendingCancelled,
            PendingScheduled, ProposalApproved, ProposalCancelled, ProposalCreated,
            ProposalFinalized, ProposalQueued, ProposalTtlSet, Pruned, RegistrySet, TimelockSet,
            TombstoneSet,
        };

        let id = ContractId::from_bytes([0x0d; 32]);
        let keys = fixed_keys();
        let entry = fixed_signature_entry(&keys[0].0, &keys[0].1);
        let digest = [0x11; 32];
        let rows: Vec<(&str, String, &str)> = vec![
            (
                "DataSet",
                archive_hex(&DataSet { data: id }),
                GOLDEN_DATA_SET_HEX,
            ),
            (
                "AccountCreated",
                archive_hex(&AccountCreated {
                    id: 7,
                    members: vec![keys[0].1],
                    threshold: 1,
                    timelock_blocks: 0,
                    nonce: 0,
                }),
                GOLDEN_ACCOUNT_CREATED_HEX,
            ),
            (
                "PendingScheduled",
                archive_hex(&PendingScheduled {
                    account_id: 7,
                    execute_at: 100,
                    change: RegistryPendingChange::SetTimelock(5),
                }),
                GOLDEN_PENDING_SCHEDULED_HEX,
            ),
            (
                "AccountChanged",
                archive_hex(&AccountChanged {
                    account_id: 7,
                    members: vec![keys[0].1],
                    threshold: 1,
                }),
                GOLDEN_ACCOUNT_CHANGED_HEX,
            ),
            (
                "TimelockSet",
                archive_hex(&TimelockSet {
                    account_id: 7,
                    blocks: 5,
                }),
                GOLDEN_TIMELOCK_SET_HEX,
            ),
            (
                "PendingCancelled",
                archive_hex(&PendingCancelled {
                    account_id: 7,
                    execute_at: 100,
                    change: RegistryPendingChange::SetTimelock(5),
                }),
                GOLDEN_PENDING_CANCELLED_HEX,
            ),
            (
                "RegistrySet",
                archive_hex(&RegistrySet {
                    registry: id,
                    epoch: 1,
                }),
                GOLDEN_REGISTRY_SET_HEX,
            ),
            (
                "ProposalTtlSet",
                archive_hex(&ProposalTtlSet { blocks: 1000 }),
                GOLDEN_PROPOSAL_TTL_SET_HEX,
            ),
            (
                "TombstoneSet",
                archive_hex(&TombstoneSet { tombstone: true }),
                GOLDEN_TOMBSTONE_SET_HEX,
            ),
            (
                "AuthorizedAccountSet",
                archive_hex(&AuthorizedAccountSet {
                    account_id: 7,
                    auth_generation: 1,
                }),
                GOLDEN_AUTHORIZED_ACCOUNT_SET_HEX,
            ),
            (
                "ProposalCreated",
                archive_hex(&ProposalCreated {
                    proposal_id: 4,
                    signed_digest: digest,
                    registry_account_id: 7,
                    deadline: 1000,
                    epoch: 1,
                    nonce: 9,
                    auth_generation: 1,
                    target: id,
                    function_name: String::from("set_value"),
                    call_args: vec![1, 2, 3],
                }),
                GOLDEN_PROPOSAL_CREATED_HEX,
            ),
            (
                "ProposalApproved",
                archive_hex(&ProposalApproved {
                    proposal_id: 4,
                    signed_digest: digest,
                    signer: entry.signer,
                    signature: entry.signature,
                }),
                GOLDEN_PROPOSAL_APPROVED_HEX,
            ),
            (
                "ProposalFinalized",
                archive_hex(&ProposalFinalized {
                    proposal_id: 4,
                    signed_digest: digest,
                    registry_account_id: 7,
                    target: id,
                    function_name: String::from("set_value"),
                    call_args: vec![1, 2, 3],
                }),
                GOLDEN_PROPOSAL_FINALIZED_HEX,
            ),
            (
                "ProposalQueued",
                archive_hex(&ProposalQueued {
                    proposal_id: 4,
                    signed_digest: digest,
                    registry_account_id: 7,
                    execute_at: 100,
                    target: id,
                    function_name: String::from("set_value"),
                    call_args: vec![1, 2, 3],
                }),
                GOLDEN_PROPOSAL_QUEUED_HEX,
            ),
            (
                "ProposalCancelled",
                archive_hex(&ProposalCancelled {
                    proposal_id: 4,
                    signed_digest: digest,
                    registry_account_id: 7,
                }),
                GOLDEN_PROPOSAL_CANCELLED_HEX,
            ),
            (
                "Pruned",
                archive_hex(&Pruned {
                    proposal_ids: vec![4],
                    digest_keys: vec![digest],
                }),
                GOLDEN_PRUNED_HEX,
            ),
            (
                "PruneReport",
                archive_hex(&PruneReport {
                    proposal_ids: vec![4],
                    digest_keys: vec![digest],
                }),
                GOLDEN_PRUNE_REPORT_HEX,
            ),
            (
                "RegistryBookEffect::TimelockSet",
                archive_hex(&RegistryBookEffect::TimelockSet { blocks: 5 }),
                GOLDEN_EFFECT_TIMELOCK_HEX,
            ),
            (
                "RegistryBookEffect::Scheduled",
                archive_hex(&RegistryBookEffect::Scheduled {
                    execute_at: 100,
                    change: RegistryPendingChange::SetTimelock(5),
                }),
                GOLDEN_EFFECT_SCHEDULED_HEX,
            ),
        ];
        let mut mismatches = String::new();
        for (name, actual, expected) in rows {
            if actual != expected {
                mismatches.push_str(name);
                mismatches.push('\n');
                mismatches.push_str(&actual);
                mismatches.push_str("\n\n");
            }
        }
        assert!(mismatches.is_empty(), "{mismatches}");
    }
}
