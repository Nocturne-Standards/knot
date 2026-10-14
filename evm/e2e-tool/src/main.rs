//! Testnet e2e helper for the knot EVM root bootstrap (D9). Throwaway BLS keys live in a mode-600 file
//! (`$KNOT_E2E_STATE/sks.bin`); argv and stdout carry public data only.
//!
//! keys                                   generate 4 keys (set A = 0,1; set B = 2,3), print public keys
//! pks                                    print public keys (base58, hex)
//! create                                 rkyv CreateAccountArgs { members: A, threshold: 2 }
//! contract-id <hex32>                    rkyv ContractId
//! addr20 <hex20>                         rkyv [u8; 20]
//! publish <chain> <reg> <acct> <nonce> <receiver20>   rkyv PublishBootstrapRootArgs signed by A
//! root <set: a|b>                        members root and concatenated compressed keys
//! rotate <dusk_chain> <evm_chain> <root20> <reg> <acct> <nonce>   cast args for rotate A -> B, signed by A
//! action <dusk_chain> <evm_chain> <root20> <reg> <acct> <rot_nonce> <consumer20> <digest32> <set>

use std::env;
use std::fs;
use std::os::unix::fs::OpenOptionsExt;
use std::io::Write;

use bls12_381_bls::{MultisigSignature, PublicKey as RawPk, SecretKey as RawSk};
use dusk_bls12_381::G2Affine;
use dusk_bytes::Serializable;
use dusk_core::abi::ContractId;
use dusk_core::signatures::bls::{PublicKey, SecretKey};
use knot_encoding::call_types::{CreateAccountArgs, PublishBootstrapRootArgs, SignatureEntry};
use knot_encoding::evm_bootstrap_message_v1;
use tiny_keccak::{Hasher, Keccak};

const ROTATE_TAG: &[u8] = b"KNOT_EVM_ROOT_ROTATE_V1";
const ACTION_TAG: &[u8] = b"KNOT_EVM_ROOT_ACTION_V1";

fn state_path() -> String {
    format!("{}/sks.bin", env::var("KNOT_E2E_STATE").expect("KNOT_E2E_STATE"))
}

fn load() -> Vec<SecretKey> {
    let b = fs::read(state_path()).expect("read sks");
    b.chunks(32).map(|c| SecretKey::from_bytes(c.try_into().unwrap()).expect("sk")).collect()
}

fn pk(sk: &SecretKey) -> PublicKey {
    PublicKey::from(sk)
}

fn hx(b: &[u8]) -> String {
    let mut s = String::from("0x");
    for x in b {
        s.push_str(&format!("{x:02x}"));
    }
    s
}

fn unhex<const N: usize>(s: &str) -> [u8; N] {
    let s = s.trim_start_matches("0x");
    assert_eq!(s.len(), 2 * N, "hex length");
    let mut o = [0u8; N];
    for i in 0..N {
        o[i] = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap();
    }
    o
}

fn rk<T: rkyv::Serialize<rkyv::ser::serializers::AllocSerializer<4096>>>(v: &T) -> String {
    hx(&rkyv::to_bytes::<_, 4096>(v).expect("rkyv")).trim_start_matches("0x").to_string()
}

fn keccak(parts: &[&[u8]]) -> [u8; 32] {
    let mut k = Keccak::v256();
    for p in parts {
        k.update(p);
    }
    let mut o = [0u8; 32];
    k.finalize(&mut o);
    o
}

fn pad64(be48: &[u8]) -> Vec<u8> {
    let mut v = vec![0u8; 16];
    v.extend_from_slice(be48);
    v
}

