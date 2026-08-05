//! Consumer-local archived-layout goldens for `knot-registry` call types
//! (Wave 5 / spec 23a + 23b Phase B). Shared layer-E types (`SignatureEntry`,
//! `VerifyQuorumArgs`, `MultisigAccountView`) assert against
//! `knot_encoding::layout_goldens` — do **not** re-paste those hex values
//! here; still call `rkyv::to_bytes` at runtime.
//!
//! **rkyv camp:** this crate pins `rkyv = "=0.7.39"`. Resolved patch:
//! `(cd crates/knot-registry && cargo tree -p rkyv)`.
//!
//! **Layer E + `repr(C)`:** Archive structs in `knot-encoding` `call_types`
//! carry `#[archive_attr(repr(C))]`. Measured **DIFFERENT** 2026-08-03 on
//! `MultisigAccountView`, `ChangeAccountArgs`, `AccountMeta`,
//! `DiagnoseQuorumResult` (IDENTICAL on `SignatureEntry`, `VerifyQuorumArgs`,
//! `CreateAccountArgs`, `VerifyQuorumAggregateArgs`). Constants below are
//! after-pin bytes where they moved.
//!
//! Fixed inputs: `StdRng::seed_from_u64(0xa11ce_u64)`; message bytes
//! `b"wave5-layout-golden-multisig"` for signatures and aggregate multisig.
//!
//! R9 corrupt-one-digit on **post-`repr(C)`** constants 2026-08-03:
//! `GOLDEN_ACCOUNT_META_HEX` final digit flipped; `account_meta_golden`
//! failed; reverted; green.

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use dusk_bytes::Serializable;
use dusk_core::signatures::bls::{
    MultisigSignature, PublicKey as BlsPublicKey, SecretKey as BlsSecretKey,
};
use rand::rngs::StdRng;
use rand::SeedableRng;
use rkyv::ser::serializers::AllocSerializer;
use rkyv::Serialize;

#[path = "../src/call_types.rs"]
mod call_types;

use call_types::{
    AccountMeta, ChangeAccountArgs, CreateAccountArgs, DiagnoseQuorumResult, MultisigAccountView,
    SignatureEntry, VerifyQuorumAggregateArgs, VerifyQuorumArgs,
};
use knot_encoding::layout_goldens::{
    GOLDEN_MULTISIG_ACCOUNT_VIEW_HEX, GOLDEN_SIGNATURE_ENTRY_HEX, GOLDEN_VERIFY_QUORUM_ARGS_HEX,
};

const MSG: &[u8] = b"wave5-layout-golden-multisig";

/// `CreateAccountArgs { members: [pk0, pk1], threshold: 2 }`.
/// Provenance: rustc 1.94.0 (4a4ef493e 2026-03-02); rkyv 0.7.39.
pub const GOLDEN_CREATE_ACCOUNT_ARGS_HEX: &str = concat!(
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
    "1ff8dd71321866759507000000000000000070feffff0200000002000000"
);

/// `ChangeAccountArgs { account_id: 1, new_members: [pk2], new_threshold: 1, sigs: [one] }`.
/// Provenance: rustc 1.94.0 (4a4ef493e 2026-03-02); rkyv 0.7.39.
pub const GOLDEN_CHANGE_ACCOUNT_ARGS_HEX: &str =
    "99e317079d0813b10186109119eed0c881d13de10903e46188fd20511aaf9c1558564601889566e5c12e0a147213fe111e12df3a6a492224e13b5c7a375002f8452bc57d68bf4a7cd4742860903323b98fcedad84c3c7e10d826bab72bdb6802b454945d4ef56850364fcc3f181bb4377e77e936696d9ca8a3aad0a185cda0529ca59c17aa9d1535178fab2002bf001920d8826fb762f61d311b489de157d3270df9decf5525456671eeaf48f195585405891617a6be24b072eeb50d507845050000000000000000e3a945bd7dbd51365c255b3a7851432419f20ddb7bc948f5b60d677c5b02ff9e6255228ee75c9dd8a3bd4a86751e9b14cf501c89e69b4b2a2169c189accff3afc07b7ff80a0acfc75a4e073ee006624f722dd52ef90ae1828d8bfdcb6c1e260aad4c44e90e1b5e5c2067d4363ee978a0db41fdba0f29829a1263e43f33f231a9dc20fc5acafc235d9c920f2772cbd716ddb84cca39704625b55a01a011e7eeae177ef0949bce380f2d64afd6038e15ff70e7aaf4d9b92e8bf4188696e1264e090000000000000000374b44e24b396af6703685cae52d9efa06485d0954ed8303f47ff2b955438a2cc14672d519da0194c99f0af3c65a370e0df875cf13bc68530d224df5959be5496703761533d81f2a3d7f3343a5b8927cc044a8cfb03f1867e123aeb71aba5b160000000000000000010000000000000000feffff0100000001000000bcfeffff0100000000000000";

