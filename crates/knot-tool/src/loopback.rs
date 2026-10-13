// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Nocturne Standards

//! Canonical message for the private loopback quorum.
//!
//! 32 bytes, domain-separated from `change_account` / timelock / proposal
//! digests, so a `verify_quorum` of this message cannot be replayed as a
//! state change. The browser never chooses the bytes.

use tiny_keccak::{Hasher, Keccak};

/// Domain for the loopback quorum attestation. Not a contract entry point.
pub const DOMAIN: &[u8] = b"nocturne.knot.loopback.quorum.v1";

/// Keccak-256 of the loopback preimage. Always 32 bytes, so the signing
/// fingerprint is the message itself.
pub fn canonical_quorum_message(
    chain_id: u64,
    registry_self_id: &[u8; 32],
    account_id: u64,
    nonce: u64,
) -> [u8; 32] {
    let mut hasher = Keccak::v256();
    hasher.update(DOMAIN);
    hasher.update(&chain_id.to_le_bytes());
    hasher.update(registry_self_id);
    hasher.update(&account_id.to_le_bytes());
    hasher.update(&nonce.to_le_bytes());
    let mut out = [0u8; 32];
    hasher.finalize(&mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_is_32_bytes_and_nonce_bound() {
        let self_id = [7u8; 32];
        let a = canonical_quorum_message(2, &self_id, 4, 0);
        let b = canonical_quorum_message(2, &self_id, 4, 1);
        assert_ne!(a, b);
        assert_eq!(a.len(), 32);
    }

    #[test]
    fn message_is_not_a_change_account_digest() {
        let self_id = [7u8; 32];
        let members = [[9u8; 96]];
        let change = knot_encoding::change_account_message_v3(2, &self_id, 4, 0, &members, 1)
            .expect("change_account digest");
        let msg = canonical_quorum_message(2, &self_id, 4, 0);
        assert_ne!(msg.as_slice(), change.as_slice());
    }
}
