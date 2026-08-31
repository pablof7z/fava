## 1. Add the sink in `fava-write`

- [x] 1.1 Add `EditApplierSink` with one method accepting an `Arc<dyn EditApplier>` and returning `Self`, exported from the crate root; verify `cargo build -p fava-write` succeeds.
- [x] 1.2 Implement `EditApplierSink` for `FavaBuilder` in terms of the existing `applier` method; verify a test enables a hand-written applier through the sink and publishes an edit of its kind.

## 2. Give each protocol crate an enabling call

- [x] 2.1 Add `fava_simple_groups::SimpleGroups` with `with_simple_groups()`, blanket-implemented for `T: EditApplierSink`, and make `SavedGroupListApplier` and `saved_group_list_applier` private; verify `cargo test -p fava-simple-groups` passes.
- [x] 2.2 Add the equivalent to `fava-nip02` and make its `applier` factory private; verify `cargo test -p fava-nip02` passes.
- [x] 2.3 Add the equivalent to `fava-bookmarks` and make its `applier` factory private; verify `cargo test -p fava-bookmarks` passes.
- [x] 2.4 Verify each protocol crate's dependency set is unchanged: `fava-simple-groups` stays exactly `fava-query`, `fava-state`, `fava-write`, `nostr` and `fava-nip02` stays those plus `fava-relay`, each checked by running its own architecture test; `fava-bookmarks` stays `fava-state`, `fava-write`, `nostr` (no `fava-query`) — add `crates/fava-bookmarks/tests/architecture.rs` asserting that set, mirroring `fava-nip02`'s test, and run it too.
- [x] 2.5 Update each crate's export allow-list, README inventory, and `.bg-shell` catalog entry to drop the factory and add the trait; verify the README-versus-catalog agreement check passes.

## 3. Point the two doors at their callers

- [x] 3.1 Rewrite the doc comments on `FavaBuilder::applier` / `appliers` to say they are for application-defined kinds, and that protocol crates are enabled through their own `with_*` call; verify `cargo test -p fava --doc` passes.
- [x] 3.2 Update every call site in `crates/fava/tests`, the downstream acceptance application, `examples/simple-groups`, and `falsifiers/` from the factory form to the enabling call; verify each of those workspaces builds — note the acceptance application, `examples`, and `falsifiers` declare their own `[workspace]` and are NOT covered by `cargo test --workspace`.

## 4. Verify enablement, absence, and coexistence

- [x] 4.1 Verify enabling several protocols in either order produces the same facade.
- [x] 4.2 Verify an application-defined applier and an enabled protocol coexist in one index, and that a collision between them is refused with neither taking precedence.
- [x] 4.3 Verify a forgotten enabling call fails at assembly when a stored write of that kind is outstanding, and at first publish otherwise, naming the unclaimed kind.
- [x] 4.4 Verify no protocol crate's public surface exposes an edit applier or a factory returning one, by inspecting each crate's generated API inventory.

## 5. Close out

- [x] 5.1 Record `docs/issues/0051-built-in-protocol-appliers.md` as resolved by this change, including the spike result — all 16 cells failed, cause is archive extraction not dead-code elimination — so nobody retries the link-time approach.
- [x] 5.2 Run `cargo test --workspace --no-fail-fast` writing full output to a file, and verify it matches the current baseline with no new failures.
- [x] 5.3 Build and test the downstream acceptance application, `examples/simple-groups`, and `falsifiers/external-semantic-capability` separately, and verify each matches its own baseline.
- [ ] 5.4 Re-sign the changed public declarations under Symbol Gate and verify the gate reports no unsigned surface.
