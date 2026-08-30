## Context

See `proposal.md` — Why.

What exists on `origin/main`: `EditApplier` in `crates/fava-write/src/edit_application.rs` has a `kind()` method; `crates/fava-publication` indexes handlers into a `BTreeMap<Kind, Arc<dyn EditApplier>>` and refuses duplicates and more than 64; `crates/fava/src/builder.rs:147,157` has `applier` / `appliers`; each protocol crate owns its handler and exports a public factory for it.

Only the last item is wrong.

## Goals / Non-Goals

**Goals:**

- An application enables a protocol by naming the protocol.
- `EditApplier` leaves every protocol crate's public surface.
- Protocol crate dependency sets do not change.

**Non-Goals:**

- Removing the enabling line entirely. The spike settled that; see below.
- Changing the index, the refusals, the bound, or any edit encoding.
- A hint on the edit naming the crate to enable. Considered and dropped as not worth the field — the failure already surfaces at first use, and at assembly when a stored write is outstanding.

## Decisions

### Extension trait over link-time registration

A spike built throwaway crates for `inventory` and `linkme` and tested a consumer that depends on a provider but names nothing from it — the shape an application has when it depends on a protocol crate only so that stored writes of that kind can be recovered at startup.

All 16 cells failed: both libraries, debug and release, binary and test harness, direct and transitive dependency. The registration is absent, silently. The cause is not dead-code elimination but archive extraction — `cargo build -v` confirms rustc passes the `.rlib` to the linker, and the linker never pulls the object out of it because no symbol resolves into it. Adding `use provider as _;` fixes every cell.

So the mechanism cannot deliver "no wiring at all" for exactly the case that motivated it, and the fix it needs is a source-level `use` — which the extension trait requires anyway, for free, because you cannot call `with_simple_groups()` without importing the trait.

Neither library documents this. `inventory`'s README claims plugins simply register; the hazard appears only in dtolnay's closed `inventory#7`.

The spike also established that a process-global registry cannot be test-isolated: `inventory` 0.3.24's public surface is `Registry`, `Node`, `ErasedNode`, `Collect`, with no scoping, reset, or clear API. No test could assemble a facade seeing an exact handler set. That alone disqualifies it for this workspace.

### The trait is written against a sink in `fava-write`, not against `FavaBuilder`

The obvious form — `impl SimpleGroups for FavaBuilder` — puts `FavaBuilder` in `fava-simple-groups`'s dependency set, meaning every protocol crate depends on the universal facade. That inverts the layering and, for `fava-simple-groups`, breaks its architecture test, which asserts dependencies are exactly `fava-query`, `fava-state`, `fava-write`, `nostr` (`fava-nip02`'s own test asserts that set plus `fava-relay`; `fava-bookmarks` has no `fava-query` dependency and gains its own architecture test asserting `fava-state`, `fava-write`, `nostr` as part of this change). The inversion is the same regardless of the exact set — each crate would still be pulling in the universal facade it exists to stay blind to.

Instead `fava-write` — which already owns `EditApplier` and is already a dependency of every protocol crate — gains a one-method sink:

```rust
pub trait EditApplierSink {
    fn accept(self, applier: Arc<dyn EditApplier>) -> Self;
}
```

`FavaBuilder` implements it. Each protocol crate writes its trait against the sink:

```rust
pub trait SimpleGroups: Sized {
    fn with_simple_groups(self) -> Self;
}

impl<T: EditApplierSink> SimpleGroups for T {
    fn with_simple_groups(self) -> Self {
        self.accept(Arc::new(SavedGroupListApplier))
    }
}
```

Dependency direction is preserved and every protocol crate's dependency set is unchanged by this change (`git diff` on all three `Cargo.toml`s is empty); `fava-simple-groups` and `fava-nip02` each carry an architecture test asserting their own exact set unchanged, and `fava-bookmarks` gains one as part of this change.

*Alternative — put the extension traits in `fava` itself:* one crate to look in, no sink needed. Rejected: `fava` would name every protocol, which is the kind-blindness the facade currently has and should keep.

### `applier` / `appliers` stay

An application defining edit semantics for its own kind has no crate to enable and genuinely holds an `EditApplier`. Hiding the contract from it would mean inventing a wrapper whose only purpose is to be unwrapped. The two doors serve two different callers, and the doc comments should say which is which.

### The handler types become private

`SavedGroupListApplier` and its siblings become private to their crates. Nothing outside needs to name them once the enabling call exists.

## Risks / Trade-offs

- **Enabling is still a line an application can forget.** → It fails loudly: at assembly when a stored write of that kind is outstanding, because `crates/fava/src/builder.rs:234` runs recovery during `build()` and propagates the error; otherwise at first publish, naming the unclaimed kind. Considered and rejected: carrying a hint string on the edit to name the crate to enable.

- **A blanket `impl<T: EditApplierSink>` makes the method appear on any future sink implementor.** → Acceptable while `FavaBuilder` is the only one. Narrowing to `impl SimpleGroups for FavaBuilder` directly is not a reachable fallback — it is exactly the inverted-layering form rejected above, and `fava` takes `fava-simple-groups` as a dependency, so the reverse edge would be a cargo package cycle. If a second sink appears and the blanket impl becomes wrong, the fix stays inside `fava-write`: add a marker supertrait there (e.g. `pub trait ProtocolExtensible: EditApplierSink {}`, implemented for `FavaBuilder` in `fava`) and have the blanket impls bind on it instead of on `EditApplierSink` directly, or seal `EditApplierSink` itself. Either keeps the narrowing inside `fava-write`, which every protocol crate already depends on, with no change to callers.

- **Three near-identical extension traits.** → Deliberate. A single shared trait would need each crate to name the others' methods.

## Migration Plan

No runtime or data migration; no wire, event-id, or persisted-value change. Applications replace `.applier(fava_simple_groups::saved_group_list_applier())` with `use fava_simple_groups::SimpleGroups;` and `.with_simple_groups()`.

Per `AGENTS.md` the project takes public API breaks directly, with no aliases or deprecation. Land after `plain-edit-vocabulary`, which is already merged.

Rollback is a revert.
