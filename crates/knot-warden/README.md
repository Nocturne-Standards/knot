# knot-warden

Delaying guardian in front of Atlas. Spec: [`docs/warden.md`](../../docs/warden.md).

Testnet `fb143b1e10288ea6527edc2e556c009d78e7a01add076ab90d6e133aec47cdcb`. Version 0.1.1.

## Build / test

Needs a sibling checkout of atlas at the 0.3 kernel (`aichbindas/atlas` next to this repo) so `knot-warden-encoding` can use `Account` and `SetServiceArgs`, and so the host tests can load `atlas.wasm`.

```bash
make -C crates/knot-warden test
```

That builds this contract, the test proxy, and Atlas, then runs encoding goldens and the host tests.

Own Cargo workspace, same reason as `knot-warden-encoding`: public knot CI does not have the atlas sibling.
