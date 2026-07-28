//! Topology-B file / BYO-channel blob transport (M2).
//!
//! JSON+hex on disk so humans can move the file over Signal/email/USB/scp.
//! Discriminated by `kind`:
//! - `proposals` — §4a / `sign_multisig` shape (v1 blobs with no `kind` load as this)
//! - `pm_council_resolve` — prediction-market council resolve (v2)
//!
//! The collector is untrusted: every signer recomputes the kind-appropriate
//! digest (`gate_blob_for_signing` / `gate_pm_blob_for_signing`) before adding
//! a partial. QR deferred.

use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};
use dusk_bytes::Serializable;
use dusk_core::signatures::bls::{
    MultisigSignature, PublicKey as BlsPublicKey, SecretKey as BlsSecretKey,
    Signature as BlsSignature,
};
use multisig_encoding::{
    gate_blob_for_signing, DecodedIntent, PartialSig, ProposalBlob, ProposalIntent,
};
use serde::{Deserialize, Deserializer, Serialize};
use tiny_keccak::{Hasher, Keccak};

use crate::bls;
use crate::pm_resolve_types::{CouncilSigEntry, ResolveArgs};

/// Wire version for `kind=proposals` blobs (legacy §4a).
pub const BLOB_FILE_VERSION: u16 = 1;
/// Wire version for `kind=pm_council_resolve` blobs.
pub const PM_BLOB_FILE_VERSION: u16 = 2;

/// Domain tag for PM council-resolve digests — must match
/// `prediction-market`'s `DOMAIN_COUNCIL_RESOLVE` / `council_resolve_message`.
pub const DOMAIN_COUNCIL_RESOLVE: &[u8] =
    b"sme-platform.prediction-market.council-resolve.v2";

/// Outer blob discriminator. Missing `kind` on the wire deserializes as
/// [`BlobKind::Proposals`] (v1 compatibility).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BlobKind {
    #[default]
    Proposals,
    PmCouncilResolve,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct BlobFile {
    pub version: u16,
    #[serde(default)]
    pub kind: BlobKind,
    pub intent: IntentFile,
    pub signed_digest: String,
    pub threshold: u32,
    #[serde(default)]
    pub partials: Vec<PartialFile>,
}

