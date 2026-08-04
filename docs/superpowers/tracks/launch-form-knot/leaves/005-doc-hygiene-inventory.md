---
id: 5
slug: doc-hygiene-inventory
status: DONE
owner: gap-map-worker
deps:
  - 1
scope:
  - README.md
  - docs/
  - crates/*/README.md
acceptance:
  - docs/doc-hygiene-inventory.md with keep-public / move / delete tags
acceptanceDone:
  - true
---
# Doc hygiene inventory

Planner context…

## Evidence (worker)

`docs/doc-hygiene-inventory.md` landed with all 4 categories
(keep-public / move-to-docs-internal / move-to-nocturne-docs /
delete-duplicate) applied to: root `README.md` (section-by-section),
`docs/security-model.md`, all 3 security-audit docs, `docs/superpowers/**`
(flagged move-to-nocturne-docs / exclude from public tag — it's agent
process scaffolding, not product docs), root `AGENTS.md` +
`.cursor/rules/*.mdc` (same disposition), and every `crates/*/README.md`
section-by-section. Separately enumerated all 18 dead `../../../` links
(and 3 bare `references/...` links, one of which — `chain.rs:315,329` —
is live CLI error text, not just a doc citation) with a per-link
disposition, verified by checking existence from each citing file's own
directory (not from repo root, which gives false positives/negatives —
confirmed the difference empirically before finalizing the table).
Summary counts: 24 keep-public, 11 move-to-docs-internal, 6
move-to-nocturne-docs, 9 delete-duplicate.

## Proposal (worker, if BLOCKED)
