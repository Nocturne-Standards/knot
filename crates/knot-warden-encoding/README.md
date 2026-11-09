# knot-warden-encoding

Host-visible warden types: `InitWardenArgs`, `PendingAdmin`, pending views, and event payloads. `Account` and `SetServiceArgs` are re-exported from `atlas-encoding` so `schedule_service` uses the same bytes as Atlas `set_service`.

rkyv layout is pinned by `tests/layout_goldens.rs`. `no_std` + `alloc`.

This crate is its own Cargo workspace. Package `nocturne-knot-warden-encoding`; the Rust name stays `knot_warden_encoding`. It depends on `nocturne-atlas-encoding` 0.5 from crates.io and is not a member of the parent workspace.

```bash
cargo test --release --manifest-path crates/knot-warden-encoding/Cargo.toml
```
