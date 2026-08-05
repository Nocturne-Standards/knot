---
id: 6
slug: tool-uniquifier-blobs
status: TODO
owner: null
deps:
  - 4
scope:
  - crates/knot-tool/
  - crates/knot-tool/
acceptance:
  - CSPRNG uniquifier + --nonce for proposals (caller-supplied; not change-account)
  - M8 fetch threshold / honest offline label
  - M9 verify partials locally
  - L7 Result aggregate; L8 atomic blob write; L14 typed errors
  - R5 collector URL allowlist (loopback or https only) before Basic Auth
  - R11 client validate proposal id as 64-hex before path use
acceptanceDone:
  - false
  - false
  - false
  - false
  - false
  - false
---
# Phase 4a: tool uniquifier + blob hardening (+ R5, R11)

Authority: `docs/internal/IMPLEMENTATION.md` §2.6 uniquifier, §4.1, §11 R5/R11.

Lab session/cookie work is **#14**, not this leaf.

## Evidence (worker)

## Proposal (worker, if BLOCKED)
