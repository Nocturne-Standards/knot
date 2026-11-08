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
}
