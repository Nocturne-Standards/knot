#!/usr/bin/env bash
set -euo pipefail
if [[ -n "${NOCTURNE_AGENT_KIT:-}" && -x "$NOCTURNE_AGENT_KIT/bin/check-main-guard.sh" ]]; then
  exec "$NOCTURNE_AGENT_KIT/bin/check-main-guard.sh" "$@"
fi
_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if [[ ! -f "$_repo_root/config/main-denylist.txt" ]]; then
  _fc=""
  if [[ -f "$_repo_root/../nocturne-agent-kit/bin/lib/estate-fail-contract.sh" ]]; then
    _fc="$_repo_root/../nocturne-agent-kit/bin/lib/estate-fail-contract.sh"
  fi
  if [[ -n "$_fc" ]]; then
    # shellcheck source=../nocturne-agent-kit/bin/lib/estate-fail-contract.sh
    source "$_fc"
  else
    estate_gate_line() { echo "GATE:$1 STATUS:$2 SEVERITY:$3" >&2; }
    estate_hint() { echo "HINT: $*" >&2; }
  fi
  estate_gate_line main-guard fail hard
  estate_hint "set NOCTURNE_AGENT_KIT to kit checkout (export NOCTURNE_AGENT_KIT=/path/to/nocturne-agent-kit)"
  exit 1
fi
# --- embedded fallback (refreshed at wire time) ---
# Exit codes: bin/EXIT-CODES.md (0 ok / 1 hard / 2 warn).
#
# Adopted repos (.nocturne-agent-kit-adopted):
#   - Under .worktrees/*: multi-file OK on feat branches.
#   - Primary checkout: only main/master + exactly one non-denylist file
#     (human one-liner). Everything else needs
#     DEV_WORKFLOW_ALLOW_MAIN=1 + allow-main: in the commit message.
# Non-adopted: legacy behavior (multi-file outside worktrees denied; main strict).
set -euo pipefail
KIT="$(cd "$(dirname "$0")/.." && pwd)"
ROOT="$(git rev-parse --show-toplevel)"
if [[ -f "$KIT/bin/lib/estate-fail-contract.sh" ]]; then
  # shellcheck disable=SC1091
  source "$KIT/bin/lib/estate-fail-contract.sh"
elif [[ -f "$ROOT/../nocturne-agent-kit/bin/lib/estate-fail-contract.sh" ]]; then
  # shellcheck disable=SC1091
  source "$ROOT/../nocturne-agent-kit/bin/lib/estate-fail-contract.sh"
else
  estate_gate_line() { echo "GATE:$1 STATUS:$2 SEVERITY:$3" >&2; }
  estate_hint() { echo "HINT: $*" >&2; }
fi
BRANCH="$(git symbolic-ref -q --short HEAD 2>/dev/null || echo main)"

STAGED=()
while IFS= read -r line; do
  [[ -n "$line" ]] && STAGED+=("$line")
done < <(git diff --cached --name-only)
COUNT=${#STAGED[@]}
[[ "$COUNT" -eq 0 ]] && exit 0

in_worktree_dir=0
case "$ROOT" in
  */.worktrees/*) in_worktree_dir=1 ;;
esac

adopted=0
[[ -f "$ROOT/.nocturne-agent-kit-adopted" ]] && adopted=1

commit_msg=""
if [[ -n "${1:-}" && -r "$1" ]]; then
  commit_msg="$(cat "$1")"
elif [[ -n "${GIT_COMMIT_MSG:-}" ]]; then
  if [[ -r "$GIT_COMMIT_MSG" ]]; then
    commit_msg="$(cat "$GIT_COMMIT_MSG")"
  else
    commit_msg="$GIT_COMMIT_MSG"
  fi
else
  msg_file="$(git rev-parse --git-path COMMIT_EDITMSG)"
  if [[ -r "$msg_file" ]]; then
    commit_msg="$(cat "$msg_file")"
  fi
fi

allow_escape=0
if [[ "${DEV_WORKFLOW_ALLOW_MAIN:-}" == "1" && "$commit_msg" == *allow-main:* ]]; then
  allow_escape=1
fi

denylist_hit=0
while IFS= read -r pat || [[ -n "$pat" ]]; do
  [[ -z "$pat" || "$pat" =~ ^# ]] && continue
  for f in "${STAGED[@]}"; do
    if [[ "$pat" == */ ]]; then
      if [[ "$f" == "$pat"* ]]; then
        denylist_hit=1
      fi
    elif [[ "$pat" == "Cargo.toml" || "$pat" == "Cargo.lock" ]]; then
      if [[ "$(basename "$f")" == "$pat" ]]; then
        denylist_hit=1
      fi
    elif [[ "$f" == "$pat" || "$f" == */"$pat" ]]; then
      denylist_hit=1
    fi
  done
done < "$KIT/config/main-denylist.txt"

# --- Adopted primary checkout: refuse agent-shaped commits without escape ---
# Human one-liner: main/master + single file + not denylist.
if [[ "$adopted" -eq 1 && "$in_worktree_dir" -eq 0 && "$allow_escape" -eq 0 ]]; then
  human_ok=0
  if [[ "$BRANCH" == "main" || "$BRANCH" == "master" ]]; then
    if [[ "$COUNT" -eq 1 && "$denylist_hit" -eq 0 ]]; then
      human_ok=1
    fi
  fi
  if [[ "$human_ok" -eq 0 ]]; then
    echo "nocturne-agent-kit: adopted repo — commit from .worktrees/<track> (bin/new-track.sh or Cursor /worktree), not primary checkout" >&2
    echo "  Escape (rare): DEV_WORKFLOW_ALLOW_MAIN=1 and allow-main: <reason> in commit message" >&2
    estate_gate_line main-guard fail hard
    estate_hint "commit from .worktrees/<track> (bin/new-track.sh or Cursor /worktree); escape: DEV_WORKFLOW_ALLOW_MAIN=1 + allow-main: in message"
    exit 1
  fi
fi

# Legacy / non-adopted: Worktree guard — not under .worktrees + count>1 → deny
if [[ "$adopted" -eq 0 && "$in_worktree_dir" -eq 0 && "$COUNT" -gt 1 && "$allow_escape" -eq 0 ]]; then
  echo "nocturne-agent-kit: multi-file commit outside .worktrees/ — use bin/new-track.sh" >&2
  estate_gate_line main-guard fail hard
  estate_hint "multi-file commit outside .worktrees/ — use bin/new-track.sh"
  exit 1
fi

# Main branch strict (all repos): on main, deny if count>1 or denylist (unless escape)
if [[ "$BRANCH" == "main" || "$BRANCH" == "master" ]]; then
  if [[ "$allow_escape" -eq 0 ]]; then
    if [[ "$COUNT" -gt 1 || "$denylist_hit" -eq 1 ]]; then
      echo "nocturne-agent-kit: blocked commit on main (strict main-guard). Escape: DEV_WORKFLOW_ALLOW_MAIN=1 and allow-main: in message" >&2
      estate_gate_line main-guard fail hard
      estate_hint "blocked on main — use .worktrees/<track> or DEV_WORKFLOW_ALLOW_MAIN=1 + allow-main: in message"
      exit 1
    fi
  fi
fi
exit 0
