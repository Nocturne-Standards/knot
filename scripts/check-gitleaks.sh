#!/usr/bin/env bash
# Optional: run gitleaks if installed. Missing binary = skip (CI installs it).
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || true)"
if [[ -z "$ROOT" ]]; then
  echo "check-gitleaks: not inside a git work tree" >&2
  exit 1
fi
cd "$ROOT"

if ! command -v gitleaks >/dev/null; then
  echo "check-gitleaks: gitleaks not on PATH — skip (install for local secret scan)"
  exit 0
fi

echo "check-gitleaks: gitleaks detect --source ."
gitleaks detect --source . --verbose
