// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Nocturne Standards

//! Dusk testnet facts (node URL, chain id) from the `dusk-networks` crate, the
//! vendored `@dusk-network/networks` data. Dusk publishes these values, so this
//! crate keeps no copy of them.

/// The Dusk testnet entry. Every chain call in this tool targets testnet.
pub fn testnet() -> &'static dusk_networks::Network {
    // The crate embeds testnet at build time; a release without it is a broken release.
    dusk_networks::by_name("testnet").expect("dusk-networks embeds testnet")
}

/// Testnet node base URL (RUES and GraphQL), without a trailing slash.
pub fn testnet_node_url() -> &'static str {
    testnet().urls.node.trim_end_matches('/')
}

/// Testnet chain id.
pub fn testnet_chain_id() -> u64 {
    testnet().chain_id
}

/// DuskEVM chain id on testnet, `Err` if the network data has no DuskEVM entry.
pub fn testnet_evm_chain_id() -> Result<u64, String> {
    testnet()
        .dusk_evm
        .as_ref()
        .map(|evm| evm.chain_id)
        .ok_or_else(|| {
            "network data has no DuskEVM entry for testnet; pass --evm-chain".to_string()
        })
}

/// Notes a signer should see beside an intent: the target's published label,
/// and any blocklisted account whose raw key appears in the call data. The
/// call data is opaque, so this is a byte search, not a decode: it catches a
/// blocked account used as a plain argument and can miss other encodings.
/// Notes inform; the chain still decides.
pub fn intent_notes(target: &[u8; 32], call_args: &[u8]) -> Vec<String> {
    let net = testnet();
    let mut notes = Vec::new();
    if let Some(label) = net.label_for(&hex::encode(target)) {
        notes.push(format!("target is {label}"));
    }
    for blocked in &net.display.blocklist {
        let Ok(raw) = bs58::decode(&blocked.account).into_vec() else {
            continue;
        };
        if raw.is_empty() || !contains(call_args, &raw) {
            continue;
        }
        notes.push(format!(
            "WARNING: call data holds an account on the Dusk blocklist ({:?}): {}",
            blocked.reason, blocked.account
        ));
    }
    notes
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn testnet_values() {
        assert_eq!(testnet().name, "testnet");
        assert_eq!(testnet_chain_id(), 2);
        assert!(testnet_node_url().starts_with("https://"));
        assert!(!testnet_node_url().ends_with('/'));
    }

    #[test]
    fn notes_label_target_and_flag_blocked_account() {
        let net = testnet();
        let blocked = net
            .display
            .blocklist
            .first()
            .expect("testnet blocklist is not empty");
        let raw = bs58::decode(&blocked.account).into_vec().expect("base58");
        let mut args = vec![7u8; 5];
        args.extend_from_slice(&raw);
        let transfer: [u8; 32] = hex::decode(&net.contracts.transfer)
            .unwrap()
            .try_into()
            .unwrap();

        let notes = intent_notes(&transfer, &args);
        assert!(
            notes.iter().any(|n| n.starts_with("target is ")),
            "{notes:?}"
        );
        assert!(notes.iter().any(|n| n.starts_with("WARNING")), "{notes:?}");
        assert!(intent_notes(&[9u8; 32], &[1, 2, 3]).is_empty());
    }

    #[test]
    fn testnet_evm_chain_id_matches_data() {
        assert_eq!(testnet_evm_chain_id().expect("testnet has DuskEVM"), 745);
    }
}
