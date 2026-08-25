//! Out-of-band fingerprint / SAS for proposal digests (M3).
//!
//! Signers compare the **full** 32-byte §4a digest via a BIP39-style 24-word
//! mnemonic (256-bit entropy + SHA-256 checksum). Never truncate the digest
//! for human comparison — an attacker can grind a short truncation to find a
//! colliding prefix cheaply enough to matter; the full digest cannot be
//! grinded in any practical time.

use alloc::string::String;
use alloc::vec::Vec;

use sha2::{Digest, Sha256};

const BIP39_ENGLISH: &str = include_str!("../wordlists/bip39_english.txt");

fn wordlist() -> Vec<&'static str> {
    BIP39_ENGLISH.lines().filter(|l| !l.is_empty()).collect()
}

/// Full hex of the digest (`0x` + 64 hex chars). Always prefer comparing this
/// or [`digest_mnemonic`] out-of-band — never a short prefix alone.
pub fn digest_hex(digest: &[u8; 32]) -> String {
    let mut out = String::from("0x");
    for b in digest {
        out.push_str(&alloc::format!("{b:02x}"));
    }
    out
}

/// BIP39-style 24-word mnemonic over the full 32-byte digest.
///
/// Entropy = digest bytes; checksum = first 8 bits of SHA-256(digest);
/// 264 bits → 24 × 11-bit word indices into the English BIP39 list.
pub fn digest_mnemonic(digest: &[u8; 32]) -> String {
    let words = wordlist();
    assert_eq!(words.len(), 2048);

    let mut hash = Sha256::digest(digest);
    let checksum_byte = hash[0];
    hash.fill(0); // best-effort wipe of stack copy (not secret, but tidy)

    // 32 bytes entropy + 1 checksum byte → 33 bytes; we only need the high
    // 8 bits of the checksum byte (BIP39 CS length = ENT/32 = 8).
    let mut bits = Vec::with_capacity(33);
    bits.extend_from_slice(digest);
    bits.push(checksum_byte);

    let mut indices = Vec::with_capacity(24);
    let mut acc: u32 = 0;
    let mut acc_bits: u32 = 0;
    for byte in bits.iter().take(33) {
        acc = (acc << 8) | u32::from(*byte);
        acc_bits += 8;
        while acc_bits >= 11 && indices.len() < 24 {
            acc_bits -= 11;
            let idx = (acc >> acc_bits) & 0x7ff;
            indices.push(idx as usize);
        }
    }
    assert_eq!(indices.len(), 24);

    let mut out = String::new();
    for (i, idx) in indices.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        out.push_str(words[*idx]);
    }
    out
}

/// Grouped decimal safety-number style rendering of the **full** digest
/// (16 groups of 5 decimal digits from successive big-endian u32 chunks of
/// the 32 bytes — covers all 256 bits; not a truncation).
pub fn digest_safety_number(digest: &[u8; 32]) -> String {
    let mut parts = Vec::with_capacity(8);
    for chunk in digest.chunks_exact(4) {
        let n = u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        // 5 decimal digits with leading zeros (mod 100_000 would truncate —
        // we print the full u32 as zero-padded 10 digits instead so every
        // bit is visible; split as 5+5 for readability).
        let s = alloc::format!("{n:010}");
        parts.push(alloc::format!("{} {}", &s[..5], &s[5..]));
    }
    parts.join("  ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bip39_wordlist_is_2048_words() {
        assert_eq!(wordlist().len(), 2048);
    }

    /// BIP39 Trezor vectors: ENT=256 all-zero → known 24-word mnemonic.
    #[test]
    fn bip39_golden_vector_ent256_all_zero() {
        let entropy = [0u8; 32];
        let expected = [
            "abandon", "abandon", "abandon", "abandon", "abandon", "abandon", "abandon", "abandon",
            "abandon", "abandon", "abandon", "abandon", "abandon", "abandon", "abandon", "abandon",
            "abandon", "abandon", "abandon", "abandon", "abandon", "abandon", "abandon", "art",
        ]
        .join(" ");
        assert_eq!(digest_mnemonic(&entropy), expected);
    }

    #[test]
    fn mnemonic_is_24_words_and_stable() {
        let d = [0x11u8; 32];
        let m1 = digest_mnemonic(&d);
        let m2 = digest_mnemonic(&d);
        assert_eq!(m1, m2);
        assert_eq!(m1.split_whitespace().count(), 24);
    }

    #[test]
    fn distinct_digests_distinct_mnemonics() {
        let a = [0x01u8; 32];
        let mut b = [0x01u8; 32];
        b[31] = 0x02;
        assert_ne!(digest_mnemonic(&a), digest_mnemonic(&b));
        assert_ne!(digest_hex(&a), digest_hex(&b));
        assert_ne!(digest_safety_number(&a), digest_safety_number(&b));
    }

    #[test]
    fn hex_covers_full_digest() {
        let d = [0xab; 32];
        let h = digest_hex(&d);
        assert_eq!(h.len(), 2 + 64);
        assert!(h.starts_with("0x"));
    }
}
