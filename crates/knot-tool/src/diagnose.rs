//! Off-chain quorum diagnostics — same counters as the removed on-chain
//! `diagnose_quorum` using `account()` + local BLS verify (IMPLEMENTATION §4.3 L3).

use dusk_bytes::Serializable;
use dusk_core::signatures::bls::{PublicKey as BlsPublicKey, Signature as BlsSignature};
use knot_encoding::call_types::{DiagnoseQuorumResult, MultisigAccountView, SignatureEntry, VerifyQuorumArgs};

pub fn diagnose_quorum(
    account: Option<&MultisigAccountView>,
    args: &VerifyQuorumArgs,
) -> DiagnoseQuorumResult {
    let Some(account) = account else {
        return DiagnoseQuorumResult {
            exists: false,
            threshold: 0,
            members_len: 0,
            member_matches: 0,
            sigs_ok: 0,
            member_pk_bytes: Vec::new(),
        };
    };

    let member_pk_bytes: Vec<Vec<u8>> = account
        .members
        .iter()
        .map(|pk| pk.to_bytes().to_vec())
        .collect();
    let (member_matches, sigs_ok) = quorum_counts(&account.members, &args.msg, &args.sigs);

    DiagnoseQuorumResult {
        exists: true,
        threshold: account.threshold,
        members_len: account.members.len() as u32,
        member_matches,
        sigs_ok,
        member_pk_bytes,
    }
}

fn quorum_counts(
    members: &[BlsPublicKey],
    msg: &[u8],
    sigs: &[SignatureEntry],
) -> (u32, u32) {
    if sigs.len() > members.len() {
        return (0, 0);
    }
    let mut counted: Vec<BlsPublicKey> = Vec::new();
    let mut matched = 0u32;
    let mut verified = 0u32;
    for entry in sigs {
        if !members.contains(&entry.signer) {
            continue;
        }
        matched += 1;
        if counted.contains(&entry.signer) {
            continue;
        }
        if verify_bls(&entry.signer, msg, &entry.signature) {
            counted.push(entry.signer);
            verified += 1;
        }
    }
    (matched, verified)
}

fn verify_bls(pk: &BlsPublicKey, msg: &[u8], sig: &BlsSignature) -> bool {
    pk.verify(sig, msg).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use dusk_core::signatures::bls::{PublicKey as BlsPublicKey, SecretKey as BlsSecretKey};
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    fn keypair(rng: &mut StdRng) -> (BlsSecretKey, BlsPublicKey) {
        let sk = BlsSecretKey::random(rng);
        let pk = BlsPublicKey::from(&sk);
        (sk, pk)
    }

    fn sign_all(msg: &[u8], pairs: &[(&BlsSecretKey, &BlsPublicKey)]) -> Vec<SignatureEntry> {
        pairs
            .iter()
            .map(|(sk, pk)| SignatureEntry {
                signer: **pk,
                signature: sk.sign(msg),
            })
            .collect()
    }

    #[test]
    fn diagnose_matches_member_and_sig_counts() {
        let rng = &mut StdRng::seed_from_u64(11);
        let (sk1, pk1) = keypair(rng);
        let (sk2, pk2) = keypair(rng);
        let view = MultisigAccountView {
            members: vec![pk1, pk2],
            threshold: 2,
            nonce: 0,
        };
        let msg = b"diagnose-me".to_vec();
        let good = diagnose_quorum(
            Some(&view),
            &VerifyQuorumArgs {
                account_id: 0,
                msg: msg.clone(),
                sigs: sign_all(&msg, &[(&sk1, &pk1), (&sk2, &pk2)]),
            },
        );
        assert!(good.exists);
        assert_eq!(good.member_matches, 2);
        assert_eq!(good.sigs_ok, 2);
        assert_eq!(good.member_pk_bytes.len(), 2);

        let (outsider_sk, outsider_pk) = keypair(rng);
        let bad = diagnose_quorum(
            Some(&view),
            &VerifyQuorumArgs {
                account_id: 0,
                msg,
                sigs: sign_all(b"x", &[(&outsider_sk, &outsider_pk)]),
            },
        );
        assert_eq!(bad.member_matches, 0);
        assert_eq!(bad.sigs_ok, 0);
    }
}
