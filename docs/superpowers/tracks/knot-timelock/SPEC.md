---
planner_model: cursor-grok-4.6-medium
worker_model: composer-2.5
reviewer_model: claude-sonnet-5-thinking-high
---
# SPEC — knot-timelock

> **Frozen.** Atlas-optional is history. Knot and Atlas stay together (D-76). Do not continue this track. `STATUS.md` is generated; do not hand-edit it.

## Goal

Historical goal, not current design: add Atlas-style per-account timelock to knot-registry (change_account / set_timelock) and queue proposal call_raw until execute_at. One delay knob per council, default 0. Proposal digest v3 unchanged. The line "Knot standalone; Atlas optional" is withdrawn.

## Scope

## Non-goals

## Acceptance (track-level)
