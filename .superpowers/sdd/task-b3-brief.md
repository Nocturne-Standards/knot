### Task B3: Contracts v3 (leaf #5)

**Worktree:** `/Users/leonidas/dev/aichbindas/knot/.worktrees/public-ready-v3-rename`
**Branch:** `feat/public-ready-v3-rename`
**Leaf:** #5 `contracts-v3` IN_PROGRESS
**Deps:** #4 encoding-v3 DONE (`DOMAIN_*_V3` + builders in `knot-encoding`); #2 chain_id OK via `genesis_session`

## Authority

Read carefully:
- `docs/internal/IMPLEMENTATION.md` §2.5–2.15 (state, methods, digests, events, tests, migration)
- Leaf `docs/superpowers/tracks/public-ready-v3/leaves/005-contracts-v3.md`

## Acceptance

1. State/methods per §2.10–2.11 (proposals: epoch, DigestRecord, remove chain_id/committee_nonces, ProposeArgs.nonce, prune, set_proposal_ttl no wipe, init_registry bumps epoch, remove init_chain_id/wipe_open_proposals)
2. Registry change_account digests use DOMAIN_CHANGE_ACCOUNT_V3 (chain_id, self_id, member_count)
3. Rich events §2.13
4. Tests §2.15 table (as many as fit; critical H1/M1/M3/prune/epoch/deadline cases must pass)
5. Document redeploy order: registry then proposals; burn v2 (README/CHANGELOG)

## Practical notes

- Use `knot_encoding` v3 digest helpers from leaf #4
- claim.py: acquire knot-registry / knot-proposals before make wasm/test; never parallel duplicate suites
- Update call_types / ProposeArgs in encoding if needed (ProposeArgs.nonce) — that may live in knot-encoding call-types; update goldens
- Tool may break until #6 — prefer keep tool compiling if cheap (feature flag or temporary dual); if tool breaks hard, note in report and leave #6 to fix

## Deliverables

- Commits + push
- Leaf Evidence filled
- Report: `.superpowers/sdd/task-b3-report.md`

If too large, implement proposals+registry core + critical tests first and report DONE_WITH_CONCERNS listing remaining §2.15 rows — do NOT silently skip critical H1/prune/epoch.
