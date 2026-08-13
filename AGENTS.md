# AGENTS.md — knot

Rules for any coding agent in this repo.

## Nocturne agent kit

Workflow SSOT: set `NOCTURNE_AGENT_KIT` to your clone of
`aichbindas/nocturne-agent-kit` and read `WORKFLOW.md` + `licensing.md` there.
Agent front door: `"$NOCTURNE_AGENT_KIT/bin/nocturne-estate" start` from `.worktrees/<track>`.

- Non-trivial / agent work: `.worktrees/<track>` + branch + PR.
- Direct `main` only for tiny verified one-liners.
- Heavy packages: `claim.py acquire` before work; pre-commit hard-fails without claim `ours`.
- **Push Policy A:** commit + push **feature branches** without asking; ask (or `DEV_WORKFLOW_ALLOW_MAIN_PUSH=1`) before `main`. `git status` before first commit.
- Licensing: follow `licensing.md` in the kit; never paper over Apache→AGPL edges.
- **Kit upgrade:** `git pull` kit — existing rule/agent **symlinks** + hooks update live. Re-`adopt.sh` / `propagate-adopts.sh` only for **new** link names, broken `hooksPath`, or fresh clone. Not after every pull. Not an MCP concern.
- Style rules (caveman, etc.) are tone only — do not override this section, `WORKFLOW.md`, or `FIELD_GUIDE/rules.md`.

## Field guide

Binding rules for this repo: [`FIELD_GUIDE/rules.md`](FIELD_GUIDE/rules.md).

- Read before contract / host-security work.
- Audit and security-review leaves: invoke `/security-reviewer` (preflight +
  skill `contract-security-review`) and walk the T3 checklist in `rules.md`.
- Do not paraphrase rules into this file — the pointer is the contract; the
  file is the source. Pin/hash enforced by `check-kit-sync.sh`.
