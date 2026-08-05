# knot-registry

BLS M-of-N quorum verification registry — a shared primitive other contracts
in this repo can point at instead of each re-implementing committee/
threshold logic themselves.

## Scope

This is a **verification** registry, not a custody wallet. It never holds
Dusk or any token — unlike Dusk's own multisig-contract example (deposits/
transfers to Moonlight accounts). This contract answers one question: *did
enough of this account's members sign this message?* Callers build
authorization on top of that answer.

## Functions

- `create_account(CreateAccountArgs) -> u64` — registers a member set +
  threshold, returns an account id. Callable by anyone (naming other keys as
  members grants no power to the caller themselves).
- `account(u64) -> Option<MultisigAccountView>` — read-only view.
- `verify_quorum(VerifyQuorumArgs) -> bool` — pure check: does `sigs` carry
  `>= threshold` valid, distinct-member signatures over the caller-supplied
  `msg`? Returns `false` (never panics) for an unknown account or an
  unmet quorum — panics are reserved for infrastructure failures elsewhere
  in this repo's contracts, not applicable here since this call has none.
  **Replay protection is the caller's responsibility** — this registry
  doesn't interpret `msg`, so a caller must fold its own nonce/context into
  it if replay matters for their use case.
- `change_account(ChangeAccountArgs)` — replaces an account's member set /
  threshold, gated by a quorum of the account's *current* members signing
  over `knot_encoding::change_account_message_v3` (§2.12 v3 domain binds
  `chain_id`, registry `self_id`, account id, on-chain nonce, member count,
  new member pks, new threshold → Keccak-256). Nonce is not an args field — it is folded from state into the digest. This path
  *does* have built-in replay protection, since the registry controls that
  message's format itself. On failure, the panic string includes
  `members`/`threshold`/`member_matches`/`sigs_ok` counters.
- `verify_quorum_aggregate(VerifyQuorumAggregateArgs) -> bool` — same
  question as `verify_quorum`, checked with **one** native
  `abi::verify_bls_multisig` pairing check instead of one `abi::verify_bls`
  per signer. The caller supplies the *subset* of members who actually
  signed plus a single pre-aggregated `MultisigSignature`
  (`MultisigSignature::aggregate` over each signer's
  `sign_multisig(sk, pk, msg)`) — see `call_types.rs`'s doc on the type for
  why a wrong/missing signer invalidates the whole aggregate rather than
  just failing to count. This is the Dusk-specific win over a
  general-purpose EVM chain: BLS signature aggregation collapses an O(N)
  cost (N pairing checks) into O(1) (one), natively, no precompile
  workaround needed — BLS aggregation is native on Dusk.

### Ops helper

- `next_account_id() -> u64` — next id `create_account` will allocate.

`account_meta`, `member_key_bytes`, and `diagnose_quorum` were removed from
the on-chain ABI (IMPLEMENTATION §4.3 L3). Use
[`knot-tool`](../knot-tool/README.md) instead — it derives the same ops
data from `account()` plus local BLS verify (no gas, no RUES verify
free-read pitfalls).

## Status

**v3** on next deploy — §2.12 `change_account` digest binds `chain_id` and
registry instance. Deploy **registry before proposals**; burn all v2
`change_account` signatures. Prior testnet pins (v0.1.x) obsolete after cutover.

`make wasm` + `make wasm-dd` + `cargo test --release`. Same dusk-forge-template
pattern (wasm32-unknown-unknown, Rust 1.94.0, `#[dusk_forge::contract]`).
`verify_quorum_aggregate`'s tests exercise the real `verify_bls_multisig`
host query under `VM::ephemeral()` (not mocked) — signing uses
`sign_multisig_insecure`, not the default secure `sign_multisig`, because
`VM::ephemeral()` unit tests cannot reach post-hardfork signing policy in
dusk-vm (PreFork default). **Live testnet clients must use secure
`sign`/`sign_multisig`** — see
[`../knot-tool/README.md`](../knot-tool/README.md).

**23b Phase B (2026-08-03):** `#[archive_attr(repr(C))]` on shared
`knot-encoding` call types (this crate re-exports). Layout goldens in
`tests/layout_goldens.rs` (post-pin hex). DIFFERENT types include
`MultisigAccountView` / `ChangeAccountArgs` / `AccountMeta` /
`DiagnoseQuorumResult`. Spec 26 source-carry paragraph cleared by this
redeploy (R7). Operator ceremony re-wire of downstream callers (e.g. PM
council) may stay deferred/unwired — OK per Phase B lessons.

## Next steps

- Consumer contracts (e.g. prediction-market council resolve) own their
  `verify_quorum_aggregate` wiring — see **wen** / prediction-market repos.
- Watch for dusk-forge codegen edge cases if a future method needs two
  `ContractId` arguments in one ABI call.
