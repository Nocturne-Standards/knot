# knot-event-decoder

Package `nocturne-knot-event-decoder`, Rust library `knot_event_decoder`.

A host-side dispatcher that decodes named Knot contract events into JSON. It covers the Knot registry, Knot proposals and warden contracts, using the event structs from `nocturne-knot-encoding` and `nocturne-knot-warden-encoding`.

Status: testnet only, not audited yet.

```toml
[dependencies]
knot-event-decoder = { package = "nocturne-knot-event-decoder", version = "0.1" }
```

```rust
use knot_event_decoder::{decode, CONTRACT_WARDEN};

let json = decode(CONTRACT_WARDEN, "delay_set", 1, &payload_bytes)?;
```

`decode(contract, topic, version, bytes)` picks the event by contract name and topic, then validates the rkyv archive of that struct and returns `serde_json::Value`. The same topic on two contracts is two separate events. Only version `1` (`NAMED_EVENT_VERSION`) is accepted. Errors are `DecodeError::{UnsupportedVersion, UnknownEvent, InvalidPayload}`. Contract names are the constants `CONTRACT_REGISTRY`, `CONTRACT_PROPOSALS` and `CONTRACT_WARDEN`.

## Related crates

Part of the [Knot](https://github.com/Nocturne-Standards/knot) repository. The warden encoding crate depends on `nocturne-atlas-encoding` 0.5 from crates.io.

## License

Apache-2.0.