/// `VerifyQuorumAggregateArgs` — two signers, aggregate over MSG.
/// Provenance: rustc 1.94.0 (4a4ef493e 2026-03-02); rkyv 0.7.39.
pub const GOLDEN_VERIFY_QUORUM_AGGREGATE_ARGS_HEX: &str = concat!(
    "77617665352d6c61796f75742d676f6c64656e2d6d756c746973696700000000",
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
    "1ff8dd713218667595070000000000000000010000000000000048feffff1c000000",
    "60feffff02000000b52c851106e766691caa7152513a069aa3345ed9f90ba9ac8",
    "af9fa718edd679c509b955435c8ae7daa435aecf2253b16e9c295b32d082cb50",
    "e09feabe42f2ccd8c20f44fb173f6bc84b87720248f5bb0da5904d93a1bd439f07",
    "e991453fa10050000000000000000"
);

/// `AccountMeta { threshold: 2, nonce: 3, members_len: 2 }`.
/// Provenance: rustc 1.94.0 (4a4ef493e 2026-03-02); rkyv 0.7.39.
pub const GOLDEN_ACCOUNT_META_HEX: &str = "020000000000000003000000000000000200000000000000";

/// `DiagnoseQuorumResult` — exists=true, one 96-byte member pk row.
/// Provenance: rustc 1.94.0 (4a4ef493e 2026-03-02); rkyv 0.7.39.
pub const GOLDEN_DIAGNOSE_QUORUM_RESULT_HEX: &str =
    "93b30e683ad74bf6811ae1512963e38615e9e6fd086b3a8fd06fbfcbbf4bf12dce0352ab0ba23ae8f2d6b8ce4686efdc04e730080687cbee86a15a745b88328ec578c3e2c4835d1b114f7932ad9d8a4a2b86893b73de1128a2684b446657090ca0ffffff600000000100000002000000020000000100000001000000e4ffffff01000000";

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
        signature: sk.sign_insecure(MSG),
    }
}

fn fixed_aggregate(signers: &[(&BlsSecretKey, &BlsPublicKey)]) -> MultisigSignature {
    let sigs: Vec<MultisigSignature> = signers
        .iter()
        .map(|(sk, pk)| sk.sign_multisig_insecure(pk, MSG))
        .collect();
    let (first, rest) = sigs.split_first().expect("at least one signer");
    first.aggregate(rest)
}

#[test]
fn signature_entry_golden() {
    let keys = fixed_keys();
    let entry = fixed_signature_entry(&keys[0].0, &keys[0].1);
    assert_eq!(archive_hex(&entry), GOLDEN_SIGNATURE_ENTRY_HEX);
}

#[test]
fn create_account_args_golden() {
    let keys = fixed_keys();
    let args = CreateAccountArgs {
        members: vec![keys[0].1, keys[1].1],
        threshold: 2,
    };
    assert_eq!(archive_hex(&args), GOLDEN_CREATE_ACCOUNT_ARGS_HEX);
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
fn change_account_args_golden() {
    let keys = fixed_keys();
    let args = ChangeAccountArgs {
        account_id: 1,
        new_members: vec![keys[2].1],
        new_threshold: 1,
        sigs: vec![fixed_signature_entry(&keys[0].0, &keys[0].1)],
    };
    assert_eq!(archive_hex(&args), GOLDEN_CHANGE_ACCOUNT_ARGS_HEX);
}

#[test]
fn verify_quorum_aggregate_args_golden() {
    let keys = fixed_keys();
    let signers = [(&keys[0].0, &keys[0].1), (&keys[1].0, &keys[1].1)];
    let args = VerifyQuorumAggregateArgs {
        account_id: 1,
        msg: MSG.to_vec(),
        signer_keys: vec![keys[0].1, keys[1].1],
        aggregate_sig: fixed_aggregate(&signers),
    };
    assert_eq!(archive_hex(&args), GOLDEN_VERIFY_QUORUM_AGGREGATE_ARGS_HEX);
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
fn account_meta_golden() {
    let meta = AccountMeta {
        threshold: 2,
        nonce: 3,
        members_len: 2,
    };
    assert_eq!(archive_hex(&meta), GOLDEN_ACCOUNT_META_HEX);
}

#[test]
fn diagnose_quorum_result_golden() {
    let keys = fixed_keys();
    let result = DiagnoseQuorumResult {
        exists: true,
        threshold: 2,
        members_len: 2,
        member_matches: 1,
        sigs_ok: 1,
        member_pk_bytes: vec![keys[0].1.to_bytes().to_vec()],
    };
    assert_eq!(archive_hex(&result), GOLDEN_DIAGNOSE_QUORUM_RESULT_HEX);
}
