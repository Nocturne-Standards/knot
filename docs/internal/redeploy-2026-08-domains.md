# Redeploy checklist — nocturne.knot domain bump (2026-08)

Breaking change: proposal and `change_account` signing digests now use v2
domain tags (`nocturne.knot.*`) instead of legacy `sme-platform.*` strings.

## What changed

| Surface | Old domain | New domain |
|---------|-----------|------------|
| Proposal §4a preimage | `sme-platform.multisig.proposal.v1` | `nocturne.knot.multisig.proposal.v2` |
| Registry `change_account` | `sme-platform.knot-registry.change_account.v1` | `nocturne.knot.multisig-registry.change_account.v2` |

Constants: `DOMAIN_PROPOSAL_V2`, `DOMAIN_CHANGE_ACCOUNT_V2` in
`crates/knot-encoding/src/lib.rs`.

## Operator steps

1. **Rebuild WASM** — `knot-registry` and `knot-proposals` (both
   depend on `knot-encoding` for digest verification).
2. **Deploy** both contracts to target network (testnet first). Use the
   `sme_platform` wrappers — they exec shared scripts in `nocturne-deployments`
   and dual-write primary + local mirror pins:

   ```bash
   # from sme_platform (wallet .env.testnet loaded via CALLER_REPO_ROOT):
   ./scripts/deploy-contract.sh /path/to/knot/crates/knot-registry -y
   ./scripts/deploy-contract.sh /path/to/knot/crates/knot-proposals -y
   ```

   Primary pin lands in `nocturne-deployments/testnet.json`; mirror in
   `sme_platform/deployments/testnet.json`. From knot without sme wrapper:

   ```bash
   export CALLER_REPO_ROOT=/path/to/knot
   export NOCTURNE_DEPLOYMENTS_ROOT=/path/to/nocturne-deployments
   $NOCTURNE_DEPLOYMENTS_ROOT/scripts/deploy-contract.sh crates/knot-registry -y
   ```

   Override primary pin only if needed: `export DEPLOYMENTS_FILE=…` before deploy.

3. **Invalidate old committee messages** — any partial signatures or quorum
   blobs signed under `sme-platform.*` domains will fail verification after
   redeploy. Operators must:
   - Re-sign pending proposals with the updated tool/collector.
   - Re-collect `change_account` quorum signatures if a rotation was in flight.
4. **Update tooling** — ensure `knot-tool` / `knot-collector` are on
   the same `knot-encoding` revision before asking signers to approve.
   Pin lookup uses `nocturne-deployments` (symlink `deployments` →
   `aichbindas/nocturne-deployments`, or `NOCTURNE_DEPLOYMENTS`).

## Verification

```bash
cargo test -p knot-encoding
(cd crates/knot-registry && make test)
(cd crates/knot-proposals && make test)
```

Golden digests (sample vectors):

- Proposal (`sample_intent`): `8426c1fa5895fe6b2e3a3fe0e3588eaff4b123fde07b075352264f41dfd9c9dd`
- `change_account` (account=1, nonce=0, 2×96-byte pks, threshold=2):
  `ab2fc0f6d9b490a645b0b5768bcfbfabfce53392251f28bc776e10b6ad22c457`
