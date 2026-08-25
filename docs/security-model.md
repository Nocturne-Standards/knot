# Security model

How trust is split across the multisig suite. Read this before integrating
`knot-registry`, `knot-proposals`, or hosting `knot-collector`.

## Dual posture: Coord vs Prove

Two distinct trust postures show up when talking about "is this quorum
valid?" — keep them separate, they are not interchangeable:

- **Prove** — the chain independently re-derives the answer. A contract call
  (`verify_quorum`, `verify_quorum_aggregate`, `change_account`, or
  `knot-proposals::approve`/`finalize`) itself checks the signer set
  against the *live* on-chain member list and threshold, and cryptographically
  verifies each BLS signature (or the aggregate) against that live state. The
  caller's inputs (which signers, which sigs) are just evidence; the contract
  never trusts them without re-checking. **This is what every write path in
  `knot-registry` and `knot-proposals` actually does today** —
  confirmed by code read (2026-08-04): `verify_quorum`/`verify_quorum_aggregate`
  check membership + threshold + `abi::verify_bls`/`verify_bls_multisig`
  in-contract; `knot-proposals::approve` panics if `signer` isn't a
  current registry member and re-verifies the BLS signature; `finalize` calls
  `verify_quorum` again over the collected approvals before `call_raw`.
- **Coord** — an off-chain coordinator (`knot-tool`, or any last-mile
  validator standing in front of a target) decides quorum is met and the
  chain-side effect trusts that decision without an equivalent independent
  re-check. Nothing in *this* suite's own contracts implements pure Coord —
  every on-chain entry point above re-verifies. Coord-style trust currently
  only exists **inside `knot-tool` itself**, before anything reaches
  chain: the tool's digest-recompute gate (anti-blind-signing) is real and
  enforced, but membership/threshold gating before producing a signature is
  not (see `crates/knot-tool/` audit, 2026-08-04) — a signature the tool
  produces for a non-member is *useless* on-chain (Prove rejects it), but the
  tool does not tell the operator that before burning a signing round.

**Decision:** Prove is the only mode this suite offers cryptographic
guarantees for, and is not optional — it is what the deployed contracts do.
Do not describe `knot-tool` as providing an independent authorization
gate ("Coord as final trust") for any flow whose target contract does not
itself re-verify quorum on-chain the way `knot-registry`/
`knot-proposals` do. If a *new* integrator's target trusts a
tool-assembled decision without its own on-chain re-check (pure Coord), that
target is taking on the tool's entire TCB — call this out explicitly in that
target's own docs; do not assume this suite's docs cover it.

## Topology

```
Local signer (knot-tool)     Untrusted relay (knot-collector)
  • holds BLS secret keys          • stores proposal blobs + partials
  • recomputes digests before sign • never holds keys, never signs
  • submits chain transactions     • never submits on-chain
            │                                  │
            └────────── HTTP (JSON) ───────────┘
                            │
                            ▼
              On-chain (registry + proposals)
```

| Crate | Role | Holds keys? |
|---|---|---|
| `knot-encoding` | Canonical §4a signing preimage / digest helpers + fingerprints | No |
| `knot-registry` | M-of-N quorum **verification** registry (no custody) | No |
| `knot-proposals` | Propose → approve → finalize + `call_raw` execute | No |
| `knot-tool` | Local CLI + loopback web UI for signing / testnet exercise | **Yes** (local only) |
| `knot-collector` | Off-chain blob + partial relay (Safe Transaction Service analogue) | No |

Licenses: Apache-2.0 for encoding/registry/proposals/tool; AGPL-3.0-only for
the collector (see each crate’s `LICENSE` / `LICENSING.md`).

## On-chain rules

### Registry

- Answers: *did ≥ threshold distinct current members sign this `msg`?*
- Does **not** interpret `msg`. **Replay protection is the caller’s job** —
  fold your own domain tag, action, and nonce/context into the bytes you
  verify (the registry’s own `change_account` path is the exception: it uses
  a registry-owned encoding that includes the account nonce).
- `create_account` is permissionless. Naming keys as members grants the
  creator no power over those keys. **Always read back** `account(id)` /
  `member_key_bytes(id)` (and the returned `account_id`) immediately after
  create before wiring the id into proposals, prediction-market councils, or
  any other consumer — never assume “next id” or that your create was the
  only concurrent registration.

### Proposals

- Anyone may propose, relay approvals, or finalize. Authorization is the
  BLS quorum over the on-chain-recomputed §4a digest
  (`chain_id ‖ committee ‖ nonce ‖ target ‖ fn ‖ args ‖ deadline`).
