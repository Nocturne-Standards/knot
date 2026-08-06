// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Nocturne Standards

//! Archived-layout golden pins for shared multisig layer-E call types (spec 26).
//!
//! Owner-crate `pub mod` — consumers (`knot-registry`, `knot-proposals`)
//! import these consts and still call `rkyv::to_bytes` at runtime (W4-11 /
//! plan 23a §4.1). Do **not** re-paste hex in consumer crates.
//!
//! **rkyv camp:** this crate pins `rkyv = "=0.7.39"` (same as the multisig
//! contracts). Resolved patch: `(cd crates/knot-encoding && cargo tree -p rkyv)`.
//!
//! **Layer E + `repr(C)` (spec 23b Phase B, 2026-08-03):** structs carry
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

/// `MultisigAccountView { members: [pk0, pk1], threshold: 2, nonce: 3 }`.
/// Provenance: rustc 1.94.0 (4a4ef493e 2026-03-02); rkyv 0.7.39.
/// After-pin hex (spec 23b `repr(C)` DIFFERENT 2026-08-03).
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
    "0300000000000000"
);

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
        };
        assert_eq!(archive_hex(&view), GOLDEN_MULTISIG_ACCOUNT_VIEW_HEX);
    }
}
