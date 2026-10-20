# knot-warden

Delaying guardian in front of Atlas. Spec: [`docs/warden.md`](../../docs/warden.md).

Testnet `113159d33377e7778d552cc48f40318c44643ca06970e2482b41b276a4d9cdb3` is 0.1.1. This crate is 0.2.1 and is not deployed (an Atlas 0.4.0 testnet warden runs 0.2.0).

## Build / test

Needs a sibling checkout of atlas at the 0.3 kernel (`aichbindas/atlas` next to this repo) so `knot-warden-encoding` can use `Account` and `SetServiceArgs`, and so the host tests can load `atlas.wasm`.

```bash
make -C crates/knot-warden test
```

That builds this contract, the test proxy, and Atlas, then runs encoding goldens and the host tests.

Own Cargo workspace, same reason as `knot-warden-encoding`: public knot CI does not have the atlas sibling.
