---
id: 11
slug: public-hygiene-b5
status: DONE
owner: cursor-agent
deps:
  - 3
scope:
  - docs/
  - README.md
  - .github/
  - crates/knot-tool/Cargo.toml
  - SECURITY.md
  - CONTRIBUTING.md
acceptance:
  - §5.1–5.3 removals/prose/templates as far as pre-squash
  - B5 optional nocturne-deployments feature
  - design-notes.md five entries
  - ALLOW_PRIVATE_TIER still OK until launch
acceptanceDone:
  - true
  - true
  - true
  - true
---
# Phase 8: public hygiene + optional deployments dep

Planner context…

## Evidence (worker)

- **Templates/docs:** `SECURITY.md`, `CONTRIBUTING.md`, `CHANGELOG.md`, `NOTICE`,
  `.github/PULL_REQUEST_TEMPLATE.md`, `.github/ISSUE_TEMPLATE/{bug_report,feature_request}.yml`,
  `.github/dependabot.yml`
- **CI:** hygiene job adds `cargo fmt --check` + `cargo clippy --workspace --all-targets -D warnings`;
  `ALLOW_PRIVATE_TIER=1` retained; test job **removed** `GH_PRIVATE_REPO_TOKEN` private-deps auth step
- **B5:** `knot-tool` feature `deployments-crate` (off default); `src/deployments.rs` local pin
  loader (`NOCTURNE_DEPLOYMENTS` + walk-up); optional git dep
  unchanged for internal `--features deployments-crate`
- **design-notes.md:** five locked entries from IMPLEMENTATION §5.1
- **Prose:** README + crate READMEs + `knot-tool` bls/chain + collector README/lib — removed §/Wave/Spec/
  private-path leaks; inlined hardfork signing substance in `bls.rs`
- **Not deleted:** `docs/internal/**`, `docs/superpowers/**`, `AGENTS.md`, `.cursor/**`, `.pituitary/**`
- **Deferred:** `cargo deny` (no `deny.toml` yet); launch squash (#12); encoding `lib.rs` module-doc § refs
  (internal comment tier, not front-door prose)
- **Tests:** `cargo test -p knot-tool --lib` (31 pass incl. 2 deployments); `cargo fmt --check`;
  `cargo clippy --workspace --all-targets -D warnings`; `ALLOW_PRIVATE_TIER=1 scripts/check-public-surface.sh` (warn-only)

## Proposal (worker, if BLOCKED)
