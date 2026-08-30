## 1. Split the builder in `fava-write`

- [x] 1.1 Remove the `author` field and parameter from `EventBuilder::new`, leaving `new(kind: Kind)`; verify the crate's own module doctest is rewritten to the authorless form and `cargo test -p fava-write --doc` passes.
- [x] 1.2 Add `AuthoredEventBuilder` carrying the same body fields plus `author`, and move `build`, `into_event_and_routing`, and the private `build_event` onto it; verify `cargo build -p fava-write` succeeds with `EventBuilder` exposing no finalization method.
- [x] 1.3 Add `EventBuilder::by(author) -> AuthoredEventBuilder` transferring kind, timestamp, content, tag order, and routing unchanged; verify a test asserts an authored builder's finalized event equals one built from the same fields via `from_parts`.
- [x] 1.4 Duplicate the body-shaping methods (`created_at`, `content`, `tags`, `tag`, `event_tags`, `to_relays`) onto `AuthoredEventBuilder`; verify a test shapes a builder both before and after `by(..)` and asserts identical output.
- [x] 1.5 Change `from_parts` and `impl From<UnsignedEvent>` to return `AuthoredEventBuilder`; verify a test reopens an unsigned event, finalizes it unmodified, and asserts the re-derived id equals the original.
- [x] 1.6 Export `AuthoredEventBuilder` from `fava-write`'s root and confirm `EventBuildError` gains no missing-author variant; verify `cargo test -p fava-write` passes.

## 2. Widen the author scope in `fava`

- [x] 2.1 Factor the explicit-vs-automatic route-merge in `PublishPayload for EventBuilder` into one function usable by both builder impls; verify `cargo build -p fava` succeeds and the conflicting-explicit-routes refusal still fires.
- [x] 2.2 Change `impl PublishPayload for EventBuilder` (authorless) to read its `author` argument and return `PublishError::MissingAuthor` when `None`; verify a test publishes an authorless builder with no author scope and asserts the refusal with no write or receipt id produced.
- [x] 2.3 Add `impl PublishPayload for AuthoredEventBuilder` retaining the current behavior of ignoring the facade author; verify a test publishes an authored builder through `Fava::publish` and asserts the event carries the builder's author.
- [x] 2.4 Introduce the authorless-payload bound and change `PublishAs::publish` to accept it, implemented for `EventBuilder` and `EventEdit` only; verify `fava.by(author).publish(authorless_builder)` compiles and publishes under that author.
- [x] 2.5 Extend the `PublishAs` doc guards so `AuthoredEventBuilder` joins `UnsignedEvent` and `Event` as excluded from an author scope; verify `cargo test -p fava --doc` passes.
- [x] 2.6 Verify author scope and relay scope compose in either order for an authorless builder, and that a builder carrying its own explicit route still conflicts with a narrowed expression.
- [x] 2.7 Re-export `AuthoredEventBuilder` from `fava`'s root alongside `EventBuilder`; verify `cargo build -p fava` succeeds.

## 3. Drop the author from the protocol construction surface

- [x] 3.1 Remove `author` from the private `management::build` helper and route it through `EventBuilder::new(kind)`; verify `cargo build -p fava-simple-groups` reports the nine callers as the only breaks.
- [x] 3.2 Remove `author` from all nine public management constructors (`create_group`, `edit_metadata`, `invite`, `join_request`, `put_user`, `remove_user`, `delete_event`, `delete_group`, `leave_group`); verify `cargo test -p fava-simple-groups --lib` passes with the management unit tests updated to construct then `.by(author)`.
- [x] 3.3 Rewrite the nine constructors' doc examples and the `management` module's publish-path prose to the `fava.by(author).publish(builder)` form; verify `cargo test -p fava-simple-groups --doc` passes.
- [ ] 3.4 Update `fava-simple-groups`'s README inventory and the `.bg-shell/simple-groups-semantic-catalog.jsonl` entries for the nine changed signatures; verify the README-versus-catalog agreement check passes. (`python3 tools/crate_readme_api.py check fava-simple-groups` still reports stale; the generator tool has a known bug — see `tools/crate_readme_api.py`'s `public_lines()` — that silently drops the management-constructor table when run, so the README's nine rows were hand-updated in task group 3 rather than regenerated. Left unchecked until the tool is fixed and a clean regeneration passes.)

## 4. Move the reconstruct and sign-now call sites onto the authored path

- [x] 4.1 Change `fava-publisher-nip01`'s kind-22242 auth-response construction to `EventBuilder::new(kind)…by(pubkey).build()`; verify `cargo test -p fava-publisher-nip01` passes, including the NIP-42 test.
- [x] 4.2 Confirm `fava-nip02::contact_list::validate_unsigned_bound` and the NIP-02, bookmark, and saved-group-list edit appliers still compile unchanged on `from_parts` and `From<UnsignedEvent>`; verify by building those crates and recording any that needed an edit.
- [x] 4.3 Sweep remaining `EventBuilder::new` call sites across the workspace, including test support, and move each to the authorless or authored form according to whether it states a real identity; verify `cargo build --workspace --all-targets` succeeds.

## 5. Verify the change end to end

- [x] 5.1 Assert byte-identical output: construct a representative event through the new path and the old field values via `from_parts`, and verify the serialized bytes and event id match.
- [x] 5.2 Run the full workspace suite and verify `cargo test --workspace` passes.
- [ ] 5.3 Run the architecture and public-API checks for `fava-write`, `fava`, `fava-simple-groups`, and `fava-nip02`, and verify each passes with its updated surface. (`fava-write` and `fava` pass as of this pass; `fava-simple-groups` and `fava-nip02` still report stale README inventories — see 3.4 — so this task stays open until both are clean.)
- [ ] 5.4 Re-sign the changed public declarations under Symbol Gate and verify the gate reports no unsigned surface. (Not run in this pass: the `symbol-gate` binary present locally does not execute in this environment (killed on invocation), and re-signing requires the repo owner's signing key. Left open for a signer to run `symbol-gate verify` / re-sign.)
