## Why

"Materializer" names something plain in obscure terms. The thing takes an edit plus the event that currently exists and produces the event that should exist next. That is applying an edit — and `apply` is already the verb the protocol crates used for their own versions of this before the contract absorbed them.

`ReplaceableEventEdit` carries a redundant qualifier for the same reason. Only replaceable and addressable kinds can be edited at all; a regular event is not edited, it is superseded by a new one. The whole subsystem is replaceable-only, so the prefix restates its context on all 228 mentions.

The cost is paid at every call site and in every doc: 1,518 occurrences of `materializ*` across 83 files, which is the sound of a word nobody reaches for naturally.

## What Changes

- **BREAKING** `ReplaceableEventMaterializer` → `EditApplier`. An edit is applied by an applier.
- **BREAKING** `ReplaceableEventMaterializer::materialize` → `EditApplier::apply`.
- **BREAKING** `ReplaceableEventEdit` → `EventEdit`.
- **BREAKING** `MaterializationId` → `RevisionId`. Its own doc already describes it as a generation of the materialized event; each application of an edit produces the next revision of that write's event.
- `crates/fava-write/src/materialization.rs` → `crates/fava-write/src/edit_application.rs`, and `crates/fava-publication/src/materialization.rs` likewise.
- **BREAKING** The facade's `materializer` and `materializers` builder methods → `applier` and `appliers`.
- Prose in doc comments, READMEs, `docs/spec/ARCHITECTURE.md`, and the semantic catalogs follows the same substitution: an edit is *applied*, not *materialized*; the result is a *revision*.
- No behavior changes. No signature changes beyond the names themselves.

## Capabilities

### New Capabilities
- `write/edit-vocabulary`: the names the write-edit surface uses for an edit, for the thing that applies one, and for the identity of a resulting revision.

### Modified Capabilities

None.

## Impact

- `crates/fava-write` — the contract, the edit type, and the revision id are all defined here.
- `crates/fava-publication` — the lookup, the apply call, the panic guard, and the refusal messages that quote these terms.
- `crates/fava-write-store`, `crates/fava-write-store-memory`, `crates/fava-write-store-redb` — `RevisionId` appears throughout recovery, semantic acceptance, and signing. The type is a `NonZeroU64` newtype serialized by value, so no persisted data changes.
- `crates/fava-nip02`, `crates/fava-bookmarks`, `crates/fava-simple-groups` — typed edit constructors' return type and doc prose.
- `crates/fava` — the two builder methods and the facade docs.
- Public API surface of every crate above changes; the affected declarations need re-signing under Symbol Gate.
- Sequencing: land this before `capability-self-registration`, whose artifacts are written in this vocabulary. The two are otherwise independent, and this one is severable if the ordering proves inconvenient — it is a rename with no design content.
