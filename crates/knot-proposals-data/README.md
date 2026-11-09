# nocturne-knot-proposals-data

Package `nocturne-knot-proposals-data`; the Rust name is `knot_proposals_data`. The proposal book of Knot proposals. It is the data half of the split contract: the logic contract (`nocturne-knot-proposals`) emits the events and does the checks, and this book only accepts writes from the logic contract that Atlas resolves.

## The Atlas id is baked in at build time

The book calls Atlas `resolve` on every write, and the Atlas contract id is compiled into the wasm. This package ships **no** id. You set your own when you build:

```bash
ATLAS_PIN_HEX=<64 hex chars, the id of the Atlas contract your deployment uses> \
  cargo build --release --target wasm32-unknown-unknown
```

The Makefile in the repository (`make -C crates/knot-proposals-data wasm`) adds the contract build flags and the wasm optimizer.

An unset or malformed `ATLAS_PIN_HEX` fails the build on purpose, so a book can never be built that trusts the wrong directory. Use your own Atlas deployment's id; the id in someone else's wasm is theirs.

Deploying the book and wiring it to its logic contract is covered in the repository: <https://github.com/Nocturne-Standards/knot>. Testnet only, not audited yet.

License: Apache-2.0.
