# nocturne-knot-warden

Package `nocturne-knot-warden`, Rust library `knot_warden`.

A delaying guardian contract (WASM) placed in front of an Atlas registry contract. Atlas's guardian slot points at the warden; the warden calls Atlas, Atlas does not call it. Service repoints, scheduler and delay changes, and guardian changes are scheduled first and take effect only after a configurable number of blocks. Atlas lives in a separate repository: https://github.com/Nocturne-Standards/atlas

Status: testnet only, not audited yet.

## Build

```bash
rustup target add wasm32-unknown-unknown
cargo build --release --features contract --target wasm32-unknown-unknown
```

The library only compiles on `wasm32`; native builds are empty. Feature flags:

- `contract`: build the contract WASM.
- `data-driver`: build the data-driver WASM for decoding calls and events.
- `data-driver-js`: `data-driver` plus the allocator support needed for JavaScript use.

Call and event types live in `nocturne-knot-warden-encoding`. That crate depends on `nocturne-atlas-encoding` 0.5 from crates.io.

## Contributors

The host tests load the Atlas contract WASM, so they need an Atlas checkout and its WASM built next to this repository. `make test` in this directory builds the pieces and runs the tests.

## Related crates

Part of the [Knot](https://github.com/Nocturne-Standards/knot) repository. Event payloads are decoded by `nocturne-knot-event-decoder`.

## License

Apache-2.0.
