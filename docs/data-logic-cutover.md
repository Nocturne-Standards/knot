# Data/logic cutover

Not landed. `scripts/deploy-knot-cutover.sh` deploys the books and the logic contracts, schedules Atlas `knot-registry` and `knot-proposals` through warden, then `init_data` and `init_registry`. It does not call `set_authorized_account` and it does not change warden's scheduler.

## Still open

- Run the script. Public pins become the logic ids. Books are separate keys.
- Prior monoliths: registry `db2c8229285f525cb717a4c8feeb4b0e270102735a164ca93f40452f2095f00f`, proposals `01df43581335297c32312738300c1371aeecc49dee1e3cbcca6924d635529537`. Their rows stay there.
- `set_authorized_account` is required before `propose`. The account is the governance committee, chosen after `create_account`.
- Warden on testnet is still the operator key (`init_warden` scheduler `External`). Leave it until that account is set, then `set_scheduler` to the proposals logic id if this executor should be the Atlas guardian's caller.
- Do not fund either data id. Neither contract custodies DUSK.

## Already true in code

- `knot-registry` is the API. Service `knot-registry`. The book is `knot-registry-data`.
- `knot-proposals` is the API and the `call_raw` caller. Service `knot-proposals`. The book is `knot-proposals-data`.
- `knot-mock-atlas` is tests only.
- `knot-warden` stays one contract.
- Events still emit from logic.
