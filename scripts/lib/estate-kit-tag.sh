# shellcheck disable=SC1091
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/estate-fail-contract.sh"

estate_require_kit_tag() {
  local kit="${NOCTURNE_AGENT_KIT:?NOCTURNE_AGENT_KIT unset}"
  if [[ -n "${NOCTURNE_AGENT_KIT_TAG:-}" ]]; then
    echo "$NOCTURNE_AGENT_KIT_TAG"
    return 0
  fi
  local tag
  tag="$(git -C "$kit" describe --tags --exact-match HEAD 2>/dev/null || true)"
  if [[ -z "$tag" ]]; then
    estate_gate_line kit-tag fail hard
    estate_hint "checkout a released tag: git -C \"\$NOCTURNE_AGENT_KIT\" fetch --tags && git checkout <tag>"
    return 1
  fi
  echo "$tag"
}
