# Contributing

Thanks for your interest in Knot. This repo is **testnet tooling** for BLS
multisig on Dusk — not production wallet software.

## Before you start

1. Read [`docs/security-model.md`](docs/security-model.md) and
   [`docs/versioning.md`](docs/versioning.md).
2. Open an issue or draft PR describing the change — especially for contract
   ABI or signing-domain changes (these require coordinated redeploys).

## Development setup

`knot-tool` depends on the sibling [`nocturne-deployments`](https://github.com/aichbindas/nocturne-deployments)
crate via a path dependency. Clone that repo **next to** this one so
`../nocturne-deployments` exists — a standalone `knot` checkout without the
sibling cannot compile `knot-tool`.

```bash
rustc --version   # 1.94+ required
cargo build -p knot-tool
cargo test -p knot-encoding
```

Contract crates build WASM via per-crate Makefiles (cwd-sensitive):

```bash
(cd crates/knot-registry && make wasm && make test)
(cd crates/knot-proposals && make wasm && make test)
```

### Contract pins (testnet)

`knot-tool` reads `deployments/testnet.json` from `NOCTURNE_DEPLOYMENTS` or
by walking up from the crate directory. Without pins, chain-integration tests
that need live contract IDs will fail — that is expected for a fresh checkout.

## Pull requests

- Keep diffs focused; separate mechanical refactors from behaviour changes.
- Run `cargo fmt`, `cargo clippy -- -D warnings`, and relevant tests before
  pushing.
- Update crate `CHANGELOG.md` entries when user-visible behaviour changes.
- Do not commit secrets, `.env` files, or absolute local paths.

## Licensing

Apache-2.0 crates: contributions under Apache-2.0.
`knot-collector` is AGPL-3.0-only — see `crates/knot-collector/LICENSING.md`
before contributing there.
