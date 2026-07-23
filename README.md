# multisig

Nested Cargo workspace for the BLS M-of-N multisig suite: an on-chain quorum
registry, a propose/approve/finalize governance contract, their shared
canonical encoding, and a local signing CLI/web UI tool for exercising both
against the real Dusk testnet.

`atlas/` (per-project service registry + Operator/Multisig governance) stays
**outside** this nest at the sme_platform root — it consumes
`multisig-proposals`/`multisig-registry` as a client, it isn't part of the
suite itself.

## Status

**2026-07-23 — nested workspace carve landed** (hard-cut, no history
preserved beyond the `git mv`; see root `docs/roadmap.md` Track 7 for
per-crate deploy status).

| Surface | License | Notes |
|---|---|---|
| `crates/multisig-encoding` | Apache-2.0 | Canonical §4a digest + §4b blob + M3 fingerprint |
| `crates/multisig-registry` | Apache-2.0 | On-chain BLS M-of-N quorum registry — testnet v0.1.2 |
| `crates/multisig-proposals` | Apache-2.0 | On-chain propose→approve→finalize `call_raw` — testnet v0.2.0 |
| `crates/multisig-tool` | Apache-2.0 | Local signing CLI + web UI (M1–M3) — testnet only |
| `crates/multisig-collector` | AGPL-3.0-only | Untrusted off-chain relay — proposals + partials API live; `/v1/party` still open; **VPS deploy operator TODO** (runbook below) |

- Contract IDs (testnet): `deployments/testnet.json` at the sme_platform root
  (`multisig-registry` / `multisig-proposals` entries).
- Design history: `docs/multisig/` and `references/multisig/` at the
  sme_platform root (not yet moved into this nest).

## Layout

```
multisig/
├── Cargo.toml                     # workspace
├── LICENSE-APACHE / LICENSE-AGPL
├── crates/multisig-encoding/      # Apache — shared digest/blob/fingerprint
├── crates/multisig-registry/      # Apache — quorum registry contract
├── crates/multisig-proposals/     # Apache — propose/approve/finalize contract
├── crates/multisig-tool/          # Apache — signing CLI + web UI
└── crates/multisig-collector/     # AGPL  — untrusted off-chain relay (proposals + partials)
```

## Deploy

- **Collector VPS (operator TODO):** follow
  [`docs/multisig/multisig-collector-deploy-runbook.md`](../docs/multisig/multisig-collector-deploy-runbook.md)
  when ready to stand up `collector.nocturne-standards.org`. Demo UI and signing
  stay on participants' machines — the host never holds BLS secret keys.
- **Licensing:** [`docs/release-strategy/multisig-licensing.md`](../docs/release-strategy/multisig-licensing.md)
  (Apache suite + AGPL collector split).

## Quick commands

```bash
cd multisig
cargo build -p multisig-tool
cargo test -p multisig-encoding

# Contract crates build/test WASM via their own Makefiles (cwd-sensitive):
(cd crates/multisig-registry && make wasm && make test)
(cd crates/multisig-proposals && make wasm && make test)
```

See each crate's own `README.md` for scope, security model, and usage detail.
