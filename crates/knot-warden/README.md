# knot-warden

Delaying guardian in front of Atlas. Spec: [`docs/warden.md`](../../docs/warden.md).

This crate is 0.3.0 (not deployed). Testnet runs 0.2.1 as the guardian of Atlas 0.4.1.

## Build / test

Package `nocturne-knot-warden`; the Rust name and the WASM file stay `knot_warden`.

Uses `nocturne-atlas-encoding` 0.5 from crates.io through `knot-warden-encoding` (`Account`, `SetServiceArgs`). The host tests load `atlas.wasm`, so they need a checkout of `Nocturne-Standards/atlas` next to this repo (`make -C crates/knot-warden test` builds it).

```bash
make -C crates/knot-warden test
```

That builds this contract, the test proxy, and Atlas, then runs encoding goldens and the host tests.

Own Cargo workspace, like `knot-warden-encoding`: public knot CI does not check out the Atlas repo the host tests need.
