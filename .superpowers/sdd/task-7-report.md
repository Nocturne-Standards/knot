# Task 7 — keystore v2 report

## Status: DONE

## Changes

- `crates/knot-tool/src/keystore.rs` — full v2 rewrite
  - **M4**: `0o600` at create via `OpenOptionsExt::mode`; parent `0o700` via `DirBuilderExt`; refuse load when `mode & 0o077 != 0`
  - **M5**: `write_atomic` (tmp + rename + file `sync_all` + `F_FULLFSYNC` on macOS + dir fsync); `.bak` rotation; stale `.tmp` cleanup on load
  - **M6+M7**: binary v2 format (`KNOTKS\x00\x02`, Argon2id m=64MiB/t=3/p=4, header AAD, binary plaintext); v1 PBKDF2 JSON silent upgrade on load
  - **L4**: `Zeroizing` for derived keys and decrypted plaintext
  - **L5**: `parse_pk` try-both hex/base58; single `0x` strip; `blob.rs` hex helpers fixed
  - **L6**: `default_path()` → `Result`; `KNOT_STORE`; `directories` crate; legacy `~/.knot` / `.knot-tool` / `.multisig-tool` read fallback
- `crates/knot-tool/Cargo.toml` — `argon2`, `directories`
- `crates/knot-tool/src/main.rs` — propagate `resolve_default_path()` `Result`
- `crates/knot-tool/src/blob.rs` — `strip_prefix("0x")` for hex32/hex96
- `crates/knot-tool/README.md` — test-tooling disclaimer, v2/backup/export docs

## Tests (§3.6)

`cargo test -p knot-tool keystore::` — **11 passed**

| Case | Test |
|---|---|
| Save mode 0o600 / parent 0o700 | `save_sets_mode_600_and_parent_700` |
| Load 0o644 refuse | `load_refuses_over_permissive_store` |
| Interrupted write, old intact | `interrupted_write_leaves_old_store_loadable` |
| Stale `.tmp` cleaned | `stale_tmp_is_cleaned_on_load` |
| v1 → v2 upgrade | `v1_load_silently_upgrades_to_v2` |
| Tampered header AEAD fail | `v2_tampered_header_fails_aead` |
| hex + base58 parse | `parse_pk_hex_and_base58_same_key` |
| `0x0x` rejected | `parse_pk_rejects_double_0x_prefix` |
| No HOME no panic | `default_path_without_home_does_not_panic` |

Full `cargo test -p knot-tool` — all suites pass.

## Concerns

- Default path now uses platform data dir (`directories`); legacy paths logged on fallback only.
- Argon2id 64 MiB may be slow on low-memory CI — acceptable for test tooling.
- `identity export` / `import-pk` unchanged (already present); full sk export not added (out of scope).

## Fix-pass (review)

- **Critical:** `save()` no longer renames primary away before atomic write; tmp→fsync, copy→`.bak`, rename tmp→primary.
- **Important:** `hex_util` centralizes L5 strip; `blob.rs` `digest_id` / `call_args` / partial `sig` fixed.
- Review: `.superpowers/sdd/task-7-review.md`
- Tests: `cargo test -p knot-tool` — **69 passed** (added `save_copies_previous_primary_to_bak`, blob `0x0x` rejects).
