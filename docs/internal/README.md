# Knot internal docs

**Everything in this directory is private.** None of it ships in the public repo.
See `IMPLEMENTATION.md` §5.1 and `PUBLIC-REPO-STANDARD.md`.

---

## Files

| File | Purpose | Authoritative for |
|---|---|---|
| **`IMPLEMENTATION.md`** | Every settled decision and specification | **Everything** |
| `PUBLIC-REPO-STANDARD.md` | Reusable framework for any public repo | Process, doc tiers, CI gate |
| `TOOLING-AUDIT-LESSONS.md` | Process gates from §6 — homes + interfaces | Kit / MCP / CI wiring |
| `AUDIT-2026-08-05.md` | Round-one findings, frozen | **Nothing** — evidence only |
| `deploy-history.md`, `redeploy-2026-08-domains.md` | Pre-existing operator notes | Deploy history |

## Precedence

> **`IMPLEMENTATION.md` wins over everything.**
> `AUDIT-2026-08-05.md` is never authoritative — do not implement from it.

`AUDIT-2026-08-05.md` was written against `2fb3c94`, before PR #3 merged. Two of its
findings were already fixed on the release candidate and one of its claims was simply
wrong. Corrections are listed in `IMPLEMENTATION.md` §7. It is kept because the
evidence and reasoning behind each finding are useful; the instructions are not.

## Where to start

- **Implementing a fix** → `IMPLEMENTATION.md`. §1–§3 are ready to build.
  §4 is agreed but unspecified — do not implement from it.
- **Preparing the public repo** → `IMPLEMENTATION.md` §5.
- **Setting up a new public repo** → `PUBLIC-REPO-STANDARD.md`.
- **Checking what was decided and why** → `IMPLEMENTATION.md` §8 (decision log).
- **Product scope, framing, roadmap** → `IMPLEMENTATION.md` §9. Not audit findings.

## Status at `7e58d4c`

| | |
|---|---|
| Ready to implement | **Everything in §1–§5.** Blockers, contracts v3, keystore v2, tool blobs, collector, registry, repo hygiene — all LOCKED |
| Open | `blst` only, and deprioritised — not blocking (§8) |
| Product scope | Per-council collector, role split, signer UI, `call_args` decoding (§9) |
| Not yet audited | `rpc.rs` (1862 lines), `main.rs` (1588), `chain.rs`, `store.rs`, `dto.rs`, `collector_client.rs`, `mock_ledger.rs`, static JS |

Triage on the unaudited files found no high-severity issues: SQL is parameterized,
the Lab escapes HTML at every sink, the API token compares in constant time, and
`rusk-wallet` is invoked with argument arrays rather than a shell string. A full read
is still outstanding.

## Rules

1. **Every audit or design document carries the commit SHA it was written against.**
   See `IMPLEMENTATION.md` §6.1 for why.
2. Decisions are recorded in `IMPLEMENTATION.md` only — never in two places.
3. Superseded content is corrected in `IMPLEMENTATION.md` §7, not edited in place in
   the frozen audit.
