#!/usr/bin/env bash
# Exit codes: bin/EXIT-CODES.md (0 ok / 1 hard / 2 warn).
# Body check for .contract-authz-baseline.
#
# WHY THIS EXISTS
# ---------------
# check-contract-authz.sh forces a written caller for every pub fn. It does
# not read the function. A line can say require_owner while the body never
# calls it.
#
# This gate reads require_* and assert_* names out of the baseline reason.
# Each named helper must be called from that export, or from a same-file
# function the export calls directly (one hop). Reasons that name neither
# stay human. This does not understand allowlists, principals, or expiry.
#
# No baseline yet: exit 2 (advisory). check-contract-authz.sh owns seeding.
#
# Bash 3.2+ compatible. The walk is python3.
set -uo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || true)"
if [[ -z "$ROOT" ]]; then
  echo "check-contract-authz-body: not inside a git work tree" >&2
  exit 1
fi
cd "$ROOT"

if ! command -v python3 >/dev/null 2>&1; then
  echo "check-contract-authz-body: python3 required" >&2
  exit 1
fi

files="$(git grep -lE '#\[dusk_forge::contract([[:space:]]*\(|[[:space:]]*\])' -- '*.rs' \
  ':!vendor' ':!target' ':!.worktrees' 2>/dev/null || true)"
if [[ -z "$files" ]]; then
  echo "ok: check-contract-authz-body (no dusk-forge contracts)"
  exit 0
fi

BASELINE=".contract-authz-baseline"
if [[ ! -f "$BASELINE" ]]; then
  echo "WARN: no $BASELINE — body check has nothing to read" >&2
  echo "  Run: scripts/check-contract-authz.sh --update" >&2
  exit 2
fi

export CONTRACT_AUTHZ_BASELINE="$BASELINE"
python3 - <<'PY'
import os, re, sys

baseline_path = os.environ["CONTRACT_AUTHZ_BASELINE"]
REQUIRE_RE = re.compile(r"\b(?:require|assert)_[A-Za-z0-9_]+\b")
FN_RE = re.compile(
    r"(?m)^[ \t]*"
    r"(?P<vis>pub[ \t]+)?"
    r"(?:const[ \t]+)?"
    r"(?:async[ \t]+)?"
    r"(?:unsafe[ \t]+)?"
    r"fn[ \t]+(?P<name>[A-Za-z_][A-Za-z0-9_]*)"
)
CALL_RE = re.compile(r"\b([A-Za-z_][A-Za-z0-9_]*)\s*\(")


def skip_line_comment(src, i):
    end = src.find("\n", i)
    return len(src) if end < 0 else end + 1


def skip_block_comment(src, i):
    end = src.find("*/", i + 2)
    return len(src) if end < 0 else end + 2


def skip_tick(src, i):
    # Char literal ('x', '\'', '{') or a lifetime ('a, 'static).
    n = len(src)
    if i + 1 < n and src[i + 1] == "\\":
        i += 2
        while i < n and src[i] != "'":
            if src[i] == "\\":
                i += 2
                continue
            i += 1
        return min(n, i + 1)
    if i + 2 < n and src[i + 2] == "'":
        return i + 3
    i += 1
    while i < n and (src[i].isalnum() or src[i] == "_"):
        i += 1
    return i


def skip_string(src, quote_i):
    hashes = 0
    j = quote_i - 1
    while j >= 0 and src[j] == "#":
        hashes += 1
        j -= 1
    raw = (
        hashes >= 0
        and j >= 0
        and src[j] == "r"
        and (j == 0 or not (src[j - 1].isalnum() or src[j - 1] == "_"))
    )
    if not raw:
        hashes = 0
    i = quote_i + 1
    n = len(src)
    closer = "#" * hashes
    while i < n:
        if not raw and src[i] == "\\":
            i += 2
            continue
        if src[i] == '"':
            if hashes == 0:
                return i + 1
            if src[i + 1 : i + 1 + hashes] == closer:
                return i + 1 + hashes
        i += 1
    return n


