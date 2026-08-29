## 1. Choose and prove the link-time mechanism

- [ ] 1.1 Evaluate `inventory` and `linkme` against the targets this workspace builds for, and record the choice with its target-support evidence in `design.md`; verify by building a throwaway two-crate spike on each target.
- [ ] 1.2 Prove the linker-elimination case: build a spike whose dependency crate registers a submission and is otherwise never referenced, and verify the submission is still collected — or record exactly which build configurations drop it.
- [ ] 1.3 Add the chosen dependency to `fava-write` and verify `cargo build -p fava-write` succeeds on each supported target.

## 2. Build the capability surface in `fava-write`

- [ ] 2.1 Add an opaque `Capability` wrapping one `Arc<dyn EditApplier>`, with crate-visible read access and no public accessor; verify a test confirms a holder can obtain neither the kind nor the applier.
- [ ] 2.2 Add `Capability::from_applier` for application-defined semantics; verify a test wraps a custom applier and reads its claimed kind through the crate-visible path.
- [ ] 2.3 Add the link-time registry and the `register_capability!` macro; verify a test crate in the workspace declares a capability and the registry enumerates it.
- [ ] 2.4 Verify a capability carries no mutable state by indexing the same one from two independent assemblies in one test process and asserting neither observes the other.

## 3. Return the appliers to the protocol crates

- [ ] 3.1 Move the kind-10009 saved-group-list applier and its edit decoding into `fava-simple-groups`, and declare its capability; verify `cargo test -p fava-simple-groups` passes with the decode coverage restored, not just one applier test.
- [ ] 3.2 Move the kind-3 contact-list applier into `fava-nip02` and declare its capability; verify `cargo test -p fava-nip02` passes.
- [ ] 3.3 Move the kind-10003 bookmark applier into `fava-bookmarks` and declare its capability; verify `cargo test -p fava-bookmarks` passes.
- [ ] 3.4 Update each crate's architecture test — dependency set, export allow-list, forbidden-symbol scan — so a declared capability is permitted and a public applier factory is not; verify each crate's architecture test passes.
- [ ] 3.5 Verify each protocol crate's public surface still exposes no kind number, by inspecting its generated API inventory.

## 4. Make the facade kind-blind

- [ ] 4.1 Delete `crates/fava-builtin-codecs/` and `fava::builder::shipped_materializers`; verify `cargo build -p fava` fails only where those were referenced.
- [ ] 4.2 Remove `fava-bookmarks`, `fava-builtin-codecs`, `fava-nip02`, and `fava-simple-groups` from `crates/fava/Cargo.toml` and its `BUILD.bazel`; verify `cargo build -p fava` succeeds and `grep` finds no kind number in the facade.
- [ ] 4.3 Replace `edit_applier` / `edit_appliers` with one `capability` door taking a `Capability`; verify a test registers an application-defined capability and publishes an edit of its kind.
- [ ] 4.4 Read the link-time registry at assembly and index it together with explicitly named capabilities before recovery runs; verify a test asserts arrival order does not change the resulting index.
- [ ] 4.5 Update the claim-bound accounting and its documentation so all 64 slots are available; verify a test registers the full bound and that one more is refused with actual-against-bound.

## 5. Verify claims, conflicts, and absences

- [ ] 5.1 Verify a program that links a protocol crate and assembles the facade naming nothing can publish and recover that crate's edits.
- [ ] 5.2 Verify two capabilities claiming one kind refuse assembly, for a declared-versus-declared pair and for a named-versus-declared pair, with neither taking precedence.
- [ ] 5.3 Verify assembly refusal happens before any write store, publication owner, or recovery has run.
- [ ] 5.4 Verify publishing an edit of an unclaimed kind is refused as unclaimed with no write accepted, and that recovering an outstanding write of a now-unclaimed kind refuses rather than dropping or completing it.
- [ ] 5.5 Document the duplicate-claim refusal's most likely causes — two crate versions in the graph, a crate linked twice — in the refusal's own documentation; verify the text names both.

## 6. Close out the change

- [ ] 6.1 Record `docs/issues/0051-built-in-protocol-materializers.md` as superseded, stating which claim it got right and why the conclusion did not follow; verify the entry reads coherently against the new design rather than being rewritten in place.
- [ ] 6.2 Update `docs/spec/ARCHITECTURE.md` and `FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md` where they describe the facade composing shipped implementations; verify no passage still claims the facade ships appliers.
- [ ] 6.3 Assert byte-identical output: apply a representative edit of each of the three kinds and verify the resulting unsigned events and ids match those produced before the move.
- [ ] 6.4 Run `cargo test --workspace` and verify it passes.
- [ ] 6.5 Re-sign the changed public declarations under Symbol Gate and verify the gate reports no unsigned surface.
