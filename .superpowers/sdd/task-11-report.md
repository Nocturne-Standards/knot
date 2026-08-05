# Leaf #11 report — public-hygiene-b5

## Delivered

1. Pre-squash public surface: SECURITY/CONTRIBUTING/CHANGELOG/NOTICE, GitHub templates,
   dependabot.yml; CI fmt + clippy; prose rewrites on front-door READMEs and knot-tool/collector leaks.
2. B5: `deployments-crate` optional feature; default `deployments.rs` loader (no private git fetch).
3. `docs/design-notes.md` with five IMPLEMENTATION §5.1 entries.
4. `ALLOW_PRIVATE_TIER=1` unchanged in hygiene CI.

## Deferred

- `cargo deny check` — no `deny.toml`; note in leaf Evidence.
- Full `knot-encoding/src/lib.rs` module-doc § scrub (not front-door prose).
- Launch squash / internal tree removal (leaf #12).

## Verification

```text
cargo test -p knot-tool --lib          # 31 passed (incl. deployments::tests ×2)
cargo fmt --check                      # ok
cargo clippy --workspace --all-targets -- -D warnings   # ok
ALLOW_PRIVATE_TIER=1 scripts/check-public-surface.sh    # warn-only (pre-launch)
cargo tree -p knot-tool -e normal      # no nocturne-deployments
```

## Commits

(filled after push)
