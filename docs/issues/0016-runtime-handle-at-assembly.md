# The Tokio requirement belongs at assembly, not at every call

**Status:** proposed (awaiting Pablo approval)
**Authority:** `AGENTS.md:74` — "No hidden runtime feature flags or silent
compatibility behavior."
**Relationship:** independent of `docs/issues/0014-publish-door-ergonomics.md`.
The door inherits whichever answer lands, because a `publish` that can refuse for
reasons unrelated to the write is a worse door than one that cannot.

## Problem

`Publication::accept` reads the ambient Tokio context on every call:

```rust
// crates/fava-publication/src/lib.rs:62-73
/// Returns [`PublicationError`] before custody when no runtime exists, or
/// after a failed acceptance commit.
pub fn accept(&self, intent: WriteIntent) -> Result<AcceptedWrite, PublicationError> {
    tokio::runtime::Handle::try_current().map_err(|_| PublicationError::RuntimeUnavailable)?;
    let accepted = self.store.accept(intent)?;
    self.start(accepted.receipt_id);
    Ok(accepted)
}
```

`recover` (`:81`) opens identically. The guard order is correct and deliberate:
`accept` commits durably and then starts delivery, so discovering a missing
runtime after custody would leave a durable obligation nothing can drive.
Refusing first is the same "refuse before custody, zero residue" discipline
WRITE-030 (`GOALS:994`) requires for already-expired events. Nothing about that
ordering should change.

Three things about it should.

**1. The requirement is undocumented.** `grep -rn "RuntimeUnavailable" docs/
.planning/` is empty. No authority states that publication requires a running
runtime. A synchronous method carries a dependency discovered only by hitting it.

**2. It already reaches engine construction, wearing the wrong error.**
`FavaBuilder::build` calls `publication.recover()` (`crates/fava/src/lib.rs:384-386`),
so constructing a `Fava` with publication selected already requires a running
runtime. Outside one it fails as
`BuildError::Publication("publication requires a running Tokio runtime")` — a
stringly-typed runtime refusal from a method documented as "naming the first
required provider role that was not selected" (`:349-352`).

**3. It refuses work that does not need a runtime.** The durable commit itself
needs no runtime context; only the delivery start does. An application that
builds a `Fava` inside a runtime and then calls `publish` from a plain thread is
refused, even though the store transaction would succeed and the delivery start
could be dispatched through a captured handle.

## Change

Capture a `tokio::runtime::Handle` when the publication owner is assembled and
spawn through it. `Handle::spawn` may be called from any thread.

- `Publication` holds the handle. `accept` and `recover` lose their
  `Handle::try_current()` guards; the ordering of commit and start is unchanged.
- `BuildError` gains a named variant for the assembly-time requirement, replacing
  the stringly-typed `Publication(String)` for this one condition.
- `PublicationError::RuntimeUnavailable` is removed from the write path once no
  call site can produce it.

The handle is taken from the ambient context at `build()` time — the one moment
the current code already demands it — rather than accepted as a builder
parameter. `ARCHITECTURE.md:2902` bars Tokio handles from crossing a neutral
contract; a handle held inside `fava-publication` does not, but a handle accepted
through `FavaBuilder` would be one at the application boundary.

## Exit gates

- Building outside a runtime with publication selected fails with the named
  `BuildError` variant, not a string.
- Publishing from a plain thread on an engine built inside a runtime succeeds and
  starts delivery.
- No durable residue on the assembly-refusal path: zero write-store rows, zero
  receipts, zero provider work.
- The requirement is stated in `docs/spec/ARCHITECTURE.md` under the `fava`
  facade or `fava-runtime`, so it is no longer hidden.
- `python3 tools/check_vocabulary.py` passes if `BuildError`'s variant set changes
  in a way the registry tracks.
