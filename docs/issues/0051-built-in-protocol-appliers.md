# 0051 — Fava owns its shipped protocol appliers

**Status:** superseded by `protocol-extension-traits`
**Owner:** `fava` facade selects shipped protocol appliers; applications
select only appliers for application-defined edit semantics.

## Decision

`fava-nip02`, `fava-bookmarks`, and `fava-simple-groups` own their edit codecs,
but Fava owns their integration with its semantic-write lifecycle. Whenever a
Fava assembly selects publication, it installs those shipped appliers
before recovery. An application publishes the protocol crate's typed edit
without applier wiring.

`EditApplier` remains the public neutral extension contract. The facade names
its registration methods `applier` and `appliers`: only an application that
adds its own edit semantics interacts with that low-level contract. A claimed
built-in kind is a duplicate applier refusal, never an override.

## Proof

- Facade integration tests publish NIP-02, bookmark, and saved-group edits
  with no application applier registration.
- The public simple-groups API no longer exports
  `saved_group_list_applier`.
- Application applier tests retain custom-kind lifecycle coverage and
  prove built-in-kind collisions refuse during assembly.

## Superseded by `protocol-extension-traits`

The decision above still holds at the level of intent — a Fava assembly gets
NIP-02, bookmarks, and saved-group publication without an application wiring
an `EditApplier` by hand — but the mechanism it specified, the facade
auto-installing shipped appliers ahead of recovery, was never carried to
a working implementation. It was spiked as `fava-builtin-codecs`: a crate
holding the shipped codec implementations behind a link-time self-registration
so the facade could discover and install them without either side naming the
concrete applier at the call site. The spike failed and was discarded:

- A crate that is a Cargo dependency but is never named in source is not
  extracted from its `.rlib` by the linker, so its self-registration silently
  disappears. All 16 tested cells failed (inventory and linkme, debug and
  release, binary and test, direct and transitive). The cause is archive
  extraction — the linker never pulls an object file out of a dependency's
  archive unless something in the source graph references a symbol in it —
  not dead-code elimination, so no `#[used]`/`#[no_mangle]`/link-section
  workaround fixes it while the dependency stays unnamed in source.
- A process-global registry (what both `inventory` and `linkme` provide) has
  no scoping API: every registrant that does link in is visible to every
  assembly in the process. No test can build a facade with a controlled,
  partial set of shipped appliers, which 0051's own "duplicate
  applier refusal" proof needs.

`protocol-extension-traits` replaces facade-side auto-installation with
explicit, compiler-visible enablement at the call site. `fava-write` gains a
one-method sink trait, `EditApplierSink`, implemented by any builder
that accepts an `Arc<dyn EditApplier>`; `FavaBuilder` implements it. Each
protocol crate then exposes its own extension trait written against that sink
— `SimpleGroups::with_simple_groups()`, `Nip02::with_nip02()`,
`Bookmarks::with_bookmarks()` — so `fava.with_simple_groups().with_nip02()...`
is itself the source-level registration, needing no link-time discovery and no
process-global state. This also resolves 0051's registration-visibility
concern more strongly than the original decision did: the applier factories
(`saved_group_list_applier`, `fava_nip02::applier`, `fava_bookmarks::applier`)
and the applier types themselves are now private, and `EditApplier` is no
longer exported from any protocol crate's public surface at all — an
application enables a shipped protocol with a trait method, never touching
the applier vocabulary. `FavaBuilder::applier`/`appliers` remain, unchanged,
for applications defining their own edit kinds.

See `openspec/changes/protocol-extension-traits/proposal.md` and
`design.md` for the full design. Each protocol crate's own dependency set is
unchanged and, where an architecture test exists, is asserted by it:
`fava-simple-groups` and `fava-nip02` each carry one (`fava-query`,
`fava-state`, `fava-write`, `nostr` — plus `fava-relay` for `fava-nip02` —
never `fava`); `fava-bookmarks` gains one as part of this change, asserting
`fava-state`, `fava-write`, `nostr`. This makes the shape a falsifiable
property rather than a convention.
