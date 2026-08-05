# Public repo standard

A reusable framework for taking any repo public and keeping it clean afterward.
Written for Knot, intended to apply to every future public-facing repo.

Two problems it solves:

1. **Contamination** — planning talk, private-doc pointers, and secrets leaking into
   public surfaces.
2. **Continuity** — keeping the private material you actually need, without it living
   one `git add -A` away from publication.

---

## 1. The core rule

> **A public document must be complete on its own terms and must never cite a
> private document as the authority for a claim.**

Everything else follows. If a doc says "see §4a of the implementation plan" for the
reason something is true, the public reader cannot verify it and the doc has failed.
Either inline the reasoning or drop the claim.

Corollary: **public docs answer "what is true now"; private docs answer "how we got
here and what's next."** Anything phrased as a decision-in-progress, a wave, a track,
a milestone, or a TODO belongs in the private tier.

---

## 2. Document tiers

Four tiers. Every document belongs to exactly one, and each has a home.

| Tier | Contains | Lives in |
|---|---|---|
| **P0 — Public product** | README, crate READMEs, security model, architecture, API reference, CHANGELOG, SECURITY.md, CONTRIBUTING.md | Public repo |
| **P1 — Public process** | ADRs, RFCs, published post-mortems, closed security advisories | Public repo, `docs/` — optional, publish only what's finished |
| **P2 — Private working** | Specs, roadmaps, wave/track plans, open audit findings, fix plans, agent workflow, editor config | **Separate private repo** |
| **P3 — Operational secrets** | Credentials, host inventories, deploy runbooks, wallet material | Secret manager — never any git repo |

