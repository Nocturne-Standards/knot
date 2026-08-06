# Task 4 Report: Quorum / change-account preview+confirm (R2)

## Status
**DONE** — preview+confirm gate mirrored from proposal approve for quorum verify, quorum-agg submit, and change-account submit paths.

## Changes
- **RPC** (`rpc.rs`): `POST /api/quorum/preview`, `/api/quorum-agg/preview`, `/api/change-account/preview`; submit endpoints require `confirm: true` (checked before live-mode gate); `QuorumSignPreviewOut` / `ChangeAccountPreviewOut` with fingerprint fields; soft `note` when multiple signers in one request.
- **BLS** (`bls.rs`): `signing_message_fingerprint` + `message_fingerprint_display` (32-byte messages direct; arbitrary quorum payloads hashed under lab domain).
- **CLI** (`main.rs`): `--confirm` on `quorum submit`, `quorum-agg submit`, `change-account submit`; prints fingerprint before signing.
- **UI** (`index.html`, `app.js`, `style.css`): developer drawer (testnet-only) with Preview → confirm → Submit for quorum verify and change_account.
- **README**: CLI examples updated for preview/`--confirm`.

## Tests
- `quorum_submit_without_confirm_is_rejected`
- `change_account_submit_without_confirm_is_rejected`
- `quorum_preview_returns_fingerprint`
- `change_account_preview_returns_fingerprint`
- Full `cargo test -p knot-tool`: **41 passed**

## Concerns
- Submit still returns 501 in `DEMO_MODE=mock` (unchanged); drawer hidden in mock. Preview works in mock for local fingerprint UX testing.
- Multi-signer per serve is soft-enforced (note only), not hard-rejected — matches IMPLEMENTATION §11 “prefer” wording.
- `quorum-agg submit` confirm test not duplicated (same gate as quorum submit); could add if desired.

## Commits
(See git log on `feat/public-ready-v3-rename` after push.)
