//! Topology-B file / BYO-channel `ProposalBlob` transport (M2).
//!
//! JSON+hex on disk so humans can move the file over Signal/email/USB/scp.
//! The collector is untrusted: every signer recomputes the §4a digest via
//! `gate_blob_for_signing` before adding a `sign_multisig` partial. QR deferred.

use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};
use dusk_bytes::Serializable;
use dusk_core::signatures::bls::{
    MultisigSignature, PublicKey as BlsPublicKey, SecretKey as BlsSecretKey,
};
use multisig_encoding::{
    gate_blob_for_signing, DecodedIntent, PartialSig, ProposalBlob, ProposalIntent,
};
use serde::{Deserialize, Serialize};

use crate::bls;

pub const BLOB_FILE_VERSION: u16 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlobFile {
    pub version: u16,
    pub intent: IntentFile,
    pub signed_digest: String,
    pub threshold: u32,
    pub partials: Vec<PartialFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentFile {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartialFile {
    pub signer_pk: String,
    pub sig: String,
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

impl BlobFile {
    pub fn from_proposal_blob(blob: &ProposalBlob) -> Self {
        let i = &blob.intent.intent;
        Self {
            version: blob.version,
            intent: IntentFile {
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
        if self.version != BLOB_FILE_VERSION {
            bail!(
                "unsupported blob file version {} (want {BLOB_FILE_VERSION})",
                self.version
            );
        }
        let call_args = hex::decode(self.intent.call_args.trim_start_matches("0x"))
            .context("call_args hex")?;
        let intent = ProposalIntent {
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
                intent,
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
    let intent = ProposalIntent {
        chain_id,
        committee_id,
        nonce,
        target_contract_id: target,
        function_name,
        call_args,
        deadline,
    };
    let signed_digest = intent.digest();
    ProposalBlob {
        version: BLOB_FILE_VERSION,
        intent: DecodedIntent {
            intent,
            human_summary,
        },
        signed_digest,
        threshold,
        partials: Vec::new(),
    }
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
}
