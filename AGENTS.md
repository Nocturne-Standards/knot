# AGENTS.md — knot

Rules for any coding agent in this repo.

## Nocturne agent kit

Workflow SSOT: set `NOCTURNE_AGENT_KIT` to your clone of
`aichbindas/nocturne-agent-kit` and read `WORKFLOW.md` + `licensing.md` there.

- Non-trivial / agent work: `.worktrees/<track>` + branch + PR.
- Direct `main` only for tiny verified one-liners.
- Heavy packages: `claim.py acquire` before work; pre-commit hard-fails without claim `ours`.
- Never commit without being asked; `git status` first.
- Licensing: follow `licensing.md` in the kit; never paper over Apache→AGPL edges.
