#!/usr/bin/env bash
# Run a gate in CI. Map exit 2 (advisory) → 0 unless STRICT_GATE_WARN=1.
# See bin/EXIT-CODES.md.
set -uo pipefail
"$@"
ec=$?
if [[ "$ec" -eq 2 && "${STRICT_GATE_WARN:-0}" != "1" ]]; then
  echo "ci-accept-warn: advisory exit 2 from $* — treating as pass (STRICT_GATE_WARN unset)" >&2
  exit 0
fi
exit "$ec"
