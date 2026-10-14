// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Nocturne Standards

//! Every `abi::emit` topic in the Knot product contracts needs a decoder arm.
//!
//! Product sources are `knot-registry`, `knot-proposals`, and `knot-warden`
//! under `src/`. `knot-proposals/test-target` is a test double, not a product
//! contract, and is outside those trees.
//!
//! Exemptions: none are written in this tree. The list stays empty. Do not
//! add a row to turn a missing arm into a pass.

use std::fs;
use std::path::Path;

use knot_event_decoder::{
    CONTRACT_PROPOSALS, CONTRACT_REGISTRY, CONTRACT_WARDEN, DecodeError, NAMED_EVENT_VERSION,
    decode,
};

const SOURCES: &[(&str, &str)] = &[
    (CONTRACT_REGISTRY, "../knot-registry/src"),
    (CONTRACT_PROPOSALS, "../knot-proposals/src"),
    (CONTRACT_WARDEN, "../knot-warden/src"),
];

/// `(contract, topic)` pairs already exempt in this tree.
const EXEMPT: &[(&str, &str)] = &[];

#[test]
fn conformance_emit_topics() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut missing = Vec::new();
    for (contract, rel) in SOURCES {
        let root = manifest_dir.join(rel);
        for (path, topic) in emit_topics(&root) {
            if EXEMPT.contains(&(contract, topic.as_str())) {
                continue;
            }
            if !arm_exists(contract, &topic) {
                missing.push(format!("{contract}/{topic} ({})", path.display()));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "abi::emit topic has no decoder arm and no exemption already in the tree:\n{}",
        missing.join("\n")
    );
}

fn arm_exists(contract: &str, topic: &str) -> bool {
    match decode(contract, topic, NAMED_EVENT_VERSION, &[]) {
        Err(DecodeError::UnknownEvent { .. }) => false,
        Ok(_) | Err(DecodeError::InvalidPayload(_)) | Err(DecodeError::UnsupportedVersion(_)) => {
            true
        }
    }
}

fn emit_topics(root: &Path) -> Vec<(std::path::PathBuf, String)> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = fs::read_dir(&dir).unwrap_or_else(|err| {
            panic!("read {}: {err}", dir.display());
        });
        for entry in entries {
            let entry = entry.expect("dir entry");
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
                continue;
            }
            let text = fs::read_to_string(&path).unwrap_or_else(|err| {
                panic!("read {}: {err}", path.display());
            });
            for topic in topics_in(&text, &path) {
                found.push((path.clone(), topic));
            }
        }
    }
    found
}

fn topics_in(text: &str, path: &Path) -> Vec<String> {
    let mut topics = Vec::new();
    let mut rest = text;
    while let Some(index) = rest.find("abi::emit(") {
        let after = rest[index + "abi::emit(".len()..].trim_start();
        let Some(after_quote) = after.strip_prefix('"') else {
            panic!(
                "{}: abi::emit topic is not a string literal",
                path.display()
            );
        };
        let end = after_quote.find('"').unwrap_or_else(|| {
            panic!("{}: unterminated abi::emit topic", path.display());
        });
        topics.push(after_quote[..end].to_owned());
        rest = &rest[index + "abi::emit(".len()..];
    }
    topics
}
