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
    fn testnet_evm_chain_id_matches_data() {
        assert_eq!(testnet_evm_chain_id().expect("testnet has DuskEVM"), 745);
    }
}
