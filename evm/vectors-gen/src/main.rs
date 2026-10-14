//! Test vectors for KnotEvmRoot: real Dusk V2 multisig signatures (bls12_381-bls 0.6.0, sign_multisig +
//! aggregate, the abi::verify_bls_multisig path) over the contract's rotation and action messages.
//!
//! Keys are test-only and derived from fixed public seeds, so the vectors are reproducible. They are not secrets
//! and must never hold value. Output: one JSON file of public data (path in argv[1]).

use bls12_381_bls::{MultisigPublicKey, MultisigSignature, PublicKey, SecretKey};
use dusk_bls12_381::G2Affine;
use dusk_bytes::Serializable;
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::fmt::Write as _;
use tiny_keccak::{Hasher, Keccak};

const DUSK_CHAIN_ID: u64 = 2;
const EVM_CHAIN_ID: u64 = 745;
/// eth_call sender for the gas harness. The harness is created at create(FROM, 0), its messenger stand-in at
/// create(harness, 1), the root at create(harness, 2).
const FROM: [u8; 20] = [
    0x4b, 0x6e, 0x6f, 0x74, 0x47, 0x61, 0x73, 0x50, 0x72, 0x6f, 0x62, 0x65, 0, 0, 0, 0, 0, 0, 0, 0x01,
];
const ROTATE_TAG: &[u8] = b"KNOT_EVM_ROOT_ROTATE_V1";
const ACTION_TAG: &[u8] = b"KNOT_EVM_ROOT_ACTION_V1";
const N_KEYS: usize = 54;
/// knot-registry account id (u64, ABI word uint256)
const ACCOUNT_ID: u64 = 1;
const OUTSIDER: usize = 20;

fn keccak(parts: &[&[u8]]) -> [u8; 32] {
    let mut k = Keccak::v256();
    for p in parts {
        k.update(p);
    }
    let mut o = [0u8; 32];
    k.finalize(&mut o);
    o
}

fn hx(b: &[u8]) -> String {
    let mut s = String::from("0x");
    for x in b {
        write!(s, "{x:02x}").unwrap();
    }
    s
}

fn word_u64(v: u64) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[24..].copy_from_slice(&v.to_be_bytes());
    w
}

fn word_addr(a: &[u8; 20]) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[12..].copy_from_slice(a);
    w
}

fn word_tag(t: &[u8]) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[..t.len()].copy_from_slice(t);
    w
}

/// CREATE address for nonce 0..=127 (single-byte RLP).
fn create_addr(sender: &[u8; 20], nonce: u8) -> [u8; 20] {
    assert!(nonce < 0x80);
    let n = if nonce == 0 { 0x80 } else { nonce };
    let h = keccak(&[&[0xd6, 0x94], sender, &[n]]);
    let mut a = [0u8; 20];
    a.copy_from_slice(&h[12..]);
    a
}

fn pad64(be48: &[u8]) -> Vec<u8> {
    let mut v = vec![0u8; 16];
    v.extend_from_slice(be48);
    v
}

/// EIP-2537 G2: x_c0, x_c1, y_c0, y_c1 (64 bytes each)
fn g2_eip(pk: &PublicKey) -> Vec<u8> {
    let p = G2Affine::from_bytes(&pk.to_bytes()).expect("pk decode");
    let raw = p.to_uncompressed(); // zcash: x_c1, x_c0, y_c1, y_c0
    let mut xc1 = raw[0..48].to_vec();
    let xc0 = raw[48..96].to_vec();
    let mut yc1 = raw[96..144].to_vec();
    let yc0 = raw[144..192].to_vec();
    xc1[0] &= 0x1f;
    yc1[0] &= 0x1f;
    [pad64(&xc0), pad64(&xc1), pad64(&yc0), pad64(&yc1)].concat()
}

struct Keys {
    sk: Vec<SecretKey>,
    pk: Vec<PublicKey>,
    pk_c: Vec<Vec<u8>>,
    pk_eip: Vec<Vec<u8>>,
}

impl Keys {
    fn root(&self, set: &[usize]) -> [u8; 32] {
        let mut leaves = Vec::new();
        for &i in set {
            leaves.extend_from_slice(&keccak(&[&self.pk_c[i], &self.pk_eip[i]]));
        }
        keccak(&[&leaves])
    }

