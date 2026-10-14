// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Nocturne Standards

//! Write decoder fixtures from `knot-encoding` event structs.
//!
//! Usage: `generate-event-fixtures <output-dir>`
//! The hex and json in each file come from `rkyv` and `serde` on the struct.
//! Nothing in this file is a hand-written golden.

use std::env;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

use dusk_core::abi::ContractId;
use dusk_core::signatures::bls::{PublicKey as BlsPublicKey, SecretKey as BlsSecret};
use knot_encoding::call_types::RegistryPendingChange;
use knot_encoding::events::{
    AccountChanged, AccountCreated, AuthorizedAccountSet, DataSet, PendingCancelled,
    PendingScheduled, ProposalApproved, ProposalCancelled, ProposalCreated, ProposalFinalized,
    ProposalQueued, ProposalTtlSet, Pruned, RegistrySet, TimelockSet, TombstoneSet,
};
use rand::SeedableRng;
use rand::rngs::StdRng;
use rkyv::ser::serializers::AllocSerializer;
use rkyv::{Serialize, to_bytes};

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let Some(dir) = args.next() else {
        eprintln!("usage: generate-event-fixtures <output-dir>");
        return ExitCode::from(2);
    };
    if args.next().is_some() {
        eprintln!("usage: generate-event-fixtures <output-dir>");
        return ExitCode::from(2);
    }
    let dir = Path::new(&dir);
    if let Err(err) = fs::create_dir_all(dir) {
        eprintln!("create {}: {err}", dir.display());
        return ExitCode::from(1);
    }

    let mut rng = StdRng::seed_from_u64(0x6e0d);
    let secret = BlsSecret::random(&mut rng);
    let signer = BlsPublicKey::from(&secret);
    let signature = secret.sign(b"knot-event-decoder");
    let members = vec![signer];
    let registry = ContractId::from_bytes([0x11; 32]);
    let target = ContractId::from_bytes([0x22; 32]);
    let digest = [0x33; 32];

    let mut failed = false;
    failed |= write(
        dir,
        "knot-registry",
        "data_set",
        &DataSet { data: registry },
    );
    failed |= write(
        dir,
        "knot-registry",
        "account_created",
        &AccountCreated {
            id: 7,
            members: members.clone(),
            threshold: 1,
            timelock_blocks: 0,
            nonce: 0,
        },
    );
    failed |= write(
        dir,
        "knot-registry",
        "pending_scheduled",
        &PendingScheduled {
            account_id: 7,
            execute_at: 20,
            change: RegistryPendingChange::SetTimelock(4),
        },
    );
    failed |= write(
        dir,
        "knot-registry",
        "account_changed",
        &AccountChanged {
            account_id: 7,
            members: members.clone(),
            threshold: 1,
        },
    );
    failed |= write(
        dir,
        "knot-registry",
        "timelock_set",
        &TimelockSet {
            account_id: 7,
            blocks: 4,
        },
    );
    failed |= write(
        dir,
        "knot-registry",
        "pending_cancelled",
        &PendingCancelled {
            account_id: 7,
            execute_at: 20,
            change: RegistryPendingChange::ChangeAccount {
                new_members: members.clone(),
                new_threshold: 1,
            },
        },
    );
    failed |= write(
        dir,
        "knot-proposals",
        "data_set",
        &DataSet { data: registry },
    );
    failed |= write(
        dir,
        "knot-proposals",
        "registry_set",
        &RegistrySet { registry, epoch: 2 },
    );
    failed |= write(
        dir,
        "knot-proposals",
        "proposal_ttl_set",
        &ProposalTtlSet { blocks: 100 },
    );
    failed |= write(
        dir,
        "knot-proposals",
        "tombstone_set",
        &TombstoneSet { tombstone: true },
    );
    failed |= write(
        dir,
        "knot-proposals",
        "authorized_account_set",
        &AuthorizedAccountSet {
            account_id: 7,
            auth_generation: 3,
        },
    );
    failed |= write(
        dir,
        "knot-proposals",
        "proposal_created",
        &ProposalCreated {
            proposal_id: 9,
            signed_digest: digest,
            registry_account_id: 7,
            deadline: 50,
            epoch: 2,
            nonce: 1,
            auth_generation: 3,
            target,
            function_name: "set".to_owned(),
            call_args: vec![1, 2, 3],
        },
    );
    failed |= write(
        dir,
        "knot-proposals",
        "proposal_approved",
        &ProposalApproved {
            proposal_id: 9,
            signed_digest: digest,
            signer,
            signature,
        },
    );
    failed |= write(
        dir,
        "knot-proposals",
        "proposal_finalized",
        &ProposalFinalized {
            proposal_id: 9,
            signed_digest: digest,
            registry_account_id: 7,
            target,
            function_name: "set".to_owned(),
            call_args: vec![1, 2, 3],
        },
    );
    failed |= write(
        dir,
        "knot-proposals",
        "proposal_queued",
        &ProposalQueued {
            proposal_id: 9,
            signed_digest: digest,
            registry_account_id: 7,
            execute_at: 30,
            target,
            function_name: "set".to_owned(),
            call_args: vec![1, 2, 3],
        },
    );
    failed |= write(
        dir,
        "knot-proposals",
        "proposal_cancelled",
        &ProposalCancelled {
            proposal_id: 9,
            signed_digest: digest,
            registry_account_id: 7,
        },
    );
    failed |= write(
        dir,
        "knot-proposals",
        "pruned",
        &Pruned {
            proposal_ids: vec![9],
            digest_keys: vec![digest],
        },
    );

    if failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

fn write<T>(dir: &Path, contract: &str, topic: &str, value: &T) -> bool
where
    T: Serialize<AllocSerializer<4096>> + serde::Serialize,
{
    let bytes = match to_bytes::<_, 4096>(value) {
        Ok(bytes) => bytes,
        Err(err) => {
            eprintln!("archive {contract}/{topic}: {err}");
            return true;
        }
    };
    let json = match serde_json::to_value(value) {
        Ok(json) => json,
        Err(err) => {
            eprintln!("json {contract}/{topic}: {err}");
            return true;
        }
    };
    let doc = serde_json::json!({
        "contract": contract,
        "topic": topic,
        "version": 1,
        "hex": hex_encode(bytes.as_slice()),
        "json": json,
    });
    let path = dir.join(format!("{contract}__{topic}.json"));
    let body = match serde_json::to_string_pretty(&doc) {
        Ok(body) => body,
        Err(err) => {
            eprintln!("encode {contract}/{topic}: {err}");
            return true;
        }
    };
    if let Err(err) = fs::write(&path, body + "\n") {
        eprintln!("write {}: {err}", path.display());
        return true;
    }
    println!("{}", path.display());
    false
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0xf) as usize] as char);
    }
    out
}
