// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Nocturne Standards

//! DuskEVM helpers for `KnotEvmRoot` (testnet only, not audited yet).
//!
//! Two signing flows, each gathered in one JSON file of public data that members pass around:
//!
//! - **bootstrap**: members sign `evm_bootstrap_message_v1` with plain BLS (`sign`), and anyone submits
//!   `publish_bootstrap_root` on DuskDS. `knot-registry` checks the quorum the same way as every other call.
//! - **action**: members sign `KnotEvmRoot.actionMessage` with `sign_multisig`; the partials are summed into one
//!   aggregate and printed as `checkAction` arguments for an EVM wallet.
//!
//! Message encodings come from `knot-encoding`. The members-root leaf and the EIP-2537 key form follow
//! `evm/src/KnotEvmRoot.sol`; tests check both against `evm/test/vectors/knot_root.json`.

use anyhow::{Context, Result, bail};
use dusk_bls12_381::G2Affine;
use dusk_bytes::Serializable;
use dusk_core::signatures::bls::{
    MultisigPublicKey, MultisigSignature, PublicKey as BlsPublicKey, SecretKey as BlsSecretKey,
    Signature as BlsSignature,
};
use knot_encoding::call_types::{PublishBootstrapRootArgs, SignatureEntry};
use serde::{Deserialize, Serialize};
use tiny_keccak::{Hasher, Keccak};

/// `KnotEvmRoot.MAX_MEMBERS`: the root refuses a larger member set.
pub const MAX_MEMBERS: usize = 16;
/// ABI signature of `checkAction` for `cast send`.
pub const CHECK_ACTION_SIG: &str =
    "checkAction(uint64,bytes32,(bytes32[],uint256,bytes,bytes,bytes))";

// ------------------------------------------------------------------ encodings

fn keccak(parts: &[&[u8]]) -> [u8; 32] {
    let mut k = Keccak::v256();
    for p in parts {
        k.update(p);
    }
    let mut o = [0u8; 32];
    k.finalize(&mut o);
    o
}

/// The 32-byte message `knot-registry::publish_bootstrap_root` checks the quorum against.
pub fn bootstrap_message(
    dusk_chain_id: u64,
    registry_id: &[u8; 32],
    account_id: u64,
    nonce: u64,
    receiver: &[u8; 20],
    members: &[[u8; 96]],
    threshold: u32,
) -> Result<Vec<u8>> {
    knot_encoding::evm_bootstrap_message_v1(
        dusk_chain_id,
        registry_id,
        account_id,
        nonce,
        receiver,
        members,
        threshold,
    )
    .map_err(|e| anyhow::anyhow!("bootstrap message encoding: {e:?}"))
}

/// `KnotEvmRoot.actionMessage(accountId, consumer, actionDigest)` for the root at `root` on `evm_chain_id`
/// (encoding from `knot-encoding`).
#[allow(clippy::too_many_arguments)]
pub fn action_message(
    dusk_chain_id: u64,
    evm_chain_id: u64,
    root: &[u8; 20],
    registry_id: &[u8; 32],
    account_id: u64,
    rotation_nonce: u64,
    consumer: &[u8; 20],
    digest: &[u8; 32],
) -> Vec<u8> {
    knot_encoding::evm_root_action_message_v1(
        dusk_chain_id,
        evm_chain_id,
        root,
        registry_id,
        account_id,
        rotation_nonce,
        consumer,
        digest,
    )
}

fn pad64(be48: &[u8]) -> Vec<u8> {
    let mut v = vec![0u8; 16];
    v.extend_from_slice(be48);
    v
}

/// EIP-2537 G2 encoding (x_c0, x_c1, y_c0, y_c1, 64 bytes each) of a compressed Dusk G2 key.
pub fn g2_eip(pk_c: &[u8; 96]) -> Result<Vec<u8>> {
    let p =
        G2Affine::from_bytes(pk_c).map_err(|_| anyhow::anyhow!("public key does not decode"))?;
    // zcash uncompressed layout: x_c1, x_c0, y_c1, y_c0, with flag bits in the top 3 bits of x_c1 / y_c1.
    let raw = p.to_uncompressed();
    let mut xc1 = raw[0..48].to_vec();
    let xc0 = raw[48..96].to_vec();
    let mut yc1 = raw[96..144].to_vec();
    let yc0 = raw[144..192].to_vec();
    xc1[0] &= 0x1f;
    yc1[0] &= 0x1f;
    Ok([pad64(&xc0), pad64(&xc1), pad64(&yc0), pad64(&yc1)].concat())
}