    /// Aggregate signature by `signers`; the bool is Rust's verify against the `claimed` keys.
    fn sign(&self, signers: &[usize], claimed: &[usize], msg: &[u8]) -> (Vec<u8>, bool) {
        let sigs: Vec<MultisigSignature> = signers.iter().map(|&i| self.sk[i].sign_multisig(&self.pk[i], msg)).collect();
        let agg = sigs[0].aggregate(&sigs[1..]);
        let pks: Vec<PublicKey> = claimed.iter().map(|&i| self.pk[i]).collect();
        let ok = MultisigPublicKey::aggregate(&pks).unwrap().verify(&agg, msg).is_ok();
        (agg.to_bytes().to_vec(), ok)
    }
}

struct Ctx {
    root_addr: [u8; 20],
    /// msg.sender that consumes actions (the gas harness)
    consumer: [u8; 20],
    registry_id: [u8; 32],
}

impl Ctx {
    fn rotation_msg(&self, new_root: &[u8; 32], t: u32, nonce: u64) -> Vec<u8> {
        [
            word_tag(ROTATE_TAG),
            word_u64(DUSK_CHAIN_ID),
            word_u64(EVM_CHAIN_ID),
            word_addr(&self.root_addr),
            self.registry_id,
            word_u64(ACCOUNT_ID),
            *new_root,
            word_u64(t as u64),
            word_u64(nonce),
        ]
        .concat()
    }

    fn action_msg(&self, nonce: u64, digest: &[u8; 32]) -> Vec<u8> {
        [
            word_tag(ACTION_TAG),
            word_u64(DUSK_CHAIN_ID),
            word_u64(EVM_CHAIN_ID),
            word_addr(&self.root_addr),
            self.registry_id,
            word_u64(ACCOUNT_ID),
            word_u64(nonce),
            word_addr(&self.consumer),
            *digest,
        ]
        .concat()
    }
}

enum Kind {
    Action { digest: [u8; 32] },
    Rotate { new_set: Vec<usize>, new_threshold: u32 },
}

struct Vector {
    name: &'static str,
    set: Vec<usize>,
    nonce: u64,
    kind: Kind,
    /// positions in `set` claimed by the bitmap
    signer_pos: Vec<usize>,
    /// key indices that actually sign; defaults to the claimed signers
    sign_keys: Option<Vec<usize>>,
}

