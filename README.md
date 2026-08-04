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
· [GitHub — aichbindas/knot](https://github.com/aichbindas/knot)

`atlas/` (per-project service registry) stays **outside** this repo — it
consumes `multisig-proposals` / `multisig-registry` as a client.

## Crates

| Crate | License | Version | Role |
|---|---|---|---|
| `multisig-encoding` | Apache-2.0 | 0.1.2 | Canonical §4a digest + blob helpers + M3 fingerprint |
| `multisig-registry` | Apache-2.0 | 0.1.5 | On-chain BLS M-of-N quorum registry |
| `multisig-proposals` | Apache-2.0 | 0.3.2 | On-chain propose → approve → finalize `call_raw` |
| `multisig-tool` | Apache-2.0 | 0.2.0 | Local signing CLI + web Lab (mock + testnet) |
| `multisig-collector` | **AGPL-3.0-only** | 0.2.0 | Untrusted off-chain relay |

The Apache suite and **AGPL collector** are intentionally split. Self-host
`multisig-collector` only if you accept AGPL (or purchase a commercial
license — see `crates/multisig-collector/LICENSING.md`). Signing stays local
in `multisig-tool`; the collector never holds secret keys.

**Consumer dep (encoding):**

```toml
multisig-encoding = { git = "https://github.com/aichbindas/knot", tag = "v0.2.0", package = "multisig-encoding", features = ["call-types"] }
```

Pin a release tag or rev — see [`docs/versioning.md`](docs/versioning.md) for
how crate semvers relate to git tags.

## Layout

```
multisig/
├── Cargo.toml
├── LICENSE-APACHE / LICENSE-AGPL
├── docs/
│   ├── security-model.md
│   └── versioning.md
├── crates/multisig-encoding/
├── crates/multisig-registry/
├── crates/multisig-proposals/
├── crates/multisig-tool/
└── crates/multisig-collector/
```

## Deploy

- **Collector:** bind the process to loopback; put TLS + auth on the reverse
  proxy. The host must never hold BLS secret keys — signing stays on
  participants' machines (`multisig-tool`). API surface is documented in
  `crates/multisig-collector/README.md`; bring your own VPS ops.
- **Licensing:** Apache suite + AGPL collector — see root `LICENSE-*` and
  `crates/multisig-collector/LICENSING.md`.

## Quick commands

```bash
cargo build -p multisig-tool
cargo test -p multisig-encoding

# Contract crates build/test WASM via their own Makefiles (cwd-sensitive):
(cd crates/multisig-registry && make wasm && make test)
(cd crates/multisig-proposals && make wasm && make test)
```

Cold-start the Lab: `./scripts/multisig-first-run.sh --serve` from repo root.
See each crate's `README.md` for API surface and usage detail.

Maintainer deploy timeline: [`docs/internal/deploy-history.md`](docs/internal/deploy-history.md).
