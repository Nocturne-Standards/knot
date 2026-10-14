// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Nocturne Standards

//! Decoder check over fixtures generated from the wire crates.
//!
//! `generate-event-fixtures` in `knot-encoding` and `knot-warden-encoding`
//! writes these files. This test does not construct the golden bytes. A
//! struct change that leaves the committed fixture stale fails the decode
//! comparison below.

use std::fs;
use std::path::Path;

use knot_event_decoder::{NAMED_EVENT_VERSION, decode};

const FIXTURES: &[(&str, &str, &str)] = &[
    ("../knot-encoding/fixtures", "knot-registry", "data_set"),
    (
        "../knot-encoding/fixtures",
        "knot-registry",
        "account_created",
    ),
    (
        "../knot-encoding/fixtures",
        "knot-registry",
        "pending_scheduled",
    ),
    (
        "../knot-encoding/fixtures",
        "knot-registry",
        "account_changed",
    ),
    ("../knot-encoding/fixtures", "knot-registry", "timelock_set"),
    (
        "../knot-encoding/fixtures",
        "knot-registry",
        "pending_cancelled",
    ),
    ("../knot-encoding/fixtures", "knot-proposals", "data_set"),
    (
        "../knot-encoding/fixtures",
        "knot-proposals",
        "registry_set",
    ),
    (
        "../knot-encoding/fixtures",
        "knot-proposals",
        "proposal_ttl_set",
    ),
    (
        "../knot-encoding/fixtures",
        "knot-proposals",
        "tombstone_set",
    ),
    (
        "../knot-encoding/fixtures",
        "knot-proposals",
        "authorized_account_set",
    ),
    (
        "../knot-encoding/fixtures",
        "knot-proposals",
        "proposal_created",
    ),
    (
        "../knot-encoding/fixtures",
        "knot-proposals",
        "proposal_approved",
    ),
    (
        "../knot-encoding/fixtures",
        "knot-proposals",
        "proposal_finalized",
    ),
    (
        "../knot-encoding/fixtures",
        "knot-proposals",
        "proposal_queued",
    ),
    (
        "../knot-encoding/fixtures",
        "knot-proposals",
        "proposal_cancelled",
    ),
    ("../knot-encoding/fixtures", "knot-proposals", "pruned"),
    (
        "../knot-warden-encoding/fixtures",
        "knot-warden",
        "scheduler_set",
    ),
    (
        "../knot-warden-encoding/fixtures",
        "knot-warden",
        "delay_set",
    ),
    (
        "../knot-warden-encoding/fixtures",
        "knot-warden",
        "service_scheduled",
    ),
    (
        "../knot-warden-encoding/fixtures",
        "knot-warden",
        "service_executed",
    ),
    (
        "../knot-warden-encoding/fixtures",
        "knot-warden",
        "service_cancelled",
    ),
    (
        "../knot-warden-encoding/fixtures",
        "knot-warden",
        "admin_scheduled",
    ),
    (
        "../knot-warden-encoding/fixtures",
        "knot-warden",
        "admin_cancelled",
    ),
    (
        "../knot-warden-encoding/fixtures",
        "knot-warden",
        "atlas_set",
    ),
    (
        "../knot-warden-encoding/fixtures",
        "knot-warden",
        "guardian_forwarded",
    ),
    (
        "../knot-warden-encoding/fixtures",
        "knot-warden",
        "timelock_forwarded",
    ),
    (
        "../knot-warden-encoding/fixtures",
        "knot-warden",
        "atlas_cancel_forwarded",
    ),
];

#[test]
fn generated_fixtures() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    for (dir, contract, topic) in FIXTURES {
        let path = manifest_dir
            .join(dir)
            .join(format!("{contract}__{topic}.json"));
        let text = fs::read_to_string(&path).unwrap_or_else(|err| {
            panic!("missing generated fixture {}: {err}", path.display());
        });
        let doc: serde_json::Value = serde_json::from_str(&text)
            .unwrap_or_else(|err| panic!("fixture {} is not json: {err}", path.display()));
        assert_eq!(doc["contract"], *contract, "{}", path.display());
        assert_eq!(doc["topic"], *topic, "{}", path.display());
        let version = doc["version"]
            .as_u64()
            .unwrap_or_else(|| panic!("fixture {} has no version", path.display()));
        assert_eq!(
            version,
            u64::from(NAMED_EVENT_VERSION),
            "{}",
            path.display()
        );
        let hex = doc["hex"].as_str().unwrap_or_else(|| {
            panic!("fixture {} has no hex", path.display());
        });
        let bytes = hex_decode(hex);
        let decoded = decode(contract, topic, NAMED_EVENT_VERSION, &bytes).unwrap_or_else(|err| {
            panic!("generated fixture no longer matches the struct {contract}/{topic}: {err}");
        });
        assert_eq!(
            decoded, doc["json"],
            "generated fixture no longer matches the struct {contract}/{topic}"
        );
    }
}

fn hex_decode(hex: &str) -> Vec<u8> {
    assert!(hex.len() % 2 == 0, "odd hex length");
    (0..hex.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).expect("hex digit"))
        .collect()
}
