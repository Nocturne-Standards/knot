#!/usr/bin/env bash
# Exit codes: bin/EXIT-CODES.md (0 ok / 1 hard / 2 warn).
set -euo pipefail
KIT="$(cd "$(dirname "$0")/.." && pwd)"
ROOT="$(git rev-parse --show-toplevel)"

if [[ "${DEV_WORKFLOW_SKIP_SESSION:-}" == "1" ]]; then
  echo "nocturne-agent-kit: SKIP_SESSION set" >&2
  exit 0
fi

resolve_session_py() {
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
  local sib1 sib2
  sib1="$(cd "$KIT/.." && pwd)/cursor-process-hygiene/bin/session.py"
  if [[ -f "$sib1" ]]; then
    echo "$sib1"
    return 0
  fi
  sib2="$(cd "$KIT/../.." && pwd)/cursor-process-hygiene/bin/session.py"
  if [[ -f "$sib2" ]]; then
    echo "$sib2"
    return 0
  fi
  return 1
}

SESSION_PY="${SESSION_PY:-}"
if [[ -z "$SESSION_PY" ]]; then
  SESSION_PY="$(resolve_session_py 2>/dev/null || true)"
fi
if [[ -z "$SESSION_PY" || ! -f "$SESSION_PY" ]]; then
  if [[ -f "$ROOT/.nocturne-agent-kit-adopted" ]]; then
    # shellcheck source=lib/estate-fail-contract.sh
    source "$KIT/bin/lib/estate-fail-contract.sh"
    estate_gate_line session fail hard
    estate_hint "install cursor-process-hygiene session.py"
    exit 1
  fi
  echo "nocturne-agent-kit: session.py missing — warn (install process-hygiene)" >&2
  exit 2
fi

python3 "$SESSION_PY" check-staged --worktree "$ROOT"
