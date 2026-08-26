## Context

See `proposal.md` for motivation and `specs/simple-groups/multi-group-publication/spec.md` for observable requirements.

`EventBuilder` currently owns only Nostr event fields and consumes itself into `UnsignedEvent`. `WriteRouting` already owns automatic versus bounded exact relay selection, `WriteIntent` already persists payload plus route, and the Fava facade already admits unsigned events through one publication lifecycle. `fava-simple-groups` currently mutates or validates one `h` context through `SimpleGroup::prepare`, while the application separately repeats `simple_group.hosts()` through `fava.to(...)`.

The design must keep the builder crate protocol-neutral, preserve the universal signer/publication/receipt owners, return the same concrete `EventBuilder` after `.simple_group(...)`, and introduce no compatibility surface.

## Goals / Non-Goals

**Goals:**

- Compose several simple-group ids and host sets into one event-building expression.
- Sign one event once and publish that exact event through the deduplicated union of selected hosts.
- Keep relay routing outside the serialized and signed Nostr body.
- Preserve ordinary automatic publication for builders without an explicit route.
- Carry the explicit route into the existing durable write and receipt lifecycle.

**Non-Goals:**

- Making NIP-29 define repeated `h` semantics for third-party relays.
- Providing independent per-group deletion or acknowledgement when several groups share one event id at one relay.
- Adding a simple-groups-owned signer, publisher, retry loop, store, receipt, or publication facade.
- Hiding routing in global state, event tags, or an unbounded extension map.

## Decisions

### `EventBuilder` carries neutral routing

Add a `WriteRouting` field to `EventBuilder`, initialized to `Automatic`. Add one public neutral route-composition operation in `fava-write` that accepts relay identities, stages a normalized candidate, preserves first occurrence, collapses duplicates, applies the existing 256-relay bound, and returns `EventBuilder`. It does not inspect tags, kinds, group ids, or protocol meaning.

This makes the builder a declarative event-plus-publication-intent builder before custody. Routing remains separate from event serialization, id construction, and signing.

Alternatives rejected:

- A simple-groups wrapper changes the concrete fluent type and violates the chosen API.
- Relay hints in Nostr tags turn local delivery intent into signed wire semantics.
- A group registry loses the exact host values supplied to the expression and introduces hidden lifecycle state.
- A type-erased extension bag creates an unforced provider and persistence framework.
- A side table keyed by builder or event identity is move-unsafe, restart-unsafe, and instance-ambiguous.

### `fava-simple-groups` extends, but does not own, the builder

Introduce the public `SimpleGroupEventBuilder` extension trait in `fava-simple-groups` with `.simple_group(&SimpleGroup) -> Result<EventBuilder, WriteIntentError>` implemented for `EventBuilder`.

Each call computes the complete candidate route before changing the builder, contributes `group.hosts()` through the neutral route operation, and appends exact `Tag::parse(["h", group.id()])` only when that exact two-cell tag is absent. Existing nonmatching, malformed, or extra-cell sibling tags remain untouched; this operation validates only the context it owns. Reusing the same id adds no second exact `h` but may add new hosts. Different ids append distinct exact `h` tags in call order.

The extension trait owns simple-group tag meaning. `EventBuilder` continues to own generic tag bounds, event encoding, id construction, and neutral route bounds. A constructed `SimpleGroup` supplies no new fallible group input here: the only refusal is route accumulation, so the existing `WriteIntentError` propagates without a simple-groups error wrapper or translation.

### Fava consumes builder routing at the existing publication door

Implement the facade's existing publication-payload contract for `EventBuilder`. The lowering first builds the unsigned event and resolves the two possible route sources:

- builder `Automatic` plus facade `Automatic` becomes automatic routing;
- builder `Automatic` plus `fava.to(...)` explicit routing uses the facade route;
- builder explicit plus facade `Automatic` uses the builder route;
- builder explicit plus facade explicit returns a typed conflict, even if equal.

