#!/usr/bin/env bash
# Builds the static, backend-free Knot demo site into demo/dist/.
#
# Source of truth stays crates/knot-tool/static/ (shared with the embedded
# Rust web UI). This script only copies it and flips one flag: the real
# Rust tool must never silently degrade into mock mode, so index.html never
# sets window.KNOT_FRONTEND_MOCK itself — this build injects it for the
# static-only deploy, where crates/knot-tool/static/mock-ledger.js's
# client-side mock ledger is the only backend that exists.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC="$ROOT/crates/knot-tool/static"
OUT="$ROOT/demo/dist"

rm -rf "$OUT"
mkdir -p "$OUT"
cp -R "$SRC/." "$OUT/"

python3 - "$OUT/index.html" <<'PY'
import sys

path = sys.argv[1]
with open(path, "r", encoding="utf-8") as f:
    html = f.read()

marker = '<script src="/mock-ledger.js"></script>'
if marker not in html:
    raise SystemExit(f"build.sh: expected marker not found in {path}")

flag = '<script>window.KNOT_FRONTEND_MOCK = true;</script>\n  ' + marker
html = html.replace(marker, flag, 1)

with open(path, "w", encoding="utf-8") as f:
    f.write(html)
PY

echo "demo/build.sh: built $OUT from $SRC (KNOT_FRONTEND_MOCK=true injected)"
