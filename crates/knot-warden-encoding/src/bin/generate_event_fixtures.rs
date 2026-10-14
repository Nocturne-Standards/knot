// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Nocturne Standards

//! Write decoder fixtures from `knot-warden-encoding` event structs.
//!
//! Usage: `generate-event-fixtures <output-dir>`
//! The hex and json in each file come from `rkyv` and `serde` on the struct.
//! Nothing in this file is a hand-written golden.

use std::env;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

use dusk_core::abi::ContractId;
use knot_warden_encoding::call_types::{Account, PendingAdmin};
use knot_warden_encoding::events::{
    AdminCancelled, AdminScheduled, AtlasCancelForwarded, AtlasSet, DelaySet, GuardianForwarded,
    SchedulerSet, ServiceCancelled, ServiceExecuted, ServiceScheduled, TimelockForwarded,
};
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

    let id = ContractId::from_bytes([0x44; 32]);
    let account = Account::Contract(id);

    let mut failed = false;
    failed |= write(
        dir,
        "knot-warden",
        "scheduler_set",
        &SchedulerSet {
            scheduler: account.clone(),
        },
    );
    failed |= write(dir, "knot-warden", "delay_set", &DelaySet { blocks: 5 });
    failed |= write(
        dir,
        "knot-warden",
        "service_scheduled",
        &ServiceScheduled {
            name: "book".to_owned(),
            id,
            execute_at: 12,
        },
    );
    failed |= write(
        dir,
        "knot-warden",
        "service_executed",
        &ServiceExecuted {
            name: "book".to_owned(),
            id,
        },
    );
    failed |= write(
        dir,
        "knot-warden",
        "service_cancelled",
        &ServiceCancelled {
            name: "book".to_owned(),
            id,
        },
    );
    failed |= write(
        dir,
        "knot-warden",
        "admin_scheduled",
        &AdminScheduled {
            change: PendingAdmin::Delay(5),
            execute_at: 12,
        },
    );
    failed |= write(
        dir,
        "knot-warden",
        "admin_cancelled",
        &AdminCancelled {
            change: PendingAdmin::Scheduler(account.clone()),
        },
    );
    failed |= write(dir, "knot-warden", "atlas_set", &AtlasSet { atlas: id });
    failed |= write(
        dir,
        "knot-warden",
        "guardian_forwarded",
        &GuardianForwarded { guardian: account },
    );
    failed |= write(
        dir,
        "knot-warden",
        "timelock_forwarded",
        &TimelockForwarded { blocks: 8 },
    );
    failed |= write(
        dir,
        "knot-warden",
        "atlas_cancel_forwarded",
        &AtlasCancelForwarded,
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
