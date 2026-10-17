#!/usr/bin/env bash
# Exit codes: bin/EXIT-CODES.md (0 ok / 1 hard / 2 warn).
# Reject `pub fn init` on a dusk-forge contract.
#
# WHY THIS EXISTS
# ---------------
# Piecrust reserves the wasm export "init" for deploy. dusk-forge exports
# every pub fn, so a method literally named init becomes that constructor.
# Deploy then deserializes init_arg (often empty) as the method's arguments
# and panics. Rename to init_owner / init_token / anything else.
#
# This is the check we can run without waiting on dusk-network/forge#40.
# It does not flag init_owner, init_token, or a private fn init.
#
# Bash 3.2+ compatible (macOS /bin/bash).
set -uo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || true)"
if [[ -z "$ROOT" ]]; then
  echo "check-contract-init: not inside a git work tree" >&2
  exit 1
fi
cd "$ROOT"

# Match both #[dusk_forge::contract] and #[dusk_forge::contract(events = ...)].
files="$(git grep -lE '#\[dusk_forge::contract([[:space:]]*\(|[[:space:]]*\])' -- '*.rs' \
  ':!vendor' ':!target' ':!.worktrees' 2>/dev/null || true)"

if [[ -z "$files" ]]; then
  echo "ok: check-contract-init (no dusk-forge contracts)"
  exit 0
fi

# No here-string: that temp file is denied in the Cursor sandbox, the loop
# runs zero times, and a repo with `pub fn init` used to pass.
file_list=()
while IFS= read -r f; do
  [[ -n "$f" ]] && file_list+=("$f")
done < <(printf '%s\n' "$files")

# Bash 3.2 + set -u: expanding an empty "${array[@]}" aborts before our check.
if [[ ${#file_list[@]} -eq 0 ]]; then
  echo "BLOCKED: check-contract-init checked 0 files (environment error)" >&2
  exit 1
fi

hits=""
checked=0
for f in "${file_list[@]}"; do
  [[ -z "$f" ]] && continue
  checked=$((checked + 1))
  case "$f" in
    vendor/*|*/vendor/*|target/*|*/target/*|.worktrees/*|*/.worktrees/*) continue ;;
  esac
  found="$(awk -v file="$f" '
    /^[[:space:]]*\/\// { next }
    /#\[dusk_forge::contract([ \t]*\(|[ \t]*\])/ { in_contract = 1 }
    in_contract && /^[[:space:]]*pub fn init[[:space:]]*[<(]/ {
      print file ":" FNR
    }
  ' "$f")"
  if [[ -n "$found" ]]; then
    hits="${hits}${found}"$'\n'
  fi
done

if [[ -n "$hits" ]]; then
  echo "BLOCKED: pub fn init is the Piecrust deploy constructor:" >&2
  printf '%s' "$hits" | sed '/^$/d; s/^/  /' >&2
  echo "" >&2
  echo "  Rename it (init_owner, init_token, …). The wasm export \"init\"" >&2
  echo "  runs at deploy, not as a later call." >&2
  exit 1
fi

echo "ok: check-contract-init (no pub fn init)"
exit 0
