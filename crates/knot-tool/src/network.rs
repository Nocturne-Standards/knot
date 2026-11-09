// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Nocturne Standards

//! Dusk testnet facts (node URL, chain id, labels, blocklist) from the vendored
//! `@dusk-network/networks` data in `data/dusk-networks/` (MPL-2.0; see the
//! README there). Dusk publishes these values; only the fields this tool reads
//! are typed, and unknown fields are ignored so a newer file still parses.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::Deserialize;

const TESTNET_JSON: &str = include_str!("../data/dusk-networks/testnet.json");

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Network {
    pub name: String,
    pub chain_id: u64,
    pub urls: Urls,
    pub contracts: Contracts,
    pub dusk_evm: Option<DuskEvm>,
    pub display: Display,
}

#[derive(Debug, Deserialize)]
pub struct Urls {
    pub node: String,
}

#[derive(Debug, Deserialize)]
pub struct Contracts {
    pub transfer: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuskEvm {
    pub chain_id: u64,
}

#[derive(Debug, Deserialize)]
pub struct Display {
    /// Names of contracts (by hex id) and accounts (by base58 key).
    pub labels: BTreeMap<String, String>,
    /// Accounts the Dusk apps do not send to.
    pub blocklist: Vec<Blocked>,
}

#[derive(Debug, Deserialize)]
pub struct Blocked {
    /// Base58 public account.
    pub account: String,
    pub reason: BlockReason,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BlockReason {
    Compromised,
    Scam,
    /// A reason added upstream after this tool was written.
    #[serde(other)]
    Other,
}

impl Network {
    /// The display name of a contract (hex id) or account (base58 key), if Dusk names it.
    pub fn label_for(&self, id_or_account: &str) -> Option<&str> {
        self.display.labels.get(id_or_account).map(String::as_str)
    }
}

/// The Dusk testnet entry. Every chain call in this tool targets testnet.
pub fn testnet() -> &'static Network {
    static TESTNET: OnceLock<Network> = OnceLock::new();
    // The data is fixed at build time and covered by the tests, so a parse
    // failure means a broken vendored file.
    TESTNET
        .get_or_init(|| serde_json::from_str(TESTNET_JSON).expect("vendored testnet.json parses"))
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
