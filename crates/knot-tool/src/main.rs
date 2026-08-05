// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Nocturne Standards

//! `knot-tool` — local signing tool + web UI for exercising
//! `knot-registry` against the real Dusk testnet. See README.md.
//!
//! Every identity's secret key stays inside this process (or the encrypted
//! local keystore file) — the web UI never receives one, and every chain
//! interaction is hard-locked to testnet. See README.md's security section.

extern crate alloc;

mod chain;
mod keystore;
mod proposals_types;
mod registry_types;
mod rpc;

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use dusk_bytes::Serializable;
use dusk_core::abi::ContractId;
use dusk_core::signatures::bls::PublicKey as BlsPublicKey;
use knot_tool::{blob, bls, collector_client, membership, mock_ledger};

use proposals_types::call_types::{
    ApproveArgs, ProposalStatus, ProposalView, ProposeArgs,
};
use registry_types::call_types::{
    AccountMeta, ChangeAccountArgs, CreateAccountArgs, DiagnoseQuorumResult, MultisigAccountView,
    SignatureEntry, VerifyQuorumAggregateArgs, VerifyQuorumArgs,
};

#[derive(Parser)]
#[command(
    name = "knot-tool",
    about = "Local signing tool for knot-registry (testnet only)",
    version
)]
struct Cli {
    /// Identity store path.
    #[arg(long, global = true)]
    store: Option<PathBuf>,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// First-run setup: create the local identity store if missing (prompts
    /// for a new password twice), optionally with a first identity. If the
    /// store already exists, just unlock-checks it and prints a summary.
    Init {
        /// Optional first identity to create when the store is being
        /// created fresh. Ignored (with a note) if the store already exists.
        #[arg(long)]
        name: Option<String>,
    },
    /// Manage local BLS "member" identities.
    Identity {
        #[command(subcommand)]
        cmd: IdentityCmd,
    },
    /// Create or query a knot-registry account.
    Account {
        #[command(subcommand)]
        cmd: AccountCmd,
    },
    /// Individual-signature quorum flow (verify_quorum).
    Quorum {
        #[command(subcommand)]
        cmd: QuorumCmd,
    },
    /// Aggregate-signature quorum flow (verify_quorum_aggregate).
    QuorumAgg {
        #[command(subcommand)]
        cmd: QuorumAggCmd,
    },
    /// Governance: replace an account's members/threshold (change_account).
    ChangeAccount {
        #[command(subcommand)]
        cmd: ChangeAccountCmd,
    },
    /// On-chain propose → approve → finalize (`knot-proposals`).
    Proposal {
        #[command(subcommand)]
        cmd: ProposalCmd,
    },
    /// Topology B: file / BYO-channel ProposalBlob (QR deferred).
    Blob {
        #[command(subcommand)]
        cmd: BlobCmd,
    },
    /// Party-finder roster on the collector (off-chain rendezvous only).
    Party {
        #[command(subcommand)]
        cmd: PartyCmd,
    },
    /// Serve the local web UI + RPC on 127.0.0.1.
    ///
    /// Requires `DEMO_MODE=mock` (in-process MockLedger for account/proposal APIs)
    /// or `DEMO_MODE=testnet` (live chain path). Unset or unknown values refuse start.
    Serve {
        #[arg(long, default_value = "127.0.0.1:8877")]
        bind: String,
    },
}

#[derive(Subcommand)]
enum IdentityCmd {
    New { name: String },
    List,
    /// Print a local identity's public key (base58 + hex) for sharing.
    Export { name: String },
    /// Import a foreign public key as a pk-only identity (cannot sign).
    ImportPk {
        name: String,
        /// Base58 or 96-byte hex compressed BLS public key.
        pk: String,
    },
}

#[derive(Subcommand)]
enum AccountCmd {
    Create {
        /// Member identity name (repeatable, in order).
        #[arg(long = "member", required = true)]
        members: Vec<String>,
        #[arg(long)]
        threshold: u32,
    },
    Query { account_id: u64 },
    /// Free-read scalars only (threshold/nonce/members_len) — no BLS keys.
    Meta { account_id: u64 },
    /// Free-read raw 96-byte member public keys.
    Keys { account_id: u64 },
    /// Free-read next account id the contract will allocate.
    NextId,
    /// Scan registry accounts `0..min(next_id, limit)` and print summaries.
    List {
        #[arg(long, default_value_t = 64)]
        limit: u64,
    },
}

#[derive(Subcommand)]
enum QuorumCmd {
    /// Broadcast verify_quorum as a real transaction (no return value surfaced).
    Submit {
        #[arg(long)]
        account: u64,
        #[arg(long)]
        msg: String,
        /// Interpret --msg as hex instead of UTF-8 text.
        #[arg(long)]
        hex: bool,
        /// Signer identity name (repeatable, must meet the account's threshold).
        #[arg(long = "signer", required = true)]
        signers: Vec<String>,
        /// Required — show fingerprint first, then re-run with `--confirm`.
        #[arg(long)]
        confirm: bool,
    },
    /// Free RUES read of verify_quorum — returns the on-chain bool (diagnostic).
    Check {
        #[arg(long)]
        account: u64,
        #[arg(long)]
        msg: String,
        #[arg(long)]
        hex: bool,
        #[arg(long = "signer", required = true)]
        signers: Vec<String>,
    },
    /// Free-read diagnose_quorum — membership/verify counters + member key dump.
    Diagnose {
        #[arg(long)]
        account: u64,
        #[arg(long)]
        msg: String,
        #[arg(long)]
        hex: bool,
        #[arg(long = "signer", required = true)]
        signers: Vec<String>,
    },
}

