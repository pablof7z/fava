## Context

See `proposal.md` — Why. The mechanics that already exist on `origin/main` and are not being rebuilt:

- `EditApplier::kind` is the claim. The trait already answers "which kind do I own".
- `fava-publication` already indexes an iterator of appliers into a `BTreeMap<Kind, _>`, refuses a second claim on a kind, and refuses more than 64 claims.
- The facade's registration door is already neutral (`applier` / `appliers`), ships nothing of its own, and takes no real protocol dependency.
- Each protocol crate already owns its applier implementation and exports a factory for it.

What does not exist is a way for that claim to reach the index without the application restating it at assembly, and without the protocol crate publishing the low-level contract to do so.

Written in the vocabulary of `plain-edit-vocabulary`, which lands first.

## Goals / Non-Goals

**Goals:**

- The facade depends on no protocol crate and contains no kind number.
- A protocol crate owns its kind completely, and publishes its claim on that kind without publishing the contract behind it.
- An application that depends on a protocol crate gets its behavior without a line of assembly wiring.
- One capability vocabulary for protocol-declared and application-defined claims alike.

**Non-Goals:**

- Changing the applier contract's shape, the kind index, the duplicate-claim refusal, or the 64-claim bound. All are reused as-is.
- Changing edit encodings, or how any specific kind's edits work. The implementations stay where they are; only how their claim is published changes.
- Letting a capability override another. A conflict stays a refusal.
- Ambient or lazy registration after assembly. The index is fixed when the facade is built.

## Decisions

### The registry lives in `fava-write`, not `fava`

`fava-write` owns `EditApplier` and is already a dependency of every protocol crate — `fava-simple-groups`'s dependency set is exactly `fava-query`, `fava-state`, `fava-write`, and `nostr`, enforced by its own architecture test. Putting the macro in `fava` would make protocol crates depend on the universal facade, inverting the layering and creating a cycle with the facade's current shape.

So the macro is `fava_write::register_capability!` and `Capability` is a `fava-write` type. `fava` reads the registry at assembly and knows only that it received opaque claims.

### `Capability` is opaque; the index reads through it

`Capability` wraps an `Arc<dyn EditApplier>` with no public accessor. `fava-write` and `fava-publication` read the inner applier through a crate-visible path; an application holding one can only hand it to the builder. `Capability::from_applier` is the application-facing constructor, so an app defining its own kind produces the same type a protocol crate declares.

This is what separates the design from 0051's counterexample. `fava_simple_groups::__fava::materializer()` returned the low-level contract to a consumer that should only construct typed edits. A capability returns nothing the consumer can use except "give this to Fava".

### Both doors, with self-registration as the normal one

Linking is the registration for protocol crates. The explicit `.capability(..)` door stays, for two reasons that are not going away: an application defining its own edit semantics has no crate to link, and a test needs to assemble a facade with an exact claim set rather than whatever the test binary happens to link. Both feed one index under one set of refusals.

### Link-time collection over an assembly-site list

This is the decision with real cost, taken deliberately: the application writes nothing, and the dependency graph is the declaration.

The mechanism is a distributed-slice registry — the `inventory` or `linkme` pattern — where each `register_capability!` invocation emits a submission the runtime enumerates at startup. `inventory` is the more portable of the two; `linkme` avoids life-before-main at the cost of narrower linker support. Choosing between them is an implementation task with a target-support check, not a design question, because the contract above is the same either way.

*Alternative — explicit `.capability(fava_simple_groups::capability())` at assembly:* every claim visible in one readable list, no linker behavior involved, trivial test isolation. Rejected: it keeps a wiring line whose only content is restating a dependency the `Cargo.toml` already declares.

*What is genuinely lost:* assembly stops being a complete inventory of what the program can do. Answering "which kinds does this binary claim" becomes a question about the dependency graph rather than about a block of code. That is the trade.

### The appliers stay in the protocol crates, and 0051 is superseded

The applier implementations do not move: on `origin/main` they already live in `fava-simple-groups`, `fava-nip02`, and `fava-bookmarks`. What changes is how the claim leaves each crate — a declared capability instead of a public factory.

0051's load-bearing claim was that "Rust requires a cross-crate factory to be public, while universal `fava` must remain kind-blind", concluding an implementation crate is necessary. The premise is right and the conclusion does not follow: what must be public is a capability, and a public capability exposes no implementation term. The facade stays kind-blind by depending on nothing, which it already does. 0051 is recorded as superseded with that reasoning, not silently rewritten.

## Risks / Trade-offs

- **A crate linked only for its capability may be dropped by the linker.** Rust and the linker may discard a dependency no symbol references, taking its registration with it. → In practice a program depending on a protocol crate calls its constructors, which references it. The dangerous shape is a dependency held *only* to enable recovery of a kind the program no longer constructs. This is verified explicitly with an integration test that links a capability crate and calls nothing from it, and it is the first thing to check when a claim goes missing.

- **Target and linker support is narrower than plain Rust.** → Verified against the targets the workspace builds for before the mechanism is chosen; the choice between `inventory` and `linkme` is made on that result.

- **Two versions of one protocol crate in the dependency graph both register, and assembly refuses as a duplicate claim.** → The refusal is correct and the message names the kind; the diagnosis is a `cargo tree` away. Worth naming in the refusal's documentation, since the cause is invisible at the assembly site.

- **Test isolation.** A process-global registry means a test binary cannot easily assemble a facade *without* a linked capability. → Refusal-path tests use a kind no linked crate claims, or a separate test binary. The explicit door covers the cases that need an exact claim set.

- **The failure mode moves from a compile error to a runtime refusal.** Forgetting a dependency used to be a missing symbol; now it is "no implementation claims this kind" at publish. → The refusal already exists and already names the kind. Its documentation gains the new most-likely cause.

- **A new external dependency in `fava-write`, at the bottom of the stack.** → Accepted as the price of the chosen mechanism; it is small, widely used, and confined to the registry module.

## Migration Plan

No runtime or data migration. Edit encodings, event ids, and wire format are unchanged — no applier implementation moves or changes behavior.

An application that today registers a protocol crate's applier at assembly deletes that line; depending on the crate is now the whole declaration.

Per `AGENTS.md` the project takes public API breaks directly, with no aliases or shims. Land after `plain-edit-vocabulary`.

Rollback is a revert.
