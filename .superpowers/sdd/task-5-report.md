# Task 5 report — Status labels + escapeHtml (R3, R9)

## R3 — `tx_status_label`

- `confirmed` only when log contains `included into a block`
- `propagated` when log contains `Transaction propagated`
- `submitted` for preverify / sent (other `WriteOutcome::Ok`)
- `failed` / `unknown` unchanged

## R9 — `app.js` escape audit

Escaped remaining string sinks in `setOutcome` / council card:
- council lookup/create/proposal/finalize IDs
- council card id/threshold/member count

Existing `escapeHtml` on names, digests, purposes, errors — unchanged.

## IMPLEMENTATION.md

Updated §0 claim: string sinks covered via `escapeHtml`/`textContent`.

## Tests

Added 6 unit tests in `chain.rs`:
`tx_status_label_propagated_not_confirmed`, `preverify_is_submitted`,
`sent_is_submitted`, `block_inclusion_is_confirmed`, `panic_is_failed`,
`unknown_outcome`.

## Verification

`cargo test -p knot-tool chain::tests::` — blocked by RTK in agent shell; run locally.

## Files

- `crates/knot-tool/src/chain.rs`
- `crates/knot-tool/static/app.js`
- `docs/internal/IMPLEMENTATION.md`
