# multisig-collector

Untrusted off-chain relay for the multisig suite (Safe Transaction Service
analogue) — holds serialized proposal blobs and partial signatures so
signers don't have to email JSON around. Never holds keys, never signs,
never submits on-chain. See
`docs/superpowers/specs/2026-07-23-multisig-collector-monorepo-demo-design.md`
§2 for the full trust model and target API surface.

## Status

**2026-07-23 — proposals + partials + party-finder API implemented.** Local/dev
only; VPS deploy is an operator TODO (see deploy runbook).

| Method | Path | Behavior |
|---|---|---|
| `GET` | `/v1/health` | Liveness |
| `POST` | `/v1/proposals` | Create; body = blob JSON, `partials` ignored → `{ "id", "signed_digest" }` |
| `GET` | `/v1/proposals` | List summaries (`id`, `signed_digest`, `threshold`, `partials_count`, `created_at`) |
| `GET` | `/v1/proposals/:id` | Full blob |
| `POST` | `/v1/proposals/:id/partials` | Append one `{ signer_pk, sig }`; rejects duplicate `signer_pk` (409) and unknown id (404) |
| `GET` | `/v1/party` | Party finder roster |
| `POST` | `/v1/party` | Sign up / upsert by pk (`{ name, pk, note? }`) |
| `DELETE` | `/v1/party/:pk` | Leave roster |

`id` = lowercase hex of `signed_digest` (32 bytes → 64 hex chars, no `0x`
prefix) — content-addressed, so re-creating an identical proposal is
idempotent and creating a different body under a colliding digest is a 409.
Partials are appended by rewriting the proposal's whole `body_json` column
under the store's connection mutex (never a separate `partials` table) —
this makes "append only, `signed_digest` never mutates" a structural
property: the append code path has no way to touch that field.

| Env var | Default | Purpose |
|---|---|---|
| `MULTISIG_COLLECTOR_BIND` | `127.0.0.1:8899` | HTTP listen address |
| `MULTISIG_COLLECTOR_DB` | `./collector.sqlite` | SQLite database path |

No `dusk_core` / BLS secret-key dependency anywhere in this crate — see
`src/lib.rs` module doc for the trust rationale. Correspondingly, this
service never calls `gate_blob_for_signing` or recomputes the §4a digest —
that anti-blind-signing check stays in `multisig-tool`, which holds the
keys; the collector only checks hex length/shape.

### Wire parity with `multisig-tool`

`src/dto.rs`'s `ProposalDto`/`IntentDto`/`PartialDto` are a **deliberate
duplicate** of `multisig-tool/src/blob.rs`'s `BlobFile`/`IntentFile`/
`PartialFile` — same field names, same JSON shape (hex strings with an
optional `0x` prefix), so a file saved by `multisig-tool blob push` round-trips
byte-for-byte through this API. They are not shared via `multisig-encoding`
because that crate is `no_std` + Apache-2.0 and consumed by the WASM
contract `multisig-proposals`; adding JSON/hex-decoding and `serde` there
would bloat a size-sensitive on-chain dependency to serve a concern only
this (off-chain, AGPL) crate has. If the two DTOs ever drift, `multisig-tool
blob push`/`pull` round-trip tests are the tripwire — fix both files
together.

Unlike `BlobFile`, the collector never hex-decodes `target_contract_id`,
`function_name`, `call_args`, or `human_summary` — those pass through as
opaque strings. Only `signed_digest` (32 bytes, drives the `id`) and
`signer_pk` (96 bytes, drives partial de-duplication) are hex-validated and
normalized to lowercase.

## Run

```bash
cargo run -p multisig-collector
curl http://127.0.0.1:8899/v1/health   # {"ok":true}
```

**VPS deploy (operator TODO):** follow
[`docs/multisig/multisig-collector-deploy-runbook.md`](../../../docs/multisig/multisig-collector-deploy-runbook.md)
when ready to stand up `collector.nocturne-standards.org`.

## License

`multisig-collector` is licensed under **AGPL-3.0-only** (see `LICENSE`). A
commercial license is available — see `LICENSING.md`.
