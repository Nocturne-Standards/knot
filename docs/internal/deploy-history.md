# Deploy history (maintainer notes)

Operator-facing timeline moved out of the public README. For current crate
versions see root `README.md` and [`../versioning.md`](../versioning.md).

## 2026-08-02 — Wave 7 carve target

Private GitHub repo [`aichbindas/knot`](https://github.com/aichbindas/knot).
Nest folder name stays `multisig/` until hard-cut. Per-crate versions live in
each `Cargo.toml`.

## 2026-07-27 — product name Knot

Public surfaces: `knot.nocturne-standards.org`, docs `/v1/knot/`. Code/repo
folder remains `multisig` until carve completes.

## 2026-07-23 — nested workspace carve

Hard-cut nested workspace; see parent platform roadmap Track 7 for per-crate
deploy status at the time.

## 2026-07-24 — first product consumer path

Prediction-market dispute council resolve (later peeled to **wen**; knot no
longer ships `pm-resolve`).

## 2026-07-28 — audit overflow/encoding cutover on testnet

Registry **v0.1.4** (`4e24b59d…`), proposals **v0.3.1** (`5e91ddb6…`,
`init_registry` + `init_chain_id=2` wired). PM dispute council on Atlas logic
**v0.2.0** (`b7543a2b…`) pointed at registry account **0**.

## 2026-07-24 — audit remediation (superseded for registry/proposals by 2026-07-28)

Registry **v0.1.3**, proposals **v0.3.0**, PM monolith **v0.3.1**
(`council-resolve.v2` + `init_treasury` re-wired). Dispute council wired to
registry account **0** (`init_dispute_council` tx `b7f02c0c…`).

## 2026-07-26 — Multisig Lab website demo

`DEMO_MODE=mock` default / `DEMO_MODE=testnet` optional; slide Lab on `:8877`;
public docs at
[`docs.nocturne-standards.org/v1/knot/`](https://docs.nocturne-standards.org/v1/knot/).
