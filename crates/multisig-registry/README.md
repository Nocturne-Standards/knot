# multisig-registry

BLS M-of-N quorum verification registry — a shared primitive other contracts
in this repo can point at instead of each re-implementing committee/
threshold logic themselves.

## Scope

This is a **verification** registry, not a custody wallet. It never holds
Dusk or any token — unlike Dusk's own [`multisig-contract` example]
(`references/repos/multisig-contract`), which deposits/transfers Dusk to
Moonlight accounts. This contract answers one question: *did enough of this
account's members sign this message?* Callers build authorization on top of
that answer — e.g. `prediction-market`'s dispute council
(`init_dispute_council(registry, account_id)` + `resolve` → `verify_quorum`
over `council-resolve.v2`; lab green 2026-07-24, live redeploy/wire still
open — see `prediction-market/crates/prediction-market/README.md`), or a
future `compliance-gate` operator council.

[`multisig-contract` example]: ../../../references/repos/multisig-contract

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
  over `multisig_encoding::change_account_message` (domain + account_id +
  on-chain nonce + new member pks + new threshold → Keccak-256). Nonce is
  not an args field — it is folded from state into the digest. This path
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
  workaround needed — see
  `references/dusk-native/crate-source-locations.md` for where
  `bls12_381-bls`'s aggregation logic actually lives.

### Diagnostic helpers (ops / investigation)

- `account_meta(u64) -> Option<AccountMeta>` — threshold, nonce, members_len
  (no BLS keys on the wire).
- `member_key_bytes(u64) -> Option<Vec<Vec<u8>>>` — raw 96-byte compressed
  member PKs (each inner vec length 96).
- `next_account_id() -> u64` — next id `create_account` will allocate.
- `diagnose_quorum(VerifyQuorumArgs) -> DiagnoseQuorumResult` — membership /
  verify counters plus member key dump. Free-read over RUES may HTTP 500
  when `abi::verify_bls` runs (same as `verify_quorum` free-read); useful
  under `VM::ephemeral()` and when the node path works.

## Status

v0.1.3 on testnet (contract id in `../../../deployments/testnet.json`).
Audit #6: `checked_add` on `next_id`/`nonce` (audit 2026-07-28); **testnet
redeploy still pending** (local wasm/tests only).
`make wasm` + `make wasm-dd` + `cargo test --release`, 14/14 green.
Same dusk-forge-template pattern as `identity-credential`/`compliance-gate`
(wasm32-unknown-unknown, Rust 1.94.0, `#[dusk_forge::contract]`).
`verify_quorum_aggregate`'s tests exercise the real `verify_bls_multisig`
host query under `VM::ephemeral()` (not mocked) — signing uses
`sign_multisig_insecure`, not the default secure `sign_multisig`, because
of a `dusk-vm` gotcha: see
`references/dusk-native/dusk-vm-issue-1-ephemeral-hardfork-policy-unreachable.md`.
**Live testnet clients must use secure `sign`/`sign_multisig`** — see
[`../multisig-tool/README.md`](../multisig-tool/README.md). Wired into
`prediction-market::resolve` via `verify_quorum` (not yet the aggregate
path — see Next steps).

## Next steps

- Wire `prediction-market::resolve`'s council path (currently
  `verify_quorum`, the per-signature form) over to
  `verify_quorum_aggregate`. The off-chain aggregation flow this needs now
  exists — `multisig-tool`'s `blob create|sign|aggregate|submit-agg` (M2)
  assembles a `MultisigSignature` aggregate off-chain and submits it into
  `verify_quorum_aggregate` (see [`../multisig-tool/README.md`](../multisig-tool/README.md)
  and its `tests/blob_aggregate_local.rs`). What remains is pointing
  prediction-market's council at that path.
- Watch for dusk-forge v0.3.0's two-`ContractId`-in-one-argument codegen
  bug (see root `CLAUDE.md`) if a future method here ever needs to
  reference another contract's `ContractId` alongside anything else.
- See `rusk-experiments/multisig-approval` for a standalone sandbox demo of
  the same `verify_bls_multisig` primitive (predates this module) — worth
  comparing notes if either one's approach changes.