#[derive(Subcommand)]
enum QuorumAggCmd {
    /// Broadcast verify_quorum_aggregate as a real transaction.
    Submit {
        #[arg(long)]
        account: u64,
        #[arg(long)]
        msg: String,
        #[arg(long)]
        hex: bool,
        #[arg(long = "signer", required = true)]
        signers: Vec<String>,
        /// Required — show fingerprint first, then re-run with `--confirm`.
        #[arg(long)]
        confirm: bool,
    },
    /// Free RUES read of verify_quorum_aggregate — returns the on-chain bool.
    Check {
        #[arg(long)]
        account: u64,
        #[arg(long)]
        msg: String,
        #[arg(long)]
        hex: bool,
        #[arg(long = "signer", required = true)]
        signers: Vec<String>,
    },
}

#[derive(Subcommand)]
enum ChangeAccountCmd {
    Submit {
        #[arg(long)]
        account: u64,
        #[arg(long = "new-member", required = true)]
        new_members: Vec<String>,
        #[arg(long)]
        new_threshold: u32,
        /// Must be a quorum of the account's *current* members.
        #[arg(long = "signer", required = true)]
        signers: Vec<String>,
        /// Bypass the broken `account` free-read by supplying the nonce
        /// directly (0 for a never-changed account). Diagnostic only —
        /// wrong nonce will fail the on-chain quorum/message check.
        #[arg(long)]
        nonce: Option<u64>,
        /// Required — show fingerprint first, then re-run with `--confirm`.
        #[arg(long)]
        confirm: bool,
    },
}

#[derive(Subcommand)]
enum ProposalCmd {
    /// Owner-only: wire proposals contract to the live registry ContractId.
    InitRegistry,
    /// Open a structured proposal (target + function + args → §4a digest).
    Create {
        #[arg(long)]
        account: u64,
        #[arg(long)]
        target: String,
        #[arg(long)]
        function: String,
        /// Call args as hex (rkyv payload), or empty.
        #[arg(long, default_value = "")]
        args_hex: String,
        #[arg(long, default_value_t = 0)]
        deadline: u64,
        /// Caller uniquifier (§2.12 v3); CSPRNG default when omitted.
        #[arg(long)]
        nonce: Option<u64>,
    },
    /// Approve with one local signing identity (recomputes digest + shows intent).
    Approve {
        #[arg(long)]
        id: u64,
        #[arg(long)]
        signer: String,
        /// Refuse to sign unless this full digest hex matches recomputed intent.
        #[arg(long)]
        expect_digest: Option<String>,
        /// Required — print fingerprint first (omit this flag), then re-run with `--confirm`.
        #[arg(long)]
        confirm: bool,
    },
    /// Free-read proposal view / status.
    Status {
        #[arg(long)]
        id: u64,
    },
    /// Finalize when approvals meet the registry account threshold.
    Finalize {
        #[arg(long)]
        id: u64,
    },
    /// Free-read next proposal id.
    NextId,
}

#[derive(Subcommand)]
enum BlobCmd {
    /// Create an empty ProposalBlob JSON file (no partials yet).
    Create {
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value_t = 1)]
        chain_id: u64,
        #[arg(long)]
        committee_id: u64,
        /// Caller uniquifier (§2.12 v3); CSPRNG default when omitted.
        #[arg(long)]
        nonce: Option<u64>,
        #[arg(long)]
        target: String,
        #[arg(long)]
        function: String,
        #[arg(long, default_value = "")]
        args_hex: String,
        #[arg(long, default_value_t = 0)]
        deadline: u64,
        #[arg(long)]
        threshold: u32,
        #[arg(long)]
        summary: Option<String>,
    },
    /// Recompute digest + print canonical intent (refuses on mismatch).
    Show {
        file: PathBuf,
    },
    /// Gate + add one local `sign_multisig` partial; write updated file.
    ///
    /// Local mode: `--file`/`--out` (unchanged).
    ///
    /// Collector mode: `--collector` + `--id` pull the proposal, sign, and
    /// POST the partial back — `--out` then optionally writes a local copy
    /// of the server's merged blob too.
    Sign {
        #[arg(long)]
        file: Option<PathBuf>,
        #[arg(long)]
        signer: String,
        #[arg(long)]
        out: Option<PathBuf>,
        /// Collector base URL (falls back to KNOT_COLLECTOR_URL). Presence
        /// of this flag (or the env var) switches to collector mode.
        #[arg(long)]
        collector: Option<String>,
        /// Proposal id on the collector — required in collector mode.
        #[arg(long)]
        id: Option<String>,
        /// Required — show fingerprint first, then re-run with `--confirm`.
        #[arg(long)]
        confirm: bool,
    },
    /// Aggregate partials (threshold must be met); print keys + aggregate hex.
    Aggregate {
        file: PathBuf,
    },
    /// Aggregate + submit `verify_quorum_aggregate` as one testnet tx.
    SubmitAgg {
        #[arg(long)]
        file: PathBuf,
        #[arg(long)]
        account: u64,
    },
    /// Print out-of-band full-digest fingerprint (hex + 24-word mnemonic).
    Fingerprint {
        file: PathBuf,
    },
    /// POST a local blob file to the collector; prints the content-addressed id.
    Push {
        #[arg(long)]
        file: PathBuf,
        #[arg(long)]
        collector: Option<String>,
    },
    /// GET a proposal from the collector by id; writes it as a local blob file.
    Pull {
        #[arg(long)]
        id: String,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        collector: Option<String>,
    },
}

#[derive(Subcommand)]
enum PartyCmd {
    /// List the party-finder roster.
    List {
        #[arg(long)]
        collector: Option<String>,
    },
    /// Sign up (or refresh, if `--pk` already present) on the roster.
    Signup {
        #[arg(long)]
        name: String,
        /// Base58 or 96-byte hex compressed BLS public key.
        #[arg(long)]
        pk: String,
        #[arg(long)]
        note: Option<String>,
        #[arg(long)]
        collector: Option<String>,
    },
}

