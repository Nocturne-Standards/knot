#!/usr/bin/env bash
# Exit codes: bin/EXIT-CODES.md (0 ok / 1 hard / 2 warn).
set -euo pipefail
KIT="$(cd "$(dirname "$0")/.." && pwd)"
ROOT="$(git rev-parse --show-toplevel)"
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

# Worktree guard: not under .worktrees + count>1 → deny (unless escape)
if [[ "$in_worktree_dir" -eq 0 && "$COUNT" -gt 1 && "$allow_escape" -eq 0 ]]; then
  echo "nocturne-agent-kit: multi-file commit outside .worktrees/ — use bin/new-track.sh" >&2
  exit 1
fi

# Main branch strict: on branch main, deny if count>1 or denylist (unless escape)
if [[ "$BRANCH" == "main" && "$allow_escape" -eq 0 ]]; then
  if [[ "$COUNT" -gt 1 || "$denylist_hit" -eq 1 ]]; then
    echo "nocturne-agent-kit: blocked commit on main (strict main-guard). Escape: DEV_WORKFLOW_ALLOW_MAIN=1 and allow-main: in message" >&2
    exit 1
  fi
fi
exit 0
