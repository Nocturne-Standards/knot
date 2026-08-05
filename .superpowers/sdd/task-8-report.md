# Task 8 — collector hardening

## Status: DONE

## Changes

- **L9–L12:** `validate_proposal_id` on append; generic 500 + stderr log (L10); `SocketAddr` loopback bind (L11); graceful shutdown + `PRAGMA synchronous=FULL` + `fullfsync` (L12)
- **M11:** `?limit`/`?offset` (default 50, max 200); 10k proposal / 1k party row caps; 90-day TTL sweep
- **C1:** `gate.rs` recomputes via `knot-encoding::recompute_and_verify`; Conflict warn log; fixed store comment
- **M10/M12:** `verify.rs` — BLS partial verify + party signup preimage `nocturne.knot.collector.party.v1`
- **R10:** party `name` capped at `MAX_NOTE_CHARS` (512)
- **Docs:** README + `lib.rs`/`api.rs` — never-holds-keys / never-signs / never-submits; `dusk-core` for verify only
- **Deps:** `knot-encoding` (default), `dusk-core`, `dusk-bytes`
- **API:** `POST /v1/party` requires `sig`; `pm_council_resolve` create rejected (no encoding path)

## Tests

`cargo test -p knot-collector` — **49 passed** (api unit + store + http_smoke)

## Gaps

- `knot-tool` `collector_client` / RPC party signup still omit `sig` — needs follow-up leaf (out of #8 scope)
- `collector_roundtrip` integration test will fail until tool sends party `sig`
- v3 `ProposalIntentV3` blob relay not on collector wire yet (tool uses v2 §4a for blobs)

## Evidence

Leaf: `docs/superpowers/tracks/public-ready-v3/leaves/008-collector-hardening.md`
