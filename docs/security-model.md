# Security model

How trust is split across the multisig suite. Read this before integrating
`multisig-registry`, `multisig-proposals`, or hosting `multisig-collector`.

## Topology

```
Local signer (multisig-tool)     Untrusted relay (multisig-collector)
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
| `multisig-encoding` | Canonical §4a signing preimage / digest helpers + fingerprints | No |
| `multisig-registry` | M-of-N quorum **verification** registry (no custody) | No |
| `multisig-proposals` | Propose → approve → finalize + `call_raw` execute | No |
| `multisig-tool` | Local CLI + loopback web UI for signing / testnet exercise | **Yes** (local only) |
| `multisig-collector` | Off-chain blob + partial relay (Safe Transaction Service analogue) | No |

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
  (`multisig-encoding::gate_blob_for_signing` / `recompute_and_verify`).

## Off-chain rules

### Tool (`multisig-tool`)

- **Testnet only.** Network endpoints are hard-locked; do not point it at
  mainnet keys or funds.
- Secret keys live in-process or in a local encrypted keystore
  (`AES-256-GCM`, password-derived). The browser UI never receives secret
  keys — only names, public keys, digests, and signatures.
- The local RPC binds loopback only and requires a process-scoped bearer
  token on `/api/*`.
- Prefer interactive password entry. `MULTISIG_TOOL_PWD` is only honored when
  `MULTISIG_TOOL_ALLOW_ENV_PWD=1` is also set (scripting opt-in); otherwise the
  tool errors with a clear message.

### Collector

- Assume the collector can lie, omit, reorder, or append junk partials.
  Same-`signer_pk` appends **replace** (last-write-wins) so a real signer can
  overwrite junk; the collector still does not verify BLS.
- Honest signers must **re-gate** digests after pull and reject mismatched
  blobs. On-chain quorum checks reject invalid signatures.
- Production deploys must put authentication and TLS in front of the process
  (the binary itself is an unauthenticated relay by design). Bind the app to
  loopback (non-loopback requires `MULTISIG_COLLECTOR_ALLOW_NON_LOOPBACK=1`)
  and terminate auth at the reverse proxy.
- Party-finder roster is upsert-only (no public DELETE); entries are discovery
  aids only — they do not prove key ownership and do not authorize chain
  actions.

## Integrator checklist

1. Domain-separate every message you pass to `verify_quorum`.
2. Bind replay context (nonce, one-shot state, or both) into that message.
3. After `create_account`, verify `account` / `member_key_bytes` (and the
   returned id) before wiring the id into another contract — concurrent
   creates can advance `next_account_id`; never trust an assumed id.
4. If you are a `call_raw` target of `multisig-proposals`, check
   `abi::caller()` (not `public_sender`) against the proposals contract id.
5. Never treat collector content or free RUES “verify” reads as authoritative
   for live signature correctness — use gated local recompute + on-chain
   writes / account reads.
6. **Prediction-market council resolve (v2):** signers authorize
   `keccak(DOMAIN_V2 || pm_contract_id[32] || registry_account_id_le64 ||
   threshold_le32 || market_id_le64 || outcome_u8)`. On-chain uses
   `abi::self_id()`, the stored dispute-council account id, and the account’s
   **current** registry threshold (read at authorize time). Mid-flight
   threshold changes stale partials; wrong submit target or account fails
   verify. Tool preview/confirm before sign; never trust `human_summary`.

## Related

- Suite overview: [`../README.md`](../README.md)
- Per-crate detail: each crate’s `README.md`
