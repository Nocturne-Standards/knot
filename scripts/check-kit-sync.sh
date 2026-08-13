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

_here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
_fc="$_here/lib/estate-fail-contract.sh"
if [[ ! -f "$_fc" && -n "${NOCTURNE_AGENT_KIT:-}" && -f "${NOCTURNE_AGENT_KIT}/bin/lib/estate-fail-contract.sh" ]]; then
  _fc="${NOCTURNE_AGENT_KIT}/bin/lib/estate-fail-contract.sh"
fi
if [[ ! -f "$_fc" ]]; then
  echo "check-kit-sync: missing estate-fail-contract.sh (copy scripts/lib or set NOCTURNE_AGENT_KIT)" >&2
  exit 1
fi
# shellcheck disable=SC1090
source "$_fc"

kit_sync_fail() {
  estate_gate_line kit-sync fail hard
  estate_hint "$1"
  exit 1
}

kit_sync_lag() {
  estate_gate_line kit-sync warn soft
  estate_hint "$1"
  exit 2
}

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || true)"
if [[ -z "$ROOT" ]]; then
  echo "check-kit-sync: not inside a git work tree" >&2
  kit_sync_fail "run check-kit-sync.sh from inside a git work tree"
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

# Destination id for .worktrees/kit-sync-<id> / chore/kit-sync-<id>.
# Prefer released kit tag (same as estate start HINT); else pin file.
kit_sync_target_id() {
  if [[ -n "${NOCTURNE_AGENT_KIT_TAG:-}" ]]; then
    printf '%s\n' "$NOCTURNE_AGENT_KIT_TAG"
    return 0
  fi
  local t
  t="$(git -C "$KIT" describe --tags --exact-match HEAD 2>/dev/null || true)"
  if [[ -n "$t" ]]; then
    printf '%s\n' "$t"
    return 0
  fi
  if [[ -f "$KIT/FIELD_GUIDE/.rules-version" ]]; then
    tr -d '[:space:]' <"$KIT/FIELD_GUIDE/.rules-version"
    return 0
  fi
  return 1
}

kit_sync_on_matching_leaf() {
  local want base branch
  want="$(kit_sync_target_id)" || return 1
  [[ -n "$want" ]] || return 1
  base="$(basename "$ROOT")"
  branch="$(git symbolic-ref --short HEAD 2>/dev/null || true)"
  [[ "$base" == "kit-sync-$want" ]] && return 0
  case "$ROOT" in
    *"/.worktrees/kit-sync-$want") return 0 ;;
  esac
  [[ "$branch" == "chore/kit-sync-$want" || "$branch" == "kit-sync-$want" ]] && return 0
  return 1
}

