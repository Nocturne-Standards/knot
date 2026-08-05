//! Consumer-local archived-layout goldens for `knot-proposals` call types
//! (Wave 5 / spec 23a + 23b Phase B). Shared layer-E types (`SignatureEntry`,
//! `VerifyQuorumArgs`, `MultisigAccountView`) assert against
//! `knot_encoding::layout_goldens` — do **not** re-paste those hex values
//! here; still call `rkyv::to_bytes` at runtime.
//!
//! **rkyv camp:** this crate pins `rkyv = "=0.7.39"`. Resolved patch:
//! `(cd crates/knot-proposals && cargo tree -p rkyv)`.
//!
//! **Layer E + `repr(C)`:** Archive structs in `knot-encoding` `call_types`
//! carry `#[archive_attr(repr(C))]`. Fieldless `ProposalStatus` keeps
//! `#[repr(u8)]` only — `archive_attr(repr(C))` rejected on archived unit enum.
//! Measured **DIFFERENT** 2026-08-03 on `ProposeArgs` / `MultisigAccountView`
//! (IDENTICAL on `SignatureEntry`, `VerifyQuorumArgs`, `ApproveArgs`,
//! `ProposalView`, `ProposalStatus`). Constants below are after-pin bytes
//! where they moved.
//!
//! Fixed inputs: `StdRng::seed_from_u64(0xa11ce_u64)`; message bytes
//! `b"wave5-layout-golden-multisig"` for signatures; target `ContractId` all
//! `0x0d`; `function_name` `"set_value"`; `call_args` rkyv `u64(42)`; digest
//! `[0x11; 32]` for `ProposalView` approval signatures.
//!
//! R9 corrupt-one-digit on **post-`repr(C)`** constants 2026-08-03:
//! `GOLDEN_PROPOSE_ARGS_HEX` final digit flipped; `propose_args_golden`
//! failed; reverted; green.

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use dusk_core::abi::ContractId;
use dusk_core::signatures::bls::{PublicKey as BlsPublicKey, SecretKey as BlsSecretKey};
use rand::SeedableRng;
use rand::rngs::StdRng;
use rkyv::Serialize;
use rkyv::ser::Serializer;
use rkyv::ser::serializers::AllocSerializer;

#[path = "../src/call_types.rs"]
mod call_types;

use call_types::{
    ApproveArgs, MultisigAccountView, ProposalStatus, ProposalView, ProposeArgs, SignatureEntry,
    VerifyQuorumArgs,
};
use knot_encoding::layout_goldens::{
    GOLDEN_MULTISIG_ACCOUNT_VIEW_HEX, GOLDEN_SIGNATURE_ENTRY_HEX, GOLDEN_VERIFY_QUORUM_ARGS_HEX,
};

const MSG: &[u8] = b"wave5-layout-golden-multisig";
const TARGET: ContractId = ContractId::from_bytes([0x0d; 32]);
const SIGNED_DIGEST: [u8; 32] = [0x11; 32];

/// `ProposalStatus::Open`.
/// Provenance: rustc 1.94.0 (4a4ef493e 2026-03-02); rkyv 0.7.39.
pub const GOLDEN_PROPOSAL_STATUS_HEX: &str = "00";

/// `ApproveArgs { proposal_id: 1, signer: pk0, signature: sign_insecure(MSG) }`.
/// Provenance: rustc 1.94.0 (4a4ef493e 2026-03-02); rkyv 0.7.39.
pub const GOLDEN_APPROVE_ARGS_HEX: &str = "0100000000000000e3a945bd7dbd51365c255b3a7851432419f20ddb7bc948f5b60d677c5b02ff9e6255228ee75c9dd8a3bd4a86751e9b14cf501c89e69b4b2a2169c189accff3afc07b7ff80a0acfc75a4e073ee006624f722dd52ef90ae1828d8bfdcb6c1e260aad4c44e90e1b5e5c2067d4363ee978a0db41fdba0f29829a1263e43f33f231a9dc20fc5acafc235d9c920f2772cbd716ddb84cca39704625b55a01a011e7eeae177ef0949bce380f2d64afd6038e15ff70e7aaf4d9b92e8bf4188696e1264e090000000000000000374b44e24b396af6703685cae52d9efa06485d0954ed8303f47ff2b955438a2cc14672d519da0194c99f0af3c65a370e0df875cf13bc68530d224df5959be5496703761533d81f2a3d7f3343a5b8927cc044a8cfb03f1867e123aeb71aba5b160000000000000000";

