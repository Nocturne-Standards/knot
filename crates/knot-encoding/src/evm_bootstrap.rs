//! EVM root bootstrap (D9): the message a knot account's quorum signs to let
//! `knot-registry` publish the account's first member set to `KnotEvmRoot` on
//! DuskEVM, and the `bootstrap(uint64,uint64,bytes,uint32)` calldata it sends through
//! the L1 messenger.

use alloc::vec::Vec;

use tiny_keccak::{Hasher, Keccak};

use crate::{EncodingError, checked_u32_len};

/// Quorum domain for `knot-registry::publish_bootstrap_root`.
pub const DOMAIN_EVM_BOOTSTRAP_V1: &[u8] = b"nocturne.knot.multisig-registry.evm_bootstrap.v1";

/// Selector for `KnotEvmRoot.bootstrap(uint64,uint64,bytes,uint32)`.
/// The second `uint64` is the DuskDS chain id, checked against the root's immutable.
pub const BOOTSTRAP_SELECTOR: [u8; 4] = [0x8c, 0xf3, 0x74, 0x67];

/// `sendMessage` min gas: fixed part plus per member. Covers `KnotEvmRoot.bootstrap` measured on DuskEVM 745
/// (269,709 / 979,864 / 1,925,814 for 2 / 8 / 16 members, which decode and subgroup-check every key).
pub const BOOTSTRAP_MIN_GAS_BASE: u32 = 150_000;
pub const BOOTSTRAP_MIN_GAS_PER_MEMBER: u32 = 125_000;

/// `DOMAIN || chain_id_le || self_id || account_id_le || nonce_le || receiver[20] || member_count_le_u32 ||
/// pk₀…pkₙ || threshold_le_u32`. `nonce` is the registry account nonce, so a `change_account` invalidates any
/// bootstrap quorum gathered before it.
pub fn evm_bootstrap_preimage_v1(
    chain_id: u64,
    self_id: &[u8; 32],
    account_id: u64,
    nonce: u64,
    receiver: &[u8; 20],
    member_pks: &[[u8; 96]],
    threshold: u32,
) -> Result<Vec<u8>, EncodingError> {
    let member_count = checked_u32_len("member_pks", member_pks.len())?;
    let capacity = member_pks
        .len()
        .checked_mul(96)
        .and_then(|n| n.checked_add(DOMAIN_EVM_BOOTSTRAP_V1.len() + 8 + 32 + 8 + 8 + 20 + 4 + 4))
        .ok_or(EncodingError::CapacityOverflow)?;
    let mut out = Vec::with_capacity(capacity);
    out.extend_from_slice(DOMAIN_EVM_BOOTSTRAP_V1);
    out.extend_from_slice(&chain_id.to_le_bytes());
    out.extend_from_slice(self_id);
    out.extend_from_slice(&account_id.to_le_bytes());
    out.extend_from_slice(&nonce.to_le_bytes());
    out.extend_from_slice(receiver);
    out.extend_from_slice(&member_count.to_le_bytes());
    for pk in member_pks {
        out.extend_from_slice(pk);
    }
    out.extend_from_slice(&threshold.to_le_bytes());
    Ok(out)
}

/// Keccak256 of [`evm_bootstrap_preimage_v1`], as the message members sign.
pub fn evm_bootstrap_message_v1(
    chain_id: u64,
    self_id: &[u8; 32],
    account_id: u64,
    nonce: u64,
    receiver: &[u8; 20],
    member_pks: &[[u8; 96]],
    threshold: u32,
) -> Result<Vec<u8>, EncodingError> {
    let preimage = evm_bootstrap_preimage_v1(
        chain_id, self_id, account_id, nonce, receiver, member_pks, threshold,
    )?;
    let mut hasher = Keccak::v256();
    hasher.update(&preimage);
    let mut out = [0u8; 32];
    hasher.finalize(&mut out);
    Ok(out.to_vec())
}

/// Solidity ABI calldata for `bootstrap(uint64 accountId, uint64 duskChainId, bytes pks, uint32 threshold)`.
/// `pks` is the concatenated compressed keys (96 bytes each, so never padded).
pub fn encode_bootstrap_calldata(
    account_id: u64,
    dusk_chain_id: u64,
    member_pks: &[[u8; 96]],
    threshold: u32,
) -> Vec<u8> {
    let pks_len = member_pks.len() * 96;
    let mut out = Vec::with_capacity(4 + 32 * 5 + pks_len);
    out.extend_from_slice(&BOOTSTRAP_SELECTOR);
    out.extend_from_slice(&word(account_id));
    out.extend_from_slice(&word(dusk_chain_id));
    out.extend_from_slice(&word(0x80));
    out.extend_from_slice(&word(u64::from(threshold)));
    out.extend_from_slice(&word(pks_len as u64));
    for pk in member_pks {
        out.extend_from_slice(pk);
    }
    out
}