/// Members-root leaf: `keccak256(pk || pkEip)`.
pub fn leaf(pk_c: &[u8; 96]) -> Result<[u8; 32]> {
    Ok(keccak(&[pk_c, &g2_eip(pk_c)?]))
}

/// `keccak256(leaf_0 || ... || leaf_{n-1})`, the value `KnotEvmRoot.membersRoot` stores.
pub fn members_root(members: &[[u8; 96]]) -> Result<[u8; 32]> {
    let leaves = members.iter().map(leaf).collect::<Result<Vec<_>>>()?;
    Ok(keccak(&[&leaves.concat()]))
}

/// rkyv bytes of `PublishBootstrapRootArgs`, as `knot-registry` reads them.
pub fn encode_publish_args(args: &PublishBootstrapRootArgs) -> Result<Vec<u8>> {
    let bytes = rkyv::to_bytes::<_, 4096>(args)
        .map_err(|e| anyhow::anyhow!("rkyv serialize failed: {e}"))?;
    Ok(bytes.to_vec())
}

// ------------------------------------------------------------------ hex parsing

pub fn parse_hex_array<const N: usize>(s: &str, label: &str) -> Result<[u8; N]> {
    let raw = crate::hex_util::decode_hex(s, label)?;
    raw.as_slice()
        .try_into()
        .map_err(|_| anyhow::anyhow!("{label} must be {N} bytes, got {}", raw.len()))
}

fn hx(b: &[u8]) -> String {
    format!("0x{}", hex::encode(b))
}

mod hex_array {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer, const N: usize>(v: &[u8; N], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&super::hx(v))
    }

    pub fn deserialize<'de, D: Deserializer<'de>, const N: usize>(
        d: D,
    ) -> Result<[u8; N], D::Error> {
        let s = String::deserialize(d)?;
        super::parse_hex_array::<N>(&s, "hex field").map_err(serde::de::Error::custom)
    }
}

mod hex_vec {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &[u8], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&super::hx(v))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let s = String::deserialize(d)?;
        crate::hex_util::decode_hex(&s, "hex field").map_err(serde::de::Error::custom)
    }
}

mod hex_pks {
    use serde::{Deserialize, Deserializer, Serializer, ser::SerializeSeq};

    pub fn serialize<S: Serializer>(v: &[[u8; 96]], s: S) -> Result<S::Ok, S::Error> {
        let mut seq = s.serialize_seq(Some(v.len()))?;
        for pk in v {
            seq.serialize_element(&super::hx(pk))?;
        }
        seq.end()
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<[u8; 96]>, D::Error> {
        let v = Vec::<String>::deserialize(d)?;
        v.iter()
            .map(|s| super::parse_hex_array::<96>(s, "member pk").map_err(serde::de::Error::custom))
            .collect()
    }
}

// ------------------------------------------------------------------ partial file

/// What the members sign. Every field is public.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EvmContext {
    Bootstrap {
        /// knot-registry account nonce the quorum signs at.
        nonce: u64,
        /// `KnotEvmRoot` address the registry is configured to send to.
        #[serde(with = "hex_array")]
        receiver: [u8; 20],
    },
    Action {
        evm_chain_id: u64,
        /// `KnotEvmRoot` address.
        #[serde(with = "hex_array")]
        root: [u8; 20],
        /// `KnotEvmRoot.rotationNonce(accountId)` at signing time.
        rotation_nonce: u64,
        /// `msg.sender` of `checkAction`.
        #[serde(with = "hex_array")]
        consumer: [u8; 20],
        #[serde(with = "hex_array")]
        digest: [u8; 32],
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvmPartial {
    #[serde(with = "hex_array")]
    pub signer_pk: [u8; 96],
    /// 48-byte compressed G1: a plain signature (bootstrap) or a multisig signature (action).
    #[serde(with = "hex_array")]
    pub sig: [u8; 48],
}

/// One signing round, passed between members. Everything except `partials` is the header and must be identical
/// for every signer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvmSigFile {
    pub dusk_chain_id: u64,
    #[serde(with = "hex_array")]
    pub registry_id: [u8; 32],
    pub account_id: u64,
    /// Member keys in account order (= leaf order of the root).
    #[serde(with = "hex_pks")]
    pub members: Vec<[u8; 96]>,
    pub threshold: u32,
    pub context: EvmContext,
    /// Exact bytes signed. Recomputed from the header on every load.
    #[serde(with = "hex_vec")]
    pub message: Vec<u8>,
    pub partials: Vec<EvmPartial>,
}

/// Where in an `EvmQuorum` signers sit: member indices, keys and signatures in leaf order.
pub struct OrderedSigners {
    pub indices: Vec<usize>,
    pub partials: Vec<EvmPartial>,
}

impl EvmSigFile {
    pub fn new(
        dusk_chain_id: u64,
        registry_id: [u8; 32],
        account_id: u64,
        members: Vec<[u8; 96]>,
        threshold: u32,
        context: EvmContext,
    ) -> Result<Self> {
        let mut f = EvmSigFile {
            dusk_chain_id,
            registry_id,
            account_id,
            members,
            threshold,
            context,
            message: Vec::new(),
            partials: Vec::new(),
        };
        f.check_members()?;
        f.message = f.compute_message()?;
        Ok(f)
    }

