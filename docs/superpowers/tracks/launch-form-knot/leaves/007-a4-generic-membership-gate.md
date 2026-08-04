---
id: 7
slug: a4-generic-membership-gate
status: DONE
owner: worker-a4
deps: []
scope:
  - crates/multisig-tool/src/rpc.rs
  - crates/multisig-tool/src/main.rs
acceptance:
  - approve/quorum/change_account sign paths fetch live account before sign
  - Document Prove-mode on-chain mitigation remains
acceptanceDone:
  - true
  - true
---
# A4 — generic Lab membership/threshold pre-check (from audit #13)

Successor of knot audit #13.

**Supersedes:** `audit-2026-08-full` leaf `#13`.

## Evidence (worker)

- **Gate:** `multisig_tool::membership::ensure_pks_are_members`; RPC
  `fetch_registry_account` + `ensure_signers_are_members` in `rpc.rs`; CLI
  `ensure_cli_signers_are_members` in `main.rs`. Wired before sign on
  `api_proposal_approve`, `api_quorum_submit` / `_check` / `_agg_*`,
  `api_change_account_submit`, and CLI twins.
- **Tests:** `membership::tests::*`, `rpc::generic_rpc_smoke::approve_rejects_non_member`,
  `serve_mock_generic_proposal_flow_smoke` + `approve_rejects_non_member_identity` integration.
- **Docs:** `docs/security-model.md` Lab section; `DECISIONS.md` A4 implemented.
