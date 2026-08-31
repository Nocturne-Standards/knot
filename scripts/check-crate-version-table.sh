#!/usr/bin/env bash
# Exit codes: bin/EXIT-CODES.md (0 ok / 1 hard / 2 warn).
# Fail when README (or VERSION_TABLE_FILE) crate versions drift from Cargo.toml.
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || true)"
if [[ -z "$ROOT" ]]; then
  echo "check-crate-version-table: not inside a git work tree" >&2
  exit 1
fi
cd "$ROOT"

TABLE="${VERSION_TABLE_FILE:-README.md}"
if [[ ! -f "$TABLE" ]]; then
  echo "check-crate-version-table: missing $TABLE" >&2
  exit 1
fi

if ! command -v cargo >/dev/null; then
  echo "check-crate-version-table: cargo not on PATH" >&2
  exit 1
fi
if ! command -v python3 >/dev/null; then
  echo "check-crate-version-table: python3 not on PATH" >&2
  exit 1
fi

python3 - "$TABLE" <<'PY'
import json, re, subprocess, sys

table_path = sys.argv[1]
text = open(table_path, encoding="utf-8").read()

# Knot README shape: | `crate-name` | license | x.y.z | ...
row_re = re.compile(
    r"^\|\s*`([a-z0-9_-]+)`\s*\|[^|]*\|\s*(\d+\.\d+\.\d+)\s*\|",
    re.M,
)
table = {m.group(1): m.group(2) for m in row_re.finditer(text)}
if not table:
    print(
        f"check-crate-version-table: no crate version rows in {table_path} — skip (advisory)",
        file=sys.stderr,
    )
    sys.exit(2)

meta = json.loads(
    subprocess.check_output(
        ["cargo", "metadata", "--format-version", "1", "--no-deps"],
        text=True,
    )
)
packages = {p["name"]: p["version"] for p in meta["packages"]}
# Prefer workspace members only
members = set(meta.get("workspace_members", []))
member_names = set()
for p in meta["packages"]:
    if p["id"] in members:
        member_names.add(p["name"])

fail = 0
for name, ver in sorted(table.items()):
    if member_names and name not in member_names:
        # Table may list non-workspace names — skip unknown
        if name not in packages:
            print(f"note: {name} in table but not in cargo metadata — skipped")
            continue
    cargo_ver = packages.get(name)
    if cargo_ver is None:
        print(f"BLOCKED: {name} in {table_path} but missing from cargo metadata", file=sys.stderr)
        fail = 1
        continue
    if cargo_ver != ver:
        print(
            f"BLOCKED: {name} table={ver} Cargo.toml={cargo_ver}",
            file=sys.stderr,
        )
        fail = 1
    else:
        print(f"ok: {name} {ver}")

sys.exit(fail)
PY