fn g2_eip(pk_c: &[u8; 96]) -> Vec<u8> {
    let p = G2Affine::from_bytes(pk_c).expect("pk decode");
    let raw = p.to_uncompressed();
    let mut xc1 = raw[0..48].to_vec();
    let xc0 = raw[48..96].to_vec();
    let mut yc1 = raw[96..144].to_vec();
    let yc0 = raw[144..192].to_vec();
    xc1[0] &= 0x1f;
    yc1[0] &= 0x1f;
    [pad64(&xc0), pad64(&xc1), pad64(&yc0), pad64(&yc1)].concat()
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

fn set(name: &str) -> Vec<usize> {
    match name {
        "a" => vec![0, 1],
        "b" => vec![2, 3],
        _ => panic!("set a|b"),
    }
}

fn leaves(sks: &[SecretKey], s: &[usize]) -> Vec<[u8; 32]> {
    s.iter()
        .map(|&i| {
            let c = pk(&sks[i]).to_bytes();
            keccak(&[&c, &g2_eip(&c)])
        })
        .collect()
}

fn root(sks: &[SecretKey], s: &[usize]) -> [u8; 32] {
    keccak(&[&leaves(sks, s).concat()])
}

/// cast tuple literal for Quorum, all of `s` signing `msg` with sign_multisig.
fn quorum(sks: &[SecretKey], s: &[usize], msg: &[u8]) -> String {
    let raw: Vec<(RawSk, RawPk)> = s
        .iter()
        .map(|&i| {
            let sk = RawSk::from_bytes(&sks[i].to_bytes()).unwrap();
            let pk = RawPk::from(&sk);
            (sk, pk)
        })
        .collect();
    let sigs: Vec<MultisigSignature> = raw.iter().map(|(sk, pk)| sk.sign_multisig(pk, msg)).collect();
    let agg = sigs[0].aggregate(&sigs[1..]);
    let lv: Vec<String> = leaves(sks, s).iter().map(|l| hx(l)).collect();
    let pks: Vec<u8> = s.iter().flat_map(|&i| pk(&sks[i]).to_bytes()).collect();
    let eips: Vec<u8> = s.iter().flat_map(|&i| g2_eip(&pk(&sks[i]).to_bytes())).collect();
    let bitmap = (1u64 << s.len()) - 1;
    format!("([{}],{},{},{},{})", lv.join(","), bitmap, hx(&pks), hx(&eips), hx(&agg.to_bytes()))
}

fn main() {
    let a: Vec<String> = env::args().collect();
    match a[1].as_str() {
        "keys" => {
            let mut rng = rand::rngs::OsRng;
            let mut f = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(state_path())
                .expect("sks.bin must not exist");
            for _ in 0..4 {
                f.write_all(&SecretKey::random(&mut rng).to_bytes()).unwrap();
            }
            println!("generated 4 keys");
        }
        "pks" => {
            for (i, sk) in load().iter().enumerate() {
                let p = pk(sk);
                println!("{i} {}", hx(&p.to_bytes()));
            }
        }
        "create" => {
            let sks = load();
            println!("{}", rk(&CreateAccountArgs { members: vec![pk(&sks[0]), pk(&sks[1])], threshold: 2 }));
        }
        "contract-id" => println!("{}", rk(&ContractId::from_bytes(unhex::<32>(&a[2])))),
        "addr20" => println!("{}", rk(&unhex::<20>(&a[2]))),
        "publish" => {
            let sks = load();
            let chain: u64 = a[2].parse().unwrap();
            let reg = unhex::<32>(&a[3]);
            let acct: u64 = a[4].parse().unwrap();
            let nonce: u64 = a[5].parse().unwrap();
            let recv = unhex::<20>(&a[6]);
            let members = [pk(&sks[0]).to_bytes(), pk(&sks[1]).to_bytes()];
            let msg = evm_bootstrap_message_v1(chain, &reg, acct, nonce, &recv, &members, 2).unwrap();
            let sigs = [0usize, 1]
                .iter()
                .map(|&i| SignatureEntry { signer: pk(&sks[i]), signature: sks[i].sign(&msg) })
                .collect();
            println!("{}", rk(&PublishBootstrapRootArgs { account_id: acct, sigs }));
        }
        "root" => {
            let sks = load();
            let s = set(&a[2]);
            let pks: Vec<u8> = s.iter().flat_map(|&i| pk(&sks[i]).to_bytes()).collect();
            println!("{} {}", hx(&root(&sks, &s)), hx(&pks));
        }
        "rotate" => {
            let sks = load();
            let (dc, ec): (u64, u64) = (a[2].parse().unwrap(), a[3].parse().unwrap());
            let r = unhex::<20>(&a[4]);
            let reg = unhex::<32>(&a[5]);
            let acct: u64 = a[6].parse().unwrap();
            let nonce: u64 = a[7].parse().unwrap();
            let b = set("b");
            let new_root = root(&sks, &b);
            let msg = [
                word_tag(ROTATE_TAG), word_u64(dc), word_u64(ec), word_addr(&r), reg, word_u64(acct), new_root,
                word_u64(2), word_u64(nonce),
            ]
            .concat();
            let pks: Vec<u8> = b.iter().flat_map(|&i| pk(&sks[i]).to_bytes()).collect();
            println!("{} {} 2 {} {}", acct, hx(&pks), nonce, quorum(&sks, &set("a"), &msg));
        }
        "action" => {
            let sks = load();
            let (dc, ec): (u64, u64) = (a[2].parse().unwrap(), a[3].parse().unwrap());
            let r = unhex::<20>(&a[4]);
            let reg = unhex::<32>(&a[5]);
            let acct: u64 = a[6].parse().unwrap();
            let rn: u64 = a[7].parse().unwrap();
            let consumer = unhex::<20>(&a[8]);
            let digest = unhex::<32>(&a[9]);
            let msg = [
                word_tag(ACTION_TAG), word_u64(dc), word_u64(ec), word_addr(&r), reg, word_u64(acct), word_u64(rn),
                word_addr(&consumer), digest,
            ]
            .concat();
            println!("{} {} {}", acct, hx(&digest), quorum(&sks, &set(&a[10]), &msg));
        }
        c => panic!("unknown command {c}"),
    }
}
