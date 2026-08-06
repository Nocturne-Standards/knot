#!/usr/bin/env bash
# check-kit-sync.sh — rules.md pin/hash + bidirectional gate↔rule validation.
#
# Kit-sync (SPEC §7): hard-fail on divergence / missing AGENTS pointer.
# Lag (pin behind kit tag) is advisory — needs kit checkout, never blocks offline.
#
# Usage:
#   check-kit-sync.sh           # verify
#   check-kit-sync.sh --update  # refresh rules.md from kit
#
# Bash 3.2+ compatible.
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || true)"
if [[ -z "$ROOT" ]]; then
  echo "check-kit-sync: not inside a git work tree" >&2
  exit 1
fi
cd "$ROOT"

# Resolve kit: env → script-in-kit-bin → sibling checkout → parent of script
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
resolve_kit() {
  if [[ -n "${NOCTURNE_AGENT_KIT:-}" && -f "${NOCTURNE_AGENT_KIT}/FIELD_GUIDE/rules.md" ]]; then
    printf '%s\n' "$NOCTURNE_AGENT_KIT"
    return
  fi
  if [[ -f "$SCRIPT_DIR/../FIELD_GUIDE/SPEC.md" && -f "$SCRIPT_DIR/check-kit-sync.sh" ]]; then
    cd "$SCRIPT_DIR/.." && pwd
    return
  fi
  if [[ -f "$ROOT/../nocturne-agent-kit/FIELD_GUIDE/rules.md" ]]; then
    cd "$ROOT/../nocturne-agent-kit" && pwd
    return
  fi
  cd "$SCRIPT_DIR/.." && pwd
}
KIT="$(resolve_kit)"
FG_RULES="FIELD_GUIDE/rules.md"
FG_VERSION="FIELD_GUIDE/.rules-version"
FG_MANIFEST="FIELD_GUIDE/.rules-manifest"
sync_fail=0

sha256_file() {
  local f="$1"
  if command -v sha256sum >/dev/null; then
    sha256sum "$f" | awk '{print $1}'
  elif command -v shasum >/dev/null; then
    shasum -a 256 "$f" | awk '{print $1}'
  else
    echo "check-kit-sync: need sha256sum or shasum" >&2
    exit 1
  fi
}

is_kit_repo() {
  [[ -f "$ROOT/FIELD_GUIDE/SPEC.md" && -f "$ROOT/bin/check-kit-sync.sh" ]]
}

cmd_update() {
  if ! is_kit_repo; then
    if [[ ! -f "$KIT/FIELD_GUIDE/rules.md" ]]; then
      echo "check-kit-sync: --update needs kit FIELD_GUIDE/rules.md at $KIT" >&2
      exit 1
    fi
    mkdir -p "$ROOT/FIELD_GUIDE"
    cp "$KIT/FIELD_GUIDE/rules.md" "$ROOT/$FG_RULES"
    cp "$KIT/FIELD_GUIDE/.rules-version" "$ROOT/$FG_VERSION"
    sha256_file "$ROOT/$FG_RULES" >"$ROOT/$FG_MANIFEST"
    echo "check-kit-sync: updated $FG_RULES → $(cat "$ROOT/$FG_VERSION") ($(cat "$ROOT/$FG_MANIFEST"))"
    echo "check-kit-sync: staged nothing — review diff then commit"
    exit 0
  fi
  sha256_file "$ROOT/$FG_RULES" >"$ROOT/$FG_MANIFEST"
  echo "check-kit-sync: kit manifest refreshed $(cat "$ROOT/$FG_MANIFEST") (version $(cat "$ROOT/$FG_VERSION"))"
  exit 0
}

if [[ "${1:-}" == "--update" ]]; then
  cmd_update
fi

if [[ ! -f "$FG_RULES" ]]; then
  echo "BLOCKED: missing $FG_RULES — run check-kit-sync.sh --update" >&2
  exit 1
fi

if [[ ! -f "$FG_VERSION" ]]; then
  echo "BLOCKED: missing $FG_VERSION" >&2
  sync_fail=1
fi

if [[ ! -f "$FG_MANIFEST" ]]; then
  echo "BLOCKED: missing $FG_MANIFEST — run check-kit-sync.sh --update" >&2
  sync_fail=1
