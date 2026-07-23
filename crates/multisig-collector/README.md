# multisig-collector

Untrusted off-chain relay for the multisig suite (Safe Transaction Service
analogue) — holds serialized proposal blobs and partial signatures so
signers don't have to email JSON around. Never holds keys, never signs,
never submits on-chain. See
`docs/superpowers/specs/2026-07-23-multisig-collector-monorepo-demo-design.md`
§2 for the full trust model and target API surface.

## Status

**2026-07-23 — health + SQLite scaffold only.** `POST/GET /v1/proposals`,
`/v1/proposals/:id/partials`, and `/v1/party` are **not implemented yet** —
tracked as follow-up tasks. Only `GET /v1/health` exists today.

| Env var | Default | Purpose |
|---|---|---|
| `MULTISIG_COLLECTOR_BIND` | `127.0.0.1:8899` | HTTP listen address |
| `MULTISIG_COLLECTOR_DB` | `./collector.sqlite` | SQLite database path |

No `dusk_core` / BLS secret-key dependency anywhere in this crate — see
`src/lib.rs` module doc for the trust rationale.

## Run

```bash
cargo run -p multisig-collector
curl http://127.0.0.1:8899/v1/health   # {"ok":true}
```

## License

`multisig-collector` is licensed under **AGPL-3.0-only** (see `LICENSE`). A
commercial license is available — see `LICENSING.md`.
