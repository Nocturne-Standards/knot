#!/usr/bin/env bash
# Exit codes: bin/EXIT-CODES.md (0 ok / 1 hard / 2 warn).
# Stamp gate: AUDIT/DESIGN (and IMPLEMENTATION) docs must cite a resolvable commit SHA.
# Bash 3.2+ compatible (macOS /bin/bash).
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || true)"
if [[ -z "$ROOT" ]]; then
  echo "check-audit-sha: not inside a git work tree" >&2
  exit 1
fi
cd "$ROOT"

# Newline-separated find predicates; override with AUDIT_SHA_FIND if needed.
# Default: AUDIT-*/DESIGN-*/IMPLEMENTATION.md under docs/.
collect_files() {
  if [[ -n "${AUDIT_SHA_FIND:-}" ]]; then
    eval "$AUDIT_SHA_FIND"
    return
  fi
  {
    find docs -type f \( -name 'AUDIT-*' -o -name 'DESIGN-*' -o -name 'IMPLEMENTATION.md' \) 2>/dev/null || true
  } | sort -u
}

fail=0
count=0
while IFS= read -r f; do
  [[ -z "$f" || ! -f "$f" ]] && continue
  count=$((count + 1))
  head10="$(head -n 10 "$f")"
  sha=""
  # Prefer `hex` backticks
  if printf '%s\n' "$head10" | grep -Eq '`[0-9a-fA-F]{7,40}`'; then
    sha="$(printf '%s\n' "$head10" | grep -Eo '`[0-9a-fA-F]{7,40}`' | head -1 | tr -d '`')"
  else
    sha="$(printf '%s\n' "$head10" | grep -Eo '[0-9a-fA-F]{7,40}' | head -1 || true)"
  fi
  if [[ -z "$sha" ]]; then
    echo "BLOCKED: $f — no 7+ hex SHA in first 10 lines" >&2
    fail=1
    continue
  fi
  if ! git cat-file -e "${sha}^{commit}" 2>/dev/null; then
    echo "BLOCKED: $f — SHA '$sha' does not resolve to a commit in this repo" >&2
    fail=1
    continue
  fi
  echo "ok: $f stamped against $(git rev-parse --short "$sha")"
done < <(collect_files)

if [[ "$count" -eq 0 ]]; then
  echo "check-audit-sha: no matching docs — ok"
fi

exit "$fail"