    pub fn kind(&self) -> &'static str {
        match self.context {
            EvmContext::Bootstrap { .. } => "bootstrap",
            EvmContext::Action { .. } => "action",
        }
    }

    fn check_members(&self) -> Result<()> {
        let n = self.members.len();
        if n == 0 || n > MAX_MEMBERS {
            bail!("account has {n} members; KnotEvmRoot accepts 1..={MAX_MEMBERS}");
        }
        if self.threshold == 0 || self.threshold as usize > n {
            bail!(
                "threshold {} is out of range for {n} members",
                self.threshold
            );
        }
        for (i, pk) in self.members.iter().enumerate() {
            if self.members[..i].contains(pk) {
                bail!("member {i} is a duplicate");
            }
        }
        Ok(())
    }

    fn compute_message(&self) -> Result<Vec<u8>> {
        match &self.context {
            EvmContext::Bootstrap { nonce, receiver } => bootstrap_message(
                self.dusk_chain_id,
                &self.registry_id,
                self.account_id,
                *nonce,
                receiver,
                &self.members,
                self.threshold,
            ),
            EvmContext::Action {
                evm_chain_id,
                root,
                rotation_nonce,
                consumer,
                digest,
            } => Ok(action_message(
                self.dusk_chain_id,
                *evm_chain_id,
                root,
                &self.registry_id,
                self.account_id,
                *rotation_nonce,
                consumer,
                digest,
            )),
        }
    }

    /// Header and every partial check out: message recomputes, signers are distinct members, signatures verify.
    pub fn validate(&self) -> Result<()> {
        self.check_members()?;
        if self.compute_message()? != self.message {
            bail!("file message does not match its header (edited or corrupt file)");
        }
        for (i, p) in self.partials.iter().enumerate() {
            if self.partials[..i]
                .iter()
                .any(|q| q.signer_pk == p.signer_pk)
            {
                bail!("duplicate partial from {}", short_pk(&p.signer_pk));
            }
            self.check_partial(p)?;
        }
        Ok(())
    }

    fn header(&self) -> EvmSigFile {
        let mut h = self.clone();
        h.partials.clear();
        h
    }

    /// Refuse to mix rounds: every header field (account, nonce, receiver/root, members, message...) must match.
    pub fn ensure_same_round(&self, other: &EvmSigFile) -> Result<()> {
        let (a, b) = (self.header(), other.header());
        if a == b {
            return Ok(());
        }
        let mut diffs = Vec::new();
        if a.dusk_chain_id != b.dusk_chain_id {
            diffs.push("dusk_chain_id");
        }
        if a.registry_id != b.registry_id {
            diffs.push("registry_id");
        }
        if a.account_id != b.account_id {
            diffs.push("account_id");
        }
        if a.members != b.members {
            diffs.push("members");
        }
        if a.threshold != b.threshold {
            diffs.push("threshold");
        }
        if a.context != b.context {
            diffs.push("context (kind/nonce/receiver/root/consumer/digest)");
        }
        if a.message != b.message {
            diffs.push("message");
        }
        bail!(
            "file belongs to a different signing round; differs in: {}",
            diffs.join(", ")
        )
    }

