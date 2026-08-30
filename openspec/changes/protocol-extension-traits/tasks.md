## 1. Add the sink in `fava-write`

- [ ] 1.1 Add `EditApplierSink` with one method accepting an `Arc<dyn EditApplier>` and returning `Self`, exported from the crate root; verify `cargo build -p fava-write` succeeds.
- [ ] 1.2 Implement `EditApplierSink` for `FavaBuilder` in terms of the existing `applier` method; verify a test enables a hand-written applier through the sink and publishes an edit of its kind.

## 2. Give each protocol crate an enabling call

- [ ] 2.1 Add `fava_simple_groups::SimpleGroups` with `with_simple_groups()`, blanket-implemented for `T: EditApplierSink`, and make `SavedGroupListApplier` and `saved_group_list_applier` private; verify `cargo test -p fava-simple-groups` passes.
- [ ] 2.2 Add the equivalent to `fava-nip02` and make its `applier` factory private; verify `cargo test -p fava-nip02` passes.
- [ ] 2.3 Add the equivalent to `fava-bookmarks` and make its `applier` factory private; verify `cargo test -p fava-bookmarks` passes.
- [ ] 2.4 Verify each protocol crate's dependency set is unchanged — still exactly `fava-query`, `fava-state`, `fava-write`, `nostr` — by running each crate's architecture test.
- [ ] 2.5 Update each crate's export allow-list, README inventory, and `.bg-shell` catalog entry to drop the factory and add the trait; verify the README-versus-catalog agreement check passes.

## 3. Point the two doors at their callers

- [ ] 3.1 Rewrite the doc comments on `FavaBuilder::applier` / `appliers` to say they are for application-defined kinds, and that protocol crates are enabled through their own `with_*` call; verify `cargo test -p fava --doc` passes.
- [ ] 3.2 Update every call site in `crates/fava/tests`, `apps/canary`, `examples/simple-groups`, and `falsifiers/` from the factory form to the enabling call; verify each of those workspaces builds — note `apps`, `examples`, and `falsifiers` declare their own `[workspace]` and are NOT covered by `cargo test --workspace`.

## 4. Verify enablement, absence, and coexistence

- [ ] 4.1 Verify enabling several protocols in either order produces the same facade.
- [ ] 4.2 Verify an application-defined applier and an enabled protocol coexist in one index, and that a collision between them is refused with neither taking precedence.
- [ ] 4.3 Verify a forgotten enabling call fails at assembly when a stored write of that kind is outstanding, and at first publish otherwise, naming the unclaimed kind.
- [ ] 4.4 Verify no protocol crate's public surface exposes an edit applier or a factory returning one, by inspecting each crate's generated API inventory.

## 5. Close out

- [ ] 5.1 Record `docs/issues/0051-built-in-protocol-materializers.md` as resolved by this change, including the spike result — all 16 cells failed, cause is archive extraction not dead-code elimination — so nobody retries the link-time approach.
- [ ] 5.2 Run `cargo test --workspace --no-fail-fast` writing full output to a file, and verify it matches the current baseline with no new failures.
- [ ] 5.3 Build and test `apps/canary`, `examples/simple-groups`, and `falsifiers/external-semantic-capability` separately, and verify each matches its own baseline.
- [ ] 5.4 Re-sign the changed public declarations under Symbol Gate and verify the gate reports no unsigned surface.
