# Redeploy checklist — nocturne.knot domain bump (2026-08)

Breaking change: proposal and `change_account` signing digests now use v2
domain tags (`nocturne.knot.*`) instead of legacy `sme-platform.*` strings.

## What changed

| Surface | Old domain | New domain |
|---------|-----------|------------|
| Proposal §4a preimage | `sme-platform.multisig.proposal.v1` | `nocturne.knot.multisig.proposal.v2` |
| Registry `change_account` | `sme-platform.multisig-registry.change_account.v1` | `nocturne.knot.multisig-registry.change_account.v2` |

Constants: `DOMAIN_PROPOSAL_V2`, `DOMAIN_CHANGE_ACCOUNT_V2` in
`crates/multisig-encoding/src/lib.rs`.

## Operator steps

1. **Rebuild WASM** — `multisig-registry` and `multisig-proposals` (both
   depend on `multisig-encoding` for digest verification).
2. **Deploy** both contracts to target network (testnet first).
3. **Invalidate old committee messages** — any partial signatures or quorum
   blobs signed under `sme-platform.*` domains will fail verification after
   redeploy. Operators must:
   - Re-sign pending proposals with the updated tool/collector.
   - Re-collect `change_account` quorum signatures if a rotation was in flight.
4. **Update tooling** — ensure `multisig-tool` / `multisig-collector` are on
   the same `multisig-encoding` revision before asking signers to approve.

## Verification

```bash
cargo test -p multisig-encoding
(cd crates/multisig-registry && make test)
(cd crates/multisig-proposals && make test)
```

Golden digests (sample vectors):

- Proposal (`sample_intent`): `8426c1fa5895fe6b2e3a3fe0e3588eaff4b123fde07b075352264f41dfd9c9dd`
- `change_account` (account=1, nonce=0, 2×96-byte pks, threshold=2):
  `ab2fc0f6d9b490a645b0b5768bcfbfabfce53392251f28bc776e10b6ad22c457`
