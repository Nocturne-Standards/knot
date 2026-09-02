# Knot (multisig)

BLS M-of-N multisig suite for Dusk: on-chain registry and proposals
(`call_raw`), canonical encoding, a local signing Lab, and an optional
untrusted collector for sharing proposal blobs and partial signatures.

**Quorum authorization is Prove mode** — on every write the chain
re-verifies membership, threshold, and BLS signatures against live on-chain
state. The Lab holds keys and helps operators sign; it is **not** the final
authority. Detail: [`docs/security-model.md`](docs/security-model.md).

**Testnet only** — no mainnet deployment claim. Versioning:
[`docs/versioning.md`](docs/versioning.md).

Architecture and long-form docs:
[docs.nocturne-standards.org — Knot](https://docs.nocturne-standards.org/v1/knot/)
· [GitHub — Nocturne-Standards/knot](https://github.com/Nocturne-Standards/knot)

`atlas/` (per-project service registry) stays **outside** this repo — it
consumes `knot-proposals` / `knot-registry` as a client. Knot is complete
without Atlas. Atlas is the reference named service-directory to pair with
Knot. When both delays exist, set Atlas `timelock_blocks` to 0 so waits do
not stack.

## Crates

| Crate | License | Version | Role |
|---|---|---|---|
| `knot-encoding` | Apache-2.0 | 0.1.2 | Canonical proposal preimage + blob helpers + M3 fingerprint |
| `knot-registry` | Apache-2.0 | 0.1.6 | On-chain BLS M-of-N quorum registry |
| `knot-proposals` | Apache-2.0 | 0.3.3 | On-chain propose → approve → finalize `call_raw` |
| `knot-tool` | Apache-2.0 | 0.2.0 | Local signing CLI + web Lab (mock + testnet) |
| `knot-collector` | **AGPL-3.0-only** | 0.2.0 | Untrusted off-chain relay |

The Apache suite and **AGPL collector** are intentionally split. Self-host
`knot-collector` only if you accept AGPL (or purchase a commercial
license — see `crates/knot-collector/LICENSING.md`). Signing stays local
in `knot-tool`; the collector never holds secret keys.

**Consumer dep (encoding):**

```toml
knot-encoding = { git = "https://github.com/Nocturne-Standards/knot", tag = "v0.2.0", package = "knot-encoding", features = ["call-types"] }
```

Pin a release tag or rev — see [`docs/versioning.md`](docs/versioning.md) for
how crate semvers relate to git tags.

## Layout

```
.
├── Cargo.toml
├── LICENSE-APACHE / LICENSE-AGPL
├── docs/
│   ├── README.md
│   ├── security-model.md
│   ├── versioning.md
│   └── design-notes.md
├── crates/knot-encoding/
├── crates/knot-registry/
├── crates/knot-proposals/
├── crates/knot-tool/
└── crates/knot-collector/
```

## Deploy

- **Collector:** bind the process to loopback; put TLS + auth on the reverse
  proxy. The host must never hold BLS secret keys — signing stays on
  participants' machines (`knot-tool`). API surface is documented in
  `crates/knot-collector/README.md`; bring your own VPS ops.
- **Licensing:** Apache suite + AGPL collector — see root `LICENSE-*` and
  `crates/knot-collector/LICENSING.md`.

## Quick commands

```bash
cargo build -p knot-tool
cargo test -p knot-encoding

# Contract crates build/test WASM via their own Makefiles (cwd-sensitive):
(cd crates/knot-registry && make wasm && make test)
(cd crates/knot-proposals && make wasm && make test)
```

Cold-start the Lab: `cargo run -p knot-tool -- init` then
`cargo run -p knot-tool -- serve --bind 127.0.0.1:8877`.
See each crate's `README.md` for API surface and usage detail.
