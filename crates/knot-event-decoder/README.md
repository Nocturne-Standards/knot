# knot-event-decoder

Host dispatcher for named Knot events.

`decode(contract, topic, version, bytes)` selects an arm by contract identity and topic, then checks the rkyv archive of that named struct. Version `1` is the only accepted generation. The same topic on two contracts is two arms.

Depends on `knot-encoding` and `knot-warden-encoding`. The warden crate needs a sibling atlas 0.3 checkout (`atlas/crates/atlas-encoding` next to this repo).

```bash
cargo test -p knot-event-decoder --manifest-path crates/knot-event-decoder/Cargo.toml
```
