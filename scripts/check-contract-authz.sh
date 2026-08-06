#!/usr/bin/env bash
# Exit codes: bin/EXIT-CODES.md (0 ok / 1 hard / 2 warn).
# Contract authorization ratchet.
#
# WHY THIS EXISTS
# ---------------
# Every `pub fn` on a `#[dusk_forge::contract]` state struct becomes an
# independent WASM export. It is callable directly by anyone, regardless of
# which contract you intended to call it. Authorization is opt-in, and its
# absence is invisible — the method looks the same either way.
#
# This class has now produced three separate findings across two repos:
#   knot   — diagnostic methods as permanent public attack surface
#   chit   — agent-compliance-gate::debit, unauthenticated (Critical, fixed)
#   chit   — agent-gas-meter::refund_gas_budget / clear_gas_budget,
#            unauthenticated, written the SAME DAY the debit fix landed
#
# The debit fix was applied as a point fix. Nobody swept the class, so the
# class reappeared within hours in new code.
#
# This is a ratchet, not an analyser. It does not try to infer whether a method
# is safe — that is a human judgement. It forces the judgement to be WRITTEN
# DOWN once per exported method, and fails when a new export appears without
# one. Same shape as check-repo-rules.sh, which the team already runs.
#
# Bash 3.2+ compatible (macOS /bin/bash).
set -uo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || true)"
if [[ -z "$ROOT" ]]; then
  echo "check-contract-authz: not inside a git work tree" >&2
  exit 1
fi
cd "$ROOT"

BASELINE=".contract-authz-baseline"
UPDATE=0
[[ "${1:-}" == "--update" ]] && UPDATE=1

tmp="$(mktemp)"
trap 'rm -f "$tmp"' EXIT

# Collect `pub fn` inside files that declare a dusk-forge contract module.
# Deliberately line-based and conservative: over-reporting a helper is cheap,
# missing an export is not.
collect() {
  local files
  files="$(git grep -lE '#\[dusk_forge::contract\]' -- '*.rs' \
    ':!vendor' ':!target' ':!.worktrees' 2>/dev/null || true)"
  [[ -z "$files" ]] && return 0
  while IFS= read -r f; do
    [[ -z "$f" ]] && continue
    awk -v file="$f" '
      /#\[dusk_forge::contract\]/ { in_contract = 1 }
      in_contract && /^[[:space:]]*pub fn [a-z_][a-z0-9_]*/ {
        line = $0
        sub(/^[[:space:]]*pub fn /, "", line)
        sub(/[(<].*$/, "", line)
        if (line != "new") print file ":" line
      }
    ' "$f"
  done <<< "$files" | sort -u
}

collect > "$tmp"

if [[ ! -s "$tmp" ]]; then
  echo "check-contract-authz: no dusk-forge contract exports found — ok"
  exit 0
fi

if [[ "$UPDATE" -eq 1 ]]; then
  {
    echo "# Contract authorization baseline."
    echo "#"
    echo "# One line per exported contract method. After each, state who may call it."
    echo "# Reviewed entries look like:"
    echo "#   crates/x/src/state.rs:debit    owner-allowlisted settlement contracts only"
    echo "#   crates/x/src/state.rs:get_info PUBLIC — read-only, leaks no session data"
    echo "#"
    echo "# 'UNREVIEWED' fails the gate. That is the point."
    echo ""
    while IFS= read -r entry; do
      if [[ -f "$BASELINE" ]] && grep -qF "$entry" "$BASELINE" 2>/dev/null; then
        grep -F "$entry" "$BASELINE" | head -1
      else
        printf '%s\tUNREVIEWED\n' "$entry"
      fi
    done < "$tmp"
  } > "$BASELINE.new"
  mv "$BASELINE.new" "$BASELINE"
  echo "wrote $BASELINE ($(wc -l < "$tmp" | tr -d ' ') exports)"
  echo "Now replace every UNREVIEWED with the actual authorization rule."
  exit 0
fi

if [[ ! -f "$BASELINE" ]]; then
  echo "BLOCKED: missing $BASELINE" >&2
  echo "  Run: scripts/check-contract-authz.sh --update" >&2
  echo "  Then state the authorization for every exported method." >&2
  exit 1
fi

fail=0

# New exports with no baseline entry.
while IFS= read -r entry; do
  if ! grep -qF "$entry" "$BASELINE"; then
    if ((fail == 0)); then
      echo "BLOCKED: contract methods exported with no declared authorization:" >&2
    fi
    echo "  $entry" >&2
    fail=1
  fi
done < "$tmp"

if ((fail)); then
  echo "" >&2
  echo "  Every pub fn on a contract state struct is a public WASM export." >&2
  echo "  Add it to $BASELINE with who may call it, then re-run." >&2
fi

# Entries still marked UNREVIEWED (skip comment lines — header mentions the token).
if grep -vE '^[[:space:]]*(#|$)' "$BASELINE" 2>/dev/null | grep -q 'UNREVIEWED'; then
  echo "BLOCKED: $BASELINE has UNREVIEWED entries:" >&2
  grep -n 'UNREVIEWED' "$BASELINE" | grep -vE '^[0-9]+:[[:space:]]*#' | sed 's/^/  /' >&2
  fail=1
fi

# Baseline entries whose method no longer exists — stale, and a stale baseline
# is how a deleted-and-reintroduced method sneaks back in pre-approved.
while IFS= read -r line; do
  case "$line" in ''|'#'*) continue ;; esac
  entry="$(printf '%s' "$line" | awk -F'\t' '{print $1}' | sed 's/[[:space:]]*$//')"
  [[ -z "$entry" ]] && continue
  if ! grep -qF "$entry" "$tmp"; then
    echo "note: $entry in baseline but no longer exported — prune it" >&2
  fi
done < "$BASELINE"

if ((fail == 0)); then
  echo "ok: check-contract-authz ($(wc -l < "$tmp" | tr -d ' ') exports, all declared)"
fi
exit "$fail"
