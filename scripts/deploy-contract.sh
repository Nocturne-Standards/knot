#!/usr/bin/env bash
# Knot wrapper → shared deploy script in nocturne-deployments.
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
KNOT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Resolve nocturne-deployments: env → sibling of primary git checkout
# (works from .worktrees/*; bare KNOT_ROOT/../ fails there).
if [ -n "${NOCTURNE_DEPLOYMENTS_ROOT:-}" ]; then
  ND_ROOT="$NOCTURNE_DEPLOYMENTS_ROOT"
else
  PRIMARY="$(cd "$(dirname "$(git -C "$KNOT_ROOT" rev-parse --path-format=absolute --git-common-dir)")" && pwd)"
  if [ -d "$PRIMARY/../nocturne-deployments/scripts" ]; then
    ND_ROOT="$(cd "$PRIMARY/../nocturne-deployments" && pwd)"
  elif [ -d "$KNOT_ROOT/../nocturne-deployments/scripts" ]; then
    ND_ROOT="$(cd "$KNOT_ROOT/../nocturne-deployments" && pwd)"
  else
    echo "error: set NOCTURNE_DEPLOYMENTS_ROOT to the nocturne-deployments checkout" >&2
    exit 1
  fi
fi

export CALLER_REPO_ROOT="$KNOT_ROOT"
export NOCTURNE_DEPLOYMENTS="${NOCTURNE_DEPLOYMENTS:-$ND_ROOT}"
if [ -z "${DEPLOYMENTS_MIRROR_FILE+x}" ]; then
  unset DEPLOYMENTS_MIRROR_FILE || true
fi

exec "$ND_ROOT/scripts/deploy-contract.sh" "$@"
