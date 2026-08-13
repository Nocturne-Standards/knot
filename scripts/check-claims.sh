#!/usr/bin/env bash
# Exit codes: bin/EXIT-CODES.md (0 ok / 1 hard / 2 warn).
set -euo pipefail
KIT="$(cd "$(dirname "$0")/.." && pwd)"
CLAIM_PY="${CLAIM_PY:-$HOME/.cursor/process-hygiene/bin/claim.py}"
ROOT="$(git rev-parse --show-toplevel)"

if [[ "${DEV_WORKFLOW_SKIP_CLAIM:-}" == "1" ]]; then
  echo "nocturne-agent-kit: SKIP_CLAIM set" >&2
  exit 0
fi
if [[ ! -f "$CLAIM_PY" ]]; then
  if [[ -f "$ROOT/.nocturne-agent-kit-adopted" ]]; then
    # shellcheck source=lib/estate-fail-contract.sh
    source "$KIT/bin/lib/estate-fail-contract.sh"
    estate_gate_line claims fail hard
    estate_hint "install cursor-process-hygiene claim.py"
    exit 1
  fi
  echo "nocturne-agent-kit: claim.py missing — warn (install process-hygiene)" >&2
  exit 2
fi

STAGED=()
while IFS= read -r line; do
  [[ -n "$line" ]] && STAGED+=("$line")
done < <(git diff --cached --name-only)
COUNT=${#STAGED[@]}
[[ "$COUNT" -eq 0 ]] && exit 0

need=()
while IFS= read -r pkg || [[ -n "$pkg" ]]; do
  [[ -z "$pkg" || "$pkg" =~ ^# ]] && continue
  for f in "${STAGED[@]}"; do
    if [[ "$f" == "$pkg"/* || "$f" == "$pkg" ]]; then
      need+=("$pkg")
    fi
  done
done < "$KIT/config/heavy-packages.txt"

if [[ ${#need[@]} -eq 0 ]]; then
  exit 0
fi

unique_need=()
while IFS= read -r pkg; do
  [[ -n "$pkg" ]] && unique_need+=("$pkg")
done < <(printf '%s\n' "${need[@]}" | sort -u)

failed=0
for pkg in "${unique_need[@]}"; do
  out=$(python3 "$CLAIM_PY" check "$pkg" --cwd "$ROOT" 2>/dev/null || true)
  status=$(python3 -c 'import json,sys; print(json.load(sys.stdin)["status"])' <<<"$out")
  if [[ "$status" != "ours" ]]; then
    echo "nocturne-agent-kit: need claim for '$pkg' (status=$status). Run: python3 $CLAIM_PY acquire $pkg" >&2
    failed=1
  fi
done
exit "$failed"
