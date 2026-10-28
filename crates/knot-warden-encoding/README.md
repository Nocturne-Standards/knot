# knot-warden-encoding

Host-visible warden types: `InitWardenArgs`, `PendingAdmin`, pending views, and event payloads. `Account` and `SetServiceArgs` are re-exported from `atlas-encoding` so `schedule_service` uses the same bytes as Atlas `set_service`.

rkyv layout is pinned by `tests/layout_goldens.rs`. `no_std` + `alloc`.

This crate is its own Cargo workspace. Package `nocturne-knot-warden-encoding`; the Rust name stays `knot_warden_encoding`. It path-depends on a sibling checkout of atlas at `../../../atlas/crates/atlas-encoding` (package `nocturne-atlas-encoding` 0.4) (the `aichbindas/atlas` repo next to `knot`). Public knot CI does not check that sibling out, so this crate is not a member of the parent workspace.

```bash
cargo test --release --manifest-path crates/knot-warden-encoding/Cargo.toml
```
