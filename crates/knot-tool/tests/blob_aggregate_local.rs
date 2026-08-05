//! Local (VM::ephemeral) 2-of-3 file blob → aggregate → verify_quorum_aggregate.
//! Uses `sign_multisig_insecure` because dusk-vm defaults to HardFork::PreFork
//! (see knot-tool README / dusk-vm-issue-1). Live testnet uses secure signing.

extern crate alloc;

use std::path::PathBuf;

use dusk_bytes::Serializable;
use dusk_core::abi::ContractId;
use dusk_core::signatures::bls::{PublicKey as BlsPublicKey, SecretKey as BlsSecretKey};
use dusk_vm::{ContractData, Session, VM};
use knot_encoding::PartialSig;
use knot_tool::blob::{
    aggregate_partials, create_blob, read_file, write_file, BlobFile,
};
use rand::rngs::StdRng;
use rand::SeedableRng;

#[path = "../../knot-registry/src/call_types.rs"]
mod call_types;
use call_types::{CreateAccountArgs, VerifyQuorumAggregateArgs};

const REGISTRY_BYTECODE: &[u8] = include_bytes!(
    "../../../target/contract/wasm32-unknown-unknown/release/knot_registry.wasm"
);
const REGISTRY_ID: ContractId = ContractId::from_bytes([0xa1; 32]);
const CHAIN_ID: u8 = 0xCA;
const POINT_LIMIT: u64 = 0x10000000;

fn keypair(rng: &mut StdRng) -> (BlsSecretKey, BlsPublicKey) {
    let sk = BlsSecretKey::random(rng);
    let pk = BlsPublicKey::from(&sk);
    (sk, pk)
}

fn deploy() -> Session {
    let vm = VM::ephemeral().expect("ephemeral");
    let mut session = vm.genesis_session(CHAIN_ID);
    session
        .deploy(
            REGISTRY_BYTECODE,
            ContractData::builder()
                .owner([0; 32])
                .contract_id(REGISTRY_ID),
            POINT_LIMIT,
        )
        .expect("deploy registry");
    session
}

#[test]
fn file_byo_two_of_three_aggregate_verifies_locally() {
    let rng = &mut StdRng::seed_from_u64(2026_07_23);
    let (sk1, pk1) = keypair(rng);
    let (sk2, pk2) = keypair(rng);
    let (_sk3, pk3) = keypair(rng);

    let mut session = deploy();
    let account_id = session
        .call::<CreateAccountArgs, u64>(
            REGISTRY_ID,
            "create_account",
            &CreateAccountArgs {
                members: alloc::vec![pk1, pk2, pk3],
                threshold: 2,
            },
            POINT_LIMIT,
        )
        .expect("create_account")
        .data;

    let mut blob = create_blob(
        1,
        account_id,
        0,
        [0x44; 32],
        "milestone_release".into(),
        b"loan-escrow-style".to_vec(),
        0,
        2,
        Some("untrusted UI hint".into()),
    )
    .expect("create_blob");

    // Simulate two machines: each adds a partial (insecure for PreFork VM).
    for (sk, pk) in [(&sk1, &pk1), (&sk2, &pk2)] {
        let digest = knot_encoding::gate_blob_for_signing(&blob).expect("gate");
        let sig = sk.sign_multisig_insecure(pk, &digest);
        blob.partials.push(PartialSig {
            signer_pk: pk.to_bytes(),
            sig: sig.to_bytes().to_vec(),
        });
    }

    // BYO channel: write JSON file, read it back on the combiner machine.
    let dir = std::env::temp_dir().join(format!("multisig-blob-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = PathBuf::from(&dir).join("proposal.json");
    write_file(&path, &BlobFile::from_proposal_blob(&blob)).unwrap();
    let reloaded = read_file(&path).unwrap().to_proposal_blob().unwrap();
    assert_eq!(reloaded.partials.len(), 2);

    let (signer_keys, aggregate_sig, digest) = aggregate_partials(&reloaded).unwrap();
    let args = VerifyQuorumAggregateArgs {
        account_id,
        msg: digest.to_vec(),
        signer_keys,
        aggregate_sig,
    };
    let passed = session
        .call::<VerifyQuorumAggregateArgs, bool>(
            REGISTRY_ID,
            "verify_quorum_aggregate",
            &args,
            POINT_LIMIT,
        )
        .expect("verify_quorum_aggregate")
        .data;
    assert!(passed, "2-of-3 file round-trip aggregate must verify locally");

    let _ = std::fs::remove_dir_all(&dir);
}
