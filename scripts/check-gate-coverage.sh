#!/usr/bin/env bash
# Exit codes: bin/EXIT-CODES.md (0 ok / 1 hard / 2 warn).
# Gate-coverage check — the meta-gate.
#
# WHY THIS EXISTS
# ---------------
# On 2026-08-05 chit re-leaked a credential that knot's public-surface gate had
# been written weeks earlier to catch. Both repos had core.hooksPath pointing at
# this kit. The pre-commit hook fired in both. It caught nothing in chit.
#
# Cause: compose-repo-hooks.sh uses `run_if`, which runs a repo-local gate only
# `if [[ -x "$ROOT/scripts/$script" ]]`. knot had six gate scripts. chit had
# zero. A repo with no gates therefore passed every commit silently — the
# architecture failed open, per repo, with no signal.
#
# This script makes the ABSENCE of a gate a failure. It is the one check that
# cannot itself be forgotten, because it is what notices the forgetting.
#
# Bash 3.2+ compatible (macOS /bin/bash).
set -uo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || true)"
if [[ -z "$ROOT" ]]; then
  echo "check-gate-coverage: not inside a git work tree" >&2
  exit 1
fi
cd "$ROOT"

KIT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# Kit is the gate source of truth — scripts/ copies are for adopters only.
if [[ -f "$ROOT/FIELD_GUIDE/SPEC.md" && -x "$ROOT/bin/check-gate-coverage.sh" ]]; then
  echo "ok: check-gate-coverage — kit repo (gates live in bin/)"
  exit 0
fi

# Gates every adopting repo must carry a copy of, in scripts/.
# Keep in sync with bin/ — a gate that exists in the kit but is required
# nowhere is a gate nobody runs.
REQUIRED="check-public-surface.sh
check-audit-sha.sh
check-repo-rules.sh
check-gitleaks.sh
check-bls-insecure.sh"

# Gates required only when the repo ships Rust contracts.
REQUIRED_IF_CONTRACTS="check-contract-authz.sh"

# Opt out per repo, with a reason, in .gate-coverage-waivers:
#   check-repo-rules.sh: no hand-copied mirrors in this repo (reviewed 2026-08-05)
WAIVERS=".gate-coverage-waivers"

fail=0
missing=""

is_waived() {
  [[ -f "$WAIVERS" ]] || return 1
  grep -qE "^[[:space:]]*$1[[:space:]]*:" "$WAIVERS"
}

has_contracts() {
  # A dusk-forge contract crate is the trigger for contract-specific gates.
  git grep -lE '#\[dusk_forge::contract\]' -- '*.rs' >/dev/null 2>&1
}

require() {
  local script="$1"
  if [[ -x "scripts/$script" ]]; then
    return 0
  fi
  if [[ -f "scripts/$script" ]]; then
    echo "BLOCKED: scripts/$script exists but is not executable (chmod +x)" >&2
    fail=1
    return 0
  fi
  if is_waived "$script"; then
    echo "waived: $script — $(grep -E "^[[:space:]]*$script[[:space:]]*:" "$WAIVERS" | head -1 | cut -d: -f2-)"
    return 0
  fi
  missing="$missing $script"
  fail=1
}

while IFS= read -r script; do
  [[ -z "$script" ]] && continue
  require "$script"
done <<< "$REQUIRED"

if has_contracts; then
  while IFS= read -r script; do
    [[ -z "$script" ]] && continue
    require "$script"
  done <<< "$REQUIRED_IF_CONTRACTS"
fi

if [[ -n "$missing" ]]; then
  echo "BLOCKED: this repo is missing required gate scripts:" >&2
  for m in $missing; do
    echo "  scripts/$m" >&2
    if [[ -f "$KIT/bin/$m" ]]; then
      echo "    install: cp \"$KIT/bin/$m\" scripts/ && chmod +x scripts/$m" >&2
    fi
  done
  echo "" >&2
  echo "  Or waive with a reason in $WAIVERS:" >&2
  echo "    <script>: <why this repo does not need it> (reviewed <date>)" >&2
fi

# A repo can carry the scripts and still not run them in CI.
# Accept any workflow under .github/workflows/ (ci.yml, hygiene.yml, …).
if [[ -d .github/workflows ]]; then
  if ! grep -Rq 'scripts/check-' .github/workflows --include='*.yml' --include='*.yaml' 2>/dev/null; then
    echo "BLOCKED: no scripts/check-* invoked in .github/workflows/*.yml" >&2
    echo "  Hooks are bypassable with --no-verify; CI is the backstop that is not." >&2
    fail=1
  fi
fi

if ((fail == 0)); then
  echo "ok: check-gate-coverage — all required gates present and wired"
fi
exit "$fail"