The resolved event and route enter `WriteIntent::event`; signing, durable acceptance, publication, restart, settlement, and receipts remain unchanged. Conflict and build errors occur before signer invocation or custody.

`fava.publish(builder)` is therefore the ordinary terminal for a grouped unsigned event. `fava.to(...).publish(...)` remains available for payloads whose route is not carried by a builder, including pre-signed events.

### Event-only building cannot erase routing

The event-only terminal refuses when `EventBuilder` contains explicit routing. Fava uses a neutral consuming operation that returns the checked unsigned event and `WriteRouting` together. This operation is public only because the facade and protocol crates are separate; it is not a second publication door and does not accept custody.

Removing or bypassing an attached route is not part of this change. An application that needs a standalone unsigned event constructs a builder without `.simple_group(...)`.

Alternative rejected: keeping today's `build() -> UnsignedEvent` behavior for routed builders would silently erase a user-selected publication obligation.

### Pre-signed validation accepts sibling group contexts

Signed events remain immutable. The simple-groups signed validation path verifies the event signature and succeeds when it finds the selected exact two-cell `h` tag. It ignores unrelated or malformed sibling `h` rows rather than allowing them to erase a valid selected context. It returns the byte-exact original event and never attaches routing.

Applications publishing pre-signed events construct the explicit union of group hosts and use the existing `fava.to(...).publish(event)` path. The unsigned `SimpleGroup::prepare` path is removed because `.simple_group(...)` on `EventBuilder` supersedes it; no alias remains.

### Multi-group truth requires relay evidence

Repeated exact `h` tags are a Fava publication contract, not a claim that every NIP-29 relay interprets them identically. Acceptance requires a controlled relay canary that admits one signed event for every selected exact `h`, returns it through each corresponding filter, and proves the event id and signature remain identical.

Ordinary receipts remain relay/event evidence. Fava does not infer per-group admission from route inclusion or a single relay acknowledgement. If one relay hosts several selected groups, one event delivery and one acknowledgement cannot prove independent group-local outcomes.

## Risks / Trade-offs

- [Third-party relays may authorize only one `h`] → State the extension explicitly, retain actual evidence, and require controlled live proof before claiming interoperability.
- [One event id couples moderation across groups on the same relay] → Make independent per-group deletion a non-goal; do not fabricate group-local settlement.
- [`EventBuilder` now spans event fields and pre-custody route intent] → Keep the two fields and validators separate; lower them only once into the existing `WriteIntent` owner.
- [A chain can exceed tag or relay bounds] → Stage each contribution atomically and reuse the owning generic typed bounds before signing or custody.
- [Two explicit route sources could silently merge or override] → Refuse every dual-explicit expression with a typed error.
- [Public names expand architectural vocabulary] → Land the focused vocabulary approval before implementation and replace superseded declarations in one clean break.

## Migration Plan

1. Approve the focused architecture and vocabulary change, including the extension trait, neutral builder route operation, consuming builder-parts operation, and typed refusals.
2. Replace authoritative `docs/spec/` and vocabulary declarations with the new current model; retain no migration narration in those files.
3. Add public Rust behavior tests that fail because `EventBuilder` lacks neutral routing, `.simple_group(...)`, and direct Fava publication; add deliberate breaks for dropped routes, duplicate tags, route conflicts, restart, and relay interpretation.
4. Implement neutral builder routing and facade lowering without changing `WriteIntent`, publication, or receipt ownership.
5. Implement the simple-groups extension and signed-event tolerant validation; remove unsigned `prepare` and superseded examples.
6. Run focused Cargo/Bazel tests, vocabulary checks and unit tests, full required validation, restart evidence, and the controlled multi-group relay canary.

No persisted-data migration is expected because accepted writes already persist event payload plus `WriteRouting`. Before merge, rollback is ordinary branch reversion; after merge, rollback reverts the complete focused change rather than adding compatibility paths.
