# Changelog

## [0.1.0] - unreleased

### Added

- `decode(contract, topic, version, bytes)` for the named events in `knot-encoding` and `knot-warden-encoding`.

### Changed

- Package renamed to `nocturne-knot-event-decoder` (the `nocturne-knot-*` crates.io pattern). `[lib] name = "knot_event_decoder"` keeps the Rust name. Dependents keep the `knot-event-decoder` key and add `package = "nocturne-knot-event-decoder"`.