else
  actual="$(sha256_file "$FG_RULES")"
  expected="$(tr -d '[:space:]' <"$FG_MANIFEST")"
  if [[ "$actual" != "$expected" ]]; then
    echo "BLOCKED: $FG_RULES diverges from .rules-manifest" >&2
    echo "  manifest: $expected" >&2
    echo "  actual:   $actual" >&2
    echo "  fix: restore rules.md or run check-kit-sync.sh --update" >&2
    sync_fail=1
  fi
fi

if ! is_kit_repo; then
  pointer_ok=0
  for f in AGENTS.md CLAUDE.md; do
    if [[ -f "$f" ]] && grep -qE '^## Field guide' "$f" && grep -q 'FIELD_GUIDE/rules.md' "$f"; then
      pointer_ok=1
      break
    fi
  done
  if [[ "$pointer_ok" -ne 1 ]]; then
    echo "BLOCKED: AGENTS.md or CLAUDE.md must have '## Field guide' linking FIELD_GUIDE/rules.md" >&2
    sync_fail=1
  fi
fi

# Gate↔rule validation: kit bin/ when available; else adopter scripts/ (CI).
if [[ -f "$KIT/FIELD_GUIDE/SPEC.md" && -d "$KIT/bin" ]]; then
  BIN_DIR="$KIT/bin"
  VALIDATE_REVERSE=1
elif [[ -d "$ROOT/scripts" ]]; then
  BIN_DIR="$ROOT/scripts"
  VALIDATE_REVERSE=0
  echo "check-kit-sync: kit checkout absent — validating gate: refs against scripts/" >&2
else
  BIN_DIR=""
  VALIDATE_REVERSE=0
  echo "WARN: no kit bin/ or scripts/ — skip gate↔rule validation" >&2
fi

gates_in_rules=""
while IFS= read -r line || [[ -n "$line" ]]; do
  case "$line" in
    *gate:check-*.sh*)
      g="${line#*gate:}"
      g="${g%%[^a-zA-Z0-9_.-]*}"
      case "$g" in
        check-*.sh) gates_in_rules="$gates_in_rules $g" ;;
      esac
      ;;
  esac
done <"$FG_RULES"

if [[ -n "$BIN_DIR" ]]; then
  for g in $gates_in_rules; do
    if [[ ! -f "$BIN_DIR/$g" ]]; then
      # Optional / deferred gates may live only in kit; in scripts/-only mode
      # require the gate file under scripts/ (coverage already enforces required set).
      if [[ "$VALIDATE_REVERSE" -eq 1 ]]; then
        echo "BLOCKED: rules.md cites gate:$g but $BIN_DIR/$g missing" >&2
        sync_fail=1
      elif [[ ! -f "$ROOT/scripts/$g" ]]; then
        # Soft: warn for gates not copied to this adopter (e.g. contract-authz deferred)
        echo "WARN: rules.md cites gate:$g but scripts/$g absent (deferred or N/A)" >&2
      fi
    fi
  done

  if [[ "$VALIDATE_REVERSE" -eq 1 ]]; then
    for script in "$BIN_DIR"/check-*.sh; do
      [[ -f "$script" ]] || continue
      base="$(basename "$script")"
      [[ "$base" == "check-kit-sync.sh" ]] && continue
      case " $gates_in_rules " in
        *" $base "*) ;;
        *)
          echo "BLOCKED: $base in bin/ not cited by any gate: rule in rules.md" >&2
          sync_fail=1
          ;;
      esac
    done
  fi
fi

if ! is_kit_repo && [[ -f "$KIT/FIELD_GUIDE/.rules-version" && -f "$FG_VERSION" ]]; then
  local_v="$(tr -d '[:space:]' <"$FG_VERSION")"
  kit_v="$(tr -d '[:space:]' <"$KIT/FIELD_GUIDE/.rules-version")"
  if [[ "$local_v" != "$kit_v" ]]; then
    echo "WARN: rules pin $local_v behind kit $kit_v — run check-kit-sync.sh --update" >&2
    # Advisory only when hash/pointer checks passed (EXIT-CODES.md).
    if [[ "$sync_fail" -eq 0 ]]; then
      echo "check-kit-sync: lag only (exit 2)"
      exit 2
    fi
  fi
fi

if [[ "$sync_fail" -ne 0 ]]; then
  exit 1
fi
echo "check-kit-sync: ok (version $(tr -d '[:space:]' <"$FG_VERSION" 2>/dev/null || echo unknown))"
exit 0
