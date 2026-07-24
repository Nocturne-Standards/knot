# multisig

BLS M-of-N multisig suite for DuskDS: an on-chain quorum registry, a
propose/approve/finalize governance contract, shared canonical encoding, a
local signing CLI/web UI, and an optional untrusted off-chain collector for
sharing proposal blobs and partial signatures.

This nested workspace is structured so it can later stand alone as a public
monorepo. **Public docs live here** (`README`s + `docs/`). Planning history and
operator-private notes stay in the parent platform’s `docs/` / `references/`.

`atlas/` (per-project service registry + Operator/Multisig governance) stays
**outside** this nest — it consumes `multisig-proposals` / `multisig-registry`
as a client.

## Status

**2026-07-23 — nested workspace carve landed** (hard-cut; see parent
`docs/roadmap.md` Track 7 for per-crate deploy status).

**2026-07-24 — first product consumer path:** prediction-market dispute
council resolve (`multisig-tool pm-resolve`, collector `kind=pm_council_resolve`,
portal web→CLI handoff). Live PM redeploy/wire still open — see the
prediction-market crate README in the parent repo.

**2026-07-24 — audit remediation (code ready, testnet cutover pending):**
proposals **0.3.0** (CEI finalize, bool `tombstone`, propose caps), registry
encoding/`change_account` digest single-source + committee caps, collector
replace-partials/caps, tool preview/confirm + PM **`council-resolve.v2`**.
Do **not** assume live `deployments/testnet.json` ids already match this
bytecode — redeploy registry / proposals / PM before lab wire.

| Surface | License | Notes |
|---|---|---|
| `crates/multisig-encoding` | Apache-2.0 | Canonical §4a digest + blob helpers + M3 fingerprint |
| `crates/multisig-registry` | Apache-2.0 | On-chain BLS M-of-N quorum registry — testnet v0.1.2 |
| `crates/multisig-proposals` | Apache-2.0 | On-chain propose→approve→finalize `call_raw` — v0.3.0 (testnet cutover pending) |
| `crates/multisig-tool` | Apache-2.0 | Local signing CLI + web UI + PM council resolve — **testnet only** |
| `crates/multisig-collector` | AGPL-3.0-only | Untrusted off-chain relay; production auth at reverse proxy |

- **Security / trust model:** [`docs/security-model.md`](docs/security-model.md)
- Contract IDs (testnet): parent repo `deployments/testnet.json`
  (`multisig-registry` / `multisig-proposals` entries).

## Layout

```
multisig/
├── Cargo.toml
├── LICENSE-APACHE / LICENSE-AGPL
├── docs/
│   └── security-model.md          # public trust boundaries
├── crates/multisig-encoding/      # Apache — digest / blob / fingerprint
├── crates/multisig-registry/      # Apache — quorum registry contract
├── crates/multisig-proposals/     # Apache — propose / approve / finalize
├── crates/multisig-tool/          # Apache — signing CLI + web UI
└── crates/multisig-collector/     # AGPL  — untrusted relay
```

## Deploy

- **Collector:** bind the process to loopback; put TLS + auth on the reverse
  proxy. The host must never hold BLS secret keys — signing stays on
  participants’ machines (`multisig-tool`). See the collector crate README
  for the HTTP API; parent-repo operator runbooks cover VPS wiring.
- **Licensing:** Apache suite + AGPL collector split — see root `LICENSE-*`
  and `crates/multisig-collector/LICENSING.md`.

## Quick commands

```bash
cd multisig
cargo build -p multisig-tool
cargo test -p multisig-encoding

# Contract crates build/test WASM via their own Makefiles (cwd-sensitive):
(cd crates/multisig-registry && make wasm && make test)
(cd crates/multisig-proposals && make wasm && make test)
```

See each crate’s `README.md` for API surface and usage detail.
