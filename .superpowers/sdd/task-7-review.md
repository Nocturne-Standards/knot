# Task 7 review — keystore v2 fix-pass

## Critical — `save()` backup ordering crash gap

**Finding:** `save()` called `rotate_backup()` (rename primary → `.bak`) *before* `write_atomic`. Crash between those steps left no primary file; `load()` returned empty `Vec`.

**Resolution:** Reordered M5 save path:
1. `write_tmp` — write+fsync `.tmp`
2. `copy_backup` — **copy** (not move) existing primary → `.bak` (primary stays present)
3. `commit_tmp` — rename `.tmp` → primary + dir fsync

Primary never absent. Added `save_copies_previous_primary_to_bak` test.

## Important — L5 `0x` / `0x0x` in `blob.rs`

**Finding:** `digest_id`, `call_args`, and partial `sig` still used `trim_start_matches("0x")`, accepting `0x0x…`.

**Resolution:** Added shared `hex_util::{strip_single_0x, decode_hex}`. Wired `blob.rs` sites; `keystore` reuses same helper. `digest_id` now returns `Result<String>`. Tests: `digest_id_rejects_double_0x_prefix`, `to_proposal_blob_rejects_double_0x_in_call_args`.

## Verification

`cargo test -p knot-tool` — 69 passed.