fn prompt_password() -> Result<String> {
    match std::env::var("KNOT_PWD") {
        Ok(pwd) if !pwd.is_empty() => {
            if std::env::var("KNOT_ALLOW_ENV_PWD").as_deref() != Ok("1") {
                bail!(
                    "KNOT_PWD is set but KNOT_ALLOW_ENV_PWD=1 is required to honor it \
                     (refusing env password to reduce casual leakage — set both for scripting)"
                );
            }
            Ok(pwd)
        }
        _ => rpassword::prompt_password("Identity store password: ").context("reading password"),
    }
}

/// Like `prompt_password`, but for creating a brand-new store: prompts twice
/// and refuses a mismatch, so a typo doesn't lock the user out of a store
/// they just created. Skipped (single env var read, no confirmation
/// possible) when scripted via `KNOT_PWD` + `KNOT_ALLOW_ENV_PWD=1`.
fn prompt_new_password() -> Result<String> {
    match std::env::var("KNOT_PWD") {
        Ok(pwd) if !pwd.is_empty() => {
            if std::env::var("KNOT_ALLOW_ENV_PWD").as_deref() != Ok("1") {
                bail!(
                    "KNOT_PWD is set but KNOT_ALLOW_ENV_PWD=1 is required to honor it \
                     (refusing env password to reduce casual leakage — set both for scripting)"
                );
            }
            Ok(pwd)
        }
        _ => {
            let pwd1 = rpassword::prompt_password("New identity store password: ")
                .context("reading password")?;
            let pwd2 = rpassword::prompt_password("Confirm password: ")
                .context("reading password confirmation")?;
            if pwd1 != pwd2 {
                bail!("passwords did not match");
            }
            Ok(pwd1)
        }
    }
}

fn require_cli_confirm(confirm: bool) -> Result<()> {
    if confirm {
        return Ok(());
    }
    bail!(
        "refusing to sign without --confirm (preview the fingerprint first, then re-run with --confirm)"
    );
}

fn print_signing_fingerprint(msg: &[u8]) {
    let (digest_hex, mnemonic, safety) = bls::message_fingerprint_display(msg);
    println!("=== message to sign ===");
    println!("  msg_len: {}", msg.len());
    println!("  msg_hex: 0x{}", hex::encode(msg));
    println!("=== out-of-band fingerprint (compare with co-signers) ===");
    println!("  hex: {digest_hex}");
    println!("  mnemonic (24 BIP39 words): {mnemonic}");
    println!("  safety-number: {safety}");
}

fn print_identity_summary(identities: &[keystore::Identity]) {
    println!(
        "{} identit{} in store:",
        identities.len(),
        if identities.len() == 1 { "y" } else { "ies" }
    );
    for i in identities {
        let kind = if i.is_pk_only() { "pk-only" } else { "signing" };
        println!("  {} [{kind}]: pk = {}", i.name, rpc::bs58_pk(&i.pk));
    }
}

fn load_store(path: &std::path::Path) -> Result<(Vec<keystore::Identity>, String)> {
    let password = prompt_password()?;
    let identities = keystore::load(path, &password)?;
    Ok((identities, password))
}

fn find_identity<'a>(identities: &'a [keystore::Identity], name: &str) -> Result<&'a keystore::Identity> {
    identities
        .iter()
        .find(|i| i.name == name)
        .ok_or_else(|| anyhow::anyhow!("no identity named '{name}' — run `identity new {name}` first"))
}

fn msg_bytes(msg: &str, hex_flag: bool) -> Result<Vec<u8>> {
    if hex_flag {
        Ok(hex::decode(msg.trim_start_matches("0x"))?)
    } else {
        Ok(msg.as_bytes().to_vec())
    }
}

fn print_write_result(label: &str, r: chain::WriteResult) {
    let outcome = chain::classify_write(&r.stdout);
    let tag = match outcome {
        chain::WriteOutcome::Panic => "FAIL (contract panic)",
        chain::WriteOutcome::Ok => "tx included/propagated",
        chain::WriteOutcome::Unknown => "unknown (see log)",
    };
    println!("=== {label}: {tag} ===");
    if let Some(h) = chain::extract_tx_hash(&r.stdout) {
        println!("tx_hash: {h}");
    }
    println!("tx_status: {}", chain::tx_status_label(outcome, &r.stdout));
    if let Some(p) = chain::panic_line(&r.stdout) {
        println!("outcome: {p}");
    }
    println!("--- rusk-wallet log ---");
    println!("{}", r.stdout);
}

