#!/usr/bin/env bash
# Exit codes: bin/EXIT-CODES.md (0 ok / 1 hard / 2 warn).
# Release-readiness: remote-tracking refs are caches — fetch before trusting them.
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || true)"
if [[ -z "$ROOT" ]]; then
  echo "check-fresh-remote: not inside a git work tree" >&2
  exit 1
fi
cd "$ROOT"

REF="${FRESH_REMOTE_REF:-origin/HEAD}"

if ! git rev-parse --verify "$REF" >/dev/null 2>&1; then
  # origin/HEAD may be unset until fetch; try origin/main / origin/master after fetch.
  :
fi

echo "check-fresh-remote: git fetch --prune"
git fetch --prune

if ! git rev-parse --verify "$REF" >/dev/null 2>&1; then
  if git rev-parse --verify origin/main >/dev/null 2>&1; then
    REF=origin/main
  elif git rev-parse --verify origin/master >/dev/null 2>&1; then
    REF=origin/master
  else
    echo "BLOCKED: cannot resolve $FRESH_REMOTE_REF / origin/main / origin/master after fetch" >&2
    exit 1
  fi
fi

HEAD="$(git rev-parse HEAD)"
REMOTE="$(git rev-parse "$REF")"
HEAD_S="$(git rev-parse --short HEAD)"
REMOTE_S="$(git rev-parse --short "$REF")"

if [[ "$HEAD" != "$REMOTE" ]]; then
  echo "BLOCKED: HEAD ($HEAD_S) != $REF ($REMOTE_S)" >&2
  echo "  Refusing release/audit claims against a stale or diverged tip." >&2
  echo "  Checkout/ff to $REF, or set FRESH_REMOTE_REF if intentional." >&2
  exit 1
fi

echo "ok: HEAD == $REF ($HEAD_S)"
