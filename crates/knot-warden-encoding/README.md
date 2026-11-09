# knot-warden-encoding

Package `nocturne-knot-warden-encoding`, Rust library `knot_warden_encoding`.

Host-visible types for the warden contract (`nocturne-knot-warden`), a delaying guardian in front of an Atlas registry (https://github.com/Nocturne-Standards/atlas). `no_std` with `alloc`; the rkyv layout is pinned by golden tests.

Status: testnet only, not audited yet.

```toml
[dependencies]
knot-warden-encoding = { package = "nocturne-knot-warden-encoding", version = "0.2" }
```

## Contents

- `call_types`: `InitWardenArgs`, `PendingAdmin`, `PendingServiceView`, `PendingAdminView`. `Account` and `SetServiceArgs` are re-exported from `nocturne-atlas-encoding` 0.5, so a warden `schedule_service` argument has the same bytes as an Atlas `set_service` argument.
- `events`: payload structs for the warden's events, such as `ServiceScheduled`, `ServiceExecuted`, `AdminScheduled` and `DelaySet`.

## Features

- `data-driver`: serde support for decoding.
- `contract-events`: event emission support for the contract crate (pulls in `dusk-forge`).
- `fixture-gen`: builds the `generate-event-fixtures` binary. Not for normal use.

## Contributors

```bash
cargo test --release
```

## Related crates

Part of the [Knot](https://github.com/Nocturne-Standards/knot) repository. Used by `nocturne-knot-warden` and `nocturne-knot-event-decoder`.

## License

Apache-2.0.