The most common failure is P2 material sitting in a P0 file (Knot's README "Status"
section) or P3 material in a P0 file (Knot's `KNOT_COLLECTOR_PASSWORD`).

---

## 3. Handling the private tier — the recommendation

**Use a separate private repo. Do not use a gitignored directory or a private
submodule inside the public repo.**

Rationale, in order of weight:

1. **A gitignored `docs/internal/` is one mistake from publication.** `git add -f`,
   a rewritten `.gitignore`, a `git stash` that picks it up, a new contributor with a
   different global ignore file. The failure is silent and permanent — and for a repo
   like Knot, where the private tier contains an unremediated security audit, the blast
   radius is real.
2. **A private submodule leaks its existence.** `.gitmodules` is public and names the
   private repo and its URL. Anyone can see there's a `knot-internal` and start guessing.
   It also breaks `git clone` for everyone without access, which makes your public repo
   look broken.
3. **Separate repos let the private tier be messy.** Planning docs *should* be full of
   half-decisions and dead ends. That's their job. Forcing them to share a working tree
   with the polished public repo creates constant pressure to sanitize them, which
   defeats the purpose.
4. **Different lifecycles.** Public repo history should be clean and squashable. Private
   planning wants dense incremental commits.

### The layout

```
aichbindas/knot                 (public)
├── README.md                   P0
├── SECURITY.md                 P0
├── CONTRIBUTING.md             P0
├── CHANGELOG.md                P0
├── docs/
│   ├── README.md               index
│   ├── architecture.md         P0
│   ├── security-model.md       P0
│   └── adr/                    P1 — optional
└── crates/*/README.md          P0

aichbindas/knot-internal        (private)
├── specs/                      P2
├── audits/                     P2 — incl. open findings and fix plans
├── roadmap.md                  P2
├── runbooks/                   P2 (pointers only; secrets in the manager)
├── agent/                      P2 — AGENTS.md, .cursor/, kit markers
└── release/
    ├── PUBLIC-REPO-STANDARD.md this file
    └── knot-cleanup.md         per-release checklists
```

Check them out as siblings, not nested:

```
~/dev/aichbindas/
├── knot/
└── knot-internal/
```

Sibling checkouts make accidental cross-contamination structurally impossible — there
is no path from one working tree into the other.

### If you truly need them co-located

Only acceptable variant: a **separate branch in the private repo**, or a worktree of
`knot-internal` mounted at a path that is in the public repo's `.gitignore` *and* in
`.git/info/exclude` *and* covered by the pre-commit hook in §5. Belt, braces, and a
third belt. Prefer siblings.

### Migrating existing private content

For each file moving from public to private: `git rm --cached`, add to `.gitignore`,
commit the removal, then copy into the private repo. Remember the file remains in
public history until you rewrite (§6).

---

## 4. The vocabulary rule

Internal shorthand is the single largest source of contamination because it is
invisible to the person who wrote it. Maintain an explicit translation table per
project and apply it mechanically.

| Kind of shorthand | Rule |
|---|---|
| Spec section refs (`§4a`, `§2.1`) | **Never public.** Inline the content, or name the concept. |
| Milestones (`M1`, `M3`) | **Never public.** Describe the capability. |
| Phases (`Wave 7`, `Track 3`, `Phase B`) | **Never public.** Delete; they describe your schedule. |
| Spec numbers (`Spec 26`, `23b`) | **Never public.** Describe the change. |
| Finding ids (`audit I6`, `R4`) | Public only if the advisory is public and linked. |
| Internal repo/product names | **Never public** unless that repo is public. Includes legacy codenames in domain tags — if you can't change them, explain them once. |
| Absolute local paths | **Never public.** No exceptions. |
| TODO / FIXME / "operator TODO" | Not in public docs. In code, only with a tracking issue link. |

Give each recurring concept a real public name once, and use it everywhere. Knot's
`§4a` → **"proposal preimage"** is a better name than `§4a` even internally.

---

## 5. Automate it — a leak gate in CI

Rules that aren't enforced decay. Add a `scripts/check-public-surface.sh`, run it in CI
and as a pre-commit hook, and fail the build on any hit.

```bash
#!/usr/bin/env bash
# Fails if internal vocabulary, private paths, or secrets reach the public tree.
set -uo pipefail
fail=0

check() { # check <label> <regex>
  if git grep -nIE "$2" -- ':!scripts/check-public-surface.sh' >/tmp/hits 2>/dev/null; then
    echo "BLOCKED: $1"; cat /tmp/hits; fail=1
  fi
}

check "spec section reference"   '§[0-9]'
check "wave/track/milestone"     '\b(Wave|Track|Phase) [0-9A-Z]\b'
check "spec number"              '\b[Ss]pec [0-9]'
check "private doc path"         '(docs/superpowers|references/|rusk-experiments|deployments/testnet\.json)'
check "internal repo name"       '(sme_platform|nocturne-agent-kit|nocturne-mcp-|pituitary)'
check "absolute local path"      '/(Users|home)/[a-z]'
check "escaping relative link"   '\]\(\.\./\.\./\.\./'
check "unresolved marker"        '\b(TODO|FIXME|XXX|HACK)\b'
check "possible credential"      '(PASSWORD|PWD|SECRET|TOKEN|API_KEY) *= *[^.$\x27"]{6,}'

exit $fail
```

Notes:

- Tune the regexes per project; the `sme-platform.*` domain-tag constants in Knot need
  an explicit allowlist since they cannot change.
- Add a real secret scanner alongside it — `gitleaks` or GitHub's push protection.
  The regex above catches the shape of Knot's leaked password but is not a substitute.
- Enable **GitHub secret scanning + push protection** on the org before the first
  public push. It is free on public repos and would have blocked `***REMOVED-LEAKED-COLLECTOR-PASSWORD***`.

---

## 6. Pre-publication procedure

Run once, in order, for any repo going public.

**Phase 1 — secrets (do this first, it has a clock)**

1. `gitleaks detect --no-git` on the working tree, and `gitleaks detect` over history.
2. For every hit: **rotate the credential first**, then remove it from the tree.
   Rotation is the fix; deletion is cleanup.
3. Decide history strategy:
   - **Squash to a single commit** — recommended when history has no external value
     and contains secrets. Fastest, zero residue, and you keep the real history in a
     private mirror (`git remote add private ...; git push private --mirror`) if you
     want it.
   - **`git filter-repo --replace-text`** — when public history genuinely matters.
     Slower, requires all collaborators to re-clone, and leaves you verifying that
     nothing was missed.

**Phase 2 — tiering**

4. Classify every tracked file into P0–P3. Anything not clearly P0/P1 goes private.
5. `git rm --cached` the private files; `.gitignore` them; move to the private repo.
6. Add `.worktrees/`, `.cursor/`, and any tool-local directories to `.gitignore`
   *before* they can be committed.

**Phase 3 — prose**

7. Run the §5 gate. Fix every hit — inline the reasoning, don't just delete the pointer.
8. Reread the README as a stranger. Can they build it? Does the first paragraph say
   what the thing is and who it's for, without jargon?
9. Verify every relative link resolves inside the repo, and every external link 200s.

**Phase 4 — completeness**

10. Add SECURITY.md, CONTRIBUTING.md, CHANGELOG.md, LICENSE(s),
    NOTICE, PR/issue templates, dependabot.

    *NOTICE is an Apache-2.0 §4(d) obligation where the work has one — not optional.
    CODE_OF_CONDUCT is pure convention; deliberately omitted for Knot. Decide per
    project rather than adding it reflexively.*
11. Verify the license story: per-file SPDX headers, per-crate LICENSE files, and a
    root explanation of any split. Add `cargo deny check` to CI if the project mixes
    licenses.
12. Clone into a clean directory and build following only the public README. Fix
    whatever fails. **This step catches more than any review.**

**Phase 5 — publish**

13. Enable branch protection, secret scanning, push protection, and required CI.
14. Push. Tag. Announce.

---

## 7. The public README template

Ordering matters more than content. Readers leave in the first fifteen seconds.

```markdown
# <Product name>

<One sentence: what it is and who it is for. No internal jargon.>

<Two or three sentences: the problem it solves and the approach. Still no jargon.>

## Status
<Stable / beta / experimental. Which networks. What is explicitly NOT supported.>
<This is a maturity signal, not a changelog. One short paragraph.>

## Quick start
<Commands that work in a fresh clone. Verified, not remembered.>

## How it works
<The diagram. The component table. Enough to stand alone if the docs site is down.>

## Documentation
<Links: full docs site, security model, API reference, contributing.>

## Security
<Trust boundaries in one paragraph. Link to SECURITY.md for disclosure.>

## License
<The split, if there is one, and why.>
```

Rules for the template:

- **Status is maturity, not history.** No dated entries, no deploy ids, no version
  archaeology. Those go in CHANGELOG.md. If Knot's README followed this, its Status
  section would be four lines, not forty.
- **Quick start must work in a fresh clone**, with no reference to a parent repo or a
  directory that only exists on your machine.
- **No "dense on purpose" disclaimers.** If a section needs an apology, it is in the
  wrong tier.
- Crate/subpackage READMEs follow the same shape at smaller scale, and link *up* to
  the root README rather than sideways into private docs.

---

## 7b. Stamp every audit and design doc with a SHA

**Any audit, design, or findings document must record the commit it was written
against, in its first few lines.**

This is not bookkeeping. In the Knot engagement, round one was written against a tree
that was 26 commits behind the actual release candidate — two findings were already
fixed and one claim was outright wrong. The reports carried no SHA, so nothing caught
it; the discrepancy surfaced only because the author happened to re-check a branch.

Two rules follow:

1. **Stamp the SHA.** Enforce in CI: any `AUDIT-*` / `DESIGN-*` file must contain a
   7+ character hex SHA in its first 10 lines, and that SHA must resolve in the repo.
2. **Never trust remote-tracking refs.** `origin/main` is a local cache and can be
   arbitrarily stale — in the same engagement it was days old and looked authoritative.
   Any release-readiness tooling opens with `git fetch --prune` and hard-fails unless
   `HEAD == origin/HEAD`.

## 8. Standing rules after launch

- **Every PR passes the §5 gate.** No exceptions, no "I'll clean it up later."
- **New concepts get a public name on day one**, before the internal shorthand sets.
  This is the cheapest possible intervention and the one most often skipped.
- **Security fixes get a public advisory** once patched. Then, and only then, the
  internal finding can move from P2 to P1.
- **Quarterly**: re-run the full §6 Phase 3 sweep. Contamination creeps back in
  through comments and commit messages faster than anyone expects.
- **Onboarding**: this document is the first thing a new contributor with private
  access reads.
