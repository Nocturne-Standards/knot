//! Talks to deployed testnet contracts two ways:
//!
//! - **Writes**: shell out to `rusk-wallet` (see `scripts/wire-contract.sh`).
//! - **Reads**: direct RUES HTTP with raw rkyv bodies
//!   (`Content-Type: application/octet-stream`).
//!
//! Supports `knot-registry` and `knot-proposals` ids from a local
//! `deployments/testnet.json` pin file (see `NOCTURNE_DEPLOYMENTS`).
//! `--network testnet` is hard-coded.

use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result, bail};
use bytecheck::CheckBytes;
use rkyv::validation::validators::DefaultValidator;
use rkyv::{Archive, Deserialize, Infallible, Serialize};

const TESTNET_RUES_BASE: &str = "https://testnet.nodes.dusk.network";
/// Accepted by current testnet (1.7.x); mirrors `agent-pay-lp`.
const RUSK_VERSION: &str = "1.0.0";

#[derive(Clone, Copy, Debug)]
pub enum Contract {
    Registry,
    Proposals,
}

impl Contract {
    fn json_key(self) -> &'static str {
        match self {
            Contract::Registry => "multisig-registry",
            Contract::Proposals => "multisig-proposals",
        }
    }
}

#[cfg(feature = "deployments-crate")]
fn deployments() -> Result<nocturne_deployments::DeploymentsFile> {
    let start = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    nocturne_deployments::load_from(&start).with_context(|| {
        format!(
            "could not load deployments/testnet.json from {} \
             (set NOCTURNE_DEPLOYMENTS or place deployments/testnet.json)",
            start.display()
        )
    })
}

#[cfg(not(feature = "deployments-crate"))]
fn deployments() -> Result<crate::deployments::DeploymentsFile> {
    let start = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    crate::deployments::load_from(&start).with_context(|| {
        format!(
            "could not load deployments/testnet.json from {} \
             (set NOCTURNE_DEPLOYMENTS or place deployments/testnet.json)",
            start.display()
        )
    })
}

/// Chain id for v3 digests on testnet (`init_chain_id=2` in deploy-history).
#[allow(dead_code)]
pub const DIGEST_CHAIN_ID: u64 = 2;

#[allow(dead_code)]
pub fn digest_chain_id() -> u64 {
    std::env::var("KNOT_CHAIN_ID")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DIGEST_CHAIN_ID)
}

pub fn contract_self_id_bytes(which: Contract) -> Result<[u8; 32]> {
    let hex = contract_id_hex(which)?;
    let bytes = hex::decode(hex.trim_start_matches("0x"))
        .with_context(|| format!("contract id hex for {:?}", which))?;
    bytes
        .as_slice()
        .try_into()
        .map_err(|_| anyhow::anyhow!("contract id must be 32 bytes"))
}

pub fn contract_id_hex(which: Contract) -> Result<String> {
    let file = deployments()?;
    let key = which.json_key();
    file.contract_id(key).map(str::to_string).with_context(|| {
        format!(
            "no {key}.current.contract_id in {} — deploy the contract and update the pin file",
            file.path().display()
        )
    })
}

pub fn encode<A>(args: &A) -> Result<Vec<u8>>
where
    A: for<'b> Serialize<rkyv::ser::serializers::AllocSerializer<256>>,
{
    let bytes = rkyv::to_bytes::<_, 256>(args)
        .map_err(|e| anyhow::anyhow!("rkyv serialize failed: {e}"))?;
    Ok(bytes.to_vec())
}

pub fn decode<R>(bytes: &[u8]) -> Result<R>
where
    R: Archive,
    R::Archived: Deserialize<R, Infallible> + for<'b> CheckBytes<DefaultValidator<'b>>,
{
    let mut aligned = rkyv::AlignedVec::with_capacity(bytes.len());
    aligned.extend_from_slice(bytes);
    let archived = rkyv::check_archived_root::<R>(&aligned)
        .map_err(|e| anyhow::anyhow!("rkyv validate failed: {e}"))?;
    archived
        .deserialize(&mut Infallible)
        .map_err(|_| anyhow::anyhow!("rkyv deserialize failed"))
}

