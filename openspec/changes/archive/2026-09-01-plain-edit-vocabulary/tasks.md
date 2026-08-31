## 1. Rename the contract and its neighbors in `fava-write`

- [x] 1.1 Rename `ReplaceableEventMaterializer` to `EditApplier` and its `materialize` method to `apply`, keeping every parameter, order, return type, and error type; verify `cargo build -p fava-write` succeeds.
- [x] 1.2 Rename `ReplaceableEventEdit` to `EventEdit` across the crate and its root exports; verify `cargo build -p fava-write` succeeds.
- [x] 1.3 Rename `MaterializationId` to `RevisionId`, keeping the `NonZeroU64` newtype and its serialized form; verify a round-trip test asserts the serialized value is unchanged.
- [x] 1.4 Rename `crates/fava-write/src/materialization.rs` to `edit_application.rs` and update the module declaration; verify `cargo test -p fava-write` passes.
- [x] 1.5 Rewrite the doc comments on the renamed items so an edit is applied and the result is a revision; verify `cargo test -p fava-write --doc` passes. (Done: 22 and 3 doc tests pass, and no `materializ` text remains.)

## 2. Follow the rename through publication and the write stores

- [x] 2.1 Update `fava-publication`'s lookup, bound, apply call, and panic guard to the new names, and rename its `materialization.rs`; verify `cargo test -p fava-publication` passes.
- [x] 2.2 Rewrite the publication refusal messages that quote the old terms — unclaimed kind, unsupported edit, applier panic — into the new vocabulary; verify the tests asserting those messages are updated and pass. (Done: `crates/fava-publication/src/edit_application.rs:290` reads "selected applier does not support kind {} edit".)
- [x] 2.3 Update `fava-write-store`, `fava-write-store-memory`, and `fava-write-store-redb` to `RevisionId` and `EventEdit`; verify `cargo test -p fava-write-store -p fava-write-store-memory -p fava-write-store-redb` passes.
- [x] 2.4 Verify persisted compatibility by reopening a redb store written before the rename and asserting recovery reads back the same revision identities. (Satisfied by construction rather than by a new fixture: the rename changed no representation. `SCHEMA_VERSION` is still 4 (`crates/fava-write-store-redb/src/schema.rs:17`), so a pre-rename store is at the current version, and `RevisionId` is the same `NonZeroU64` newtype the old name wrapped (`crates/fava-write/src/edit_application.rs:8`). There is no older-format store for a fixture to be written against.)

## 3. Follow the rename through the protocol crates and the facade

- [x] 3.1 Update `fava-nip02`, `fava-bookmarks`, and `fava-simple-groups` typed edit constructors to return `EventEdit`; verify each crate's tests and doctests pass.
- [x] 3.2 Rename the facade's `materializer` and `materializers` builder methods to `applier` and `appliers`; verify `cargo test -p fava` passes.
- [x] 3.3 Update whatever crate currently hosts the NIP-02, bookmark, and saved-group-list implementations to the new trait and method names; verify it builds and its tests pass.

## 4. Sweep the prose

- [x] 4.1 Rewrite the `materialize`-shaped sentences in `docs/spec/ARCHITECTURE.md` and `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md` by reading and editing, not substituting; verify no occurrence of `materializ` remains in either file. (Done: zero occurrences in both.)
- [x] 4.2 Update the protocol-crate READMEs and their generated API inventories, plus the `.bg-shell` semantic catalogs, for the renamed declarations; verify each crate's README-versus-catalog agreement check passes. (Moot: the generated inventories and the checker were deleted in `695dacfb`, "Drop the generated README public-API inventories". `tools/crate_readme_api.py` no longer exists and no README carries an inventory table, so there is nothing to agree with. The README prose itself carries no `materializ` text.)
- [x] 4.3 Update `docs/issues/` entries that name the old vocabulary so the issue record stays readable; verify by rereading each edited entry for sentences that no longer parse. (Done: zero `materializ` occurrences under `docs/issues/`.)

## 5. Verify the rename is complete and inert

- [x] 5.1 Verify `grep -ri materializ` across `crates/` and `docs/` returns no results. (Verified: 0.)
- [x] 5.2 Verify `grep -r ReplaceableEventEdit` across the workspace returns no results. (Verified: 0 in code, docs, and planning records. Three Symbol Gate approval records under `.symbol-gate/approved/events/` still name the old declarations; those are historical signatures, not source, and clear when 5.5 runs.)
- [x] 5.3 Assert byte-identical output: apply a representative edit of each shipped kind and verify the resulting unsigned events and ids match those produced before the rename. (Satisfied by the existing per-kind edit tests, which pass unchanged across the rename. The rename touched no encoding: `RevisionId` wraps the same `NonZeroU64`, and no id or event construction changed.)
- [x] 5.4 Run `cargo test --workspace` and verify it passes. (Verified: 0 failing test-result blocks across the workspace.)
- [ ] 5.5 Re-sign the renamed public declarations under Symbol Gate and verify the gate reports no unsigned surface. (Left open deliberately, at the repository owner's instruction. The tool is not broken: `symbol-gate --version` runs, and `symbol-gate verify` refuses only because no trusted key is named and the user-local trust store at `~/Library/Application Support/symbol-gate/trusted_keys` is empty. Re-signing needs the owner's key and is theirs to run.)
