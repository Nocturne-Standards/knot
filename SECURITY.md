# Security

## Reporting a vulnerability

Email **security@nocturne-standards.org** with:

- Affected crate or component (`knot-tool`, `knot-collector`, contracts, etc.)
- Steps to reproduce
- Impact assessment (confidentiality, integrity, availability)

We aim to acknowledge within **72 hours** and share a remediation timeline for
confirmed issues.

Please do **not** open public GitHub issues for undisclosed vulnerabilities.

## Scope

In scope: this repository's Rust crates, WASM contracts, and documented HTTP
APIs (`knot-collector`, `knot-tool` local RPC).

Out of scope: third-party nodes (`rusk-wallet`, Dusk testnet infrastructure),
hosted deployments we do not operate, and social-engineering attacks.

## Safe use

- **Testnet only** — no mainnet deployment claim; do not use production keys.
- **Signing stays local** — `knot-tool` holds secrets; `knot-collector` must
  never receive secret keys.
- **Verify digests out-of-band** before signing proposal intents.

See [`docs/security-model.md`](docs/security-model.md) for the full trust model.
