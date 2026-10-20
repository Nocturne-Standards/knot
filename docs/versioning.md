# Versioning policy

Knot ships **testnet-only** contracts and tooling today. There is no mainnet
deployment claim in this repo.

## Per-crate semver

Each workspace crate carries its own `version` in `Cargo.toml`
(`knot-encoding`, `knot-registry`, `knot-registry-data`, `knot-proposals`,
`knot-proposals-data`, `knot-tool`, `knot-collector`, `knot-warden`,
`knot-warden-encoding`, `knot-event-decoder`).
That number is the **source of truth**
for that crate's API and WASM artifact. Crates version independently — a
registry bump does not force a tool bump unless the dependency edge requires it.

`knot-tool --version` and `knot-collector`'s `/v1/health` `version`
field print the same semver as their crate `Cargo.toml`.

## Git tags vs crate versions

The README pins `knot-encoding` as `x.y.z`. That string is a placeholder.
Nothing in this repo is on crates.io yet (`publish = false` on every crate).
When a crate is published, replace `x.y.z` with its released version.
The list of what still changes at publish time is [`RELEASE-PENDING.md`](../RELEASE-PENDING.md).

A git tag (for example `v0.2.0`) is an **operator-chosen bundle** of crate
versions for a WASM deploy. It is not required to equal every inner crate's
semver. Check the tag's `Cargo.toml` files for the exact crate versions you
are building against.

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
