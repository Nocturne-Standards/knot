//! Consumer-local archived-layout goldens for `multisig-proposals` call types
//! (Wave 5 / spec 23a).
//!
//! **rkyv camp:** this crate pins `rkyv = "=0.7.39"` (compliance-gate / identity-
//! credential arm). Resolved patch: run `(cd multisig && cargo tree -p rkyv)`.
//!
//! **Layer E, no `repr(C)`:** these pins record the archived byte layout as
//! rkyv 0.7.39 emits it today. Spec 23b (`#[archive_attr(repr(C))]`) is out of
//! scope — do not add `repr(C)` here.
//!
//! Fixed inputs: `StdRng::seed_from_u64(0xa11ce_u64)`; message bytes
//! `b"wave5-layout-golden-multisig"` for signatures; target `ContractId` all
//! `0x0d`; `function_name` `"set_value"`; `call_args` rkyv `u64(42)`; digest
//! `[0x11; 32]` for `ProposalView` approval signatures.
//!
//! R9 corrupt-one-digit check performed 2026-08-01 on `GOLDEN_PROPOSAL_STATUS_HEX`
//! (flipped final `0` to `1`; `proposal_status_golden` failed; reverted; green).

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use dusk_core::abi::ContractId;
use dusk_core::signatures::bls::{PublicKey as BlsPublicKey, SecretKey as BlsSecretKey};
use rand::rngs::StdRng;
use rand::SeedableRng;
use rkyv::ser::serializers::AllocSerializer;
use rkyv::ser::Serializer;
use rkyv::Serialize;

#[path = "../src/call_types.rs"]
mod call_types;

use call_types::{
    ApproveArgs, MultisigAccountView, ProposalStatus, ProposalView, ProposeArgs, SignatureEntry,
    VerifyQuorumArgs,
};

const MSG: &[u8] = b"wave5-layout-golden-multisig";
const TARGET: ContractId = ContractId::from_bytes([0x0d; 32]);
const SIGNED_DIGEST: [u8; 32] = [0x11; 32];

/// `ProposalStatus::Open`.
/// Provenance: rustc 1.94.0 (4a4ef493e 2026-03-02); rkyv 0.7.39.
pub const GOLDEN_PROPOSAL_STATUS_HEX: &str = "00";

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

/// `ApproveArgs { proposal_id: 1, signer: pk0, signature: sign_insecure(MSG) }`.
/// Provenance: rustc 1.94.0 (4a4ef493e 2026-03-02); rkyv 0.7.39.
pub const GOLDEN_APPROVE_ARGS_HEX: &str =
    "0100000000000000e3a945bd7dbd51365c255b3a7851432419f20ddb7bc948f5b60d677c5b02ff9e6255228ee75c9dd8a3bd4a86751e9b14cf501c89e69b4b2a2169c189accff3afc07b7ff80a0acfc75a4e073ee006624f722dd52ef90ae1828d8bfdcb6c1e260aad4c44e90e1b5e5c2067d4363ee978a0db41fdba0f29829a1263e43f33f231a9dc20fc5acafc235d9c920f2772cbd716ddb84cca39704625b55a01a011e7eeae177ef0949bce380f2d64afd6038e15ff70e7aaf4d9b92e8bf4188696e1264e090000000000000000374b44e24b396af6703685cae52d9efa06485d0954ed8303f47ff2b955438a2cc14672d519da0194c99f0af3c65a370e0df875cf13bc68530d224df5959be5496703761533d81f2a3d7f3343a5b8927cc044a8cfb03f1867e123aeb71aba5b160000000000000000";

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

/// **This type is declared independently in both `multisig-registry` and
/// `multisig-proposals`** (an unclosed layer-E mirror of `MultisigAccountView`
/// across a live `abi::call` boundary — see `WAVE5-IMPLEMENTATION-REVIEW.md`
/// §5.1). The twin golden in the other crate uses the same fixture and, as of
/// 2026-08-01, records byte-identical hex. **If you change this constant,
/// change the other one, and explain why the two encoders diverged.**
///
/// `MultisigAccountView { members: [pk0, pk1], threshold: 2, nonce: 3 }`.
/// Provenance: rustc 1.94.0 (4a4ef493e 2026-03-02); rkyv 0.7.39.
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
    "1ff8dd71321866759507000000000000000070feffff020000000300000000000000",
    "0200000000000000"
);

/// `ProposeArgs` — account 1, TARGET, `"set_value"`, rkyv `u64(42)`, deadline 0.
/// Provenance: rustc 1.94.0 (4a4ef493e 2026-03-02); rkyv 0.7.39.
pub const GOLDEN_PROPOSE_ARGS_HEX: &str =
    "7365745f76616c75652a00000000000000000000000000000d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d010000000000000009000000c0ffffffc1ffffff080000000000000000000000";

/// `ProposalView` — open proposal with one approval over `SIGNED_DIGEST`.
/// Provenance: rustc 1.94.0 (4a4ef493e 2026-03-02); rkyv 0.7.39.
pub const GOLDEN_PROPOSAL_VIEW_HEX: &str =
    "7365745f76616c75652a0000000000000000000000000000e3a945bd7dbd51365c255b3a7851432419f20ddb7bc948f5b60d677c5b02ff9e6255228ee75c9dd8a3bd4a86751e9b14cf501c89e69b4b2a2169c189accff3afc07b7ff80a0acfc75a4e073ee006624f722dd52ef90ae1828d8bfdcb6c1e260aad4c44e90e1b5e5c2067d4363ee978a0db41fdba0f29829a1263e43f33f231a9dc20fc5acafc235d9c920f2772cbd716ddb84cca39704625b55a01a011e7eeae177ef0949bce380f2d64afd6038e15ff70e7aaf4d9b92e8bf4188696e1264e09000000000000000057a5f95daf1b385e4bd7077fc420acb449575577d18f66c0ba58f26695eb2d0fa6235b9b497f7dcd49a9079274b2650f8d3525d3318ec030378cc17875031cbd031d8006257e6df23d7ec75830b3f9ba3cf06df01ae4216e8b6bfc971dbb181600000000000000000100000000000000ca0000000000000002000000000000000d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0900000080feffff81feffff080000000000000000000000111111111111111111111111111111111111111111111111111111111111111160feffff0100000020ffffff010000000000000000000000";

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
    assert_eq!(archive_hex(&ProposalStatus::Open), GOLDEN_PROPOSAL_STATUS_HEX);
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
        deadline: 0,
    };
    assert_eq!(archive_hex(&args), GOLDEN_PROPOSE_ARGS_HEX);
}

#[test]
fn proposal_view_golden() {
    let keys = fixed_keys();
    let view = ProposalView {
        registry_account_id: 1,
        chain_id: 0xCA,
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