cmd_update() {
  if ! is_kit_repo; then
    if [[ "${DEV_WORKFLOW_ALLOW_KIT_SYNC_UPDATE:-}" != "1" ]]; then
      want="$(kit_sync_target_id 2>/dev/null || true)"
      if [[ -z "$want" ]]; then
        kit_sync_fail "checkout a released kit tag (or set NOCTURNE_AGENT_KIT_TAG) before --update"
      fi
      if ! kit_sync_on_matching_leaf; then
        echo "BLOCKED: --update only on .worktrees/kit-sync-$want (branch chore/kit-sync-$want)" >&2
        kit_sync_fail "open .worktrees/kit-sync-$want — do not --update on feat or recycle an old kit-sync leaf. Escape: DEV_WORKFLOW_ALLOW_KIT_SYNC_UPDATE=1"
      fi
    fi
    if [[ ! -f "$KIT/FIELD_GUIDE/rules.md" ]]; then
      echo "check-kit-sync: --update needs kit FIELD_GUIDE/rules.md at $KIT" >&2
      kit_sync_fail "set NOCTURNE_AGENT_KIT to kit checkout with FIELD_GUIDE/rules.md"
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
  kit_sync_fail "missing FIELD_GUIDE/rules.md — run check-kit-sync.sh --update"
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
# Untagged kit HEAD: reverse-check last tag's bin/, not live extras (F6).
kit_bin_ref=""
if [[ -f "$KIT/FIELD_GUIDE/SPEC.md" && -d "$KIT/bin" ]]; then
  BIN_DIR="$KIT/bin"
  VALIDATE_REVERSE=1
  if [[ -n "${NOCTURNE_AGENT_KIT_TAG:-}" ]]; then
    kit_bin_ref="$NOCTURNE_AGENT_KIT_TAG"
  elif git -C "$KIT" rev-parse --git-dir >/dev/null 2>&1; then
    if ! git -C "$KIT" describe --tags --exact-match HEAD >/dev/null 2>&1; then
      kit_bin_ref="$(git -C "$KIT" describe --tags --abbrev=0 2>/dev/null || true)"
      if [[ -n "$kit_bin_ref" ]]; then
        echo "WARN: kit HEAD untagged — reverse-check vs $kit_bin_ref bin/" >&2
      fi
    fi
  fi
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
    if [[ -n "$kit_bin_ref" ]]; then
      while IFS= read -r rel || [[ -n "$rel" ]]; do
        [[ -n "$rel" ]] || continue
        base="$(basename "$rel")"
        case "$base" in
          check-*.sh) ;;
          *) continue ;;
        esac
        [[ "$base" == "check-kit-sync.sh" ]] && continue
        case " $gates_in_rules " in
          *" $base "*) ;;
          *)
            echo "BLOCKED: $base in $kit_bin_ref bin/ not cited by any gate: rule in rules.md" >&2
            sync_fail=1
            ;;
        esac
      done < <(git -C "$KIT" ls-tree --name-only "$kit_bin_ref" bin/ 2>/dev/null)
    else
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
fi

# F5: kit repo — rules.md changed vs last tag requires .rules-version bump.
if is_kit_repo; then
  last_tag="$(git describe --tags --abbrev=0 2>/dev/null || true)"
  if [[ -n "$last_tag" ]] && ! git diff --quiet "$last_tag" -- FIELD_GUIDE/rules.md 2>/dev/null; then
    tagged_ver="$(git show "$last_tag:FIELD_GUIDE/.rules-version" 2>/dev/null | tr -d '[:space:]')"
    local_ver="$(tr -d '[:space:]' <"$FG_VERSION" 2>/dev/null || true)"
    if [[ -n "$tagged_ver" && "$tagged_ver" == "$local_ver" ]]; then
      echo "BLOCKED: FIELD_GUIDE/rules.md changed vs $last_tag but .rules-version still $local_ver — bump pin" >&2
      sync_fail=1
    fi
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
      kit_sync_lag "rules pin behind kit — run check-kit-sync.sh --update (use .worktrees/kit-sync-<tag> when product tree is dirty)"
    fi
  fi
fi

# F9: carve CI still wrapping kit-sync in ci-accept-warn (template is HARD).
ci_soft=0
if ! is_kit_repo && [[ -d "$ROOT/.github/workflows" ]]; then
  for wf in "$ROOT/.github/workflows"/*.yml "$ROOT/.github/workflows"/*.yaml; do
    [[ -f "$wf" ]] || continue
    if grep -qE 'ci-accept-warn\.sh[[:space:]].*check-kit-sync\.sh' "$wf"; then
      echo "WARN: $(basename "$wf") wraps kit-sync in ci-accept-warn (template is HARD)" >&2
      ci_soft=1
    fi
  done
fi

if [[ "$sync_fail" -ne 0 ]]; then
  kit_sync_fail "fix kit-sync — see BLOCKED messages above (run check-kit-sync.sh --update when pin/hash diverges)"
fi
if [[ "$ci_soft" -ne 0 ]]; then
  kit_sync_lag "CI still uses ci-accept-warn on check-kit-sync.sh — drop wrap (templates/hygiene.yml is HARD)"
fi
echo "check-kit-sync: ok (version $(tr -d '[:space:]' <"$FG_VERSION" 2>/dev/null || echo unknown))"
exit 0
