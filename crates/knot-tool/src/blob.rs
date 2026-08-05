//! Topology-B file / BYO-channel blob transport (M2).
//!
//! JSON+hex on disk so humans can move the file over Signal/email/USB/scp.
//! Discriminated by `kind` (`proposals` — §4a / `sign_multisig` shape; v1 blobs
//! with no `kind` load as this).
//!
//! The collector is untrusted: every signer recomputes the digest via
//! [`gate_blob`] before adding a partial. QR deferred.

use std::fs;
use std::io::Write;
use std::path::Path;

use anyhow::{bail, Context, Result};
use dusk_bytes::Serializable;
use dusk_core::signatures::bls::{
    MultisigSignature, PublicKey as BlsPublicKey, SecretKey as BlsSecretKey,
};
use knot_encoding::{
    DecodedIntent, EncodingError, PartialSig, ProposalBlob, ProposalIntent,
};
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Deserializer, Serialize};

use crate::bls;

/// Caller-supplied proposal uniquifier (§2.6). Default CSPRNG `u64`.
pub fn random_proposal_nonce() -> u64 {
    let mut buf = [0u8; 8];
    OsRng.fill_bytes(&mut buf);
    u64::from_le_bytes(buf)
}

/// Resolve CLI/RPC nonce: explicit value or CSPRNG default.
pub fn resolve_proposal_nonce(cli_nonce: Option<u64>) -> u64 {
    cli_nonce.unwrap_or_else(random_proposal_nonce)
}

/// Typed gate errors (L14) — distinguish digest mismatch from encoding limits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateError {
    DigestMismatch,
    Encoding(EncodingError),
}

impl core::fmt::Display for GateError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            GateError::DigestMismatch => {
                write!(f, "signed_digest does not match recomputed digest")
            }
            GateError::Encoding(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for GateError {}

fn gate_error_to_anyhow(err: GateError) -> anyhow::Error {
    match err {
        GateError::DigestMismatch => {
            anyhow::anyhow!("REFUSING: signed_digest does not match recomputed §4a digest")
        }
        GateError::Encoding(e) => anyhow::anyhow!("REFUSING: {e}"),
    }
}

/// Signer-side anti-blind-signing gate with typed errors (L14).
pub fn gate_blob(blob: &ProposalBlob) -> Result<[u8; 32], GateError> {
    let _ = blob.intent.human_summary.as_ref();
    let got = blob.intent.intent.digest().map_err(GateError::Encoding)?;
    if &got == &blob.signed_digest {
        Ok(got)
    } else {
        Err(GateError::DigestMismatch)
    }
}

/// Threshold source for M8 aggregate guard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThresholdGuard {
    pub required: u32,
    /// `true` when fetched from the registry; `false` when using blob-declared value offline.
    pub verified: bool,
}

impl ThresholdGuard {
    pub fn verified(threshold: u32) -> Self {
        Self {
            required: threshold,
            verified: true,
        }
    }

    pub fn unverified_blob(threshold: u32) -> Self {
        Self {
            required: threshold,
            verified: false,
        }
    }
}

#[cfg(target_os = "macos")]
fn full_fsync(f: &fs::File) -> std::io::Result<()> {
    use std::os::unix::io::AsRawFd;
    let rc = unsafe { libc::fcntl(f.as_raw_fd(), libc::F_FULLFSYNC) };
    if rc == -1 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(target_os = "macos"))]
fn full_fsync(f: &fs::File) -> std::io::Result<()> {
    f.sync_all()
}

/// Atomic blob write (L8) — tmp + rename + directory fsync per IMPLEMENTATION §3.2.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let dir = path
        .parent()
        .context("blob path has no parent directory")?;
    fs::create_dir_all(dir)?;
    let tmp = path.with_extension("tmp");

    let mut f = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&tmp)
        .with_context(|| format!("creating {}", tmp.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600))?;
    }
    f.write_all(bytes)?;
    f.sync_all()?;
    full_fsync(&f)?;
    drop(f);

    fs::rename(&tmp, path).with_context(|| format!("renaming {} -> {}", tmp.display(), path.display()))?;

    let dfd = fs::File::open(dir).with_context(|| format!("opening {}", dir.display()))?;
    dfd.sync_all()?;
    full_fsync(&dfd)?;
    Ok(())
}
pub const BLOB_FILE_VERSION: u16 = 1;

