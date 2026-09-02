# Design notes

Rationale for choices that look like oversights but are deliberate. Integrators
ask about these regularly.

## 1. No on-chain diagnostics

A WASM contract cannot be feature-flagged at runtime: shipping two artifacts would
mean mainnet is not the bytecode tested on testnet. Diagnostics are pure
computation over already-readable data, so they live in `knot-tool` (off-chain).
Registry methods such as `diagnose_quorum` were removed from the on-chain ABI for
this reason.

## 2. Flat preimage, not EIP-712 nesting

Proposal and `change_account` digests use a length-prefixed flat byte
concatenation hashed with Keccak-256. This is cryptographically equivalent to
nested EIP-712-style structures. Reviewers arriving from Ethereum should not have
to guess — the encoding is explicit in `knot-encoding`.

## 3. `finalize` is permissionless

Authorization lives in the BLS signatures over the proposal digest, not in the
caller identity. Anyone may call `finalize` once threshold approvals are on
chain (and anyone may `execute` a queued proposal after `execute_at`). That
lets a funded relayer pay gas while council members hold no DUSK.

## 4. Delay vs deadline

`deadline` is an expiry: the action must happen **by** that block height.
`timelock_blocks` is a delay: after quorum, the action must wait **until**
`execute_at`. They are not interchangeable. Cancel of a queued proposal or
registry pending is immediate (new quorum), so observers can stop a stolen-key
action during the wait.

Atlas is a separate layer (named services, roles, admin gate), not a second
Knot. Atlas can delay its own admin path. Two delays stack. The usual pairing
leaves Atlas `timelock_blocks` at 0 so Knot’s wait is the only one. Atlas
remains optional — Knot does not depend on it.

## 5. No proof of possession required

`sign_multisig` scales by `t = h1(pk)` (BDN construction), which defeats
rogue-key attacks by design. Separate proof-of-possession ceremonies are not
required for registry membership keys.

## 6. Collector threat model and `gate_blob_for_signing`

An untrusted `knot-collector` can withhold blobs, reorder partials, squat ids,
or (without hardening) serve corrupted partials. It **cannot** forge a signature
or induce signing over the wrong intent: every signer must pass
`gate_blob_for_signing`, which recomputes the digest from canonical intent fields
before signing. That gate is the load-bearing control — name it in reviews and
runbooks.