    pub fn member_index(&self, pk: &[u8; 96]) -> Option<usize> {
        self.members.iter().position(|m| m == pk)
    }

    /// Refuse a non-member or someone who already signed. Call before asking for the signature.
    pub fn ensure_can_sign(&self, pk: &[u8; 96]) -> Result<()> {
        if self.member_index(pk).is_none() {
            bail!(
                "signer {} is not a member of account {}",
                short_pk(pk),
                self.account_id
            );
        }
        if self.partials.iter().any(|p| &p.signer_pk == pk) {
            bail!("signer {} already signed this file", short_pk(pk));
        }
        Ok(())
    }

    fn check_partial(&self, p: &EvmPartial) -> Result<()> {
        if self.member_index(&p.signer_pk).is_none() {
            bail!("partial from non-member {}", short_pk(&p.signer_pk));
        }
        let pk = BlsPublicKey::from_bytes(&p.signer_pk)
            .map_err(|_| anyhow::anyhow!("partial signer key does not decode"))?;
        let ok = match self.context {
            EvmContext::Bootstrap { .. } => BlsSignature::from_bytes(&p.sig)
                .ok()
                .is_some_and(|s| pk.verify(&s, &self.message).is_ok()),
            EvmContext::Action { .. } => {
                let sig = MultisigSignature::from_bytes(&p.sig).ok();
                let apk = MultisigPublicKey::aggregate(core::slice::from_ref(&pk)).ok();
                matches!((sig, apk), (Some(s), Some(a)) if a.verify(&s, &self.message).is_ok())
            }
        };
        if !ok {
            bail!("bad signature from {}", short_pk(&p.signer_pk));
        }
        Ok(())
    }

    /// Sign with `sk` (the scheme follows the kind) and add the partial. Refuses non-members and duplicates.
    pub fn sign_and_add(&mut self, sk: &BlsSecretKey, pk: &BlsPublicKey) -> Result<()> {
        let signer_pk = pk.to_bytes();
        self.ensure_can_sign(&signer_pk)?;
        let sig = match self.context {
            EvmContext::Bootstrap { .. } => sk.sign(&self.message).to_bytes(),
            EvmContext::Action { .. } => sk.sign_multisig(pk, &self.message).to_bytes(),
        };
        self.add_partial(EvmPartial { signer_pk, sig })
    }

    /// Add a partial produced elsewhere; verified before it is kept.
    pub fn add_partial(&mut self, p: EvmPartial) -> Result<()> {
        self.ensure_can_sign(&p.signer_pk)?;
        self.check_partial(&p)?;
        self.partials.push(p);
        Ok(())
    }

    /// Signers sorted by member index; refuses when fewer than `threshold` valid partials.
    pub fn ordered_signers(&self, threshold: u32) -> Result<OrderedSigners> {
        self.validate()?;
        let mut v: Vec<(usize, EvmPartial)> = self
            .partials
            .iter()
            .map(|p| {
                (
                    self.member_index(&p.signer_pk).expect("validated"),
                    p.clone(),
                )
            })
            .collect();
        v.sort_by_key(|(i, _)| *i);
        if (v.len() as u64) < u64::from(threshold) {
            bail!(
                "below threshold: {} valid partial(s), threshold {threshold}",
                v.len()
            );
        }
        Ok(OrderedSigners {
            indices: v.iter().map(|(i, _)| *i).collect(),
            partials: v.into_iter().map(|(_, p)| p).collect(),
        })
    }

