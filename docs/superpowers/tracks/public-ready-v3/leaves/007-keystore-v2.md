---
id: 7
slug: keystore-v2
status: DONE
owner: cursor-agent
deps:
  - 3
scope:
  - crates/knot-tool/src/keystore.rs
  - crates/knot-tool/src/keystore.rs
acceptance:
  - M4 mode at create; M5 atomic+fsync+bak; M6+M7 binary format Argon2id
  - L4/L5/L6 fixed; tests §3.6
acceptanceDone:
  - true
  - true
---
# Phase 4b: keystore v2

Planner context…

## Evidence (worker)

- Commit on `feat/public-ready-v3-rename`
- `cargo test -p knot-tool keystore::` — 11/11 pass
- `cargo test -p knot-tool` — all pass
- Report: `.superpowers/sdd/task-7-report.md`
