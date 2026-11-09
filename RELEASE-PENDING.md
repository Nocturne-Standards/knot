# Release pending

Status: **not published**. crates.io publishing is paused. This file is the
checklist for that publish. The README still pins `knot-encoding` with a git
tag. The `nocturne-knot-encoding = "x.y.z"` snippet, the versioning paragraph and the
Integrate page change only when the crates ship.

Nothing here is a chain step. This repo records no live contract ids.

## Publish order

1. `nocturne-knot-encoding` first. That reserves the prefix.
2. `nocturne-knot-registry` and `nocturne-knot-proposals`.
3. The other libraries in this repo, except warden.
4. `knot-warden` and `knot-warden-encoding` only after Atlas is public, or
   after `atlas-encoding` is on crates.io. They build next to Atlas today.

Leave unpublished: `knot-mock-atlas`, `knot-proposals/test-target`,
`knot-warden/test-proxy`, `evm/e2e-tool`, `evm/vectors-gen`.

`nocturne-knot-collector` is AGPL-3.0-only. The other shipped crates are Apache-2.0.

The data books (`nocturne-knot-registry-data`, `nocturne-knot-proposals-data`) bake
`ATLAS_PIN_HEX` at build time and call Atlas `resolve` on every write.
Publishing them does not remove that pin.

## Replace `x.y.z` at release

These change at release:

- [ ] `README.md` consumer snippet (plain dep and the `call-types` form)
- [ ] `docs/versioning.md` placeholder paragraph
- [ ] Integrate page (`nocturne-docs` `docs/v1/knot/integrate.md`)
- [ ] Quickstart (`docs/v1/knot/run-locally.md`) has no crates.io line yet

## Flip at publish time

- [ ] Set `publish = true` only on the crates in the order above.
      Every workspace crate is `publish = false` today. Versions to publish:
      see `docs/cargo-release.md`.
- [ ] Every dependency needs a `version`. Cargo publishes that version and
      drops a sibling `git` or `path` key. `dusk-core` and `dusk-vm` are
      on crates.io at `=1.7.0`. Path deps on `nocturne-knot-encoding` become version
      deps after step 1.
- [ ] Date the root `CHANGELOG.md` `[Unreleased]` block, and the crate
      changelogs, for the versions that ship.
- [ ] Re-read crate `description` fields against the published API.
- [x] SPDX headers on `knot-registry-data` and `knot-proposals-data`
      (`lib.rs`, `state.rs`, `build.rs`). Logic crates and `knot-encoding`
      already had them.
- [ ] Confirm licence files: root `LICENSE-APACHE` and `LICENSE-AGPL`
      exist. Per-crate `LICENSE` copies are still absent.
- [x] `ATLAS_PIN_HEX` unset or empty fails the book build. CI sets the
      lab id `c1c1…c1` (64 hex chars). A release build takes the Atlas id
      from the operator's environment.
- [ ] `knot-warden` / `knot-warden-encoding`: do not publish while they
      need a private Atlas tree.

## Still true after the docs say "published"

- Testnet only. Not audited yet.
- The Atlas pin stays on the data books. They keep calling `resolve`.
  Do not store a logic contract id in place of that pin.
- Keystores, passwords, and wallet seeds stay out of this repo.
