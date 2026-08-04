# Design: nocturne-deployments pin reader (scoped)

Date: 2026-08-04  
Status: approved for scoped cut (“our case now”); full migrate deferred to track leaves

## Goal

Knot (and wen tools) can resolve live contract IDs from a shared pin file without
opening `sme_platform` as the only home. Redeploy of `multisig-registry` /
`multisig-proposals` records into that pin home.

## Non-goals (later leaves)

- Move `deploy-contract.sh` / `lib.sh` out of sme_platform
- Migrate every product consumer / nocturne-mcp-gates JS loader
- History GC or pin schema redesign

## Shape

1. **Pin home repo** — use / revive `aichbindas/nocturne-deployments` (or sibling path
   `~/dev/aichbindas/nocturne-deployments`). Seed with `testnet.json` keys needed for
   knot: at least `multisig-registry`, `multisig-proposals` (copy current
   entries from `sme_platform/deployments/testnet.json`). Keep machine-local
   `wasm_path` as today (ops laptop paths OK; consumers need `contract_id`).

2. **Rust crate `nocturne-deployments`** (new small repo or under
   `deployments/crates/nocturne-deployments`):
   - `load(path) -> DeploymentsFile`
   - `current(key) -> &DeploymentCurrent` (`contract_id`, `version`, …)
   - Path resolve: `NOCTURNE_DEPLOYMENTS` env → else walk-up
     `deployments/testnet.json` → else optional default sibling.

3. **Wire knot `multisig-tool`** to use the crate instead of local walk-only
   assuming monorepo root under sme_platform.

4. **Redeploy knot contracts** via existing
   `sme_platform/scripts/deploy-contract.sh` pointed at knot crate dirs, with
   `DEPLOYMENTS_FILE` / record path aimed at the shared pin home if script
   supports it — else record into shared file manually / small script flag.

## Success

- `multisig-tool` reads registry/proposals IDs from shared pins
- Domain-bump redeploy can update those pins without editing sme_platform tree
  for the JSON (scripts may still run from sme_platform)

## Follow-up leaves (consume later)

**Superseded by cut A design:**  
`docs/superpowers/specs/2026-08-04-nocturne-deployments-extract-design.md`  
in `aichbindas/nocturne-deployments` (scripts + wrappers + gates path; lazy pin migrate).