/// `sendMessage` min gas for a bootstrap of `member_count` keys.
pub fn bootstrap_min_gas(member_count: u32) -> u32 {
    BOOTSTRAP_MIN_GAS_PER_MEMBER
        .saturating_mul(member_count)
        .saturating_add(BOOTSTRAP_MIN_GAS_BASE)
}

fn word(v: u64) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[24..].copy_from_slice(&v.to_be_bytes());
    w
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(b: &[u8]) -> alloc::string::String {
        b.iter().map(|x| alloc::format!("{x:02x}")).collect()
    }

    #[test]
    fn selector_matches_solidity() {
        // cast sig 'bootstrap(uint64,uint64,bytes,uint32)'
        assert_eq!(hex(&BOOTSTRAP_SELECTOR), "8cf37467");
    }

    #[test]
    fn calldata_matches_cast_golden() {
        // cast calldata 'bootstrap(uint64,uint64,bytes,uint32)' 7 2 0x(11*96)(22*96) 2
        let golden = alloc::format!(
            "8cf37467{}{}{}{}{}{}{}",
            "0000000000000000000000000000000000000000000000000000000000000007",
            "0000000000000000000000000000000000000000000000000000000000000002",
            "0000000000000000000000000000000000000000000000000000000000000080",
            "0000000000000000000000000000000000000000000000000000000000000002",
            "00000000000000000000000000000000000000000000000000000000000000c0",
            "11".repeat(96),
            "22".repeat(96),
        );
        let data = encode_bootstrap_calldata(7, 2, &[[0x11; 96], [0x22; 96]], 2);
        assert_eq!(data.len(), 356);
        assert_eq!(hex(&data), golden);
    }

    #[test]
    fn preimage_layout() {
        let pre =
            evm_bootstrap_preimage_v1(2, &[0xa1; 32], 5, 9, &[0xbb; 20], &[[0x11; 96]], 1).unwrap();
        let d = DOMAIN_EVM_BOOTSTRAP_V1.len();
        assert_eq!(&pre[..d], DOMAIN_EVM_BOOTSTRAP_V1);
        assert_eq!(&pre[d..d + 8], &2u64.to_le_bytes());
        assert_eq!(&pre[d + 8..d + 40], &[0xa1; 32]);
        assert_eq!(&pre[d + 40..d + 48], &5u64.to_le_bytes());
        assert_eq!(&pre[d + 48..d + 56], &9u64.to_le_bytes());
        assert_eq!(&pre[d + 56..d + 76], &[0xbb; 20]);
        assert_eq!(&pre[d + 76..d + 80], &1u32.to_le_bytes());
        assert_eq!(&pre[d + 80..d + 176], &[0x11; 96]);
        assert_eq!(&pre[d + 176..], &1u32.to_le_bytes());
    }

    #[test]
    fn message_binds_every_field() {
        let base = || {
            evm_bootstrap_message_v1(2, &[0xa1; 32], 5, 9, &[0xbb; 20], &[[0x11; 96]], 1).unwrap()
        };
        let m = base();
        assert_eq!(m.len(), 32);
        assert_eq!(m, base());
        for other in [
            evm_bootstrap_message_v1(3, &[0xa1; 32], 5, 9, &[0xbb; 20], &[[0x11; 96]], 1),
            evm_bootstrap_message_v1(2, &[0xa2; 32], 5, 9, &[0xbb; 20], &[[0x11; 96]], 1),
            evm_bootstrap_message_v1(2, &[0xa1; 32], 6, 9, &[0xbb; 20], &[[0x11; 96]], 1),
            evm_bootstrap_message_v1(2, &[0xa1; 32], 5, 10, &[0xbb; 20], &[[0x11; 96]], 1),
            evm_bootstrap_message_v1(2, &[0xa1; 32], 5, 9, &[0xbc; 20], &[[0x11; 96]], 1),
            evm_bootstrap_message_v1(2, &[0xa1; 32], 5, 9, &[0xbb; 20], &[[0x12; 96]], 1),
            evm_bootstrap_message_v1(
                2,
                &[0xa1; 32],
                5,
                9,
                &[0xbb; 20],
                &[[0x11; 96], [0x12; 96]],
                1,
            ),
            evm_bootstrap_message_v1(2, &[0xa1; 32], 5, 9, &[0xbb; 20], &[[0x11; 96]], 2),
        ] {
            assert_ne!(other.unwrap(), m);
        }
    }

    #[test]
    fn min_gas_covers_measured_bootstrap() {
        for (n, measured) in [(2u32, 269_709u32), (8, 979_864), (16, 1_925_814)] {
            assert!(bootstrap_min_gas(n) > measured, "n={n}");
        }
        assert_eq!(bootstrap_min_gas(16), 2_150_000);
    }
}
