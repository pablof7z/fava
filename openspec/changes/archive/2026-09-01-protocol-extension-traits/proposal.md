## Why

To use simple groups today, an application has to name a thing it should never have heard of:

```rust
Fava::builder()
    .applier(fava_simple_groups::saved_group_list_applier())
    .build()?
```

`saved_group_list_applier()` returns an `Arc<dyn EditApplier>` — the low-level contract for turning an edit plus the current event into the next one. An app that only wants to save a group has no business holding one. The only reason `fava-simple-groups` exports it at all is so the app can hand it back to Fava. That is the exposure issue 0051 objected to.

The replacement should read as what it means:

```rust
use fava_simple_groups::SimpleGroups;

Fava::builder()
    .with_simple_groups()
    .build()?
```

This supersedes the earlier `capability-self-registration` plan, which tried to remove the line entirely via a link-time registry. A spike killed it: a crate that is a Cargo dependency but never named in source is never pulled out of its `.rlib`, so its registration silently disappears. All 16 matrix cells failed — `inventory` and `linkme`, debug and release, binary and test, direct and transitive dependency — and the only fix is a source-level `use` of the crate, which is the line the plan existed to delete. The same spike found that a process-global registry has no scoping API, so no test could ever assemble a facade with a controlled handler set.

The extension trait gets the readability without the magic, and the `use` line that makes it work is the same line that keeps the crate linked.

## What Changes

- **BREAKING** `fava-write` gains a small sink trait, `EditApplierSink`, with one method for accepting an `Arc<dyn EditApplier>`. `FavaBuilder` implements it.
- **BREAKING** Each protocol crate gains an extension trait written against that sink, not against `FavaBuilder`: `fava_simple_groups::SimpleGroups` with `with_simple_groups()`, blanket-implemented for any `T: EditApplierSink`, plus the equivalents for `fava-nip02` and `fava-bookmarks`. This leaves each protocol crate's own dependency set unchanged — `fava-simple-groups` stays at exactly `fava-query`, `fava-state`, `fava-write`, `nostr`; `fava-nip02` at those plus `fava-relay`; `fava-bookmarks` at `fava-state`, `fava-write`, `nostr` — and none of the three takes on `fava`.
- **BREAKING** The public applier factories — `fava_simple_groups::saved_group_list_applier`, `fava_nip02::applier`, `fava_bookmarks::applier` — become private. `EditApplier` disappears from every protocol crate's public surface.
- The builder keeps `applier` / `appliers` for applications defining edit semantics for their own kinds. Those callers genuinely own an `EditApplier` and should say so.
- No change to the kind index, the duplicate-kind refusal, the 64-claim bound, or any edit encoding.

## Capabilities

### New Capabilities
- `publication/protocol-enablement`: how an application turns on a protocol crate's write semantics, and what it can and cannot see of the mechanism.

### Modified Capabilities

None.

## Impact

- `crates/fava-simple-groups`, `crates/fava-nip02`, `crates/fava-bookmarks` — each gains an extension trait and drops its public factory. Dependency sets are unchanged: the trait is written against `fava-write`'s sink, so no protocol crate takes a dependency on `fava`.
- `crates/fava-write` — gains `EditApplierSink`.
- `crates/fava/src/builder.rs` — implements `EditApplierSink`; behavior otherwise unchanged; `applier` / `appliers` stay and gain doc comments pointing app-defined semantics at them and protocol users at the extension traits.
- Each protocol crate's architecture test — the export allow-list loses the factory. `fava-simple-groups` and `fava-nip02` already carry one; `fava-bookmarks` gains one as part of this change, since its dependency set was previously unenforced.
- `docs/issues/0051-built-in-protocol-appliers.md` — recorded as resolved by this, with the spike result noted so nobody retries the link-time approach.
