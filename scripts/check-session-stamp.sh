#!/usr/bin/env bash
# Exit codes: bin/EXIT-CODES.md (0 ok / 1 hard / 2 warn).
# Adopted repos require a fresh nocturne-estate start stamp.
# DEV_WORKFLOW_SKIP_SESSION does NOT waive this check.
set -euo pipefail
KIT="$(cd "$(dirname "$0")/.." && pwd)"
ROOT="$(git rev-parse --show-toplevel)"
_here="$(cd "$(dirname "$0")" && pwd)"
_fc=""
if [[ -f "$_here/lib/estate-fail-contract.sh" ]]; then
  _fc="$_here/lib/estate-fail-contract.sh"
elif [[ -n "${NOCTURNE_AGENT_KIT:-}" && -f "${NOCTURNE_AGENT_KIT}/bin/lib/estate-fail-contract.sh" ]]; then
  _fc="${NOCTURNE_AGENT_KIT}/bin/lib/estate-fail-contract.sh"
elif [[ -f "$KIT/bin/lib/estate-fail-contract.sh" ]]; then
  _fc="$KIT/bin/lib/estate-fail-contract.sh"
fi
if [[ -n "$_fc" ]]; then
  # shellcheck disable=SC1090
  source "$_fc"
fi

[[ -f "$ROOT/.nocturne-agent-kit-adopted" ]] || exit 0

resolve_session_py() {
  if [[ -n "${SESSION_PY:-}" && -f "${SESSION_PY}" ]]; then
    echo "$SESSION_PY"
    return 0
  fi
  if [[ -n "${CURSOR_PROCESS_HYGIENE:-}" ]]; then
    local p="${CURSOR_PROCESS_HYGIENE%/}/bin/session.py"
    if [[ -f "$p" ]]; then
      echo "$p"
      return 0
    fi
  fi
  local home_py="$HOME/.cursor/process-hygiene/bin/session.py"
  if [[ -f "$home_py" ]]; then
    echo "$home_py"
    return 0
  fi
  local sib
  sib="$(cd "$KIT/.." && pwd)/cursor-process-hygiene/bin/session.py"
  if [[ -f "$sib" ]]; then
    echo "$sib"
    return 0
  fi
  return 1
}

SESSION_PY="$(resolve_session_py 2>/dev/null || true)"
if [[ -z "$SESSION_PY" || ! -f "$SESSION_PY" ]]; then
  estate_gate_line stamp fail hard
  estate_hint "install cursor-process-hygiene session.py for stamp check"
  exit 1
fi

SID="${NOCTURNE_SESSION_ID:-}"
if [[ -z "$SID" ]]; then
  estate_gate_line stamp fail hard
  estate_hint "run nocturne-estate start (NOCTURNE_SESSION_ID unset) — SKIP_SESSION does not waive stamp"
  exit 1
fi

if ! python3 "$SESSION_PY" check-stamp --worktree "$ROOT" --session-id "$SID"; then
  estate_gate_line stamp fail hard
  estate_hint "re-run nocturne-estate start from this worktree (stamp missing/stale)"
  exit 1
fi
exit 0
