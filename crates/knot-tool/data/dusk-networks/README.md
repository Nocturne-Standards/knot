# Vendored Dusk network data

`testnet.json` is `@dusk-network/networks` 0.3.0 (upstream commit
`0bebc7799b99bba1f32e9dc6c24b4e4994bc864a`), copied unmodified. It is MPL-2.0
(see `LICENSE`); the rest of `knot-tool` is Apache-2.0. `src/network.rs` embeds
it at build time. To update, replace the file with a newer upstream release and
run `cargo test -p knot-tool --lib`.
