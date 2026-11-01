# Contributing

Thanks for your interest in Knot. This repo is **testnet tooling** for BLS
multisig on Dusk — not production wallet software.

## Before you start

1. Read [`docs/security-model.md`](docs/security-model.md) and
   [`docs/versioning.md`](docs/versioning.md).
2. Open an issue or draft PR describing the change — especially for contract
   ABI or signing-domain changes (these require coordinated redeploys).

## Development setup

```bash
rustc --version   # 1.94+ required
cargo build -p knot-tool
cargo test -p knot-encoding
```

A fresh clone builds `knot-tool` without a sibling repo. Contract **pins**
(testnet IDs) are optional for compile and read at run time from a pins-v2
checkout of
[`Nocturne-Standards/nocturne-deployments`](https://github.com/Nocturne-Standards/nocturne-deployments):
clone it next to this repo, or set `NOCTURNE_PINS` (a path list of checkouts).
`NOCTURNE_DEPLOYMENTS` still works when it names a checkout or a file inside
one. Without pins, chain-integration paths that need live contract IDs fail —
expected for a pin-less checkout. The testnet node URL and chain id come from
the `dusk-networks` crate of the same repo.

Contract crates build WASM via per-crate Makefiles (cwd-sensitive):

```bash
(cd crates/knot-registry && make wasm && make test)
(cd crates/knot-proposals && make wasm && make test)
```

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