pub async fn query_contract_id<R>(
    contract_id_hex: &str,
    fn_name: &str,
    args_bytes: Vec<u8>,
) -> Result<R>
where
    R: Archive,
    R::Archived: Deserialize<R, Infallible> + for<'b> CheckBytes<DefaultValidator<'b>>,
{
    let id = contract_id_hex.trim_start_matches("0x");
    let url = format!("{TESTNET_RUES_BASE}/on/contracts:{id}/{fn_name}");
    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .header("Content-Type", "application/octet-stream")
        .header("rusk-version", RUSK_VERSION)
        .body(args_bytes)
        .send()
        .await
        .with_context(|| format!("POST {url}"))?;
    let status = resp.status();
    let body = resp.bytes().await?.to_vec();
    if !status.is_success() {
        bail!(
            "query {fn_name} failed ({status}): {}",
            String::from_utf8_lossy(&body)
        );
    }
    decode::<R>(&body)
}

pub async fn query_contract<R>(which: Contract, fn_name: &str, args_bytes: Vec<u8>) -> Result<R>
where
    R: Archive,
    R::Archived: Deserialize<R, Infallible> + for<'b> CheckBytes<DefaultValidator<'b>>,
{
    let id = contract_id_hex(which)?;
    query_contract_id(&id, fn_name, args_bytes).await
}

/// Free read against `knot-registry`.
pub async fn query<R>(fn_name: &str, args_bytes: Vec<u8>) -> Result<R>
where
    R: Archive,
    R::Archived: Deserialize<R, Infallible> + for<'b> CheckBytes<DefaultValidator<'b>>,
{
    query_contract(Contract::Registry, fn_name, args_bytes).await
}

pub struct WriteResult {
    pub stdout: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteOutcome {
    Panic,
    Ok,
    Unknown,
}

/// Strip CSI/OSC ANSI sequences so progress spinners don't garble the UI log.
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            out.push(c);
            continue;
        }
        match chars.peek().copied() {
            Some('[') => {
                chars.next();
                for n in chars.by_ref() {
                    if n.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            Some(']') => {
                chars.next();
                for n in chars.by_ref() {
                    if n == '\u{7}' || n == '\u{1b}' {
                        break;
                    }
                }
            }
            Some(_) => {
                chars.next();
            }
            None => {}
        }
    }
    out
}

/// rusk-wallet redraws progress with `\r`; keep the last segment per line so
/// the captured log is readable end-to-end instead of a half-overwritten mess.
pub fn normalize_wallet_log(raw: &str) -> String {
    let stripped = strip_ansi(raw);
    let mut lines = Vec::new();
    for chunk in stripped.split('\n') {
        let last = chunk.split('\r').next_back().unwrap_or("").trim_end();
        lines.push(last.to_string());
    }
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}

pub fn classify_write(log: &str) -> WriteOutcome {
    if log.contains("Panic:") || log.contains("panic:") {
        WriteOutcome::Panic
    } else if log.contains("Transaction propagated")
        || log.contains("Transaction sent")
        || log.contains("included into a block")
        || log.contains("Preverify success")
    {
        WriteOutcome::Ok
    } else {
        WriteOutcome::Unknown
    }
}

/// Human status for the UI status row.
pub fn tx_status_label(outcome: WriteOutcome, log: &str) -> &'static str {
    match outcome {
        WriteOutcome::Panic => "failed",
        WriteOutcome::Ok => {
            if log.contains("included into a block") {
                "confirmed"
            } else if log.contains("Transaction propagated") {
                "propagated"
            } else {
                // Preverify success / transaction sent — submitted but not yet in a block.
                "submitted"
            }
        }
        WriteOutcome::Unknown => "unknown",
    }
}

pub fn panic_line(log: &str) -> Option<String> {
    log.lines()
        .find(|l| l.contains("Panic:") || l.contains("panic:"))
        .map(|l| l.trim().to_string())
}

fn looks_like_tx_hash(s: &str) -> bool {
    (s.len() == 64 || s.len() == 66)
        && s.trim_start_matches("0x")
            .chars()
            .all(|c| c.is_ascii_hexdigit())
        && s.trim_start_matches("0x").len() == 64
}

