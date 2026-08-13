#!/usr/bin/env bash
# estate_who — peer view: git worktree list + session ledgers + claims.

_wt_session_hash() {
  python3 -c 'import hashlib, os, sys; print(hashlib.sha256(os.path.realpath(sys.argv[1]).encode()).hexdigest()[:16])' "$1"
}

_resolve_claim_py() {
  if [[ -n "${CLAIM_PY:-}" && -f "${CLAIM_PY}" ]]; then
    echo "$CLAIM_PY"
    return 0
  fi
  if [[ -n "${CURSOR_PROCESS_HYGIENE:-}" ]]; then
    local p="${CURSOR_PROCESS_HYGIENE%/}/bin/claim.py"
    if [[ -f "$p" ]]; then
      echo "$p"
      return 0
    fi
  fi
  local home_py="$HOME/.cursor/process-hygiene/bin/claim.py"
  if [[ -f "$home_py" ]]; then
    echo "$home_py"
    return 0
  fi
  local sib1
  sib1="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)/cursor-process-hygiene/bin/claim.py"
  if [[ -f "$sib1" ]]; then
    echo "$sib1"
    return 0
  fi
  return 1
}

estate_who() {
  local sessions_root="$HOME/.cursor/process-hygiene/sessions"
  local path h claim_py claim_json

  while IFS= read -r path; do
    [[ -n "$path" ]] || continue
    echo "worktree=$path"
    h="$(_wt_session_hash "$path")"
    if [[ -d "$sessions_root/$h" ]]; then
      local f sid
      for f in "$sessions_root/$h"/*.json; do
        [[ -f "$f" ]] || continue
        sid="$(basename "$f" .json)"
        echo "  session=$sid"
      done
    fi
  done < <(git worktree list --porcelain | awk '/^worktree /{print $2}')

  claim_py="$(_resolve_claim_py 2>/dev/null || true)"
  if [[ -n "$claim_py" && -f "$claim_py" ]]; then
    claim_json="$(python3 "$claim_py" list 2>/dev/null || true)"
    if [[ -n "$claim_json" ]]; then
      local common repo_id filtered
      common="$(git rev-parse --path-format=absolute --git-common-dir 2>/dev/null || true)"
      [[ -n "$common" ]] || common="$(git rev-parse --git-common-dir 2>/dev/null || true)"
      if [[ -n "$common" ]]; then
        # Match claim.py: sha256(git_common_dir)[:16], no realpath.
        repo_id="$(python3 -c 'import hashlib,sys; print(hashlib.sha256(sys.argv[1].encode()).hexdigest()[:16])' "$common")"
        filtered="$(
          printf '%s\n' "$claim_json" | python3 -c '
import json, sys
want = sys.argv[1]
try:
    data = json.load(sys.stdin)
except json.JSONDecodeError:
    raise SystemExit(0)
rows = [c for c in (data.get("claims") or []) if str(c.get("repo_id", "")) == want]
if rows:
    print("claims:")
    print(json.dumps({"claims": rows}, indent=2))
' "$repo_id"
        )"
        if [[ -n "$filtered" ]]; then
          printf '%s\n' "$filtered"
        fi
      fi
    fi
  fi
}