fn main() {
    let out_path = std::env::args().nth(1).expect("usage: knot-evm-root-vectors <out.json>");

    let mut keys = Keys { sk: vec![], pk: vec![], pk_c: vec![], pk_eip: vec![] };
    for i in 0..N_KEYS {
        let mut rng = StdRng::seed_from_u64(0x6b6e_6f74_0000_0000 + i as u64);
        let sk = SecretKey::random(&mut rng);
        let pk = PublicKey::from(&sk);
        keys.pk_c.push(pk.to_bytes().to_vec());
        keys.pk_eip.push(g2_eip(&pk));
        keys.sk.push(sk);
        keys.pk.push(pk);
    }

    let harness = create_addr(&FROM, 0);
    let ctx = Ctx {
        root_addr: create_addr(&harness, 2),
        consumer: harness,
        registry_id: keccak(&[b"knot-test-registry"]),
    };
    let digest = keccak(&[b"knot-test-action-1"]);

    let s2: Vec<usize> = vec![0, 1];
    let s8: Vec<usize> = (2..10).collect();
    let s2b: Vec<usize> = vec![10, 11];
    let s8b: Vec<usize> = (12..20).collect();
    let all8: Vec<usize> = (0..8).collect();
    let s16: Vec<usize> = (21..37).collect();
    let s16b: Vec<usize> = (37..53).collect();
    let all16: Vec<usize> = (0..16).collect();

    let vectors = vec![
        Vector { name: "act_s2", set: s2.clone(), nonce: 0, kind: Kind::Action { digest }, signer_pos: vec![0, 1], sign_keys: None },
        Vector {
            name: "rot_s2_to_s8",
            set: s2.clone(),
            nonce: 1,
            kind: Kind::Rotate { new_set: s8.clone(), new_threshold: 6 },
            signer_pos: vec![0, 1],
            sign_keys: None,
        },
        Vector {
            name: "rot_s2_to_s8_one",
            set: s2.clone(),
            nonce: 1,
            kind: Kind::Rotate { new_set: s8.clone(), new_threshold: 6 },
            signer_pos: vec![0],
            sign_keys: None,
        },
        Vector {
            name: "rot_s2_to_s8_outsider",
            set: s2.clone(),
            nonce: 1,
            kind: Kind::Rotate { new_set: s8.clone(), new_threshold: 6 },
            signer_pos: vec![0, 1],
            sign_keys: Some(vec![0, OUTSIDER]),
        },
        Vector { name: "act_s8_n1", set: s8.clone(), nonce: 1, kind: Kind::Action { digest }, signer_pos: (0..6).collect(), sign_keys: None },
        Vector {
            name: "rot_s8_to_s2b_n2",
            set: s8.clone(),
            nonce: 2,
            kind: Kind::Rotate { new_set: s2b.clone(), new_threshold: 2 },
            signer_pos: vec![1, 2, 3, 4, 6, 7],
            sign_keys: None,
        },
        Vector {
            name: "g2_rot",
            set: s2.clone(),
            nonce: 1,
            kind: Kind::Rotate { new_set: s2b.clone(), new_threshold: 2 },
            signer_pos: vec![0, 1],
            sign_keys: None,
        },
        Vector { name: "g8_act", set: s8.clone(), nonce: 0, kind: Kind::Action { digest }, signer_pos: all8.clone(), sign_keys: None },
        Vector {
            name: "g8_rot",
            set: s8.clone(),
            nonce: 1,
            kind: Kind::Rotate { new_set: s8b.clone(), new_threshold: 8 },
            signer_pos: all8.clone(),
            sign_keys: None,
        },
        Vector { name: "g16_act", set: s16.clone(), nonce: 0, kind: Kind::Action { digest }, signer_pos: all16.clone(), sign_keys: None },
        Vector {
            name: "g16_rot",
            set: s16.clone(),
            nonce: 1,
            kind: Kind::Rotate { new_set: s16b.clone(), new_threshold: 16 },
            signer_pos: all16.clone(),
            sign_keys: None,
        },
    ];

    let mut js = String::from("{\n");
    write!(js, "  \"from\": \"{}\",\n  \"harness\": \"{}\",\n  \"root_addr\": \"{}\",\n  \"consumer\": \"{}\",\n", hx(&FROM), hx(&harness), hx(&ctx.root_addr), hx(&ctx.consumer)).unwrap();
    write!(js, "  \"dusk_chain_id\": {DUSK_CHAIN_ID},\n  \"evm_chain_id\": {EVM_CHAIN_ID},\n").unwrap();
    write!(
        js,
        "  \"registry_id\": \"{}\",\n  \"account_id\": {},\n  \"digest\": \"{}\",\n",
        hx(&ctx.registry_id),
        ACCOUNT_ID,
        hx(&digest)
    )
    .unwrap();
    let list = |v: &Vec<Vec<u8>>| v.iter().map(|b| format!("\"{}\"", hx(b))).collect::<Vec<_>>().join(", ");
    write!(js, "  \"pk_c\": [{}],\n  \"pk_eip\": [{}],\n", list(&keys.pk_c), list(&keys.pk_eip)).unwrap();
    write!(js, "  \"root_s2\": \"{}\",\n  \"root_s8\": \"{}\",\n", hx(&keys.root(&s2)), hx(&keys.root(&s8))).unwrap();

    for v in &vectors {
        let (msg, new_root) = match &v.kind {
            Kind::Action { digest } => (ctx.action_msg(v.nonce, digest), [0u8; 32]),
            Kind::Rotate { new_set, new_threshold } => {
                let r = keys.root(new_set);
                (ctx.rotation_msg(&r, *new_threshold, v.nonce), r)
            }
        };
        let claimed: Vec<usize> = v.signer_pos.iter().map(|&p| v.set[p]).collect();
        let signing = v.sign_keys.clone().unwrap_or_else(|| claimed.clone());
        let (sig, ok) = keys.sign(&signing, &claimed, &msg);
        let bitmap: u64 = v.signer_pos.iter().map(|&p| 1u64 << p).sum();
        write!(
            js,
            "  \"{}\": {{\"msg\": \"{}\", \"sig_c\": \"{}\", \"bitmap\": {}, \"new_root\": \"{}\", \"rust_verify\": {}}},\n",
            v.name,
            hx(&msg),
            hx(&sig),
            bitmap,
            hx(&new_root),
            ok
        )
        .unwrap();
        println!("{:24} signers={:?} rust_verify={}", v.name, signing, ok);
    }
    js.push_str("  \"end\": true\n}\n");
    std::fs::write(&out_path, js).unwrap();
    println!("root_addr {}", hx(&ctx.root_addr));
}
