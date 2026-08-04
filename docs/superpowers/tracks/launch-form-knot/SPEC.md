---
planner_model: cursor-grok-4.5-medium
worker_model: composer-2.5
reviewer_model: claude-sonnet-5-thinking-high
---
# SPEC — launch-form-knot

## Goal

Gap map Kind A (fix/improve existing) + Kind B/C (missing required vs optional) for knot public-ready launch form. No fix code. Then discuss launch-form-knot.md.

## Scope

Per design docs/superpowers/specs/2026-08-04-knot-launch-form-design.md:
- Kind A: existing fix/improve (audit cross-walk)
- Kind B: missing required for launch
- Kind C: missing optional / defer
- Tests/goldens map; doc hygiene inventory
- Deliverable: docs/launch-gap-map-2026-08.md

## Non-goals

Fix coding; chit/ambit maps; publish.

## Acceptance (track-level)

- docs/launch-gap-map-2026-08.md complete with A/B/C rows
- docs/doc-hygiene-inventory.md landed
- Every audit Medium+ cross-walked or explicit wontfix/claim-change
