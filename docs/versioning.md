# Versioning policy

Knot ships **testnet-only** contracts and tooling today. There is no mainnet
deployment claim in this repo.

## Per-crate semver

Each workspace crate carries its own `version` in `Cargo.toml`
(`knot-encoding`, `knot-registry`, `knot-proposals`,
`knot-tool`, `knot-collector`). That number is the **source of truth**
for that crate's API and WASM artifact. Crates version independently — a
registry bump does not force a tool bump unless the dependency edge requires it.

`knot-tool --version` and `knot-collector`'s `/v1/health` `version`
field print the same semver as their crate `Cargo.toml`.

## Git tags vs crate versions

Public consumers pin via **git tag or rev** on `https://github.com/aichbindas/knot`,
not crates.io (not published here). A repo tag (e.g. `v0.2.0`) is an
**operator-chosen bundle** of crate versions at a point in time — it is not
required to equal every inner crate's semver. Check the tag's `Cargo.toml`
files for the exact crate versions you are building against.

## PINNED-DIFFERENT-REDEPLOYED

Some releases change **on-wire or on-chain layout** (rkyv `repr(C)` pins,
domain-string bumps, ABI field layout). Those releases are labeled
**PINNED-DIFFERENT-REDEPLOYED** in crate READMEs and changelogs:

- **PINNED** — layout/domain is fixed intentionally (goldens, explicit constants).
- **DIFFERENT** — byte layout or signing digest changed vs the prior deploy.
- **REDEPLOYED** — operators must deploy new WASM and re-wire downstream
  callers; old partial signatures or quorum blobs from the prior layout will not verify.

Treat every contract redeploy as a **new trust anchor** until you confirm IDs
and domain tags match your tooling revision.
