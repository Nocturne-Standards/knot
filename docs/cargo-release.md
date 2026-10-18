# Cargo registry release plan

Status: **not published**. Nothing in this repo is on crates.io.
`publish = false` stays until the wave below actually ships.
Git tags remain the pin for WASM deploys. A crates.io version does not
say which bytecode is on testnet.

The crates.io name `knot` is already taken (unrelated indexer).
Knot packages publish as `nocturne-knot-*`.

## Names

Package name changes. Rust library name does not.

```toml
[package]
name = "nocturne-knot-encoding"

[lib]
name = "knot_encoding"
```

`use knot_encoding::…` stays. Same pattern for every library:
`nocturne-knot-registry` / `knot_registry`, and so on.

## Dusk pins

One dependency, the crates.io crate. Drop every `git =` URL before
`cargo publish` (crates.io rejects `git` and `path` deps, including
optional and dev-dependencies).

```toml
dusk-core = { version = "=1.6.0", default-features = false }
dusk-vm = { version = "=1.6.0", default-features = false }
```

Checked 2026-10-18 against tag `dusk-core-1.6.0`
(`ae1a38a2079c681126a96f94c17d282ea2639946`):

- `dusk-core` 1.6.0 and `dusk-vm` 1.6.0 on crates.io were packed from
  later commit `54871241565b5a57592f4538e5039e90106a6a77`.
- Own `src/` is byte-identical to the tag. `dusk-vm` `tests/` is too.
- `dusk-vm` `CHANGELOG.md` only moves the 1.6.0 notes under a dated
  heading. Not code.
- The published crates resolve the stable dependency set, not the tag
  workspace's `-rc.0` pins (`bls12_381-bls` 0.6.0, `dusk-jubjub` 0.15.2,
  `dusk-plonk` 0.22.0, `phoenix-core` 0.35.0, `phoenix-circuits` 0.8.0,
  `piecrust` 0.30.0, `piecrust-uplink` 0.20.0). Knot's lock is already
  on that stable side.

## Order

One wave with Atlas and Vigil. Each step waits until the previous
crates are actually on crates.io (path deps become `version` deps).

1. **Atlas** publishes `atlas-encoding` 0.3 first. Knot does not publish
   Atlas. Warden and warden-encoding path-depend on
   `atlas-encoding` 0.3.0 today. A new Atlas *chain* deploy does not
   change this file or any Knot source: a later Atlas bytecode is a new
   `ContractId` and a new warden deployment (`docs/warden.md`).
2. **`nocturne-knot-encoding`** 0.1.2, plus Vigil libraries that do not
   need Atlas. Encoding is the leaf; registry, proposals, and warden
   depend on it.
3. **Contract and data crates**, after encoding is a version dep:
   `nocturne-knot-registry` 0.2.1, `nocturne-knot-registry-data` 0.1.0,
   `nocturne-knot-proposals` 0.4.0, `nocturne-knot-proposals-data` 0.1.0.
4. **After `atlas-encoding` is on crates.io:**
   `nocturne-knot-warden-encoding` 0.2.0, `nocturne-knot-warden` 0.2.0,
   `nocturne-knot-event-decoder` 0.1.0.

Versions above are the public `main` numbers on 2026-10-18. Bump in the
crate `Cargo.toml` if the API moved before the publish commit.

## Vigil

Same wave, same `nocturne-` prefix, same `dusk-core` / `dusk-vm`
`=1.6.0`. Vigil libraries are `publish = false` and still named
`vigil-*` (`vigil-encoding`, `vigil-policy`, `vigil-wire`,
`vigil-session`, `vigil-vault`, `vigil-event-decoder`,
`vigil-policy-contract`). Rename on publish the same way
(`nocturne-vigil-encoding`, `[lib] name = "vigil_encoding"`).
Stubs, test harnesses, `vigil-tool`, and the desktop app stay off
crates.io.

## Stay off crates.io

- Test and harness crates: `knot-mock-atlas`, `knot-warden-test-proxy`,
  `proposals-test-target`, `knot-evm-root-vectors`, `knot-evm-root-e2e`.
- `knot-tool` and `knot-collector` are programs (`cargo install` only,
  if ever). Collector is AGPL. Not this wave.
- Git tags stay the WASM deploy pin. Publishing a library does not
  redeploy a contract.

## Check before publish

`cargo publish --dry-run -p <crate>` from a tree whose `git =` and
`path =` deps are already `version` deps. Dry-run fails closed on
either.
