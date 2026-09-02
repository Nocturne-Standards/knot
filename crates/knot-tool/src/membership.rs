//! Live registry membership checks before Lab produces BLS signatures.
//!
//! Prove-mode contracts still re-verify on-chain; this module fails fast so
//! operators do not burn a signing round with a non-member key.

use dusk_core::signatures::bls::PublicKey as BlsPublicKey;
use knot_encoding::call_types::MultisigAccountView;

/// Every `signer_pks` entry must appear in `view.members` (compressed PK equality).
pub fn ensure_pks_are_members(
    account_id: u64,
    signer_pks: &[BlsPublicKey],
    view: &MultisigAccountView,
) -> Result<(), String> {
    for pk in signer_pks {
        if !view.members.contains(pk) {
            return Err(format!(
                "signer is not a member of registry account {account_id}"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dusk_core::signatures::bls::{PublicKey as BlsPublicKey, SecretKey as BlsSecretKey};
    use rand::rngs::OsRng;

    fn random_pk() -> BlsPublicKey {
        BlsPublicKey::from(&BlsSecretKey::random(&mut OsRng))
    }

    #[test]
    fn member_pk_passes() {
        let pk = random_pk();
        let view = MultisigAccountView {
            members: vec![pk],
            threshold: 1,
            nonce: 0,
            timelock_blocks: 0,
            pending: None,
        };
        ensure_pks_are_members(0, &[pk], &view).expect("member");
    }

    #[test]
    fn non_member_pk_fails() {
        let pk = random_pk();
        let other = random_pk();
        let view = MultisigAccountView {
            members: vec![pk],
            threshold: 1,
            nonce: 0,
            timelock_blocks: 0,
            pending: None,
        };
        let err = ensure_pks_are_members(7, &[other], &view).unwrap_err();
        assert!(err.contains("not a member"));
        assert!(err.contains("7"));
    }
}