/// Kind-discriminated intent. Serialized untagged (fields only); the outer
/// `kind` selects which shape is expected on deserialize.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum IntentFile {
    Proposals(ProposalsIntentFile),
    PmCouncilResolve(PmCouncilResolveIntentFile),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProposalsIntentFile {
    pub chain_id: u64,
    pub committee_id: u64,
    pub nonce: u64,
    pub target_contract_id: String,
    pub function_name: String,
    pub call_args: String,
    pub deadline: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub human_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PmCouncilResolveIntentFile {
    pub market_id: u64,
    pub winning_outcome: u8,
    pub pm_contract_id: String,
    pub registry_account_id: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub human_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PartialFile {
    pub signer_pk: String,
    pub sig: String,
}

#[derive(Deserialize)]
struct BlobFileDe {
    version: u16,
    #[serde(default)]
    kind: BlobKind,
    intent: serde_json::Value,
    signed_digest: String,
    threshold: u32,
    #[serde(default)]
    partials: Vec<PartialFile>,
}

impl<'de> Deserialize<'de> for BlobFile {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = BlobFileDe::deserialize(deserializer)?;
        let intent = match raw.kind {
            BlobKind::Proposals => IntentFile::Proposals(
                serde_json::from_value(raw.intent).map_err(serde::de::Error::custom)?,
            ),
            BlobKind::PmCouncilResolve => IntentFile::PmCouncilResolve(
                serde_json::from_value(raw.intent).map_err(serde::de::Error::custom)?,
            ),
        };
        Ok(BlobFile {
            version: raw.version,
            kind: raw.kind,
            intent,
            signed_digest: raw.signed_digest,
            threshold: raw.threshold,
            partials: raw.partials,
        })
    }
}

fn hex32(s: &str) -> Result<[u8; 32]> {
    let bytes = hex::decode(s.trim_start_matches("0x")).context("expected hex")?;
    bytes
        .as_slice()
        .try_into()
        .map_err(|_| anyhow::anyhow!("expected 32 bytes, got {}", bytes.len()))
}

fn hex96(s: &str) -> Result<[u8; 96]> {
    let bytes = hex::decode(s.trim_start_matches("0x")).context("expected hex")?;
    bytes
        .as_slice()
        .try_into()
        .map_err(|_| anyhow::anyhow!("expected 96 bytes, got {}", bytes.len()))
}

fn hex48(s: &str) -> Result<[u8; 48]> {
    let bytes = hex::decode(s.trim_start_matches("0x")).context("expected hex")?;
    bytes
        .as_slice()
        .try_into()
        .map_err(|_| anyhow::anyhow!("expected 48 bytes, got {}", bytes.len()))
}

fn encode_hex(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}

/// Content-addressed collector id = lowercase hex of the 32-byte digest (no `0x`).
pub fn digest_id(digest_hex: &str) -> String {
    digest_hex.trim_start_matches("0x").to_ascii_lowercase()
}

/// `keccak256(DOMAIN_V2 || contract_id[32] || registry_account_id_le64 ||
/// threshold_le32 || market_id_le64 || winning_outcome_u8)` — byte-for-byte
/// match with the monolith's private `council_resolve_message`.
pub fn council_resolve_digest(
    pm_contract_id: &[u8; 32],
    registry_account_id: u64,
    threshold: u32,
    market_id: u64,
    winning_outcome: u8,
) -> [u8; 32] {
    let mut hasher = Keccak::v256();
    hasher.update(DOMAIN_COUNCIL_RESOLVE);
    hasher.update(pm_contract_id);
    hasher.update(&registry_account_id.to_le_bytes());
    hasher.update(&threshold.to_le_bytes());
    hasher.update(&market_id.to_le_bytes());
    hasher.update(&[winning_outcome]);
    let mut out = [0u8; 32];
    hasher.finalize(&mut out);
    out
}

/// Build a local v2 `pm_council_resolve` blob (unsigned).
pub fn create_pm_blob_file(
    market_id: u64,
    winning_outcome: u8,
    pm_contract_id: [u8; 32],
    registry_account_id: u64,
    threshold: u32,
    human_summary: Option<String>,
) -> BlobFile {
    let signed_digest = council_resolve_digest(
        &pm_contract_id,
        registry_account_id,
        threshold,
        market_id,
        winning_outcome,
    );
    BlobFile {
        version: PM_BLOB_FILE_VERSION,
        kind: BlobKind::PmCouncilResolve,
        intent: IntentFile::PmCouncilResolve(PmCouncilResolveIntentFile {
            market_id,
            winning_outcome,
            pm_contract_id: encode_hex(&pm_contract_id),
            registry_account_id,
            human_summary,
        }),
        signed_digest: encode_hex(&signed_digest),
        threshold,
        partials: Vec::new(),
    }
}

/// Anti-blind-signing gate for `kind=pm_council_resolve`: recompute the
/// council-resolve.v2 digest from intent + blob threshold and refuse if
/// ≠ `signed_digest`.
pub fn gate_pm_blob_for_signing(file: &BlobFile) -> Result<[u8; 32]> {
    match (&file.kind, &file.intent) {
        (BlobKind::PmCouncilResolve, IntentFile::PmCouncilResolve(intent)) => {
            if file.version != PM_BLOB_FILE_VERSION {
                bail!(
                    "unsupported pm_council_resolve blob version {} (want {PM_BLOB_FILE_VERSION})",
                    file.version
                );
            }
            let pm_bytes = hex32(&intent.pm_contract_id)?;
            let expected = council_resolve_digest(
                &pm_bytes,
                intent.registry_account_id,
                file.threshold,
                intent.market_id,
                intent.winning_outcome,
            );
            let got = hex32(&file.signed_digest)?;
            if got != expected {
                bail!(
                    "REFUSING: signed_digest does not match recomputed council-resolve.v2 digest"
                );
            }
            Ok(expected)
        }
        _ => bail!("not a pm_council_resolve blob (wrong kind or intent shape)"),
    }
}

/// Print canonical PM intent (never trusts `human_summary`) and gate the digest.
pub fn print_pm_canonical_intent(file: &BlobFile) -> Result<[u8; 32]> {
    let digest = gate_pm_blob_for_signing(file)?;
    let IntentFile::PmCouncilResolve(intent) = &file.intent else {
        bail!("not a pm_council_resolve blob");
    };
    println!("=== pm council resolve intent (canonical; do not trust human_summary) ===");
    println!("  market_id: {}", intent.market_id);
    println!("  winning_outcome: {}", intent.winning_outcome);
    println!("  pm_contract_id: {}", intent.pm_contract_id);
    println!("  registry_account_id: {}", intent.registry_account_id);
    println!("  threshold: {}", file.threshold);
    println!("  digest: 0x{}", hex::encode(digest));
    println!("=== out-of-band fingerprint (compare with co-signers) ===");
    println!("  hex: {}", multisig_encoding::digest_hex(&digest));
    println!(
        "  mnemonic (24 BIP39 words): {}",
        multisig_encoding::digest_mnemonic(&digest)
    );
    println!(
        "  safety-number: {}",
        multisig_encoding::digest_safety_number(&digest)
    );
    println!("  partials: {}", file.partials.len());
    if let Some(s) = &intent.human_summary {
        println!("  human_summary (untrusted): {s}");
    }
    Ok(digest)
}

/// Add one secure `sign` partial after gating. Refuses duplicate signers.
/// PM council path uses ordinary `BlsSignature` (not `sign_multisig`).
pub fn add_pm_partial(
    file: &mut BlobFile,
    sk: &BlsSecretKey,
    pk: &BlsPublicKey,
) -> Result<[u8; 32]> {
    let digest = print_pm_canonical_intent(file)?;
    let pk_hex = encode_hex(&pk.to_bytes());
    if file
        .partials
        .iter()
        .any(|p| p.signer_pk.trim_start_matches("0x").eq_ignore_ascii_case(pk_hex.trim_start_matches("0x")))
    {
        bail!("this signer already has a partial in the blob");
    }
    let sig = bls::sign(sk, &digest);
    file.partials.push(PartialFile {
        signer_pk: pk_hex,
        sig: encode_hex(&sig.to_bytes()),
    });
    Ok(digest)
}

/// Gate locally before pushing any blob kind to the collector.
pub fn gate_blob_file_for_push(file: &BlobFile) -> Result<[u8; 32]> {
    match file.kind {
        BlobKind::Proposals => {
            let proposal = file.to_proposal_blob()?;
            print_canonical_intent(&proposal)
        }
        BlobKind::PmCouncilResolve => print_pm_canonical_intent(file),
    }
}

/// Build on-chain `ResolveArgs` from a gated PM blob. Refuses if partials
/// are below the blob's client-side threshold.
pub fn build_pm_resolve_args(file: &BlobFile) -> Result<ResolveArgs> {
    let _digest = gate_pm_blob_for_signing(file)?;
    let IntentFile::PmCouncilResolve(intent) = &file.intent else {
        bail!("not a pm_council_resolve blob");
    };
    if (file.partials.len() as u32) < file.threshold {
        bail!(
            "partials {} below threshold {} — refusing submit",
            file.partials.len(),
            file.threshold
        );
    }
    let mut quorum_sigs = Vec::with_capacity(file.partials.len());
    for p in &file.partials {
        let pk_bytes = hex96(&p.signer_pk)?;
        let signer = BlsPublicKey::from_bytes(&pk_bytes)
            .map_err(|e| anyhow::anyhow!("invalid signer_pk in partial: {e:?}"))?;
        let sig_bytes = hex48(&p.sig)?;
        let signature = BlsSignature::from_bytes(&sig_bytes)
            .map_err(|e| anyhow::anyhow!("invalid BlsSignature in partial: {e:?}"))?;
        quorum_sigs.push(CouncilSigEntry { signer, signature });
    }
    Ok(ResolveArgs {
        market_id: intent.market_id,
        winning_outcome: intent.winning_outcome,
        quorum_sigs,
    })
}

impl BlobFile {
    pub fn from_proposal_blob(blob: &ProposalBlob) -> Self {
        let i = &blob.intent.intent;
        Self {
            version: blob.version,
            kind: BlobKind::Proposals,
            intent: IntentFile::Proposals(ProposalsIntentFile {
                chain_id: i.chain_id,
                committee_id: i.committee_id,
                nonce: i.nonce,
                target_contract_id: encode_hex(&i.target_contract_id),
                function_name: i.function_name.clone(),
                call_args: encode_hex(&i.call_args),
                deadline: i.deadline,
                human_summary: blob.intent.human_summary.clone(),
            }),
            signed_digest: encode_hex(&blob.signed_digest),
            threshold: blob.threshold,
            partials: blob
                .partials
                .iter()
                .map(|p| PartialFile {
                    signer_pk: encode_hex(&p.signer_pk),
                    sig: encode_hex(&p.sig),
                })
                .collect(),
        }
    }

    pub fn to_proposal_blob(&self) -> Result<ProposalBlob> {
        if self.kind != BlobKind::Proposals {
            bail!(
                "cannot convert kind={:?} blob to §4a ProposalBlob",
                self.kind
            );
        }
        if self.version != BLOB_FILE_VERSION {
            bail!(
                "unsupported blob file version {} (want {BLOB_FILE_VERSION})",
                self.version
            );
        }
        let IntentFile::Proposals(intent) = &self.intent else {
            bail!("kind=proposals blob has non-proposals intent shape");
        };
        let call_args =
            hex::decode(intent.call_args.trim_start_matches("0x")).context("call_args hex")?;
        let proposal_intent = ProposalIntent {
            chain_id: intent.chain_id,
            committee_id: intent.committee_id,
            nonce: intent.nonce,
            target_contract_id: hex32(&intent.target_contract_id)?,
            function_name: intent.function_name.clone(),
            call_args,
            deadline: intent.deadline,
        };
        let mut partials = Vec::with_capacity(self.partials.len());
        for p in &self.partials {
            partials.push(PartialSig {
                signer_pk: hex96(&p.signer_pk)?,
                sig: hex::decode(p.sig.trim_start_matches("0x")).context("partial sig hex")?,
            });
        }
        Ok(ProposalBlob {
            version: self.version,
            intent: DecodedIntent {
                intent: proposal_intent,
                human_summary: intent.human_summary.clone(),
            },
            signed_digest: hex32(&self.signed_digest)?,
            threshold: self.threshold,
            partials,
        })
    }
}

pub fn read_file(path: &Path) -> Result<BlobFile> {
    let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_str(&text).context("parsing ProposalBlob JSON")
}

pub fn write_file(path: &Path, file: &BlobFile) -> Result<()> {
    let text = serde_json::to_string_pretty(file).context("serializing ProposalBlob JSON")?;
    fs::write(path, format!("{text}\n")).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

pub fn create_blob(
    chain_id: u64,
    committee_id: u64,
    nonce: u64,
    target: [u8; 32],
    function_name: String,
    call_args: Vec<u8>,
    deadline: u64,
    threshold: u32,
    human_summary: Option<String>,
) -> ProposalBlob {
    build_proposal_blob(
        chain_id,
        committee_id,
        nonce,
        target,
        function_name,
        call_args,
        deadline,
        threshold,
        human_summary,
    )
    .expect("create_blob inputs stay within encode caps")
}

fn build_proposal_blob(
    chain_id: u64,
    committee_id: u64,
    nonce: u64,
    target: [u8; 32],
    function_name: String,
    call_args: Vec<u8>,
    deadline: u64,
    threshold: u32,
    human_summary: Option<String>,
) -> Result<ProposalBlob> {
    let intent = ProposalIntent {
        chain_id,
        committee_id,
        nonce,
        target_contract_id: target,
        function_name,
        call_args,
        deadline,
    };
    let signed_digest = intent.digest().context("proposal digest")?;
    Ok(ProposalBlob {
        version: BLOB_FILE_VERSION,
        intent: DecodedIntent {
            intent,
            human_summary,
        },
        signed_digest,
        threshold,
        partials: Vec::new(),
    })
}

/// Print canonical intent (never trusts `human_summary`) and gate the digest.
pub fn print_canonical_intent(blob: &ProposalBlob) -> Result<[u8; 32]> {
    let digest = gate_blob_for_signing(blob).map_err(|_| {
        anyhow::anyhow!("REFUSING: signed_digest does not match recomputed §4a digest")
    })?;
    let i = &blob.intent.intent;
    println!("=== intent (canonical; do not trust human_summary) ===");
    println!("  chain_id: {}", i.chain_id);
    println!("  committee_id: {}", i.committee_id);
    println!("  nonce: {}", i.nonce);
    println!("  target: 0x{}", hex::encode(i.target_contract_id));
    println!("  function: {}", i.function_name);
    println!("  call_args: 0x{}", hex::encode(&i.call_args));
    println!("  deadline: {}", i.deadline);
    println!("  digest: 0x{}", hex::encode(digest));
    println!("=== out-of-band fingerprint (compare with co-signers) ===");
    println!("  hex: {}", multisig_encoding::digest_hex(&digest));
    println!("  mnemonic (24 BIP39 words): {}", multisig_encoding::digest_mnemonic(&digest));
    println!(
        "  safety-number: {}",
        multisig_encoding::digest_safety_number(&digest)
    );
    println!("  threshold: {}", blob.threshold);
    println!("  partials: {}", blob.partials.len());
    if let Some(s) = &blob.intent.human_summary {
        println!("  human_summary (untrusted): {s}");
    }
    Ok(digest)
}

/// Add one `sign_multisig` partial after gating. Refuses duplicate signers.
pub fn add_partial(
    blob: &mut ProposalBlob,
    sk: &BlsSecretKey,
    pk: &BlsPublicKey,
) -> Result<[u8; 32]> {
    let digest = print_canonical_intent(blob)?;
    let pk_bytes = pk.to_bytes();
    if blob.partials.iter().any(|p| p.signer_pk == pk_bytes) {
        bail!("this signer already has a partial in the blob");
    }
    let sig = bls::sign_multisig(sk, pk, &digest);
    blob.partials.push(PartialSig {
        signer_pk: pk_bytes,
        sig: sig.to_bytes().to_vec(),
    });
    Ok(digest)
}

/// Aggregate all partials into one `MultisigSignature` + ordered signer keys.
pub fn aggregate_partials(
    blob: &ProposalBlob,
) -> Result<(Vec<BlsPublicKey>, MultisigSignature, [u8; 32])> {
    let digest = gate_blob_for_signing(blob).map_err(|_| {
        anyhow::anyhow!("REFUSING: signed_digest does not match recomputed §4a digest")
    })?;
    if blob.partials.is_empty() {
        bail!("no partials to aggregate");
    }
    if (blob.partials.len() as u32) < blob.threshold {
        bail!(
            "partials {} below threshold {}",
            blob.partials.len(),
            blob.threshold
        );
    }
    let mut keys = Vec::with_capacity(blob.partials.len());
    let mut sigs = Vec::with_capacity(blob.partials.len());
    for p in &blob.partials {
        let pk = BlsPublicKey::from_bytes(&p.signer_pk)
            .map_err(|e| anyhow::anyhow!("invalid signer_pk in partial: {e:?}"))?;
        let sig_bytes: [u8; 48] = p
            .sig
            .as_slice()
            .try_into()
            .map_err(|_| anyhow::anyhow!("partial sig must be 48 bytes"))?;
        let sig = MultisigSignature::from_bytes(&sig_bytes)
            .map_err(|e| anyhow::anyhow!("invalid MultisigSignature bytes: {e:?}"))?;
        keys.push(pk);
        sigs.push(sig);
    }
    let aggregate_sig = bls::aggregate(&sigs);
    Ok((keys, aggregate_sig, digest))
}

#[cfg(test)]
mod tests {
    use super::*;
    use dusk_core::signatures::bls::SecretKey as BlsSecretKey;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    fn keypair(rng: &mut StdRng) -> (BlsSecretKey, BlsPublicKey) {
        let sk = BlsSecretKey::random(rng);
        let pk = BlsPublicKey::from(&sk);
        (sk, pk)
    }

    #[test]
    fn file_round_trip_preserves_fields() {
        let blob = create_blob(
            1,
            7,
            3,
            [0xab; 32],
            "set_service".into(),
            b"\x00\x01".to_vec(),
            1000,
            2,
            Some("hint".into()),
        );
        let file = BlobFile::from_proposal_blob(&blob);
        assert_eq!(file.kind, BlobKind::Proposals);
        let back = file.to_proposal_blob().unwrap();
        assert_eq!(back.signed_digest, blob.signed_digest);
        assert_eq!(back.intent.intent.function_name, "set_service");
        assert_eq!(back.threshold, 2);
    }

    #[test]
    fn two_of_three_sign_and_aggregate() {
        let rng = &mut StdRng::seed_from_u64(42);
        let (sk1, pk1) = keypair(rng);
        let (sk2, pk2) = keypair(rng);
        let (_sk3, _pk3) = keypair(rng);

        let mut blob = create_blob(
            1,
            0,
            0,
            [0x11; 32],
            "noop".into(),
            Vec::new(),
            0,
            2,
            None,
        );
        add_partial(&mut blob, &sk1, &pk1).unwrap();
        add_partial(&mut blob, &sk2, &pk2).unwrap();
        assert_eq!(blob.partials.len(), 2);
        let (keys, _agg, digest) = aggregate_partials(&blob).unwrap();
        assert_eq!(keys.len(), 2);
        assert_eq!(digest, blob.signed_digest);
        assert!(add_partial(&mut blob, &sk1, &pk1).is_err());
    }

    #[test]
    fn mismatched_digest_refuses_sign() {
        let rng = &mut StdRng::seed_from_u64(7);
        let (sk, pk) = keypair(rng);
        let mut blob = create_blob(1, 0, 0, [0; 32], "x".into(), Vec::new(), 0, 1, None);
        blob.signed_digest[0] ^= 0xff;
        assert!(add_partial(&mut blob, &sk, &pk).is_err());
    }

    #[test]
    fn pm_blob_json_round_trip() {
        let file = create_pm_blob_file(
            42,
            1,
            [0xab; 32],
            7,
            2,
            Some("resolve market 42 → YES".into()),
        );
        let json = serde_json::to_string_pretty(&file).unwrap();
        assert!(json.contains("\"kind\": \"pm_council_resolve\""));
        assert!(json.contains("\"version\": 2"));
        let back: BlobFile = serde_json::from_str(&json).unwrap();
        assert_eq!(back.kind, BlobKind::PmCouncilResolve);
        assert_eq!(back.version, PM_BLOB_FILE_VERSION);
        assert_eq!(back.threshold, 2);
        assert_eq!(back.signed_digest, file.signed_digest);
        match &back.intent {
            IntentFile::PmCouncilResolve(i) => {
                assert_eq!(i.market_id, 42);
                assert_eq!(i.winning_outcome, 1);
                assert_eq!(i.registry_account_id, 7);
                assert_eq!(i.human_summary.as_deref(), Some("resolve market 42 → YES"));
            }
            IntentFile::Proposals(_) => panic!("expected pm intent"),
        }
        let expected = council_resolve_digest(&[0xab; 32], 7, 2, 42, 1);
        assert_eq!(hex32(&back.signed_digest).unwrap(), expected);
    }

    #[test]
    fn v1_proposals_blob_without_kind_loads_as_proposals() {
        let digest = format!("0x{}", "11".repeat(32));
        let json = format!(
            r#"{{
                "version": 1,
                "intent": {{
                    "chain_id": 1,
                    "committee_id": 0,
                    "nonce": 0,
                    "target_contract_id": "0x{}",
                    "function_name": "noop",
                    "call_args": "0x",
                    "deadline": 0
                }},
                "signed_digest": "{digest}",
                "threshold": 1,
                "partials": []
            }}"#,
            "22".repeat(32)
        );
        let file: BlobFile = serde_json::from_str(&json).unwrap();
        assert_eq!(file.kind, BlobKind::Proposals);
        assert_eq!(file.version, 1);
        match file.intent {
            IntentFile::Proposals(i) => assert_eq!(i.function_name, "noop"),
            IntentFile::PmCouncilResolve(_) => panic!("expected proposals intent"),
        }
    }

    #[test]
    fn pm_gate_binds_contract_account_and_threshold() {
        let file = create_pm_blob_file(1, 0, [0xaa; 32], 9, 2, None);
        assert!(gate_pm_blob_for_signing(&file).is_ok());

        // Tamper pm_contract_id without updating digest → refuse.
        let mut bad = file.clone();
        if let IntentFile::PmCouncilResolve(ref mut i) = bad.intent {
            i.pm_contract_id = encode_hex(&[0xbb; 32]);
        }
        assert!(gate_pm_blob_for_signing(&bad).is_err());

        // Tamper registry_account_id → refuse.
        let mut bad = file.clone();
        if let IntentFile::PmCouncilResolve(ref mut i) = bad.intent {
            i.registry_account_id = 99;
        }
        assert!(gate_pm_blob_for_signing(&bad).is_err());

        // Tamper threshold → refuse.
        let mut bad = file.clone();
        bad.threshold = 3;
        assert!(gate_pm_blob_for_signing(&bad).is_err());
    }

    #[test]
    fn pm_gate_refuses_tampered_digest() {
        let mut file = create_pm_blob_file(1, 0, [0; 32], 0, 1, None);
        assert!(gate_pm_blob_for_signing(&file).is_ok());
        // Flip a nibble in the hex digest without breaking length.
        let mut chars: Vec<char> = file.signed_digest.chars().collect();
        let idx = chars.len() - 1;
        chars[idx] = if chars[idx] == '0' { '1' } else { '0' };
        file.signed_digest = chars.into_iter().collect();
        assert!(gate_pm_blob_for_signing(&file).is_err());
    }

    #[test]
    fn pm_add_partial_uses_secure_sign_and_refuses_dup() {
        let rng = &mut StdRng::seed_from_u64(99);
        let (sk1, pk1) = keypair(rng);
        let (sk2, pk2) = keypair(rng);
        let mut file = create_pm_blob_file(5, 1, [0x11; 32], 3, 2, None);
        add_pm_partial(&mut file, &sk1, &pk1).unwrap();
        add_pm_partial(&mut file, &sk2, &pk2).unwrap();
        assert_eq!(file.partials.len(), 2);
        // BlsSignature is 48 bytes.
        let sig_bytes = hex::decode(file.partials[0].sig.trim_start_matches("0x")).unwrap();
        assert_eq!(sig_bytes.len(), 48);
        assert!(add_pm_partial(&mut file, &sk1, &pk1).is_err());
    }

    #[test]
    fn pm_add_partial_refuses_tampered_digest() {
        let rng = &mut StdRng::seed_from_u64(8);
        let (sk, pk) = keypair(rng);
        let mut file = create_pm_blob_file(1, 0, [0; 32], 0, 1, None);
        let mut chars: Vec<char> = file.signed_digest.chars().collect();
        let idx = chars.len() - 1;
        chars[idx] = if chars[idx] == '0' { '1' } else { '0' };
        file.signed_digest = chars.into_iter().collect();
        assert!(add_pm_partial(&mut file, &sk, &pk).is_err());
    }

    #[test]
    fn pm_build_resolve_args_requires_threshold() {
        let rng = &mut StdRng::seed_from_u64(11);
        let (sk, pk) = keypair(rng);
        let mut file = create_pm_blob_file(9, 0, [0x22; 32], 1, 2, None);
        add_pm_partial(&mut file, &sk, &pk).unwrap();
        assert!(build_pm_resolve_args(&file).is_err());
        let (sk2, pk2) = keypair(rng);
        add_pm_partial(&mut file, &sk2, &pk2).unwrap();
        let args = build_pm_resolve_args(&file).unwrap();
        assert_eq!(args.market_id, 9);
        assert_eq!(args.winning_outcome, 0);
        assert_eq!(args.quorum_sigs.len(), 2);
    }
}