/// Outer blob discriminator. Missing `kind` on the wire deserializes as
/// [`BlobKind::Proposals`] (v1 compatibility).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BlobKind {
    #[default]
    Proposals,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct BlobFile {
    pub version: u16,
    #[serde(default)]
    pub kind: BlobKind,
    pub intent: ProposalsIntentFile,
    pub signed_digest: String,
    pub threshold: u32,
    #[serde(default)]
    pub partials: Vec<PartialFile>,
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
        if raw.kind != BlobKind::Proposals {
            return Err(serde::de::Error::custom(format!(
                "unsupported blob kind {:?} in knot-tool (proposals only; PM council-resolve lives in wen pm-council-tool)",
                raw.kind
            )));
        }
        let intent: ProposalsIntentFile =
            serde_json::from_value(raw.intent).map_err(serde::de::Error::custom)?;
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

fn encode_hex(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}

/// Content-addressed collector id = lowercase hex of the 32-byte digest (no `0x`).
pub fn digest_id(digest_hex: &str) -> String {
    digest_hex.trim_start_matches("0x").to_ascii_lowercase()
}

/// Gate locally before pushing any blob to the collector.
pub fn gate_blob_file_for_push(file: &BlobFile) -> Result<[u8; 32]> {
    let proposal = file.to_proposal_blob()?;
    print_canonical_intent(&proposal)
}

impl BlobFile {
    pub fn from_proposal_blob(blob: &ProposalBlob) -> Self {
        let i = &blob.intent.intent;
        Self {
            version: blob.version,
            kind: BlobKind::Proposals,
            intent: ProposalsIntentFile {
                chain_id: i.chain_id,
                committee_id: i.committee_id,
                nonce: i.nonce,
                target_contract_id: encode_hex(&i.target_contract_id),
                function_name: i.function_name.clone(),
                call_args: encode_hex(&i.call_args),
                deadline: i.deadline,
                human_summary: blob.intent.human_summary.clone(),
            },
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
        let call_args =
            hex::decode(self.intent.call_args.trim_start_matches("0x")).context("call_args hex")?;
        let proposal_intent = ProposalIntent {
            chain_id: self.intent.chain_id,
            committee_id: self.intent.committee_id,
            nonce: self.intent.nonce,
            target_contract_id: hex32(&self.intent.target_contract_id)?,
            function_name: self.intent.function_name.clone(),
            call_args,
            deadline: self.intent.deadline,
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
                human_summary: self.intent.human_summary.clone(),
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
    write_atomic(path, format!("{text}\n").as_bytes())
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
    let digest = gate_blob(blob).map_err(gate_error_to_anyhow)?;
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
    println!("  hex: {}", knot_encoding::digest_hex(&digest));
    println!("  mnemonic (24 BIP39 words): {}", knot_encoding::digest_mnemonic(&digest));
    println!(
        "  safety-number: {}",
        knot_encoding::digest_safety_number(&digest)
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

fn check_partial_count(count: usize, guard: ThresholdGuard) -> Result<()> {
    if (count as u32) < guard.required {
        if guard.verified {
            bail!(
                "REFUSING: partials {count} below registry threshold {}",
                guard.required
            );
        }
        bail!(
            "partials {count} below blob-declared threshold {} (unverified locally; the chain enforces the real threshold)",
            guard.required
        );
    }
    Ok(())
}

/// Aggregate verified partials into one `MultisigSignature` + ordered signer keys.
///
/// `threshold` is the M8 guard: use [`ThresholdGuard::verified`] when fetched from
/// the registry, or [`ThresholdGuard::unverified_blob`] when offline.
pub fn aggregate_partials(
    blob: &ProposalBlob,
    threshold: ThresholdGuard,
) -> Result<(Vec<BlsPublicKey>, MultisigSignature, [u8; 32])> {
    let digest = gate_blob(blob).map_err(gate_error_to_anyhow)?;
    if blob.partials.is_empty() {
        bail!("no partials to aggregate");
    }
    let mut keys = Vec::with_capacity(blob.partials.len());
    let mut sigs = Vec::with_capacity(blob.partials.len());
    for p in &blob.partials {
        let pk = match BlsPublicKey::from_bytes(&p.signer_pk) {
            Ok(pk) => pk,
            Err(e) => {
                eprintln!("dropping partial with invalid signer_pk: {e:?}");
                continue;
            }
        };
        let sig_bytes: [u8; 48] = match p.sig.as_slice().try_into() {
            Ok(bytes) => bytes,
            Err(_) => {
                eprintln!(
                    "dropping partial from signer 0x{}: sig must be 48 bytes",
                    hex::encode(pk.to_bytes())
                );
                continue;
            }
        };
        let sig = match MultisigSignature::from_bytes(&sig_bytes) {
            Ok(sig) => sig,
            Err(e) => {
                eprintln!(
                    "dropping partial from signer 0x{}: invalid MultisigSignature bytes: {e:?}",
                    hex::encode(pk.to_bytes())
                );
                continue;
            }
        };
        if !bls::verify_multisig(&pk, &digest, &sig) {
            eprintln!(
                "dropping invalid partial from signer 0x{}",
                hex::encode(pk.to_bytes())
            );
            continue;
        }
        keys.push(pk);
        sigs.push(sig);
    }
    if sigs.is_empty() {
        bail!("no valid partials to aggregate after local verification");
    }
    check_partial_count(sigs.len(), threshold)?;
    let aggregate_sig = bls::aggregate(&sigs).map_err(|e| anyhow::anyhow!("{e}"))?;
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
        )
        .unwrap();
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
        )
        .unwrap();
        add_partial(&mut blob, &sk1, &pk1).unwrap();
        add_partial(&mut blob, &sk2, &pk2).unwrap();
        assert_eq!(blob.partials.len(), 2);
        let (keys, _agg, digest) =
            aggregate_partials(&blob, ThresholdGuard::unverified_blob(blob.threshold)).unwrap();
        assert_eq!(keys.len(), 2);
        assert_eq!(digest, blob.signed_digest);
        assert!(add_partial(&mut blob, &sk1, &pk1).is_err());
    }

    #[test]
    fn mismatched_digest_refuses_sign() {
        let rng = &mut StdRng::seed_from_u64(7);
        let (sk, pk) = keypair(rng);
        let mut blob = create_blob(1, 0, 0, [0; 32], "x".into(), Vec::new(), 0, 1, None).unwrap();
        blob.signed_digest[0] ^= 0xff;
        assert!(add_partial(&mut blob, &sk, &pk).is_err());
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
        assert_eq!(file.intent.function_name, "noop");
    }

    #[test]
    fn pm_council_resolve_kind_rejected_on_load() {
        let json = r#"{
            "version": 2,
            "kind": "pm_council_resolve",
            "intent": {
                "market_id": 1,
                "winning_outcome": 0,
                "pm_contract_id": "0x00",
                "registry_account_id": 0
            },
            "signed_digest": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "threshold": 1,
            "partials": []
        }"#;
        let err = serde_json::from_str::<BlobFile>(json).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("unsupported blob kind") || msg.contains("pm_council_resolve"),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn csprng_nonce_differs_from_explicit() {
        let explicit = resolve_proposal_nonce(Some(42));
        assert_eq!(explicit, 42);
        let a = random_proposal_nonce();
        let b = random_proposal_nonce();
        assert_ne!(a, b, "CSPRNG nonces should almost always differ");
    }

    #[test]
    fn gate_blob_typed_encoding_error() {
        let mut blob = create_blob(1, 0, 0, [0; 32], "x".into(), Vec::new(), 0, 1, None).unwrap();
        blob.intent.intent.function_name = "x".repeat((u32::MAX as usize) + 1);
        assert!(matches!(
            gate_blob(&blob),
            Err(GateError::Encoding(EncodingError::FieldTooLarge { .. }))
        ));
    }

    #[test]
    fn aggregate_drops_invalid_partial() {
        let rng = &mut StdRng::seed_from_u64(99);
        let (sk1, pk1) = keypair(rng);
        let (sk2, pk2) = keypair(rng);
        let mut blob = create_blob(1, 0, 0, [0x22; 32], "noop".into(), Vec::new(), 0, 2, None).unwrap();
        add_partial(&mut blob, &sk1, &pk1).unwrap();
        add_partial(&mut blob, &sk2, &pk2).unwrap();
        blob.partials[0].sig[0] ^= 0xff;
        let err = aggregate_partials(&blob, ThresholdGuard::unverified_blob(2)).unwrap_err();
        assert!(
            err.to_string().contains("below blob-declared threshold"),
            "unexpected: {err}"
        );
    }

    #[test]
    fn verified_threshold_uses_refusing_label() {
        let rng = &mut StdRng::seed_from_u64(11);
        let (sk1, pk1) = keypair(rng);
        let mut blob = create_blob(1, 0, 0, [0x33; 32], "noop".into(), Vec::new(), 0, 2, None).unwrap();
        add_partial(&mut blob, &sk1, &pk1).unwrap();
        let err = aggregate_partials(&blob, ThresholdGuard::verified(2)).unwrap_err();
        assert!(err.to_string().contains("REFUSING"), "unexpected: {err}");
    }

    #[test]
    fn write_atomic_round_trip() {
        let dir = std::env::temp_dir().join(format!("knot-blob-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("proposal.json");
        let blob = create_blob(1, 0, 7, [0x44; 32], "noop".into(), Vec::new(), 0, 1, None).unwrap();
        write_file(&path, &BlobFile::from_proposal_blob(&blob)).unwrap();
        let loaded = read_file(&path).unwrap();
        assert_eq!(loaded.intent.nonce, 7);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