    /// `PublishBootstrapRootArgs { account_id, sigs }` as e2e-tool builds it, after the threshold and every
    /// signature are checked locally.
    pub fn publish_args(&self, threshold: u32) -> Result<PublishBootstrapRootArgs> {
        if !matches!(self.context, EvmContext::Bootstrap { .. }) {
            bail!("not a bootstrap file (kind = {})", self.kind());
        }
        let signers = self.ordered_signers(threshold)?;
        let sigs = signers
            .partials
            .iter()
            .map(|p| {
                Ok(SignatureEntry {
                    signer: BlsPublicKey::from_bytes(&p.signer_pk)
                        .map_err(|_| anyhow::anyhow!("signer key"))?,
                    signature: BlsSignature::from_bytes(&p.sig)
                        .map_err(|_| anyhow::anyhow!("signature"))?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(PublishBootstrapRootArgs {
            account_id: self.account_id,
            sigs,
        })
    }

    /// `KnotEvmRoot.Quorum` for `checkAction`: all leaves in member order, signer bitmap, signers' keys in
    /// bitmap order, one aggregate signature. The aggregate is verified locally against the message.
    pub fn quorum(&self, threshold: u32) -> Result<EvmQuorum> {
        let EvmContext::Action { digest, .. } = self.context else {
            bail!("not an action file (kind = {})", self.kind());
        };
        let signers = self.ordered_signers(threshold)?;
        let leaves = self.members.iter().map(leaf).collect::<Result<Vec<_>>>()?;
        let bitmap = signers.indices.iter().fold(0u64, |b, &i| b | (1u64 << i));
        let mut signer_pks = Vec::new();
        let mut signer_pks_eip = Vec::new();
        let mut sigs = Vec::new();
        let mut pks = Vec::new();
        for p in &signers.partials {
            signer_pks.extend_from_slice(&p.signer_pk);
            signer_pks_eip.extend_from_slice(&g2_eip(&p.signer_pk)?);
            sigs.push(
                MultisigSignature::from_bytes(&p.sig).map_err(|_| anyhow::anyhow!("signature"))?,
            );
            pks.push(
                BlsPublicKey::from_bytes(&p.signer_pk)
                    .map_err(|_| anyhow::anyhow!("signer key"))?,
            );
        }
        let (first, rest) = sigs.split_first().context("no signatures")?;
        let agg = first.aggregate(rest);
        let apk = MultisigPublicKey::aggregate(&pks)
            .map_err(|_| anyhow::anyhow!("aggregate public key"))?;
        if apk.verify(&agg, &self.message).is_err() {
            bail!("aggregate signature does not verify against the action message");
        }
        Ok(EvmQuorum {
            account_id: self.account_id,
            digest,
            leaves,
            bitmap,
            signer_pks,
            signer_pks_eip,
            sig: agg.to_bytes(),
        })
    }
}

/// Arguments of `KnotEvmRoot.checkAction(accountId, actionDigest, Quorum)`.
pub struct EvmQuorum {
    pub account_id: u64,
    pub digest: [u8; 32],
    pub leaves: Vec<[u8; 32]>,
    pub bitmap: u64,
    pub signer_pks: Vec<u8>,
    pub signer_pks_eip: Vec<u8>,
    pub sig: [u8; 48],
}

impl EvmQuorum {
    /// cast tuple literal for `Quorum`, same shape as e2e-tool prints.
    pub fn tuple(&self) -> String {
        let lv: Vec<String> = self.leaves.iter().map(|l| hx(l)).collect();
        format!(
            "([{}],{},{},{},{})",
            lv.join(","),
            self.bitmap,
            hx(&self.signer_pks),
            hx(&self.signer_pks_eip),
            hx(&self.sig)
        )
    }

    /// `<accountId> <digest> <tuple>`, the line e2e-tool's `action` prints.
    pub fn args_line(&self) -> String {
        format!("{} {} {}", self.account_id, hx(&self.digest), self.tuple())
    }
}

pub fn short_pk(pk: &[u8; 96]) -> String {
    let h = hex::encode(pk);
    format!("0x{}..{}", &h[..8], &h[h.len() - 8..])
}

pub fn read_file(path: &std::path::Path) -> Result<EvmSigFile> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let f: EvmSigFile = serde_json::from_str(&text).context("parsing EVM signing file")?;
    f.validate()?;
    Ok(f)
}

pub fn write_file(path: &std::path::Path, f: &EvmSigFile) -> Result<()> {
    let text = serde_json::to_string_pretty(f).context("serializing EVM signing file")?;
    crate::blob::write_atomic(path, format!("{text}\n").as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    const VECTORS: &str = include_str!("../../../evm/test/vectors/knot_root.json");

    fn vectors() -> serde_json::Value {
        serde_json::from_str(VECTORS).unwrap()
    }

    fn vs(v: &serde_json::Value, k: &str) -> String {
        v[k].as_str().unwrap().to_string()
    }

    /// The fixed public-seed test keys of `evm/vectors-gen` (never hold value).
    fn vector_key(i: u64) -> (BlsSecretKey, BlsPublicKey) {
        let mut rng = StdRng::seed_from_u64(0x6b6e_6f74_0000_0000 + i);
        let sk = BlsSecretKey::random(&mut rng);
        let pk = BlsPublicKey::from(&sk);
        (sk, pk)
    }

    fn keys(n: u64) -> Vec<(BlsSecretKey, BlsPublicKey)> {
        (0..n).map(vector_key).collect()
    }

    /// e2e-tool `publish`: `evm_bootstrap_message_v1(chain, &reg, acct, nonce, &recv, &members, 2)`; written
    /// out here as the spec'd preimage so the wrapper is checked against the layout, not against itself.
    fn e2e_bootstrap_message(
        chain: u64,
        reg: &[u8; 32],
        acct: u64,
        nonce: u64,
        recv: &[u8; 20],
        members: &[[u8; 96]],
        threshold: u32,
    ) -> Vec<u8> {
        let mut pre = knot_encoding::DOMAIN_EVM_BOOTSTRAP_V1.to_vec();
        pre.extend_from_slice(&chain.to_le_bytes());
        pre.extend_from_slice(reg);
        pre.extend_from_slice(&acct.to_le_bytes());
        pre.extend_from_slice(&nonce.to_le_bytes());
        pre.extend_from_slice(recv);
        pre.extend_from_slice(&(members.len() as u32).to_le_bytes());
        for m in members {
            pre.extend_from_slice(m);
        }
        pre.extend_from_slice(&threshold.to_le_bytes());
        keccak(&[&pre]).to_vec()
    }

    fn bootstrap_file(members: &[(BlsSecretKey, BlsPublicKey)], threshold: u32) -> EvmSigFile {
        EvmSigFile::new(
            2,
            [0xa1; 32],
            7,
            members.iter().map(|(_, pk)| pk.to_bytes()).collect(),
            threshold,
            EvmContext::Bootstrap {
                nonce: 3,
                receiver: [0xbb; 20],
            },
        )
        .unwrap()
    }

    fn act_s2_file() -> (EvmSigFile, Vec<(BlsSecretKey, BlsPublicKey)>) {
        let v = vectors();
        let k = keys(2);
        let f = EvmSigFile::new(
            v["dusk_chain_id"].as_u64().unwrap(),
            parse_hex_array(&vs(&v, "registry_id"), "reg").unwrap(),
            v["account_id"].as_u64().unwrap(),
            k.iter().map(|(_, pk)| pk.to_bytes()).collect(),
            2,
            EvmContext::Action {
                evm_chain_id: v["evm_chain_id"].as_u64().unwrap(),
                root: parse_hex_array(&vs(&v, "root_addr"), "root").unwrap(),
                rotation_nonce: 0,
                consumer: parse_hex_array(&vs(&v, "consumer"), "consumer").unwrap(),
                digest: parse_hex_array(&vs(&v, "digest"), "digest").unwrap(),
            },
        )
        .unwrap();
        (f, k)
    }

    #[test]
    fn vector_keys_match_vector_pks() {
        let v = vectors();
        for (i, (_, pk)) in keys(10).iter().enumerate() {
            assert_eq!(hx(&pk.to_bytes()), v["pk_c"][i].as_str().unwrap());
        }
    }

    #[test]
    fn bootstrap_message_equals_e2e_tool() {
        let k = keys(3);
        let members: Vec<[u8; 96]> = k.iter().map(|(_, pk)| pk.to_bytes()).collect();
        let reg = [0x5a; 32];
        let recv = [0x42; 20];
        let ours = bootstrap_message(2, &reg, 9, 4, &recv, &members, 2).unwrap();
        assert_eq!(
            ours,
            e2e_bootstrap_message(2, &reg, 9, 4, &recv, &members, 2)
        );
        assert_eq!(ours.len(), 32);
        let f = EvmSigFile::new(
            2,
            reg,
            9,
            members,
            2,
            EvmContext::Bootstrap {
                nonce: 4,
                receiver: recv,
            },
        )
        .unwrap();
        assert_eq!(f.message, ours);
    }

    #[test]
    fn publish_args_bytes_equal_e2e_tool() {
        let k = keys(2);
        let mut f = bootstrap_file(&k, 2);
        for (sk, pk) in &k {
            f.sign_and_add(sk, pk).unwrap();
        }
        let args = f.publish_args(2).unwrap();
        // e2e-tool: SignatureEntry { signer: pk, signature: sk.sign(&msg) } for each member, rkyv 4096.
        let e2e = PublishBootstrapRootArgs {
            account_id: 7,
            sigs: k
                .iter()
                .map(|(sk, pk)| SignatureEntry {
                    signer: *pk,
                    signature: sk.sign(&f.message),
                })
                .collect(),
        };
        assert_eq!(
            encode_publish_args(&args).unwrap(),
            encode_publish_args(&e2e).unwrap()
        );
        // and the 256-scratch encoder chain::encode uses gives the same bytes
        let small = rkyv::to_bytes::<_, 256>(&args).unwrap().to_vec();
        assert_eq!(small, encode_publish_args(&e2e).unwrap());
    }

    #[test]
    fn action_message_equals_vector_and_e2e_layout() {
        let v = vectors();
        let (f, _) = act_s2_file();
        assert_eq!(f.message.len(), 9 * 32);
        assert_eq!(hx(&f.message), v["act_s2"]["msg"].as_str().unwrap());
    }

    #[test]
    fn g2_eip_and_root_match_vectors() {
        let v = vectors();
        let k = keys(10);
        for (i, (_, pk)) in k.iter().enumerate() {
            assert_eq!(
                hx(&g2_eip(&pk.to_bytes()).unwrap()),
                v["pk_eip"][i].as_str().unwrap()
            );
        }
        let s2: Vec<[u8; 96]> = k[..2].iter().map(|(_, pk)| pk.to_bytes()).collect();
        let s8: Vec<[u8; 96]> = k[2..10].iter().map(|(_, pk)| pk.to_bytes()).collect();
        assert_eq!(hx(&members_root(&s2).unwrap()), vs(&v, "root_s2"));
        assert_eq!(hx(&members_root(&s8).unwrap()), vs(&v, "root_s8"));
    }

    #[test]
    fn action_aggregate_equals_vector_signature() {
        let v = vectors();
        let (mut f, k) = act_s2_file();
        // sign in reverse order: the quorum must still come out in member order
        for (sk, pk) in k.iter().rev() {
            f.sign_and_add(sk, pk).unwrap();
        }
        let q = f.quorum(2).unwrap();
        assert_eq!(q.bitmap, v["act_s2"]["bitmap"].as_u64().unwrap());
        assert_eq!(hx(&q.sig), v["act_s2"]["sig_c"].as_str().unwrap());
        let eips: String = (0..2)
            .map(|i| v["pk_eip"][i].as_str().unwrap().trim_start_matches("0x"))
            .collect();
        assert_eq!(hx(&q.signer_pks_eip), format!("0x{eips}"));
        assert!(
            q.args_line()
                .starts_with(&format!("1 {} ([", vs(&v, "digest")))
        );
    }

    #[test]
    fn action_subset_matches_vector_act_s8_n1() {
        // vectors-gen act_s8_n1: members = keys 2..10, signers = first 6, rotation nonce 1.
        let v = vectors();
        let k = keys(10);
        let members: Vec<[u8; 96]> = k[2..10].iter().map(|(_, pk)| pk.to_bytes()).collect();
        let mut f = EvmSigFile::new(
            2,
            parse_hex_array(&vs(&v, "registry_id"), "reg").unwrap(),
            1,
            members,
            6,
            EvmContext::Action {
                evm_chain_id: 745,
                root: parse_hex_array(&vs(&v, "root_addr"), "root").unwrap(),
                rotation_nonce: 1,
                consumer: parse_hex_array(&vs(&v, "consumer"), "consumer").unwrap(),
                digest: parse_hex_array(&vs(&v, "digest"), "digest").unwrap(),
            },
        )
        .unwrap();
        assert_eq!(hx(&f.message), v["act_s8_n1"]["msg"].as_str().unwrap());
        for (sk, pk) in &k[2..8] {
            f.sign_and_add(sk, pk).unwrap();
        }
        let q = f.quorum(6).unwrap();
        assert_eq!(q.bitmap, 63);
        assert_eq!(q.leaves.len(), 8);
        assert_eq!(hx(&q.sig), v["act_s8_n1"]["sig_c"].as_str().unwrap());
    }

    #[test]
    fn merge_refuses_mismatched_rounds() {
        let k = keys(2);
        let base = bootstrap_file(&k, 2);
        let mut other = base.clone();
        other.context = EvmContext::Bootstrap {
            nonce: 4,
            receiver: [0xbb; 20],
        };
        other.message = other.compute_message().unwrap();
        let err = base.ensure_same_round(&other).unwrap_err().to_string();
        assert!(err.contains("context"), "{err}");

        let mut other = base.clone();
        other.account_id = 8;
        other.message = other.compute_message().unwrap();
        assert!(base.ensure_same_round(&other).is_err());

        let mut other = base.clone();
        other.context = EvmContext::Bootstrap {
            nonce: 3,
            receiver: [0xbc; 20],
        };
        other.message = other.compute_message().unwrap();
        assert!(base.ensure_same_round(&other).is_err());

        // partials alone don't make a different round
        let mut signed = base.clone();
        signed.sign_and_add(&k[0].0, &k[0].1).unwrap();
        base.ensure_same_round(&signed).unwrap();
    }

    #[test]
    fn tampered_message_is_refused_on_load() {
        let k = keys(2);
        let mut f = bootstrap_file(&k, 2);
        f.message[0] ^= 1;
        assert!(f.validate().is_err());
    }

    #[test]
    fn refuses_duplicates_and_non_members() {
        let k = keys(3);
        let mut f = bootstrap_file(&k[..2], 2);
        f.sign_and_add(&k[0].0, &k[0].1).unwrap();
        let dup = f.sign_and_add(&k[0].0, &k[0].1).unwrap_err().to_string();
        assert!(dup.contains("already signed"), "{dup}");
        let outsider = f.sign_and_add(&k[2].0, &k[2].1).unwrap_err().to_string();
        assert!(outsider.contains("not a member"), "{outsider}");
        // a duplicate smuggled into the file is caught on load
        let mut g = f.clone();
        g.partials.push(f.partials[0].clone());
        assert!(g.validate().is_err());
    }

    #[test]
    fn submit_refuses_below_threshold_and_bad_signatures() {
        let k = keys(3);
        let mut f = bootstrap_file(&k, 2);
        f.sign_and_add(&k[0].0, &k[0].1).unwrap();
        let err = f.publish_args(2).unwrap_err().to_string();
        assert!(err.contains("below threshold"), "{err}");

        // a signature over another message, attributed to member 1
        let mut bad = f.clone();
        bad.partials.push(EvmPartial {
            signer_pk: k[1].1.to_bytes(),
            sig: k[1].0.sign(b"something else").to_bytes(),
        });
        let err = bad.publish_args(2).unwrap_err().to_string();
        assert!(err.contains("bad signature"), "{err}");

        // member 2's signature claimed as member 1's
        let mut swapped = f.clone();
        swapped.partials.push(EvmPartial {
            signer_pk: k[1].1.to_bytes(),
            sig: k[2].0.sign(&f.message).to_bytes(),
        });
        assert!(swapped.publish_args(2).is_err());

        f.sign_and_add(&k[2].0, &k[2].1).unwrap();
        assert_eq!(f.publish_args(2).unwrap().sigs.len(), 2);
    }

    #[test]
    fn action_refuses_plain_signature_and_below_threshold() {
        let (mut f, k) = act_s2_file();
        f.sign_and_add(&k[0].0, &k[0].1).unwrap();
        assert!(f.quorum(2).is_err());
        // a plain (non-multisig) signature is not what checkAction verifies
        let mut plain = f.clone();
        plain.partials.push(EvmPartial {
            signer_pk: k[1].1.to_bytes(),
            sig: k[1].0.sign(&f.message).to_bytes(),
        });
        assert!(plain.validate().is_err());
        // kinds don't cross
        assert!(f.publish_args(1).is_err());
    }

    #[test]
    fn file_round_trips_through_json() {
        let (mut f, k) = act_s2_file();
        f.sign_and_add(&k[1].0, &k[1].1).unwrap();
        let text = serde_json::to_string_pretty(&f).unwrap();
        assert!(text.contains("\"kind\": \"action\""));
        let back: EvmSigFile = serde_json::from_str(&text).unwrap();
        back.validate().unwrap();
        assert_eq!(back, f);
    }

    #[test]
    fn refuses_too_many_members() {
        let k = keys(17);
        let members: Vec<[u8; 96]> = k.iter().map(|(_, pk)| pk.to_bytes()).collect();
        assert!(
            EvmSigFile::new(
                2,
                [0; 32],
                0,
                members,
                2,
                EvmContext::Bootstrap {
                    nonce: 0,
                    receiver: [1; 20]
                }
            )
            .is_err()
        );
    }
}