- On finalize, the contract calls `call_raw(target, function_name, call_args)`.
  **Targets must independently require `abi::caller() == proposals`.**
  This suite does not maintain a target allowlist.
- **Never propose owner-gated methods on the proposals contract itself**
  (e.g. `set_tombstone`, `init_chain_id`, `set_proposal_ttl`) unless that is
  intentional governance of the proposals contract — finalize would invoke
  them via `call_raw` with `caller == proposals`, which satisfies typical
  owner checks that only compare `abi::caller()` / owner to the proposals id
  (ops hazard M2; not a protocol lock).
- Treat proposal `human_summary` (off-chain) as untrusted display text.
  Sign only after recomputing the digest from canonical fields
  (`knot-encoding::gate_blob_for_signing` / `recompute_and_verify`).

## Off-chain rules

### Tool (`knot-tool`)

- **Testnet only.** Network endpoints are hard-locked; do not point it at
  mainnet keys or funds.
- Secret keys live in-process or in a local encrypted keystore
  (`AES-256-GCM`, password-derived). The browser UI never receives secret
  keys — only names, public keys, digests, and signatures.
- The local RPC binds loopback only and requires a process-scoped bearer
  token on `/api/*`.
- Prefer interactive password entry. `KNOT_PWD` is only honored when
  `KNOT_ALLOW_ENV_PWD=1` is also set (scripting opt-in); otherwise the
  tool errors with a clear message.
- **Lab membership pre-check (A4 / 2026-08-04 audit):** generic approve /
  quorum / change-account sign paths fetch live registry `account` and refuse
  to sign when a local signer PK is not in `view.members` (HTTP **403** /
  CLI error). Prove-mode on-chain re-verification remains the cryptographic
  guarantee if this gate is bypassed.

### Member public-key order (A8)

`change_account_digest` folds `member_pks` in **caller-supplied order** (same
order as `new_members` on-chain). Encoding does not sort or canonicalize.
Co-signers must agree on that order; a permutation yields a different digest.

### Owner gates vs `call_raw` (A9)

Owner-only methods use `require_owner()` against `abi::public_sender()`. A
proposals `finalize` that `call_raw`s an owner-gated method on the proposals
contract itself can satisfy `abi::caller() == proposals` checks — do not
propose those methods unless intentional. Execute targets should authorize
via `abi::caller() == proposals` (checklist item 4).

### Collector

- Assume the collector can lie, omit, reorder, or append junk partials.
  Same-`signer_pk` appends **replace** (last-write-wins) so a real signer can
  overwrite junk; the collector still does not verify BLS.
- Honest signers must **re-gate** digests after pull and reject mismatched
  blobs. On-chain quorum checks reject invalid signatures.
- Production deploys must put authentication and TLS in front of the process
  (the binary itself is an unauthenticated relay by design). Bind the app to
  loopback (non-loopback requires `KNOT_COLLECTOR_ALLOW_NON_LOOPBACK=1`)
  and terminate auth at the reverse proxy.
- Party-finder roster is upsert-only (no public DELETE); entries are discovery
  aids only — they do not prove key ownership and do not authorize chain
  actions.
- Collector may still accept `kind=pm_council_resolve` JSON for **wen** wire
  compatibility; Knot Lab product surface no longer creates those blobs.

## Integrator checklist

1. Domain-separate every message you pass to `verify_quorum`.
2. Bind replay context (nonce, one-shot state, or both) into that message.
3. After `create_account`, verify `account` / `member_key_bytes` (and the
   returned id) before wiring the id into another contract — concurrent
   creates can advance `next_account_id`; never trust an assumed id.
4. If you are a `call_raw` target of `knot-proposals`, check
   `abi::caller()` (not `public_sender`) against the proposals contract id.
5. Never treat collector content or free RUES “verify” reads as authoritative
   for live signature correctness — use gated local recompute + on-chain
   writes / account reads.
6. **Prediction-market council resolve** lives in **wen** (`pm-council-tool` +
   `pm-council-encoding`). Domain:
   `nocturne.wen.prediction-market.council-resolve.v3`. Knot does not export
   `council_resolve_*` digests.

## Related

- Suite overview: [`../README.md`](../README.md)
- Per-crate detail: each crate’s `README.md`
- Domain redeploy history: `knot-registry` and `knot-proposals`
  `CHANGELOG.md` entries; redeploy semantics: [`versioning.md`](versioning.md)
  (`PINNED-DIFFERENT-REDEPLOYED`)
