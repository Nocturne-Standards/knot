# knot-warden

Delaying guardian in front of Atlas. Spec: [`docs/warden.md`](../../docs/warden.md).

This crate is 0.3.0 (not deployed). Testnet runs 0.2.1 as the guardian of Atlas 0.4.1; the live ids are in [Deployments](https://docs.nocturne-standards.org/v1/deployments/). Older ids, including `113159d3…` (0.1.1), are history.

## Build / test

Package `nocturne-knot-warden`; the Rust name and the WASM file stay `knot_warden`.

Needs a sibling checkout of atlas with `atlas-encoding` 0.4 (package `nocturne-atlas-encoding`; `aichbindas/atlas` next to this repo) so `knot-warden-encoding` can use `Account` and `SetServiceArgs`, and so the host tests can load `atlas.wasm`.

```bash
make -C crates/knot-warden test
```

That builds this contract, the test proxy, and Atlas, then runs encoding goldens and the host tests.

Own Cargo workspace, same reason as `knot-warden-encoding`: public knot CI does not have the atlas sibling.
