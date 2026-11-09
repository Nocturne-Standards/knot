# nocturne-knot-collector

Package `nocturne-knot-collector`, Rust library `knot_collector`, binary `knot-collector`.

An untrusted off-chain relay for Knot multisig proposals and partial signatures. It stores serialized proposal blobs and BLS partial signatures in SQLite so signers can exchange them over HTTP. It holds no secret keys, never signs, and never submits transactions. It recomputes proposal digests with `nocturne-knot-encoding` and verifies public BLS signatures, so it cannot be used as an unauthenticated junk relay.

Status: testnet only, not audited yet.

## Run

```bash
cargo install nocturne-knot-collector
knot-collector
curl http://127.0.0.1:8899/v1/health
```

| Env var | Default | Purpose |
|---|---|---|
| `KNOT_COLLECTOR_BIND` | `127.0.0.1:8899` | Listen address. Loopback only unless the escape hatch is set. |
| `KNOT_COLLECTOR_DB` | `./collector.sqlite` | SQLite database path |
| `KNOT_COLLECTOR_ALLOW_NON_LOOPBACK` | unset | Set to `1` to bind outside loopback |

The API has no authentication. Bind loopback and put a reverse proxy with TLS and auth in front of it.

## HTTP API

| Method | Path | Behavior |
|---|---|---|
| `GET` | `/v1/health` | Liveness and version |
| `POST` | `/v1/proposals` | Create a proposal blob; the digest is recomputed and verified |
| `GET` | `/v1/proposals` | List summaries; `?limit` (default 50, max 200) and `?offset` |
| `GET` | `/v1/proposals/{id}` | Full blob; `id` is the 64-char hex signed digest |
| `POST` | `/v1/proposals/{id}/partials` | Add or replace a verified BLS partial signature |
| `GET` | `/v1/party` | Roster list; `?limit` and `?offset` |
| `POST` | `/v1/party` | Sign up or update `{ name, pk, sig, note? }`; `sig` proves key possession |

Limits: 64 KiB request bodies, 32 partials per proposal, 512 characters for names and notes, 10,000 proposal rows, 1,000 roster rows. Proposals older than 90 days are swept.

## Related crates

Part of the [Knot](https://github.com/Nocturne-Standards/knot) repository. It depends on `nocturne-knot-encoding` for digest computation.

## License

AGPL-3.0-only. See `LICENSE`. A commercial license is available; see `LICENSING.md`.
