#!/usr/bin/env bash
# Parse fail-contract log lines (GATE:/HINT:) or estate-ci-report.json.
# Prints HINT lines to stdout; exit 1 when any hard fail is present.

_estate_diagnose_from_log() {
  local input="$1"
  local hard=0 line name status severity
  while IFS= read -r line || [[ -n "$line" ]]; do
    if [[ "$line" =~ ^GATE:([^[:space:]]+)[[:space:]]+STATUS:([^[:space:]]+)[[:space:]]+SEVERITY:([^[:space:]]+) ]]; then
      name="${BASH_REMATCH[1]}"
      status="${BASH_REMATCH[2]}"
      severity="${BASH_REMATCH[3]}"
      if [[ "$status" == "fail" && "$severity" == "hard" ]]; then
        hard=1
      fi
      continue
    fi
    if [[ "$line" =~ ^HINT:[[:space:]]*(.*)$ ]]; then
      printf '%s\n' "HINT: ${BASH_REMATCH[1]}"
    fi
  done <"$input"
  return "$hard"
}

_estate_diagnose_from_report() {
  local report="$1"
  python3 - "$report" <<'PY'
import json
import sys

path = sys.argv[1]
with open(path, encoding="utf-8") as f:
    data = json.load(f)

hard = 0
for gate in data.get("gates") or []:
    status = str(gate.get("status", "")).lower()
    severity = str(gate.get("severity", "")).lower()
    hint = gate.get("hint")
    if hint:
        print(f"HINT: {hint}")
    if status == "fail" and severity == "hard":
        hard = 1

raise SystemExit(hard)
PY
}

estate_diagnose() {
  local report="" input="-"

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --report)
        [[ $# -ge 2 ]] || {
          echo "estate-diagnose: --report requires path" >&2
          return 2
        }
        report="$2"
        shift 2
        ;;
      -h|--help)
        echo "usage: estate_diagnose [--report path.json] [logfile]" >&2
        echo "  Reads fail-contract log (file or stdin) or estate-ci-report.json." >&2
        echo "  Prints HINT lines; exit 1 if any hard fail present." >&2
        return 0
        ;;
      --)
        shift
        break
        ;;
      -*)
        echo "estate-diagnose: unknown option: $1" >&2
        return 2
        ;;
      *)
        if [[ "$input" != "-" ]]; then
          echo "estate-diagnose: extra argument: $1" >&2
          return 2
        fi
        input="$1"
        shift
        ;;
    esac
  done

  if [[ -n "$report" ]]; then
    if [[ ! -f "$report" ]]; then
      echo "estate-diagnose: report not found: $report" >&2
      return 2
    fi
    _estate_diagnose_from_report "$report"
    return $?
  fi

  if [[ "$input" == "-" ]]; then
    local tmp
    tmp="$(mktemp)"
    cat >"$tmp"
    _estate_diagnose_from_log "$tmp"
    local rc=$?
    rm -f "$tmp"
    return "$rc"
  fi

  if [[ ! -f "$input" ]]; then
    echo "estate-diagnose: log not found: $input" >&2
    return 2
  fi
  _estate_diagnose_from_log "$input"
}
