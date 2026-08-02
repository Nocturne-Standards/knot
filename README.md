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

## For newcomers

**Knot** (this nest: `multisig`) is an M-of-N BLS quorum suite: on-chain
registry + proposals, a local signing Lab, and an optional collector relay.

Architecture and crate interaction diagram:
[docs.nocturne-standards.org — Knot](https://docs.nocturne-standards.org/v1/knot/)
· [source on GitHub](https://github.com/aichbindas/sme_platform/blob/main/nocturne-docs/docs/v1/knot/index.md)

Everything below is nest **Status** and maintainer notes (deploy IDs, layout).
Dense on purpose.

## Status

**2026-08-02 — Wave 7 carve target:** private GitHub repo
[`aichbindas/knot`](https://github.com/aichbindas/knot). Repo-level consumer pin
**`v0.1.0`** (not crates.io). Per-crate versions inside the workspace today:
`multisig-encoding` 0.1.1 · `multisig-registry` 0.1.4 · `multisig-proposals`
0.3.1 · `multisig-tool` 0.2.0 · `multisig-collector` 0.2.0 (AGPL). Nest folder
name stays `multisig/` until monorepo hard-cut.

**Consumer dep (encoding):**
```toml
multisig-encoding = { git = "https://github.com/aichbindas/knot", tag = "v0.1.0", package = "multisig-encoding", features = ["call-types"] }
```

**2026-07-27 — product name Knot** (public surfaces: `knot.nocturne-standards.org`,
docs `/v1/knot/`). Code/repo remains `multisig` until carve completes.

**2026-07-23 — nested workspace carve landed** (hard-cut; see parent
`docs/roadmap.md` Track 7 for per-crate deploy status).

**2026-07-24 — first product consumer path:** prediction-market dispute
council resolve (`multisig-tool pm-resolve`, collector `kind=pm_council_resolve`,
portal web→CLI handoff).

**2026-07-28 — audit overflow/encoding cutover on testnet:** registry
**v0.1.4** (`4e24b59d…`), proposals **v0.3.1** (`5e91ddb6…`, `init_registry` →
new registry + `init_chain_id=2` wired). **PM dispute council** on Atlas logic
**v0.2.0** (`b7543a2b…`) points at registry account **0** (alice + partner;
`dispute_council_of` live). Atlas `"prediction-market"` remapped to the new
logic id.

**2026-07-24 — audit remediation live on testnet:** registry **v0.1.3**
(`66d763b2…`), proposals **v0.3.0** (`6b8ba51c…`, `init_registry` +
`init_chain_id=2` wired), PM monolith **v0.3.1** (`1c095ae2…`,
`council-resolve.v2` + `init_treasury` re-wired). **Dispute council wired**
to registry account **0** (`init_dispute_council` tx `b7f02c0c…`; portal
`accountId: 0`). Superseded for registry/proposals by the 2026-07-28
cutover above.

**2026-07-26 — Multisig Lab website demo:** `DEMO_MODE=mock` default /
`DEMO_MODE=testnet` optional; slide Lab (cover / demo studio / use cases) +
five-beat walkthrough + developer drawer on `:8877`; public docs at
[`docs.nocturne-standards.org/v1/knot/`](https://docs.nocturne-standards.org/v1/knot/)
(no hosted signing; marketing `/docs/multisig.html` redirects there).
Canonical ports/modes:
[`crates/multisig-tool/README.md`](crates/multisig-tool/README.md) Status.

| Surface | License | Notes |
|---|---|---|
| `crates/multisig-encoding` | Apache-2.0 | Canonical §4a digest + blob helpers + M3 fingerprint |
| `crates/multisig-registry` | Apache-2.0 | On-chain BLS M-of-N quorum registry — **testnet v0.1.4** |
| `crates/multisig-proposals` | Apache-2.0 | On-chain propose→approve→finalize `call_raw` — **testnet v0.3.1** |
| `crates/multisig-tool` | Apache-2.0 | Local signing CLI + web UI + PM council resolve — **mock + testnet; no mainnet** |
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
