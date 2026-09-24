#!/usr/bin/env bash
# Deploy knot-registry and knot-proposals as data + logic.
# Pins land in nocturne-deployments duskds/testnet.json.
# Does not fund data ids. Does not set_authorized_account. Does not move warden's scheduler.
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
ND_ROOT="${NOCTURNE_DEPLOYMENTS:-$ROOT/../nocturne-deployments}"
DUSK_PIN="$ND_ROOT/duskds/testnet.json"

if [ -z "${RUSK_WALLET_PWD:-}" ] && [ -f "$ROOT/../sme_platform/.env.testnet" ]; then
  set -a
  # shellcheck disable=SC1091
  source "$ROOT/../sme_platform/.env.testnet"
  set +a
fi

# duskds Atlas. Guardian is knot-warden. Direct set_service panics.
ATLAS_PIN_HEX="${ATLAS_PIN_HEX:-21c653385e3b7cc8be32edd5e6df2e30b6464be867f39442171ba8ca8d089612}"
if [ "${#ATLAS_PIN_HEX}" -ne 64 ]; then
  echo "error: ATLAS_PIN_HEX must be 64 hex chars" >&2
  exit 1
fi
export ATLAS_PIN_HEX
export CALLER_REPO_ROOT="$ROOT"
export NOCTURNE_DEPLOYMENTS="$ND_ROOT"
unset DEPLOYMENTS_MIRROR_FILE || true

RESUME=0
NO_SCHEDULE=0
for arg in "$@"; do
  case "$arg" in
    --resume) RESUME=1 ;;
    --no-schedule) NO_SCHEDULE=1 ;;
    *) echo "error: unknown arg $arg" >&2; exit 1 ;;
  esac
done

DEPLOY="$ND_ROOT/scripts/deploy-contract.sh"
WIRE="$ND_ROOT/scripts/wire-contract.sh"
LAB_PIN="c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1"
WASM_DIR="$ROOT/target/contract/wasm32-unknown-unknown/release"

pin_id() {
  jq -r --arg k "$1" '.contracts[$k].current.contract_id // empty' "$DUSK_PIN"
}

if [ "$RESUME" -eq 0 ]; then
  rm -f \
    "$WASM_DIR/knot_registry_data.wasm" \
    "$WASM_DIR/knot_registry.wasm" \
    "$WASM_DIR/knot_proposals_data.wasm" \
    "$WASM_DIR/knot_proposals.wasm"

  echo "Building registry book against duskds Atlas $ATLAS_PIN_HEX"
  "$DEPLOY" "$ROOT/crates/knot-registry-data" --record-as knot-registry-data -y
  echo "Building registry logic"
  "$DEPLOY" "$ROOT/crates/knot-registry" --record-as knot-registry -y
  echo "Building proposals book"
  "$DEPLOY" "$ROOT/crates/knot-proposals-data" --record-as knot-proposals-data -y
  echo "Building proposals logic"
  "$DEPLOY" "$ROOT/crates/knot-proposals" --record-as knot-proposals -y
fi

REG_DATA="$(pin_id knot-registry-data)"
REG_LOGIC="$(pin_id knot-registry)"
PROP_DATA="$(pin_id knot-proposals-data)"
PROP_LOGIC="$(pin_id knot-proposals)"
for id in "$REG_DATA" "$REG_LOGIC" "$PROP_DATA" "$PROP_LOGIC"; do
  if [ -z "$id" ]; then
    echo "error: missing a pin id" >&2
    exit 1
  fi
done

python3 - <<PY
from pathlib import Path
atlas = bytes.fromhex("$ATLAS_PIN_HEX")
lab = bytes.fromhex("$LAB_PIN")
root = Path("$WASM_DIR")
for name in ("knot_registry_data.wasm", "knot_proposals_data.wasm"):
    body = (root / name).read_bytes()
    if atlas not in body:
        raise SystemExit(f"{name} is missing the duskds Atlas pin")
    if lab in body:
        raise SystemExit(f"{name} still contains the lab Atlas pin")
    print(name, "pins duskds Atlas at", body.index(atlas))
PY

if [ "$NO_SCHEDULE" -eq 0 ]; then
  echo "Scheduling Atlas services via warden"
  for pair in "knot-registry:$REG_LOGIC" "knot-proposals:$PROP_LOGIC"; do
    name="${pair%%:*}"
    id="${pair#*:}"
    sched_json=$(printf '{"name":"%s","id":"%s"}' "$name" "$id")
    "$WIRE" knot-warden schedule_service "$sched_json" --simulate-first -y
  done
  echo "Applying warden repoints"
  for name in knot-registry knot-proposals; do
    exec_json=$(printf '"%s"' "$name")
    ok=0
    for _ in 1 2 3 4 5 6 7 8; do
      if "$WIRE" knot-warden execute_service "$exec_json" --simulate-first -y; then
        ok=1
        break
      fi
      sleep 15
    done
    if [ "$ok" -ne 1 ]; then
      echo "error: execute_service $name did not land" >&2
      exit 1
    fi
  done
fi

echo "init_data"
"$WIRE" knot-registry init_data "\"$REG_DATA\"" --simulate-first -y
"$WIRE" knot-proposals init_data "\"$PROP_DATA\"" --simulate-first -y
echo "init_registry"
"$WIRE" knot-proposals init_registry "\"$REG_LOGIC\"" --simulate-first -y

echo "knot cutover wired"
echo "  registry logic $REG_LOGIC data $REG_DATA"
echo "  proposals logic $PROP_LOGIC data $PROP_DATA"
echo "Next: create_account, then set_authorized_account. Do not fund the data ids."
