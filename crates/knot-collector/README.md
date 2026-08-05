# knot-collector

Untrusted off-chain relay for the multisig suite (Safe Transaction Service
analogue) — holds serialized proposal blobs and partial signatures so
signers don't have to email JSON around. **Never holds secret keys, never
signs, and never submits on-chain transactions.** It may verify public BLS
signatures and recompute proposal digests so it cannot be used as an
unauthenticated griefing relay. See [`docs/security-model.md`](../../docs/security-model.md)
and [`docs/design-notes.md`](../../docs/design-notes.md) for the trust model.

## Status

**Package version `0.2.0`** — see [CHANGELOG.md](CHANGELOG.md). Policy: [docs/versioning.md](../../docs/versioning.md).

**2026-08-05 — collector hardening (C1, M10–M12, L9–L12, M11, R10):** digest
recompute via `knot-encoding`; BLS verify on partials and party signup;
pagination + row caps + TTL sweep; loopback bind parses `SocketAddr`;
graceful shutdown; `PRAGMA synchronous=FULL` + `fullfsync`; generic 500 bodies.

**2026-07-24 — availability hardening (audit I8/I9/I10/M10):** last-write-wins
partial replace (same `signer_pk`); sig capped at 48 bytes (BLS); max 32
partials / proposal; max 512-char note/summary/name; 64 KiB body limit; GET id
must be 64 hex; `DELETE /v1/party/:pk` removed; non-loopback bind requires
`KNOT_COLLECTOR_ALLOW_NON_LOOPBACK=1`.

| Method | Path | Behavior |
|---|---|---|
| `GET` | `/v1/health` | Liveness — `{"ok":true,"version":"<semver>"}` |
| `POST` | `/v1/proposals` | Create verified proposals blob (`version=1`, `kind=proposals`); recomputes digest; `partials` ignored |
| `GET` | `/v1/proposals` | List summaries; `?limit` (default 50, max 200) + `?offset` |
| `GET` | `/v1/proposals/:id` | Full blob; `:id` must be exactly 64 hex chars (else 400) |
| `POST` | `/v1/proposals/:id/partials` | Append/replace verified BLS partial for digest |
| `GET` | `/v1/party` | Roster list; `?limit` + `?offset` |
| `POST` | `/v1/party` | Signup/upsert `{ name, pk, sig, note? }` — `sig` proves key possession |

There is **no** `DELETE /v1/party/:pk` — roster is upsert-only.

`id` = lowercase hex of `signed_digest` (content-addressed). Re-creating the
same identity is idempotent; a different intent under the same digest is 409
(logged at warn). `pm_council_resolve` blobs are rejected at create (no
verified digest path in knot-encoding).

| Env var | Default | Purpose |
|---|---|---|
| `KNOT_COLLECTOR_BIND` | `127.0.0.1:8899` | HTTP listen address (loopback IP only unless escape hatch) |
| `KNOT_COLLECTOR_DB` | `./collector.sqlite` | SQLite database path |
| `KNOT_COLLECTOR_ALLOW_NON_LOOPBACK` | unset | Set to `1` to allow binding outside loopback |

Request bodies capped at **64 KiB**. Row caps: 10_000 proposals, 1_000 party
rows; proposals older than 90 days swept on read/write paths.

### Wire parity with `knot-tool`

`src/dto.rs` field-aligns with `knot-tool/src/blob.rs` JSON shapes. Digest
recomputation uses `knot-encoding::ProposalIntent` (C1) — hex parsing follows
the same single-`0x` strip as the tool. Party signup requires `sig` over
`nocturne.knot.collector.party.v1 || name_len || name || pk[96]`.

## Run

```bash
cargo run -p knot-collector
curl http://127.0.0.1:8899/v1/health   # {"ok":true,"version":"0.2.0"}
```

**Deployment:** bind loopback and reverse-proxy with TLS + auth. Setting
`KNOT_COLLECTOR_ALLOW_NON_LOOPBACK=1` and binding `0.0.0.0` exposes an API
with **no authentication** — state this in operator runbooks.

## License

`knot-collector` is licensed under **AGPL-3.0-only** (see `LICENSE`). A
commercial license is available — see `LICENSING.md`.