def find_body_open(src, i):
    paren = 0
    n = len(src)
    while i < n:
        c = src[i]
        if c == "/" and i + 1 < n and src[i + 1] == "/":
            i = skip_line_comment(src, i)
            continue
        if c == "/" and i + 1 < n and src[i + 1] == "*":
            i = skip_block_comment(src, i)
            continue
        if c == '"':
            i = skip_string(src, i)
            continue
        if c == "'":
            i = skip_tick(src, i)
            continue
        if c == "(":
            paren += 1
        elif c == ")":
            paren = max(0, paren - 1)
        elif c == "{" and paren == 0:
            return i
        i += 1
    return -1


def body_code(src, open_i):
    i = open_i + 1
    depth = 1
    out = []
    n = len(src)
    while i < n and depth:
        c = src[i]
        if c == "/" and i + 1 < n and src[i + 1] == "/":
            i = skip_line_comment(src, i)
            continue
        if c == "/" and i + 1 < n and src[i + 1] == "*":
            i = skip_block_comment(src, i)
            continue
        if c == '"':
            i = skip_string(src, i)
            continue
        if c == "'":
            i = skip_tick(src, i)
            continue
        if c == "{":
            depth += 1
        elif c == "}":
            depth -= 1
            if depth == 0:
                break
        out.append(c)
        i += 1
    return "".join(out)


def functions(src):
    found = []
    for match in FN_RE.finditer(src):
        open_i = find_body_open(src, match.end())
        if open_i < 0:
            continue
        code = body_code(src, open_i)
        found.append((match.group("name"), bool(match.group("vis")), code))
    return found


def has_call(code, name):
    return re.search(r"\b" + re.escape(name) + r"\s*\(", code) is not None


def load(path, cache):
    if path in cache:
        return cache[path]
    try:
        text = open(path, encoding="utf-8", errors="replace").read()
    except OSError:
        cache[path] = None
        return None
    fns = functions(text)
    pub = {}
    all_bodies = {}
    known = set()
    for name, is_pub, code in fns:
        known.add(name)
        all_bodies.setdefault(name, [])
        all_bodies[name].append(code)
        if is_pub:
            pub[name] = code
    cache[path] = (pub, all_bodies, known)
    return cache[path]


def calls_to_known(code, known):
    out = []
    seen = set()
    for match in CALL_RE.finditer(code):
        name = match.group(1)
        if name in known and name not in seen:
            seen.add(name)
            out.append(name)
    return out


cache = {}
failures = []
checked = 0

with open(baseline_path, encoding="utf-8", errors="replace") as handle:
    for raw in handle:
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        entry, sep, reason = line.partition("\t")
        if not sep:
            continue
        reason = reason.strip()
        if reason == "UNREVIEWED":
            continue
        helpers = []
        seen_helper = set()
        for token in REQUIRE_RE.findall(reason):
            if token not in seen_helper:
                seen_helper.add(token)
                helpers.append(token)
        if not helpers:
            continue
        if ":" not in entry:
            failures.append(f"{entry}: baseline entry has no path:method")
            continue
        path, method = entry.rsplit(":", 1)
        path = path.strip()
        method = method.strip()
        parsed = load(path, cache)
        if parsed is None:
            failures.append(f"{path}:{method}: file missing")
            continue
        pub, all_bodies, known = parsed
        if method not in pub:
            # Stale baseline row. check-contract-authz.sh already notes it.
            continue
        checked += 1
        export_body = pub[method]
        searched = export_body
        for callee in calls_to_known(export_body, known):
            if callee == method:
                continue
            searched += "\n" + "\n".join(all_bodies.get(callee, []))
        for helper in helpers:
            if not has_call(searched, helper):
                failures.append(
                    f"{path}:{method}: baseline names {helper} but the export "
                    f"does not call it (nor a same-file function it calls directly)"
                )

if failures:
    print("BLOCKED: baseline helper missing from the function:", file=sys.stderr)
    for item in failures:
        print(f"  {item}", file=sys.stderr)
    print("", file=sys.stderr)
    print("  Name the helper in the baseline only when the body calls it.", file=sys.stderr)
    print("  One hop counts (export calls inner, inner calls the helper).", file=sys.stderr)
    sys.exit(1)

print(f"ok: check-contract-authz-body ({checked} helper checks)")
PY
