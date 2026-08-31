## Context

See `proposal.md` — Why. This is a rename with no design content; what follows records the name choices and the mechanical shape of the change.

The affected vocabulary is concentrated in two files named `materialization.rs` — one in `fava-write` defining `ReplaceableEventMaterializer`, `MaterializationId`, and the edit type's neighbors, one in `fava-publication` holding the kind lookup, the bound, the apply call, and the panic guard — and then spreads through `fava-write-store*`, the three protocol crates, the facade builder, and the prose in READMEs, `docs/spec/ARCHITECTURE.md`, and the semantic catalogs.

## Goals / Non-Goals

**Goals:**

- Names a reader can predict without being taught them.
- One substitution per term, applied everywhere including prose, so the old vocabulary leaves no residue.
- Zero behavior change, provable by the events being byte-identical.

**Non-Goals:**

- Changing any signature's parameters, order, return type, or error type.
- Changing the trait's shape or the kind-lookup mechanism. `capability-self-registration` does that, on top of this vocabulary.
- Changing persisted data. `RevisionId` stays a `NonZeroU64` newtype serialized by value.

## Decisions

### `EditApplier`, not `EventEditor` or `EventUpdater`

`EditApplier` pairs with the `EventEdit` it consumes: an edit is applied by an applier, and `apply` says exactly what the call does. `apply` is also the verb the protocol crates already used for their own versions of this function before the contract absorbed them, so it is a return to the repo's own word rather than a new coinage.

*Alternative — `EventEditor`:* shortest, and reads as a role rather than a machine. Rejected because it sits one letter from `EventEdit`, blurring which is the value and which is the actor, and because "editor" invites a text-editor reading.

*Alternative — `EventUpdater` / `update`:* the most ordinary word available, but `update` loses the tie to the edit value being consumed and suggests mutating something in place rather than producing a new event.

### `EventEdit`, dropping the `Replaceable` qualifier

Only replaceable and addressable kinds can be edited; a regular event is superseded, not edited. The qualifier is therefore always true and never distinguishing, and it is paid on 228 mentions. The subsystem's own boundary already carries the constraint — `EditApplier::kind` returns a replaceable or addressable kind, and publication refuses anything else.

### `RevisionId`, not `ApplyId` or `GenerationId`

The type's own doc calls it "one immutable event materialization generation": a counter that advances each time a write's edit is re-applied against a changed current event. "Revision" is the ordinary word for that. `GenerationId` collides with the signer generation already tracked alongside it in `fava-publication`; `ApplyId` names the action instead of the thing identified.

### `edit_applier` / `edit_appliers` on the builder

The `application_` prefix encoded a distinction — application-defined versus Fava's shipped ones — that reads at every call site as "the application object". Naming the methods for what is registered drops the ambiguity. The distinction the prefix carried is documented in the method's doc comment where it belongs, and `capability-self-registration` removes the distinction entirely.

### One commit, mechanical, verified by identical output

The rename is done as a single sweep rather than crate by crate, because the workspace does not compile between the halves of a cross-crate rename. Prose is included in the same pass: leaving `materialize` in doc comments while the code says `apply` is worse than either name alone.

## Risks / Trade-offs

- **A 1,518-site diff is unreviewable line by line.** → It is reviewable as a substitution: the check is that every occurrence of each old term is gone and that the events are byte-identical, not that each line is individually correct. A grep for `materializ` returning empty is the actual review artifact.

- **Prose substitution is not purely mechanical.** → Sentences built around "materialize" sometimes need rewording rather than word replacement, particularly in `docs/spec/ARCHITECTURE.md` and the protocol-crate READMEs. Those files are read and edited rather than swept.

- **`RevisionId` appears throughout the redb write store, including recovery and signing.** → Serialization is by value, not by type name, so no stored data is affected. Verified by reopening a store written before the change.

- **Conflicts with the in-flight `fix/private-builtin-materializers` branch and the pending removal of `fava-builtin-codecs`.** → This change touches nearly every file that work touches. Land one, rebase the other; do not run them concurrently. Sequencing this first is cheaper, since it makes the deletion a smaller diff rather than a larger one.

- **Symbol Gate signatures drop across most of the workspace at once.** → Unavoidable for a rename of this reach, and the re-sign is bulk rather than per-decision because no declaration's shape changed.

## Migration Plan

No runtime migration, no data migration, no wire change. Per `AGENTS.md` the project takes public API breaks directly, with no aliases or deprecation shims — the old names are removed, not re-exported.

Rollback is a revert.
