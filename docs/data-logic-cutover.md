# Data/logic cutover

Landed on Dusk testnet 2026-09-27 against Atlas `21c653385e3b7cc8be32edd5e6df2e30b6464be867f39442171ba8ca8d089612`. Warden `113159d33377e7778d552cc48f40318c44643ca06970e2482b41b276a4d9cdb3` applied both service names. `init_data` and `init_registry` are set. `set_authorized_account` was not called. Warden's scheduler is still the operator key.

The ids below are the 2026-09-27 cutover and are history now. Current ids are on [Deployments](https://docs.nocturne-standards.org/v1/deployments/).

| contract | id |
|---|---|
| `knot-registry` | `c8991fa3d54b0a16c1789a0821db4096f9d501ba83dd9de4bd620c3377cee02d` |
| `knot-registry-data` | `67528ea5f357af9683409234b5445234ebe99b2e0b6f5d2760ae0565aacb0939` |
| `knot-proposals` | `45ca19e8795584a01d357541db18eac39f2d7684ff208ce241d3fd963acd1f0a` |
| `knot-proposals-data` | `60a451b6dd7a5de3a50d89c0b596e5213fd05e3d672370c94b171fe4778d78a6` |

The proposals logic above was replaced on 2026-10-21 by `3539ba8dc769929c4deef920af74e00ddce98aa67d419c3fc4bb927d7c6fdeb5` (warden `schedule_service` then `execute_service`, tx `1ab6336de627ee7f96831769025c684851fa34114ed95936c896c34b2167b628`). Current ids are in [Deployments](https://docs.nocturne-standards.org/v1/deployments/).

## Still open

- Prior monoliths: registry `db2c8229285f525cb717a4c8feeb4b0e270102735a164ca93f40452f2095f00f` (`next_account_id` 9), proposals `01df43581335297c32312738300c1371aeecc49dee1e3cbcca6924d635529537`. Their rows stay there. The new registry book starts empty.
- `set_authorized_account` is required before `propose`. The account is the governance committee, chosen after `create_account` on the new registry.
- A consumer contract whose `init_registry` is one-shot still stores the old registry id. It needs a new deploy to point at the new logic id.
- Warden on testnet is still the operator key (`init_warden` scheduler `External`). Leave it until that account is set, then `set_scheduler` to the proposals logic id if this executor should be the Atlas guardian's caller.
- Do not fund either data id. Neither contract custodies DUSK.
- Book crates have no JSON schema. `make wasm-dd` builds the contract feature into the data-driver target directory so the deploy record has a path.

## Already true in code

- `knot-registry` is the API. Service `knot-registry`. The book is `knot-registry-data`.
- `knot-proposals` is the API and the `call_raw` caller. Service `knot-proposals`. The book is `knot-proposals-data`.
- `knot-mock-atlas` is tests only.
- `knot-warden` stays one contract.
- Events still emit from logic.
