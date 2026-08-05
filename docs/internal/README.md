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
- **Execution order** → `IMPLEMENTATION.md` §10. Track: `public-ready-v3`.
- **Residual findings** → `IMPLEMENTATION.md` §11 — **review before phase 2**.
- **Preparing the public repo** → `IMPLEMENTATION.md` §5.
- **Checking what was decided and why** → `IMPLEMENTATION.md` §8.
- **Product scope** → `IMPLEMENTATION.md` §9.

## Status at `b1d883d`

| | |
|---|---|
| Ready to implement | **§1–§5 LOCKED** |
| Residual audit | **Done** — §11 dispositions locked; leaf **#14** Lab/RPC; **#6**/**#8** extended |
| Next | Phase 2 rename (`#3`) and/or `#14` Lab hardening |
| Deferred | Launch ops §10; `nocturne-event-decoder` extract; product §9 |

## Rules

1. **Every audit or design document carries the commit SHA it was written against.**
   See `IMPLEMENTATION.md` §6.1 for why.
2. Decisions are recorded in `IMPLEMENTATION.md` only — never in two places.
3. Superseded content is corrected in `IMPLEMENTATION.md` §7, not edited in place in
   the frozen audit.
4. Do not invent compatibility work for private, unused prior deployments.