/// Best-effort tx id from wallet / explorer lines in the captured log.
pub fn extract_tx_hash(log: &str) -> Option<String> {
    for line in log.lines() {
        if let Some(idx) = line.find("?id=") {
            let rest = &line[idx + 4..];
            let hash: String = rest.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
            if looks_like_tx_hash(&hash) {
                return Some(hash);
            }
        }
        for key in ["TX:", "Tx:", "tx:", "hash:", "Hash:"] {
            if let Some(rest) = line.split(key).nth(1) {
                let token = rest
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .trim_matches(|c: char| !c.is_ascii_hexdigit() && c != 'x');
                if looks_like_tx_hash(token) {
                    return Some(token.trim_start_matches("0x").to_string());
                }
            }
        }
    }
    for word in log.split_whitespace() {
        let token = word.trim_matches(|c: char| !c.is_ascii_hexdigit() && c != 'x');
        if looks_like_tx_hash(token) {
            return Some(token.trim_start_matches("0x").to_string());
        }
    }
    None
}

pub fn submit_call_to(which: Contract, fn_name: &str, args_bytes: &[u8]) -> Result<WriteResult> {
    let id = contract_id_hex(which)?;
    let id = id.trim_start_matches("0x").to_ascii_lowercase();
    if id.len() != 64 || !id.chars().all(|c| c.is_ascii_hexdigit()) {
        bail!("contract id must be 32-byte hex, got len={}", id.len());
    }
    let args_hex = hex::encode(args_bytes);

    if std::env::var("RUSK_WALLET_PWD").is_err() {
        bail!(
            "RUSK_WALLET_PWD is not set — set this env var to the rusk-wallet keystore password for gas-paying chain writes on testnet"
        );
    }

    let output = Command::new("rusk-wallet")
        .arg("--network")
        .arg("testnet")
        .arg("contract-call")
        .arg("--contract-id")
        .arg(&id)
        .arg("--fn-name")
        .arg(fn_name)
        .arg("--fn-args")
        .arg(&args_hex)
        .output()
        .context(
            "failed to spawn rusk-wallet — install the rusk-wallet CLI and ensure it is on PATH; set RUSK_WALLET_PWD to the keystore password for testnet chain writes",
        )?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let combined = normalize_wallet_log(&format!("{stdout}\n{stderr}"));
    if !output.status.success() {
        bail!("rusk-wallet contract-call failed:\n{combined}");
    }
    Ok(WriteResult { stdout: combined })
}

/// Broadcast against `knot-registry`.
pub fn submit_call(fn_name: &str, args_bytes: &[u8]) -> Result<WriteResult> {
    submit_call_to(Contract::Registry, fn_name, args_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_cr_progress_and_ansi() {
        let raw = "Attempt\rPreverify success!\n\x1b[32mTransaction propagated!\x1b[0m\n";
        let n = normalize_wallet_log(raw);
        assert!(n.contains("Preverify success!"));
        assert!(n.contains("Transaction propagated!"));
        assert!(!n.contains('\u{1b}'));
        assert!(!n.contains("Attempt"));
    }

    #[test]
    fn extract_hash_from_explorer_url() {
        let log = "open https://apps.testnet.dusk.network/explorer/transactions/transaction/?id=abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789\n";
        assert_eq!(
            extract_tx_hash(log).as_deref(),
            Some("abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789")
        );
    }

    #[test]
    fn tx_status_label_propagated_not_confirmed() {
        assert_eq!(
            classify_write("Transaction propagated!\n"),
            WriteOutcome::Ok
        );
        assert_eq!(
            tx_status_label(WriteOutcome::Ok, "Transaction propagated!"),
            "propagated"
        );
    }

    #[test]
    fn tx_status_label_preverify_is_submitted() {
        assert_eq!(
            tx_status_label(WriteOutcome::Ok, "Preverify success!\n"),
            "submitted"
        );
    }

    #[test]
    fn tx_status_label_sent_is_submitted() {
        assert_eq!(
            tx_status_label(WriteOutcome::Ok, "Transaction sent to network\n"),
            "submitted"
        );
    }

    #[test]
    fn tx_status_label_block_inclusion_is_confirmed() {
        assert_eq!(
            tx_status_label(
                WriteOutcome::Ok,
                "Transaction included into a block at height 42\n"
            ),
            "confirmed"
        );
    }

    #[test]
    fn tx_status_label_panic_is_failed() {
        assert_eq!(
            tx_status_label(WriteOutcome::Panic, "Panic: nope"),
            "failed"
        );
    }

    #[test]
    fn tx_status_label_unknown_outcome() {
        assert_eq!(tx_status_label(WriteOutcome::Unknown, ""), "unknown");
    }
}
