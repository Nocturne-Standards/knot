# knot-collector

Untrusted off-chain relay for the multisig suite (Safe Transaction Service
analogue) — holds serialized proposal blobs and partial signatures so
signers don't have to email JSON around. Never holds keys, never signs,
never submits on-chain. See
`docs/superpowers/specs/2026-07-23-knot-collector-monorepo-demo-design.md`
§2 for the full trust model and target API surface.

## Status

**Package version `0.2.0`** — see [CHANGELOG.md](CHANGELOG.md). Policy: [docs/versioning.md](../../docs/versioning.md).

**2026-07-24 — availability hardening (audit I8/I9/I10/M10):** last-write-wins
partial replace (same `signer_pk`); sig capped at 48 bytes (BLS); max 32
partials / proposal; max 512-char note/summary; 64 KiB body limit; GET id must
be 64 hex; `DELETE /v1/party/:pk` removed; non-loopback bind requires
`KNOT_COLLECTOR_ALLOW_NON_LOOPBACK=1`.

**2026-07-24 — `kind` on proposal summaries** (`proposals` | `pm_council_resolve`) so
clients can filter blob kinds without pulling every body. **`pm_council_resolve`**
is **wen-facing wire compatibility** until collector carve — product UX for PM
council resolve is wen `pm-council-tool`, not knot `knot-tool`.

**2026-07-23 — proposals + partials + party-finder API implemented.** Local/dev
only; VPS deploy is an operator TODO (see deploy runbook).

| Method | Path | Behavior |
|---|---|---|
| `GET` | `/v1/health` | Liveness — `{"ok":true,"version":"<semver>"}` (check after every redeploy) |
| `POST` | `/v1/proposals` | Create; body = blob JSON, `partials` ignored → `{ "id", "signed_digest" }`; `human_summary` ≤ 512 chars |
| `GET` | `/v1/proposals` | List summaries (`id`, `signed_digest`, `kind`, `threshold`, `partials_count`, `created_at`) |
| `GET` | `/v1/proposals/:id` | Full blob; `:id` must be exactly 64 hex chars (else 400) |
| `POST` | `/v1/proposals/:id/partials` | Append or **replace** `{ signer_pk, sig }` for that pk (last-write-wins, never 409 on duplicate); `sig` must decode to 48 bytes; max 32 distinct pks (400 if a *new* pk would exceed); never mutates `signed_digest` |
| `GET` | `/v1/party` | Party finder roster |
| `POST` | `/v1/party` | Sign up / upsert by pk (`{ name, pk, note? }`); `note` ≤ 512 chars |

There is **no** `DELETE /v1/party/:pk` — roster is upsert-only; operators clear
the SQLite DB if a row must go.

`id` = lowercase hex of `signed_digest` (32 bytes → 64 hex chars, no `0x`
prefix) — content-addressed. Re-creating a proposal with the same identity
(version/intent/digest/threshold) is idempotent even after partials have been
appended; a different identity under the same digest id is a 409.
Partials are stored by rewriting the proposal's whole `body_json` column
under the store's connection mutex (never a separate `partials` table) —
`signed_digest` never mutates on that path.

| Env var | Default | Purpose |
|---|---|---|
| `KNOT_COLLECTOR_BIND` | `127.0.0.1:8899` | HTTP listen address (loopback only unless escape hatch below) |
| `KNOT_COLLECTOR_DB` | `./collector.sqlite` | SQLite database path |
| `KNOT_COLLECTOR_ALLOW_NON_LOOPBACK` | unset | Set to `1` to allow binding outside `127.0.0.1` / `localhost` (prefer reverse-proxy → loopback) |

Request bodies are capped at **64 KiB** (`DefaultBodyLimit`). The binary does
not verify BLS signatures — length/shape only; on-chain quorum still filters
junk.

No `dusk_core` / BLS secret-key dependency anywhere in this crate — see
`src/lib.rs` module doc for the trust rationale. Correspondingly, this
service never calls `gate_blob_for_signing` or recomputes the §4a digest —
that anti-blind-signing check stays in `knot-tool`, which holds the
keys; the collector only checks hex length/shape.

### Wire parity with `knot-tool`

`src/dto.rs`'s `ProposalDto`/`IntentDto`/`PartialDto` are a **deliberate
duplicate** of `knot-tool/src/blob.rs`'s `BlobFile`/`IntentFile`/
`PartialFile` — same field names, same JSON shape (hex strings with an
optional `0x` prefix), so a file saved by `knot-tool blob push` round-trips
byte-for-byte through this API. They are not shared via `knot-encoding`
because that crate is `no_std` + Apache-2.0 and consumed by the WASM
contract `knot-proposals`; adding JSON/hex-decoding and `serde` there
would bloat a size-sensitive on-chain dependency to serve a concern only
this (off-chain, AGPL) crate has. If the two DTOs ever drift, `knot-tool
blob push`/`pull` round-trip tests are the tripwire — fix both files
together.

Unlike `BlobFile`, the collector never hex-decodes `target_contract_id`,
`function_name`, `call_args`, or `human_summary` — those pass through as
opaque strings. Only `signed_digest` (32 bytes, drives the `id`),
`signer_pk` (96 bytes, drives partial replace), and `sig` (48 bytes) are
length-validated and normalized to lowercase hex where applicable.

## Run

```bash
cargo run -p knot-collector
curl http://127.0.0.1:8899/v1/health   # {"ok":true,"version":"0.2.0"}
```

**VPS deploy (operator TODO):** bring your own ops (TLS, auth, SQLite backup).
The HTTP API table above is the full contract — no separate runbook ships in
this repo.

## License

`knot-collector` is licensed under **AGPL-3.0-only** (see `LICENSE`). A
commercial license is available — see `LICENSING.md`.
