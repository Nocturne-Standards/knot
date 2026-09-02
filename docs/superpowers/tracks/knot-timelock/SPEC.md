---
planner_model: cursor-grok-4.6-medium
worker_model: composer-2.5
reviewer_model: claude-sonnet-5-thinking-high
---
# SPEC — knot-timelock

## Goal

Add Atlas-style per-account timelock to knot-registry (change_account / set_timelock) and queue proposal call_raw until execute_at. One delay knob per council, default 0. Proposal digest v3 unchanged. Docs: Knot standalone; Atlas optional; Atlas delay 0 when paired.

## Scope

## Non-goals

## Acceptance (track-level)
