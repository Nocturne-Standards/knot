# DECISIONS — audit-2026-08-full

_Append-only. Use decision_add / planner._

## 2026-08-04T13:50:02.313Z — Dual mode Coord vs Prove — offer Prove only

Explored whether knot's multisig suite should offer both a "Coord" (off-chain
coordinator/knot-tool is final trust) and a "Prove" (on-chain verify_quorum /
verify_quorum_aggregate re-derives quorum from live state) integration mode.

Finding (code-verified 2026-08-04): every write path in knot-registry
(verify_quorum, verify_quorum_aggregate, change_account) and knot-proposals
(approve, finalize) independently re-checks membership + threshold + BLS
signatures against live on-chain state before honoring a quorum claim. There is
no pure-Coord path anywhere in this suite's own contracts today — Coord-style
trust only exists transiently inside knot-tool, before anything reaches
chain (the digest-recompute gate is real; membership/threshold pre-checks are
not — see signing-tool-tcb-* fix leaves in docs/security-audit-2026-08-04.md).

Decision: offer Prove as the only supported/documented mode for this suite.
Do not market knot-tool as providing an independent Coord-style
authorization boundary for any target whose contract doesn't itself
re-verify quorum on-chain the way registry/proposals do. docs/security-model.md
updated with an explicit "Dual posture: Coord vs Prove" section so integrators
don't assume "tool decided" is sufficient without an on-chain re-check.
Future integrators who do want pure Coord (trusting the tool's decision
without independent on-chain re-verification) take on the tool's entire TCB
and must document that themselves — not covered by this suite's guarantees.

## 2026-08-04T13:54:35.074Z — Fix leaves queued from 2026-08-04 audit (Medium+)

Opened leaves for Critical/High/Medium findings in docs/security-audit-2026-08-04.md. Dual-mode decision already recorded: Prove only. Implementation deferred.

## 2026-08-04T16:24:57.957Z — Fix leaves 010–014 superseded; goals moved

#10 → wen://pm-peel-and-fixes#2 a1-membership-gate-sign
#11 → wen://pm-peel-and-fixes#3 a2-submit-target-crosscheck
#12 → wen://pm-peel-and-fixes#4 a3-pm-abi-parity
#13 → knot://launch-form-knot#7 a4-generic-membership-gate
#14 → wen://pm-peel-and-fixes#5 a5-threshold-live-check
