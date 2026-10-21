# Release pending

Status: **not published**. crates.io publishing is paused. This file is the
checklist for that publish. The README still pins `knot-encoding` with a git
tag. The placeholder `knot-encoding = "x.y.z"` snippet, the versioning
paragraph, and the Integrate page stay on `cursor/knot-council-setup-8037`
until release. Do not merge that branch before the crates ship.

Nothing here is a chain step. Contract ids stay in
[Deployments](https://docs.nocturne-standards.org/v1/deployments/).

## Publish order

1. `knot-encoding` first. That reserves the name.
2. `knot-registry` and `knot-proposals`.
3. The other libraries in this repo, except warden.
4. `knot-warden` and `knot-warden-encoding` only after Atlas is public, or
   after `atlas-encoding` is on crates.io. They build next to Atlas today.

Leave unpublished: `knot-mock-atlas`, `knot-proposals/test-target`,
`knot-warden/test-proxy`, `evm/e2e-tool`, `evm/vectors-gen`.

`knot-collector` is AGPL-3.0-only. The other shipped crates are Apache-2.0.

The data books (`knot-registry-data`, `knot-proposals-data`) bake
`ATLAS_PIN_HEX` at build time and call Atlas `resolve` on every write.
Publishing them does not remove that pin.

## Replace `x.y.z` at release

These live on `cursor/knot-council-setup-8037` until then:

- [ ] `README.md` consumer snippet (plain dep and the `call-types` form)
- [ ] `docs/versioning.md` placeholder paragraph
- [ ] Integrate page (`nocturne-docs` `docs/v1/knot/integrate.md`)
- [ ] Quickstart (`docs/v1/knot/run-locally.md`) has no crates.io line yet

## Flip at publish time

- [ ] Set `publish = true` only on the crates in the order above.
      Every workspace crate is `publish = false` today, including
      `knot-encoding` 0.1.2, `knot-registry` 0.2.1, `knot-proposals` 0.4.0,
      `knot-registry-data` 0.1.0, `knot-proposals-data` 0.1.0,
      `knot-tool` 0.2.0, `knot-collector` 0.2.0,
      `knot-warden` 0.2.1, `knot-warden-encoding` 0.2.0,
      `knot-event-decoder` 0.1.0.
- [ ] Every dependency needs a `version`. Cargo publishes that version and
      drops a sibling `git` or `path` key. `dusk-core` still has
      `git = "https://github.com/dusk-network/rusk"` next to a version.
      Path deps on `knot-encoding` become version deps after step 1.
- [ ] Date the root `CHANGELOG.md` `[Unreleased]` block, and the crate
      changelogs, for the versions that ship.
- [ ] Re-read crate `description` fields against the published API.
- [ ] SPDX headers: logic crates and `knot-encoding` already carry
      `SPDX-License-Identifier`. `knot-registry-data` and
      `knot-proposals-data` sources do not. Add Apache-2.0 headers
      before those crates publish.
- [ ] Confirm licence files: root `LICENSE-APACHE` and `LICENSE-AGPL`,
      plus each crate `LICENSE`.
- [ ] `knot-warden` / `knot-warden-encoding`: do not publish while they
      need a private Atlas tree.

## Still true after the docs say "published"

- Testnet only. Not audited yet.
- The Atlas pin stays on the data books. They keep calling `resolve`.
  Do not store a logic contract id in place of that pin.
- Warden delay on the current testnet pin is 30 blocks.
- Keystores, passwords, and wallet seeds stay out of this repo.
