#!/usr/bin/env bash
# Optional: run gitleaks if installed. Exit codes: bin/EXIT-CODES.md
# Missing binary = 2 (warn). Scan finding = 1. Clean / not a git tree misuse = 0/1.
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || true)"
if [[ -z "$ROOT" ]]; then
  echo "check-gitleaks: not inside a git work tree" >&2
  exit 1
fi
cd "$ROOT"

if ! command -v gitleaks >/dev/null; then
  echo "check-gitleaks: gitleaks not on PATH — warn (install for local secret scan)" >&2
  exit 2
fi

echo "check-gitleaks: gitleaks detect --source ."
gitleaks detect --source . --verbose