/// Free-read verify can return false / sigs_ok=0 on live testnet even when
/// the same secure signatures succeed on mutating paths (`change_account`).
/// Surface counters and warn when membership matches but verifies fail.
async fn print_quorum_free_read(
    label: &str,
    _account: u64,
    args: &VerifyQuorumArgs,
    local_signer_hex: &[String],
) -> Result<()> {
    let bytes = chain::encode(args)?;
    match chain::query::<DiagnoseQuorumResult>("diagnose_quorum", bytes.clone()).await {
        Ok(d) => {
            println!(
                "{label} diagnose: exists={}, threshold={}, members_len={}, member_matches={}, sigs_ok={}",
                d.exists, d.threshold, d.members_len, d.member_matches, d.sigs_ok
            );
            if d.member_matches > 0 && d.sigs_ok == 0 {
                println!(
                    "note: free-read verify looks untrusted here (members matched, sigs_ok=0). \
                     Prefer change_account panic counters or a proposals finalize for crisp demos; \
                     see README Known caveats."
                );
            }
            for (i, k) in d.member_pk_bytes.iter().enumerate() {
                let hex_k = hex::encode(k);
                let local_hit = local_signer_hex.iter().any(|p| p == &hex_k);
                println!("  on-chain member[{i}] {hex_k} local_signer_match={local_hit}");
            }
        }
        Err(e) => println!("{label} diagnose failed: {e}"),
    }
    match chain::query::<bool>("verify_quorum", bytes).await {
        Ok(passed) => println!("{label} check => {passed}"),
        Err(e) => println!("{label} check failed: {e}"),
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let store_path = match cli.store.clone() {
        Some(p) => p,
        None => keystore::resolve_default_path()?,
    };

    match cli.cmd {
        Cmd::Init { name } => {
            if store_path.exists() {
                let (identities, _) = load_store(&store_path)?;
                println!("identity store already exists at {}", store_path.display());
                if let Some(name) = name
                    && !identities.iter().any(|i| i.name == name)
                {
                    println!(
                        "note: store already initialized — run `identity new {name}` to add it"
                    );
                }
                print_identity_summary(&identities);
            } else {
                let password = prompt_new_password()?;
                let mut identities = Vec::new();
                if let Some(name) = &name {
                    let identity = keystore::generate(name);
                    println!("{name}: pk = {}", rpc::bs58_pk(&identity.pk));
                    identities.push(identity);
                }
                keystore::save(&store_path, &password, &identities)?;
                println!("created identity store at {}", store_path.display());
                print_identity_summary(&identities);
            }
        }

        Cmd::Identity { cmd } => match cmd {
            IdentityCmd::New { name } => {
                let (mut identities, password) = load_store(&store_path)?;
                if identities.iter().any(|i| i.name == name) {
                    bail!("identity '{name}' already exists");
                }
                let identity = keystore::generate(&name);
                println!("{name}: pk = {}", rpc::bs58_pk(&identity.pk));
                identities.push(identity);
                keystore::save(&store_path, &password, &identities)?;
            }
            IdentityCmd::List => {
                let (identities, _) = load_store(&store_path)?;
                print_identity_summary(&identities);
            }
            IdentityCmd::Export { name } => {
                let (identities, _) = load_store(&store_path)?;
                let id = find_identity(&identities, &name)?;
                use dusk_bytes::Serializable;
                println!("name: {}", id.name);
                println!("kind: {}", if id.is_pk_only() { "pk-only" } else { "signing" });
                println!("pk_base58: {}", rpc::bs58_pk(&id.pk));
                println!("pk_hex: {}", hex::encode(id.pk.to_bytes()));
            }
            IdentityCmd::ImportPk { name, pk } => {
                let (mut identities, password) = load_store(&store_path)?;
                if identities.iter().any(|i| i.name == name) {
                    bail!("identity '{name}' already exists");
                }
                let parsed = keystore::parse_pk(&pk)?;
                let identity = keystore::from_pk_only(&name, parsed);
                println!("{name} [pk-only]: pk = {}", rpc::bs58_pk(&identity.pk));
                identities.push(identity);
                keystore::save(&store_path, &password, &identities)?;
            }
        },

        Cmd::Account { cmd } => match cmd {
            AccountCmd::Create { members, threshold } => {
                let (identities, _) = load_store(&store_path)?;
                let member_pks: Vec<BlsPublicKey> = members
                    .iter()
                    .map(|name| find_identity(&identities, name).map(|i| i.pk))
                    .collect::<Result<_>>()?;
                let args = CreateAccountArgs { members: member_pks, threshold };
                let bytes = chain::encode(&args)?;
                let result = chain::submit_call("create_account", &bytes)?;
                print_write_result("create_account", result);
            }
            AccountCmd::Query { account_id } => {
                let bytes = chain::encode(&account_id)?;
                let view: Option<MultisigAccountView> = chain::query("account", bytes).await?;
                match view {
                    Some(v) => {
                        println!("account {account_id}: threshold={}, nonce={}", v.threshold, v.nonce);
                        for pk in &v.members {
                            println!("  member: {}", rpc::bs58_pk(pk));
                        }
                    }
                    None => println!("account {account_id}: not found"),
                }
            }
            AccountCmd::Meta { account_id } => {
                let bytes = chain::encode(&account_id)?;
                let meta: Option<AccountMeta> = chain::query("account_meta", bytes).await?;
                match meta {
                    Some(m) => println!(
                        "account_meta {account_id}: threshold={}, nonce={}, members_len={}",
                        m.threshold, m.nonce, m.members_len
                    ),
                    None => println!("account_meta {account_id}: not found"),
                }
            }
            AccountCmd::Keys { account_id } => {
                let bytes = chain::encode(&account_id)?;
                let keys: Option<Vec<Vec<u8>>> = chain::query("member_key_bytes", bytes).await?;
                match keys {
                    Some(keys) => {
                        println!("member_key_bytes {account_id}: {} keys", keys.len());
                        for (i, k) in keys.iter().enumerate() {
                            println!("  [{i}] {}", hex::encode(k));
                        }
                    }
                    None => println!("member_key_bytes {account_id}: not found"),
                }
            }
            AccountCmd::NextId => {
                let bytes = chain::encode(&())?;
                let next: u64 = chain::query("next_account_id", bytes).await?;
                println!("next_account_id => {next}");
            }
            AccountCmd::List { limit } => {
                let scan = limit.clamp(1, 256);
                let bytes = chain::encode(&())?;
                let next: u64 = chain::query("next_account_id", bytes).await?;
                let end = next.min(scan);
                println!("registry accounts 0..{end} (next_account_id={next}):");
                for id in 0..end {
                    let bytes = chain::encode(&id)?;
                    let view: Option<MultisigAccountView> = chain::query("account", bytes).await?;
                    match view {
                        Some(v) => println!(
                            "  [{id}] threshold={} nonce={} members={}",
                            v.threshold,
                            v.nonce,
                            v.members.len()
                        ),
                        None => println!("  [{id}] (empty)"),
                    }
                }
            }
        },

        Cmd::Quorum { cmd } => match cmd {
            QuorumCmd::Submit { account, msg, hex, signers, confirm } => {
                let (identities, _) = load_store(&store_path)?;
                ensure_cli_signers_are_members(account, &identities, &signers).await?;
                let msg_bytes = msg_bytes(&msg, hex)?;
                print_signing_fingerprint(&msg_bytes);
                if signers.len() > 1 {
                    eprintln!(
                        "note: prefer one signer identity per serve process — use separate machines when possible"
                    );
                }
                require_cli_confirm(confirm)?;
                let sigs = build_sigs(&identities, &signers, &msg_bytes)?;
                let local_pks = signer_pk_hexs(&identities, &signers)?;
                let args = VerifyQuorumArgs {
                    account_id: account,
                    msg: msg_bytes,
                    sigs,
                };
                let bytes = chain::encode(&args)?;
                let result = chain::submit_call("verify_quorum", &bytes)?;
                print_write_result("verify_quorum", result);
                println!(
                    "note: verify_quorum returns bool with no event — free-read follow-up below \
                     (may be untrusted on live testnet; see README)."
                );
                print_quorum_free_read("post-submit", account, &args, &local_pks).await?;
            }
            QuorumCmd::Check { account, msg, hex, signers } => {
                let (identities, _) = load_store(&store_path)?;
                ensure_cli_signers_are_members(account, &identities, &signers).await?;
                let msg_bytes = msg_bytes(&msg, hex)?;
                let sigs = build_sigs(&identities, &signers, &msg_bytes)?;
                let local_pks = signer_pk_hexs(&identities, &signers)?;
                let args = VerifyQuorumArgs {
                    account_id: account,
                    msg: msg_bytes,
                    sigs,
                };
                print_quorum_free_read("verify_quorum", account, &args, &local_pks).await?;
            }
            QuorumCmd::Diagnose { account, msg, hex, signers } => {
                let (identities, _) = load_store(&store_path)?;
                ensure_cli_signers_are_members(account, &identities, &signers).await?;
                let msg_bytes = msg_bytes(&msg, hex)?;
                let sigs = build_sigs(&identities, &signers, &msg_bytes)?;
                let local_pks = signer_pk_hexs(&identities, &signers)?;
                let args = VerifyQuorumArgs {
                    account_id: account,
                    msg: msg_bytes,
                    sigs,
                };
                print_quorum_free_read("diagnose_quorum", account, &args, &local_pks).await?;
            }
        },

        Cmd::QuorumAgg { cmd } => match cmd {
            QuorumAggCmd::Submit { account, msg, hex, signers, confirm } => {
                let (identities, _) = load_store(&store_path)?;
                ensure_cli_signers_are_members(account, &identities, &signers).await?;
                let msg_bytes = msg_bytes(&msg, hex)?;
                print_signing_fingerprint(&msg_bytes);
                if signers.len() > 1 {
                    eprintln!(
                        "note: prefer one signer identity per serve process — use separate machines when possible"
                    );
                }
                require_cli_confirm(confirm)?;
                let (signer_keys, aggregate_sig) = build_aggregate(&identities, &signers, &msg_bytes)?;
                let args = VerifyQuorumAggregateArgs {
                    account_id: account,
                    msg: msg_bytes,
                    signer_keys,
                    aggregate_sig,
                };
                let bytes = chain::encode(&args)?;
                let result = chain::submit_call("verify_quorum_aggregate", &bytes)?;
                print_write_result("verify_quorum_aggregate", result);
            }
            QuorumAggCmd::Check { account, msg, hex, signers } => {
                let (identities, _) = load_store(&store_path)?;
                ensure_cli_signers_are_members(account, &identities, &signers).await?;
                let msg_bytes = msg_bytes(&msg, hex)?;
                let (signer_keys, aggregate_sig) = build_aggregate(&identities, &signers, &msg_bytes)?;
                let args = VerifyQuorumAggregateArgs {
                    account_id: account,
                    msg: msg_bytes,
                    signer_keys,
                    aggregate_sig,
                };
                let bytes = chain::encode(&args)?;
                let passed: bool = chain::query("verify_quorum_aggregate", bytes).await?;
                println!("verify_quorum_aggregate(account={account}) => {passed}");
            }
        },

        Cmd::ChangeAccount { cmd } => match cmd {
            ChangeAccountCmd::Submit {
                account,
                new_members,
                new_threshold,
                signers,
                nonce,
                confirm,
            } => {
                let (identities, _) = load_store(&store_path)?;
                let new_member_pks: Vec<BlsPublicKey> = new_members
                    .iter()
                    .map(|name| find_identity(&identities, name).map(|i| i.pk))
                    .collect::<Result<_>>()?;

                let nonce = match nonce {
                    Some(n) => {
                        if !mock_ledger::DemoMode::change_account_nonce_override_allowed() {
                            bail!(
                                "change-account --nonce is refused by default (bypasses account \
                                 free-read); set KNOT_ALLOW_CHANGE_ACCOUNT_NONCE=1 for diagnostics only"
                            );
                        }
                        eprintln!("warning: using --nonce {n} bypass (account free-read skipped)");
                        n
                    }
                    None => {
                        let current: Option<MultisigAccountView> =
                            chain::query("account", chain::encode(&account)?).await?;
                        let current = current
                            .ok_or_else(|| anyhow::anyhow!("account {account} not found"))?;
                        current.nonce
                    }
                };

                let registry_self_id =
                    chain::contract_self_id_bytes(chain::Contract::Registry)?;

                let msg = bls::change_account_message(
                    &registry_self_id,
                    account,
                    nonce,
                    &new_member_pks,
                    new_threshold,
                );
                ensure_cli_signers_are_members(account, &identities, &signers).await?;
                print_signing_fingerprint(&msg);
                if signers.len() > 1 {
                    eprintln!(
                        "note: prefer one signer identity per serve process — use separate machines when possible"
                    );
                }
                require_cli_confirm(confirm)?;
                let sigs = build_sigs(&identities, &signers, &msg)?;

                let args = ChangeAccountArgs {
                    account_id: account,
                    new_members: new_member_pks,
                    new_threshold,
                    sigs,
                };
                let bytes = chain::encode(&args)?;
                let result = chain::submit_call("change_account", &bytes)?;
                print_write_result("change_account", result);
            }
        },

        Cmd::Proposal { cmd } => match cmd {
            ProposalCmd::InitRegistry => {
                let registry_hex = chain::contract_id_hex(chain::Contract::Registry)?;
                let bytes_arr: [u8; 32] = hex::decode(registry_hex.trim_start_matches("0x"))?
                    .as_slice()
                    .try_into()
                    .map_err(|_| anyhow::anyhow!("registry contract id must be 32 bytes"))?;
                let cid = ContractId::from_bytes(bytes_arr);
                let bytes = chain::encode(&cid)?;
                let result =
                    chain::submit_call_to(chain::Contract::Proposals, "init_registry", &bytes)?;
                print_write_result("init_registry", result);
            }
            ProposalCmd::Create {
                account,
                target,
                function,
                args_hex,
                deadline,
                nonce,
            } => {
                let target_bytes: [u8; 32] = hex::decode(target.trim_start_matches("0x"))?
                    .as_slice()
                    .try_into()
                    .map_err(|_| anyhow::anyhow!("target must be 32-byte hex ContractId"))?;
                let call_args = if args_hex.is_empty() {
                    Vec::new()
                } else {
                    hex::decode(args_hex.trim_start_matches("0x"))?
                };
                let nonce = blob::resolve_proposal_nonce(nonce);
                let before: u64 = chain::query_contract(
                    chain::Contract::Proposals,
                    "next_proposal_id",
                    chain::encode(&())?,
                )
                .await?;
                let args = ProposeArgs {
                    registry_account_id: account,
                    target: ContractId::from_bytes(target_bytes),
                    function_name: function,
                    call_args,
                    nonce,
                    deadline,
                };
                let bytes = chain::encode(&args)?;
                let result = chain::submit_call_to(chain::Contract::Proposals, "propose", &bytes)?;
                print_write_result("propose", result);
                println!("hint: allocated proposal id should be {before}");
            }
            ProposalCmd::Approve {
                id,
                signer,
                expect_digest,
                confirm,
            } => {
                let (identities, _) = load_store(&store_path)?;
                let identity = find_identity(&identities, &signer)?;
                let sk = identity.require_sk()?;
                let view: Option<ProposalView> = chain::query_contract(
                    chain::Contract::Proposals,
                    "proposal",
                    chain::encode(&id)?,
                )
                .await?;
                let view = view.ok_or_else(|| anyhow::anyhow!("proposal {id} not found"))?;
                if view.status != ProposalStatus::Open {
                    bail!("proposal {id} is not Open");
                }
                let proposals_self_id =
                    chain::contract_self_id_bytes(chain::Contract::Proposals)?;
                let intent = bls::proposal_intent_v3_from_view(
                    &view,
                    bls::digest_chain_id(),
                    &proposals_self_id,
                );
                let digest = knot_encoding::recompute_and_verify_v3(&intent, &view.signed_digest)
                    .map_err(|_| {
                        anyhow::anyhow!(
                            "REFUSING TO SIGN: on-chain digest does not match recomputed intent"
                        )
                    })?;
                if let Some(expected) = expect_digest {
                    let want = hex::decode(expected.trim_start_matches("0x"))?;
                    if want.as_slice() != digest.as_slice() {
                        bail!("REFUSING TO SIGN: digest does not match --expect-digest");
                    }
                }
                println!("=== intent (canonical) ===");
                println!("  chain_id: {}", intent.chain_id);
                println!("  epoch: {}", intent.epoch);
                println!("  committee_id: {}", intent.committee_id);
                println!("  nonce: {}", intent.nonce);
                println!("  target: 0x{}", hex::encode(intent.target_contract_id));
                println!("  function: {}", intent.function_name);
                println!("  call_args: 0x{}", hex::encode(&intent.call_args));
                println!("  deadline: {}", intent.deadline);
                println!("  digest: 0x{}", hex::encode(digest));
                println!("=== out-of-band fingerprint (compare with co-signers) ===");
                println!("  hex: {}", knot_encoding::digest_hex(&digest));
                println!(
                    "  mnemonic (24 BIP39 words): {}",
                    knot_encoding::digest_mnemonic(&digest)
                );
                println!(
                    "  safety-number: {}",
                    knot_encoding::digest_safety_number(&digest)
                );
                require_cli_confirm(confirm)?;
                ensure_cli_signers_are_members(view.registry_account_id, &identities, &[signer.clone()])
                    .await?;
                let signature = bls::sign(sk, &digest);
                let args = ApproveArgs {
                    proposal_id: id,
                    signer: identity.pk,
                    signature,
                };
                let bytes = chain::encode(&args)?;
                let result = chain::submit_call_to(chain::Contract::Proposals, "approve", &bytes)?;
                print_write_result("approve", result);
            }
            ProposalCmd::Status { id } => {
                let view: Option<ProposalView> = chain::query_contract(
                    chain::Contract::Proposals,
                    "proposal",
                    chain::encode(&id)?,
                )
                .await?;
                match view {
                    None => println!("proposal {id}: not found"),
                    Some(v) => {
                        let status = match v.status {
                            ProposalStatus::Open => "Open",
                            ProposalStatus::Executed => "Executed",
                            ProposalStatus::Tombstoned => "Tombstoned",
                        };
                        println!(
                            "proposal {id}: status={status}, committee={}, nonce={}, fn={}, digest=0x{}",
                            v.registry_account_id,
                            v.nonce,
                            v.function_name,
                            hex::encode(v.signed_digest)
                        );
                        println!(
                            "  target=0x{} args_len={} deadline={} approvals={}",
                            hex::encode(v.target.to_bytes()),
                            v.call_args.len(),
                            v.deadline,
                            v.approvals.len()
                        );
                        for pk in &v.approvals {
                            println!("  approval: {}", rpc::bs58_pk(pk));
                        }
                    }
                }
            }
            ProposalCmd::Finalize { id } => {
                let bytes = chain::encode(&id)?;
                let result = chain::submit_call_to(chain::Contract::Proposals, "finalize", &bytes)?;
                print_write_result("finalize", result);
            }
            ProposalCmd::NextId => {
                let next: u64 = chain::query_contract(
                    chain::Contract::Proposals,
                    "next_proposal_id",
                    chain::encode(&())?,
                )
                .await?;
                println!("next_proposal_id => {next}");
            }
        },

        Cmd::Blob { cmd } => match cmd {
            BlobCmd::Create {
                out,
                chain_id,
                committee_id,
                nonce,
                target,
                function,
                args_hex,
                deadline,
                threshold,
                summary,
            } => {
                let target_bytes: [u8; 32] = hex::decode(target.trim_start_matches("0x"))?
                    .as_slice()
                    .try_into()
                    .map_err(|_| anyhow::anyhow!("target must be 32-byte hex ContractId"))?;
                let call_args = if args_hex.is_empty() {
                    Vec::new()
                } else {
                    hex::decode(args_hex.trim_start_matches("0x"))?
                };
                let nonce = blob::resolve_proposal_nonce(nonce);
                let proposal = blob::create_blob(
                    chain_id,
                    committee_id,
                    nonce,
                    target_bytes,
                    function,
                    call_args,
                    deadline,
                    threshold,
                    summary,
                )?;
                blob::print_canonical_intent(&proposal)?;
                blob::write_file(&out, &blob::BlobFile::from_proposal_blob(&proposal))?;
                println!("wrote {}", out.display());
            }
            BlobCmd::Show { file } => {
                let file_blob = blob::read_file(&file)?;
                let proposal = file_blob.to_proposal_blob()?;
                blob::print_canonical_intent(&proposal)?;
            }
            BlobCmd::Sign {
                file,
                signer,
                out,
                collector,
                id,
                confirm,
            } => {
                let (identities, _) = load_store(&store_path)?;
                let identity = find_identity(&identities, &signer)?;
                let sk = identity.require_sk()?;

                let use_collector = collector.is_some()
                    || std::env::var(collector_client::URL_ENV).is_ok();
                if use_collector {
                    let id = id.ok_or_else(|| {
                        anyhow::anyhow!("--id is required in collector mode (with --collector)")
                    })?;
                    if file.is_some() {
                        bail!("--file is ignored in collector mode — use --id instead");
                    }
                    let client = collector_client::CollectorClient::resolve(collector.as_deref())?;
                    let pulled = client.pull(&id).await?;
                    let mut proposal = pulled.to_proposal_blob()?;
                    blob::print_canonical_intent(&proposal)?;
                    require_cli_confirm(confirm)?;
                    blob::add_partial(&mut proposal, sk, &identity.pk)?;
                    let new_partial = proposal
                        .partials
                        .last()
                        .expect("add_partial just pushed one")
                        .clone();
                    let partial_file = blob::PartialFile {
                        signer_pk: format!("0x{}", hex::encode(new_partial.signer_pk)),
                        sig: format!("0x{}", hex::encode(&new_partial.sig)),
                    };
                    let updated = client.append_partial(&id, &partial_file).await?;
                    println!(
                        "added partial from {signer} via collector; partials={}/{}",
                        updated.partials.len(),
                        updated.threshold
                    );
                    if let Some(out) = out {
                        blob::write_file(&out, &updated)?;
                        println!("wrote {}", out.display());
                    }
                } else {
                    let file = file.ok_or_else(|| {
                        anyhow::anyhow!("--file is required in local mode (or use --collector/--id)")
                    })?;
                    let out = out.ok_or_else(|| {
                        anyhow::anyhow!("--out is required in local mode (with --file)")
                    })?;
                    let file_blob = blob::read_file(&file)?;
                    let mut proposal = file_blob.to_proposal_blob()?;
                    blob::print_canonical_intent(&proposal)?;
                    require_cli_confirm(confirm)?;
                    blob::add_partial(&mut proposal, sk, &identity.pk)?;
                    blob::write_file(&out, &blob::BlobFile::from_proposal_blob(&proposal))?;
                    println!(
                        "added partial from {signer}; partials={}/{}",
                        proposal.partials.len(),
                        proposal.threshold
                    );
                    println!("wrote {}", out.display());
                }
            }
            BlobCmd::Aggregate { file } => {
                use dusk_bytes::Serializable;
                let file_blob = blob::read_file(&file)?;
                let proposal = file_blob.to_proposal_blob()?;
                let threshold = threshold_guard_for_blob(&proposal).await;
                let (keys, agg, digest) = blob::aggregate_partials(&proposal, threshold)?;
                println!("digest: 0x{}", hex::encode(digest));
                println!("signers: {}", keys.len());
                for (i, pk) in keys.iter().enumerate() {
                    println!("  [{i}] {}", rpc::bs58_pk(pk));
                }
                println!("aggregate_sig: 0x{}", hex::encode(agg.to_bytes()));
            }
            BlobCmd::SubmitAgg { file, account } => {
                let file_blob = blob::read_file(&file)?;
                let proposal = file_blob.to_proposal_blob()?;
                let threshold = threshold_guard_for_blob(&proposal).await;
                let (signer_keys, aggregate_sig, digest) =
                    blob::aggregate_partials(&proposal, threshold)?;
                let args = VerifyQuorumAggregateArgs {
                    account_id: account,
                    msg: digest.to_vec(),
                    signer_keys,
                    aggregate_sig,
                };
                let bytes = chain::encode(&args)?;
                let result = chain::submit_call("verify_quorum_aggregate", &bytes)?;
                print_write_result("verify_quorum_aggregate (from blob)", result);
            }
            BlobCmd::Fingerprint { file } => {
                let file_blob = blob::read_file(&file)?;
                let proposal = file_blob.to_proposal_blob()?;
                blob::print_canonical_intent(&proposal)?;
            }
            BlobCmd::Push { file, collector } => {
                let file_blob = blob::read_file(&file)?;
                // Gate locally first — refuse to push a blob whose stored
                // digest doesn't match its own intent (kind-appropriate).
                blob::gate_blob_file_for_push(&file_blob)?;
                let client = collector_client::CollectorClient::resolve(collector.as_deref())?;
                let resp = client.push(&file_blob).await?;
                println!("pushed: id={} signed_digest={}", resp.id, resp.signed_digest);
            }
            BlobCmd::Pull { id, out, collector } => {
                let client = collector_client::CollectorClient::resolve(collector.as_deref())?;
                let file_blob = client.pull(&id).await?;
                blob::gate_blob_file_for_push(&file_blob)?;
                blob::write_file(&out, &file_blob)?;
                println!("wrote {}", out.display());
            }
        },

        Cmd::Party { cmd } => match cmd {
            PartyCmd::List { collector } => {
                let client = collector_client::CollectorClient::resolve(collector.as_deref())?;
                let members = client.list_party().await?;
                println!("party roster: {} member(s)", members.len());
                for m in &members {
                    let note = m.note.as_deref().unwrap_or("");
                    println!("  {} pk={} note={note}", m.name, m.pk);
                }
            }
            PartyCmd::Signup { name, pk, note, collector } => {
                let (identities, _) = load_store(&store_path)?;
                let identity = find_identity(&identities, &name)?;
                let sk = identity.require_sk()?;
                let parsed_pk = keystore::parse_pk(&pk)?;
                if identity.pk.to_bytes() != parsed_pk.to_bytes() {
                    bail!("--pk does not match identity '{name}'");
                }
                let pk_hex = format!("0x{}", hex::encode(identity.pk.to_bytes()));
                let sig = bls::party_signup_sig_hex(sk, &identity.pk, &name)?;
                let client = collector_client::CollectorClient::resolve(collector.as_deref())?;
                let member = client
                    .signup_party(&name, &pk_hex, &sig, note.as_deref())
                    .await?;
                println!("signed up: {} pk={}", member.name, member.pk);
            }
        },

        Cmd::Serve { bind } => {
            let mode = mock_ledger::DemoMode::from_env().map_err(anyhow::Error::msg)?;
            eprintln!(
                "starting serve (DEMO_MODE={}) — mock skips rusk-wallet for account/proposal APIs",
                mode.as_str()
            );
            rpc::serve(&bind, store_path).await?;
        }
    }

    Ok(())
}

fn signer_pk_hexs(identities: &[keystore::Identity], signers: &[String]) -> Result<Vec<String>> {
    use dusk_bytes::Serializable;
    signers
        .iter()
        .map(|name| {
            let id = find_identity(identities, name)?;
            Ok(hex::encode(id.pk.to_bytes()))
        })
        .collect()
}

async fn fetch_registry_account(account_id: u64) -> Result<MultisigAccountView> {
    let view: Option<MultisigAccountView> =
        chain::query("account", chain::encode(&account_id)?).await?;
    view.ok_or_else(|| anyhow::anyhow!("account {account_id} not found"))
}

async fn ensure_cli_signers_are_members(
    account_id: u64,
    identities: &[keystore::Identity],
    signers: &[String],
) -> Result<()> {
    let view = fetch_registry_account(account_id).await?;
    let pks: Vec<BlsPublicKey> = signers
        .iter()
        .map(|name| find_identity(identities, name).map(|id| id.pk))
        .collect::<Result<_>>()?;
    membership::ensure_pks_are_members(account_id, &pks, &view).map_err(anyhow::Error::msg)
}

fn build_sigs(
    identities: &[keystore::Identity],
    signers: &[String],
    msg: &[u8],
) -> Result<Vec<SignatureEntry>> {
    signers
        .iter()
        .map(|name| {
            let id = find_identity(identities, name)?;
            let sk = id.require_sk()?;
            Ok(SignatureEntry {
                signer: id.pk,
                signature: bls::sign(sk, msg),
            })
        })
        .collect()
}

fn build_aggregate(
    identities: &[keystore::Identity],
    signers: &[String],
    msg: &[u8],
) -> Result<(Vec<BlsPublicKey>, dusk_core::signatures::bls::MultisigSignature)> {
    let ids: Vec<&keystore::Identity> = signers
        .iter()
        .map(|name| find_identity(identities, name))
        .collect::<Result<_>>()?;
    let mut per_signer_sigs = Vec::with_capacity(ids.len());
    for id in &ids {
        let sk = id.require_sk()?;
        per_signer_sigs.push(bls::sign_multisig(sk, &id.pk, msg));
    }
    let aggregate_sig = bls::aggregate(&per_signer_sigs).map_err(|e| anyhow::anyhow!("{e}"))?;
    let signer_keys: Vec<BlsPublicKey> = ids.iter().map(|i| i.pk).collect();
    Ok((signer_keys, aggregate_sig))
}

async fn fetch_registry_threshold(committee_id: u64) -> Result<u32> {
    let bytes = chain::encode(&committee_id)?;
    let view: Option<MultisigAccountView> = chain::query("account", bytes).await?;
    view.map(|v| v.threshold)
        .ok_or_else(|| anyhow::anyhow!("registry account {committee_id} not found"))
}

async fn threshold_guard_for_blob(proposal: &knot_encoding::ProposalBlob) -> blob::ThresholdGuard {
    let committee_id = proposal.intent.intent.committee_id;
    match fetch_registry_threshold(committee_id).await {
        Ok(t) => blob::ThresholdGuard::verified(t),
        Err(e) => {
            eprintln!(
                "warning: could not fetch registry threshold for committee {committee_id}: {e}; \
                 using blob-declared threshold {} (unverified)",
                proposal.threshold
            );
            blob::ThresholdGuard::unverified_blob(proposal.threshold)
        }
    }
}
