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

- **Implementing a fix** → `IMPLEMENTATION.md`. §1–§5 LOCKED. Read per-item markers.
- **Execution order** → `IMPLEMENTATION.md` §10 (phasing). Leaves after §10 accepted.
- **Preparing the public repo** → `IMPLEMENTATION.md` §5.
- **Setting up a new public repo** → `PUBLIC-REPO-STANDARD.md`.
- **Checking what was decided and why** → `IMPLEMENTATION.md` §8 (decision log).
- **Product scope, framing, roadmap** → `IMPLEMENTATION.md` §9. Not audit findings.

## Status at `46a64b4`

| | |
|---|---|
| Ready to implement | **§1–§5 LOCKED** — blockers, contracts v3, keystore v2, tool, collector, registry, hygiene |
| Settled this pass | Deadline forbid-0, TTL/`MAX_PROPOSAL_TTL`, uniquifier, events/decoder, dusk-core doc rewrite (§8) |
| Phasing | `IMPLEMENTATION.md` §10 — accept before cutting leaves |
| Open / later | `blst` deprioritised; §9.3/§9.4 product; `nocturne-event-decoder` extract DEFERRED |
| Not yet audited | `rpc.rs`, `main.rs`, `chain.rs`, tool `store`/`dto`/`collector_client`/`mock_ledger`, Lab JS — phase 1 |

Triage on the unaudited files found no high-severity issues: SQL is parameterized,
the Lab escapes HTML at every sink, the API token compares in constant time, and
`rusk-wallet` is invoked with argument arrays rather than a shell string. A full read
is still outstanding (phase 1).

## Rules

1. **Every audit or design document carries the commit SHA it was written against.**
   See `IMPLEMENTATION.md` §6.1 for why.
2. Decisions are recorded in `IMPLEMENTATION.md` only — never in two places.
3. Superseded content is corrected in `IMPLEMENTATION.md` §7, not edited in place in
   the frozen audit.
4. Do not invent compatibility work for private, unused prior deployments.