/// `ProposeArgs` — account 1, TARGET, `"set_value"`, rkyv `u64(42)`, nonce 4, deadline 1000.
/// Provenance: rustc 1.94.0 (4a4ef493e 2026-03-02); rkyv 0.7.39.
pub const GOLDEN_PROPOSE_ARGS_HEX: &str = "7365745f76616c75652a000000000000000000000000000001000000000000000d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d09000000c0ffffffc1ffffff080000000400000000000000e803000000000000";

/// `ProposalView` — open proposal with one approval over `SIGNED_DIGEST`.
/// Provenance: rustc 1.94.0 (4a4ef493e 2026-03-02); rkyv 0.7.39.
pub const GOLDEN_PROPOSAL_VIEW_HEX: &str = "7365745f76616c75652a0000000000000000000000000000e3a945bd7dbd51365c255b3a7851432419f20ddb7bc948f5b60d677c5b02ff9e6255228ee75c9dd8a3bd4a86751e9b14cf501c89e69b4b2a2169c189accff3afc07b7ff80a0acfc75a4e073ee006624f722dd52ef90ae1828d8bfdcb6c1e260aad4c44e90e1b5e5c2067d4363ee978a0db41fdba0f29829a1263e43f33f231a9dc20fc5acafc235d9c920f2772cbd716ddb84cca39704625b55a01a011e7eeae177ef0949bce380f2d64afd6038e15ff70e7aaf4d9b92e8bf4188696e1264e09000000000000000057a5f95daf1b385e4bd7077fc420acb449575577d18f66c0ba58f26695eb2d0fa6235b9b497f7dcd49a9079274b2650f8d3525d3318ec030378cc17875031cbd031d8006257e6df23d7ec75830b3f9ba3cf06df01ae4216e8b6bfc971dbb181600000000000000000100000000000000020000000000000002000000000000000d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0900000080feffff81feffff080000000000000000000000111111111111111111111111111111111111111111111111111111111111111160feffff0100000020ffffff010000000000000000000000";

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

fn rkyv_u64(value: u64) -> Vec<u8> {
    let mut ser = AllocSerializer::<256>::default();
    ser.serialize_value(&value).expect("rkyv serialize");
    ser.into_serializer().into_inner().to_vec()
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
        signature: sk.sign_insecure(MSG),
    }
}

#[test]
fn proposal_status_golden() {
    assert_eq!(
        archive_hex(&ProposalStatus::Open),
        GOLDEN_PROPOSAL_STATUS_HEX
    );
}

#[test]
fn signature_entry_golden() {
    let keys = fixed_keys();
    let entry = fixed_signature_entry(&keys[0].0, &keys[0].1);
    assert_eq!(archive_hex(&entry), GOLDEN_SIGNATURE_ENTRY_HEX);
}

#[test]
fn approve_args_golden() {
    let keys = fixed_keys();
    let args = ApproveArgs {
        proposal_id: 1,
        signer: keys[0].1,
        signature: keys[0].0.sign_insecure(MSG),
    };
    assert_eq!(archive_hex(&args), GOLDEN_APPROVE_ARGS_HEX);
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
    };
    assert_eq!(archive_hex(&view), GOLDEN_MULTISIG_ACCOUNT_VIEW_HEX);
}

#[test]
fn propose_args_golden() {
    let args = ProposeArgs {
        registry_account_id: 1,
        target: TARGET,
        function_name: String::from("set_value"),
        call_args: rkyv_u64(42),
        nonce: 4,
        deadline: 1000,
    };
    assert_eq!(archive_hex(&args), GOLDEN_PROPOSE_ARGS_HEX);
}

#[test]
fn proposal_view_golden() {
    let keys = fixed_keys();
    let view = ProposalView {
        registry_account_id: 1,
        epoch: 2,
        nonce: 2,
        target: TARGET,
        function_name: String::from("set_value"),
        call_args: rkyv_u64(42),
        deadline: 0,
        signed_digest: SIGNED_DIGEST,
        approvals: vec![keys[0].1],
        approval_sigs: vec![keys[0].0.sign_insecure(&SIGNED_DIGEST)],
        status: ProposalStatus::Open,
    };
    assert_eq!(archive_hex(&view), GOLDEN_PROPOSAL_VIEW_HEX);
}
