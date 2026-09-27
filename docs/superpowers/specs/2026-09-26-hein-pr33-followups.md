# Hein review on PR 33

Follow-ups to [pull request 33](https://github.com/Nocturne-Standards/knot/pull/33), from Hein's review on 2026-09-25. The four original fixes stay. This spec covers the five notes on that review.

Testnet cutover has not been run. These contracts are not live, so layout changes in this spec do not migrate state.

## Decisions

1. `init_data` is one-shot on registry logic and on proposals logic. The owner cannot point an already-bound logic contract at a different book.
2. A replacement proposals logic contract may attach to an existing book, but it cannot finalize or execute proposals whose digest was signed for the previous logic contract. Open and queued rows stay in the book until cancel or prune. They do not run.
3. Changing the authorized registry account invalidates in-flight proposals. It does not pause them. Switching the account from A to B and back to A does not make A's old proposals executable again. No full-map scan.
4. The `prune(0)` test and the bounded-prune test change so the old bugs fail them.

No new migration function in this change. A later book handover, if one is added, needs its own owner call and must keep rules 2 and 3.

## `init_data` is one-shot

Today `init_data` on `knot-registry` and `knot-proposals` always assigns `data`. A second call can point the same logic contract at another book. Account ids and proposal ids would then refer to different rows, and Atlas would still resolve the old logic id.

New behavior, both contracts:

- `data` is `None`: store the argument.
- `data` is already that same id: return. A retried deploy must not panic.
- `data` is some other id: panic.

The book does not record the logic id. A newly deployed logic contract has its own empty slot, so its first `init_data` may still name an existing book. That path is the logic upgrade below, and it is allowed.

## Replacement logic cannot run old proposals

`proposal_digest_v3` includes the logic contract id (`abi::self_id()`). Finalize currently verifies signatures over the stored digest and does not recompute it. Execute does not look at the digest at all. After Atlas points `knot-proposals` at a new logic contract that calls `init_data` on the same book, that new contract can finalize an open proposal, or execute a queued one, and `call_raw` runs as the new contract. The committee signed the old id.

Do not add a digest version. Do not walk the proposal map when the service changes. Atlas has no callback on `set_service`.

On finalize and on execute, before any book write and before `call_raw`, recompute `proposal_digest_v3` with this contract's `abi::self_id()` and the proposal's stored fields (epoch, account, nonce, target, function name, call args, deadline). Panic if the result differs from `signed_digest`.

The book checks the same thing in `queue` and `commit_executed`, using `abi::caller()` as the logic id. A logic bug that skips the check must not commit or run the call.

Cancel stays. The cancel message is already bound to the current logic id, so the committee signs a new cancel for the new contract if they want the row cleared. Prune still removes expired rows under the existing bounds.

Cover both shapes in contract tests, with a second proposals logic contract deployed from the same wasm:

- Open proposal, approvals collected, Atlas moved to the second logic, second logic `init_data` on the same book. `finalize` on the second logic fails. The target value is unchanged.
- Proposal queued by the first logic (timelock greater than 0), then the same handover. `execute` on the second logic fails. The target value is unchanged. Status stays `Queued`.

## Rebind invalidates

`set_authorized_account` today only stores the account id. Finalize and execute compare the proposal's account to that id. After A → B → A, an unexpired proposal from the first binding, including one already queued, will run again.

That is a pause. This spec makes it an invalidation.

Add `auth_generation: u64` to the book, starting at 0. Every `set_authorized_account` increments it, including a call that sets the same account id again. `open_proposal` stamps the current generation onto the proposal. Finalize, execute, `queue`, and `commit_executed` panic unless the proposal's generation equals the book's current generation.

No scan of `proposals` or `by_digest`. Rows stay until cancel or prune. A new proposal opened after the rebind carries the new generation and can run.

`set_registry` already bumps `epoch` and clears `authorized_account`. Epoch mismatch still rejects old proposals. Do not use `auth_generation` for that.

The generation is not part of the signed digest. Old signatures stay mathematically valid. The contract refuses them.

Expose the counters on the shared types in `knot-encoding` only:

- `ProposalsConfig.auth_generation`
- `ProposalView.auth_generation`

`OpenProposal` does not carry the generation. The book stamps it. No hand-copied mirror. `ProposalView` and `ProposalsConfig` are not in the layout goldens today. If a golden starts failing, update that golden in the same change. Do not bump the proposal digest domain.

Document this in the proposals README, next to `set_authorized_account`: rebind invalidates open and queued proposals for good, including the round trip back to the previous account.

Tests:

- Open proposal, rebind to another account, rebind back to the first account. `finalize` fails. A new proposal for the restored account can still finalize.
- Same round trip for a queued proposal (timelock greater than 0). `execute` fails. Status stays `Queued`. A new proposal after the round trip can be queued and executed.

## Prune tests

Behavior of `prune` stays as it is. The tests are what Hein said do not catch the old bugs.

### `prune(0)`

`prune_zero_removes_nothing` finalizes a proposal and calls `prune(0)` while the digest deadline is still in the future. The old bug deleted expired digests even when the limit was 0. Proposing again does not prove the row survived: a past deadline rejects the new propose on its own.

Change the test:

- Advance the block height past the proposal deadline and past the digest deadline.
- Call `prune(0)`.
- Read the proposal and the digest back. Both are still present. The digest is still consumed.

### Bounded prefix

`prune_examines_a_bounded_prefix` inserts the expired proposal first. It is the first key in the proposal map, so `prune(1)` removes it. A scan that walks until it has removed `limit` rows does the same thing.

Change the proposal half:

- Insert the live proposal first (higher deadline) and the expired proposal second.
- Advance the height so only the second is expired.
- `prune(1)` examines the live row, removes nothing, and leaves the expired proposal in place.
- A second `prune(1)` removes the expired proposal.

The proposal map is ordered by id, so insertion order is scan order.

### Digest map

Digest keys are the signed digest bytes, not proposal ids. The test must sort the two digest keys and assign deadlines so the smaller key is still live and the larger key is expired.

- `prune(1)` examines the smaller key only. The expired digest is still readable.
- A later `prune(1)` may remove the expired digest.

### Cursor wrap

After the last key, the next call starts again at the first key.

- Two live proposals. `prune(1)` then `prune(1)` moves the cursor past the second id.
- Expire the first proposal only.
- The next `prune(1)` wraps and removes that first proposal. A cursor that stuck past the end would remove nothing.

One digest-map wrap as well: two live digests, two `prune(1)` calls to pass the last key, expire the first key in byte order, and the next `prune(1)` removes it.

## Files

- `crates/knot-registry/src/state.rs` — one-shot `init_data`
- `crates/knot-proposals/src/state.rs` — one-shot `init_data`; recompute digest on finalize and execute; compare `auth_generation`
- `crates/knot-proposals-data/src/state.rs` — generation stamp and checks; digest recompute in `queue` and `commit_executed`
- `crates/knot-encoding/src/call_types.rs` — `auth_generation` on `ProposalsConfig` and `ProposalView`
- `crates/knot-proposals/tests/contract.rs` — rebind round trip, second logic contract, prune tests
- `crates/knot-registry/tests/contract.rs` — second `init_data` panics, same id is idempotent
- `crates/knot-proposals/README.md` — rebind invalidates; replacement logic does not run old proposals
- Changelogs under Unreleased, still at registry 0.2.0 and proposals 0.4.0

## Non-goals

- No `adopt_book` or other migration call.
- No change to prune limits, cursors, or what a consumed digest waits for.
- No testnet deploy.
- No new comment on the pull request until these tests exist.
