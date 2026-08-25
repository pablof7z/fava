# Full Fava Rewrite Specification: Goals and Objectives

**Status:** proposed normative specification for the Fava rewrite  
**Supersedes:** `FAVA_REWRITE_SPEC.md`  
**Companion documents:** `ARCHITECTURE.md` defines crate boundaries, contracts, ownership, and composition. `FAVA_TDD_BDD_TESTING_GUIDE.md` defines how behavior is specified and proved. This document defines required behavior and product goals.
**Audience:** implementors, provider authors, protocol-crate authors, SDK authors, application developers, and reviewers.

---

## 1. Purpose

Fava is an embeddable Nostr client engine with a Rust implementation and first-class platform SDKs. It packages reusable Nostr client behavior—live queries, relay work, local event reuse, signing, routing, publication, receipts, and supporting protocol services—behind an API an application calls.

Fava is a library, not an application framework. The application owns:

- product state and domain models;
- navigation and presentation;
- ranking and recommendation policy;
- moderation policy and user choices;
- account UX and secret-storage UX;
- when to open or close a live query;
- when to create, cancel, or remove a write; and
- which Fava providers, services, and protocol crates are compiled into its product.

Fava owns the reusable machinery and lifecycle correctness needed to make those requests work across relays, signers, local data, cancellation, failure, and restart.

Fava has two primary long-lived workload concepts:

1. **Live query.** A declarative request for a current event view. Fava keeps that view coherent as local sources, reactive dependencies, routing knowledge, relay results, and event evidence change.
2. **Write intent.** An accepted publication obligation. Fava carries it through event materialization where needed, signing, routing, delivery, and exact per-destination outcomes under one reattachable receipt.

Supporting operations—session management, signing without publishing, NIP-05 resolution, NIP-11 relay information, diagnostics, content parsing, and destructive reset—do not create parallel workload models.

---

## 2. Nature of this specification

This specification is behavioral. It defines:

- application-facing behavior;
- observable guarantees;
- failure, race, cancellation, restart, and recovery behavior;
- ownership boundaries between Fava and the application;
- the dimensions in which providers may vary;
- the dimensions that remain universal; and
- explicit non-requirements.

It does not prescribe:

- exact Rust symbol names;
- internal module names;
- database table layouts;
- channel implementations;
- executor choices;
- a universal effect enum;
- public API spelling in Swift or Kotlin; or
- the exact crate list, which belongs in `ARCHITECTURE.md`.

Illustrative terms in this document describe semantic roles rather than mandatory public type names.

### Normative language

- **MUST** and **MUST NOT** define required behavior.
- **SHOULD** and **SHOULD NOT** define the expected behavior unless a documented product profile deliberately chooses otherwise.
- **MAY** identifies permitted behavior.

---

## 3. Core terminology

### Fava assembly

A concrete application build containing one selected implementation for each required provider contract and zero or more selected protocol crates.

Provider and protocol-crate selection is a build/application-composition decision. Two different applications may use different event caches, write stores, routers, subscription planners, transports, publishers, delivery policies, signers, services, and protocol crates.

Fava does not promise persistence continuity between unrelated provider implementations merely because they implement the same contract.

### Product profile

A documented Fava assembly with declared guarantees. A profile states, at minimum:

- event-cache retention and restart behavior;
- write-store durability;
- selected routing contributors and their order;
- subscription-planning behavior;
- transport and publisher behavior;
- delivery policy;
- selected protocol services and event-kind protocol crates; and
- supported platform artifacts.

### Event record

The application-facing representation of one logical event plus the evidence Fava currently has about it.

An event record may include:

- an unsigned local event materialization;
- a signed event;
- relays that actually served the signed event;
- local publication evidence such as a receipt and signing state;
- deletion, replacement, expiry, or shortfall facts relevant to presentation; and
- source-scoped acquisition evidence.

The same event contributed by several local or remote sources appears once, with evidence merged according to universal rules.

### Event cache

A selected cache implementation for signed, relay-observed events and their cache-owned evidence. It may be memory-only, bounded, persistent, remote, or deliberately absent.

Persistence, restart reuse, provenance retention, tombstone retention, and historical-coverage guarantees belong to the selected event-cache implementation and product profile. They are not implied merely by implementing the baseline event-cache contract.

### Write store

The authoritative owner of accepted local publication obligations, replaceable-event edits, current unsigned or signed materializations, receipts, route revisions, delivery facts, cancellation, supersession, and restart recovery.

The write store is also a local query source. Unpublished events do not need to be copied into the event cache in order to appear in live queries.

### Service cache

A generic storage facility that supporting services may use for opaque cached payloads. The service that understands the data owns validation, freshness, staleness, negative caching, and failure semantics.

For example, NIP-05 and NIP-11 have different meanings and cache policies even when both use the same physical cache provider.

### Automatic routing

Routing through the application-selected ordered router chain.

### Explicit routing

Routing to an exact non-empty relay set supplied for the operation. Explicit routing bypasses the automatic router chain and is not widened by it.

### Route contribution

One router's complete current answer for an automatic route session: currently known destinations, the logical targets they cover, unresolved needs, and router-owned explanation.

### Route plan

The merged current answer of the router chain. It may contain immediately usable destinations while other routing needs remain unresolved. Later route contributions update the same live query or write lifecycle.

### Settled source fact

A fact established by a specific source completing a specific request, such as a relay sending EOSE for an exact subscription. Settlement is always scoped; it never means global completeness.

---

# Part I — Product and composition goals

## GOAL-001 — Fava remains a library

Fava MUST NOT own application navigation, UI state, rendering, ranking, moderation decisions, or product-specific workflows.

Fava MAY provide platform-native reactive and lifecycle wrappers, content parsing, testing infrastructure, and optional UI-independent helpers. Those remain library facilities rather than an application framework.

**Acceptance:** an ordinary application can use Fava while retaining its own state architecture and presentation stack.

## GOAL-002 — The public surface remains conceptually small

The primary application mental model MUST remain live queries and accepted
publication obligations. Applications describe the payload and optional
author/route scope, then receive one `Write` that follows the durable receipt.
Neutral publication owners continue to use `WriteIntent`, `WritePayload`, and
`AcceptedWrite` internally; those contract values are not a second application
publication door.

Supporting operations MUST reuse the same underlying primitives rather than creating parallel query, publication, routing, receipt, or lifecycle systems.

**Acceptance:** a protocol crate such as NIP-02 or bookmarks produces ordinary query or write values and uses the standard query/publication lifecycles.

## GOAL-003 — Provider selection is static application composition

Applications MUST be able to select different provider implementations without forking Fava or modifying unrelated providers.

The selected implementation set is fixed for an engine instance and compiled into the product artifact. Runtime hot swapping, provider migration, unload handles, and plugin registries are not part of the required product model.

Changing provider implementation in a later application release is an application/provider migration decision. Fava does not require one implementation to open another implementation's private persisted format.

**Acceptance:** two external applications can compile different storage or routing implementations against the same public contracts with zero Fava source changes.

## GOAL-004 — One owner exists for each mutable fact and lifecycle

Every mutable fact, queue, retry loop, connection, observation, receipt transition, and durable obligation MUST have one authoritative owner.

The facade MAY order actions between owners. Ordering MUST NOT absorb their policy or duplicate their state.

**Acceptance:** an ownership ledger can name exactly one owner for every stateful concept. Any duplicate owner is treated as an architecture defect.

## GOAL-005 — Contracts and implementations remain separate

Every replaceable subsystem MUST expose a public, implementation-neutral contract. Default implementations MUST use the same contract available to external crates and MUST have no privileged bypass.

Provider contracts MUST use domain values and explicit facts. They MUST NOT expose implementation-specific database handles, runtime internals, or private state from another owner.

**Acceptance:** each standard provider and at least one deliberately different provider pass the same conformance suite.

## GOAL-006 — Higher-level policy remains independently selectable

A policy or protocol interpretation that builds on a primitive MUST remain outside the primitive's contract, even when its implementation is small.

Examples include:

- NIP-65 outbox routing;
- relay-hint routing;
- always-include application relays;
- fallback-relay policy;
- event-kind protocol crates; and
- service-specific caching policy.

**Acceptance:** replacing or omitting one such policy does not require editing the primitive subsystem.

## GOAL-007 — Universal correctness does not become provider policy

Providers may vary in algorithms and declared guarantees. They MUST NOT vary universal meanings such as:

- whether an event id or signature is valid;
- whether a relay actually served an event;
- whether an accepted write exists;
- whether bytes may have left Fava;
- whether a relay sent EOSE;
- whether a completion belongs to the current operation;
- whether two relay-access identities are isolated; or
- whether a limit caused shortfall.

**Acceptance:** a custom provider cannot construct stronger evidence or success than the facts supplied to it justify.

## GOAL-008 — Providers cannot block unrelated progress

Application-supplied providers may block, fail, panic, require human action, or become unavailable. Provider execution MUST NOT occur while holding another subsystem's authoritative transaction or lock, and MUST NOT indefinitely block unrelated relays, queries, writes, signers, or shutdown.

Late completions MUST carry enough identity to be dropped when stale.

**Acceptance:** deliberately blocking or panicking one provider leaves unrelated work and shutdown within declared bounds.

## GOAL-009 — Every provider boundary has a falsifier

Each replaceable contract MUST ship a public conformance kit covering:

- ordinary behavior;
- refusal and malformed input;
- cancellation and close;
- late completion;
- boundedness and overload;
- restart where the provider owns persistent state;
- account and relay-access isolation; and
- negative tests proving it cannot bypass universal invariants.

The test kit MUST work from public APIs.

## GOAL-010 — The replaceable boundaries are explicit

The rewrite MUST expose independent contracts for the following semantic responsibilities:

| Responsibility | Provider-owned variation | Universal boundary it cannot redefine |
|---|---|---|
| **Event cache** | retention, persistence, indexing, eviction, physical backend | event validity and coherent event-state semantics |
| **Write store** | physical backend, indexing, retention beyond required custody | meaning of accepted writes, receipts, generations, and exact outcomes |
| **Service cache** | physical storage and capacity for opaque service entries | service-specific validation, freshness, and failure meaning |
| **Local query evaluator** | indexing/evaluation algorithm and incremental strategy | query language meaning, ordering contract, and source/evidence isolation |
| **Router** | one read/write routing policy and its own input acquisition | explicit-route bypass, additive contribution identity, and exact route evidence |
| **Subscription planner** | safe grouping/coalescing and admission strategy | logical query meaning and exact attribution |
| **Transport** | relay connection implementation and session mechanics | route choice, durable retry, Nostr event admission, and receipt truth |
| **Publisher** | execution of one destination-specific publication attempt | retry scheduling, route selection, and durable write ownership |
| **Delivery policy** | attempt timing, fairness, ceiling, and ambiguity policy | exact transport/publisher facts and receipt identity |
| **Signer/crypto provider** | key custody and cryptographic execution | event composition, routing, persistence, and publication success |
| **Protocol service or crate** | NIP/service/event-kind meaning | generic engine lifecycle and primitive query/write paths |

Bundling several implementations into one distribution artifact MUST NOT merge these authorities.

---

# Part II — Live queries and reactive observation

## QUERY-001 — A live query is declarative and inspectable

A live query MUST state its selection, routing mode, source authority, relay access, freshness policy, cache-use policy, and result/acquisition bounds without embedding application callbacks or hidden side effects.

The query language MUST support:

- authors, ids, kinds, and tag values;
- exact event and address coordinates;
- a reactive current-account input;
- values projected from another query;
- union, intersection, and difference over derived values; and
- independently configured nested queries.

An empty derived set MUST mean “match nothing.” It MUST NOT erase a filter axis and widen the query.

Malformed query structure, unsupported nesting, empty combined queries, zero limits, incompatible bounds, and over-limit query structure MUST be refused before opening relay work.

**Acceptance:** malformed derived pubkeys or ids never reach relay filters or crash the engine.

## QUERY-002 — Equivalent queries have stable identity

Queries that describe the same selection, authority, and freshness behavior MUST be recognized as equivalent regardless of construction order.

Equivalent observations MAY share local evaluation, relay connections, and wire subscriptions. Distinct source authority, relay access, freshness, or presentation-relevant evidence MUST NOT be erased merely because the event filters are equal.

**Acceptance:** two equivalent handles share work; closing one does not close work still needed by the other.

## QUERY-003 — Opening is all-or-nothing

Opening a query MUST either:

- return a usable handle with a coherent initial local view; or
- return a typed refusal and leave no ownerless demand, partial dependency, or relay work.

Engine shutdown refusal and inability to read the initial local sources MUST remain distinguishable.

**Acceptance:** injected failure during open leaves existing queries unchanged and creates no leaked subscription.

## QUERY-004 — The initial view never waits on a relay

The initial query value MUST be produced from the configured local query sources without waiting for any relay response.

Depending on the selected product profile, those sources may include:

- a persistent event cache;
- an in-memory event cache;
- no retained relay-event cache;
- the write store's current local materializations; and
- other selected local query sources.

A persistent cache profile may return relay-observed events after restart. An ephemeral cache profile may return none. Both MUST return the local view they currently have without blocking on relay connectivity.

**Acceptance:** with every relay unreachable, opening a query returns its local view or a local-source error, never hangs waiting for the network.

## QUERY-005 — Local query state is the merge of independent sources

The application-facing event view MUST be the deterministic merge of all configured query sources.

At minimum, the standard publication-capable assembly merges:

- signed relay-observed events and relay evidence from the event cache; and
- current local unsigned or signed materializations and publication evidence from the write store.

The merge MUST:

- deduplicate the same event id;
- combine relay and local publication evidence;
- apply Nostr replacement and address rules;
- prefer the current local materialization when it supersedes a cached predecessor;
- allow the cached predecessor to become visible again when the local materialization is cancelled and still exists in another source;
- accept admitted live relay occurrences as current query input even when the selected event cache does not retain them; and
- never require copying an unsigned event into the event cache.

Caching is not a prerequisite for delivering a verified live event to an already-open query. A newly opened query later sees only what its configured local sources still retain.

**Acceptance:** a pending local replaceable event overlays a cached predecessor; cancelling it retracts the local event and naturally reveals the predecessor without compensating cache writes. With a null event cache, a verified live event still reaches the open query but is absent from a later newly opened query.

## QUERY-006 — Query updates carry every relevant change

A live query MUST update for:

- event additions;
- source removal or cache eviction;
- replaceable-event winner changes;
- valid deletion;
- expiry;
- replaceable-event rematerialization;
- local write cancellation;
- relay provenance changes;
- publication evidence changes;
- ordering changes; and
- source-scoped evidence or shortfall changes.

When a derived dependency shrinks, records that matched only through the removed values MUST be retracted from the same open query.

**Acceptance:** unfollowing one author removes only that author's records and remote demand while unrelated authors continue uninterrupted.

## QUERY-007 — Nested queries retain independent authority

An inner query and an outer query MUST each retain their own:

- routing/source selection;
- relay access;
- freshness policy;
- cache-use policy;
- evidence; and
- acquisition lifecycle.

An outer live query MUST NOT force a cache-only inner query to contact relays. A stale inner query MUST NOT cause unrelated outer branches to be reopened.

**Acceptance:** an event record observed outside the inner query's permitted sources contributes no derived value to the outer query.

## QUERY-007A — Derived references preserve useful relay evidence

When a query projects event, address, or pubkey references from another event, any valid relay hint carried by that reference—and any compatible relay evidence Fava has for the referenced entity—MAY contribute to automatic routing.

Such evidence remains scoped to the reference and MUST pass ordinary relay admission/safety rules. A hint cannot authorize a disallowed relay or become proof that the referenced content exists there.

**Acceptance:** a derived event reference can add a permitted hinted relay while a local/private relay rejected by admission contributes no route.

## QUERY-008 — Combined queries produce one result with scoped evidence

A combined query MUST deliver one deduplicated event view. An event matching several branches appears once.

A whole-query result bound applies to the combined result, not independently to each branch, unless a branch is an explicitly independent nested query.

Per-branch and per-relay evidence MUST remain associated with the branch and source that produced it.

**Acceptance:** overlapping branches deliver one event record while preserving each branch's separate EOSE/error/auth state.

## QUERY-009 — Fava never claims global completeness

Fava MUST expose the records it currently has and exact facts for relays and requests it actually used.

Fava MUST NOT expose or imply:

- global `synced`;
- global `complete`;
- percentage completeness;
- authoritative empty;
- end of global history; or
- proof that no matching event exists elsewhere.

An empty result from one relay/request means only that the source returned no matching events for that request.

**Acceptance:** waiting indefinitely on an empty query never produces a global-complete claim.

## QUERY-010 — EOSE is exact, source-scoped evidence

A relay is “finished sending stored events” for a request only after an actual EOSE for the exact current subscription/request identity.

Timeout, disconnect, retry exhaustion, silence, local cancellation, and relay refusal MUST remain distinct and MUST NOT be reinterpreted as EOSE or emptiness.

Reopening dropped demand MUST use fresh request identity so a late EOSE or event from the old request cannot settle the new one.

**Acceptance:** dropping and quickly reopening a request cannot inherit the old request's in-flight completion.

## QUERY-011 — Observation delivery is bounded and loss-honest

Current-state streams, including query results and diagnostics, MAY coalesce intermediate states. The next delivered state MUST be correctly rebased onto what the application actually received and MUST represent exact current state.

Causal streams, including receipt transitions, cancellation, signer completion, and lifecycle termination, MUST NOT silently lose facts. Any bounded loss MUST be explicit and typed.

Observation memory MUST remain bounded even when an application is slow.

**Acceptance:** a slow consumer under a burst eventually receives the exact latest event view within the declared memory bound.

## QUERY-012 — Pull cancellation and close are exact

For a pull-based observation surface:

- at most one `next` operation may be pending per handle;
- a second concurrent pull is refused without consuming data;
- cancelling a pending pull does not lose an undelivered update;
- an update delivered once is never delivered again;
- invalid acknowledge/cancel/close ordering is refused;
- closing wakes pending pulls promptly;
- repeated close is harmless; and
- shutdown ends all pending pulls without hanging.

Platform wrappers MUST preserve these outcomes through Swift and Kotlin cancellation semantics.

**Acceptance:** cancel-before-ready, cancel-after-ready, concurrent-next, close races, and engine shutdown each have exactly one deterministic result.

## QUERY-013 — Live demand begins at open

Opening a live-freshness query MUST contribute relay demand immediately. Relay work MUST NOT be deferred until the application first iterates or collects the result.

Cache-only queries contribute no relay work. Reiterating an already-open handle does not create another underlying query.

**Acceptance:** opening a live query and never iterating still opens the required relay work; opening cache-only opens none.

## QUERY-013A — Freshness policy is evaluated at open

A query's declared freshness policy is evaluated when the query opens against that query's own local source and source-scoped completion facts.

Opening one query MUST NOT trigger unrelated expiry sweeps, publication-retry sweeps, or re-evaluation of other already-open queries. Cache-only and maximum-age queries do not become implicit background polling loops.

**Acceptance:** opening an unrelated live query does not change the freshness decision or relay work of an already-open cache-only query.

## QUERY-014 — Routing knowledge may expand an open query asynchronously

For automatic routing, query work MUST begin against currently known destinations without waiting for all routers to settle.

Later router contributions MAY add relay work to the same query. A route contribution that disappears MAY withdraw relay work when no other router still contributes that destination.

Unchanged destinations and unrelated query branches MUST remain running.

**Acceptance:** two known author relays begin immediately while a third author's route is acquired; the third relay is added later without reopening the query handle.

## QUERY-015 — Reconnect restores active demand without promising missing history

When a relay session drops and reconnects, Fava MUST re-establish still-active logical demand with fresh session/request identity and without application resubscription.

This guarantees restoration of current demand, not gap-free retrieval of every event published during the outage. Applications express additional backfill explicitly.

**Acceptance:** a post-reconnect event reaches the already-open query with no app action; no global outage-backfill claim is emitted.

## QUERY-016 — App-authored time windows remain exact

Fava MUST preserve application-supplied `since`, `until`, and limit semantics.

Internal cache coverage or acquisition progress MUST NOT widen, narrow, erase, or reinterpret those bounds. Coverage may avoid redundant work only inside the exact scope it proves.

If cached data covered by a progress record is evicted, the cache implementation MUST lower or remove that progress claim consistently where it promises persisted coverage.

**Acceptance:** an all-time query is never accidentally floored by an unrelated cache watermark.

## QUERY-017 — Windowing remains part of the live-query problem

If the rewrite exposes growable/windowed acquisition, it MUST use the same live-query lifecycle rather than a parallel “history query” workload.

The acquisition window—what Fava should obtain and retain—MUST remain separate from the presentation window—what the UI currently renders.

The public windowing API and restart-resume token model remain product decisions. No application should infer pagination from an ordinary result limit.

---

# Part III — Event admission, state, and caches

## EVENT-001 — Relay input is untrusted until admitted

Before relay input can affect any cache, query, routing fact, or application result, Fava MUST:

- parse the relay frame under bounded input rules;
- attribute it to the exact current relay session and request where applicable;
- recompute the event id;
- verify the Schnorr signature;
- verify that the event is admissible for the request/context that carried it; and
- reject stale-session or off-filter input.

Malformed frames, invalid ids, invalid signatures, injected bytes, off-filter events, and invalid protocol sequencing MUST be scoped to the offending session/input and MUST NOT corrupt unrelated work.

**Acceptance:** a forged or off-filter event never enters any local source, route input, or application query.

## EVENT-002 — Nostr event-state rules are deterministic

Within the facts currently known to an assembly, event state MUST apply deterministic Nostr rules for:

- exact event-id deduplication;
- replaceable-event coordinates, including addressable coordinates;
- timestamp and event-id tie-breaking;
- same-author deletion;
- expiration;
- local materialization precedence; and
- provenance/evidence merging.

The same semantics MUST apply regardless of which event-cache or write-store implementation is selected.

**Acceptance:** two provider implementations produce the same event view for the same admitted event/source sequence.

## EVENT-003 — Relay provenance credits only actual service

An event record MUST name only relays that actually delivered that exact event occurrence.

A relay that was queried but did not return the event is not credited. The same event returned by several relays remains one event record with several source facts.

If a selected event cache advertises persistent provenance, those facts MUST survive ordinary restart. An ephemeral cache makes no such restart promise.

**Acceptance:** two relays serving the same event yield one record naming both; a bystander relay is absent.

## EVENT-004 — Event-cache guarantees are implementation/profile guarantees

Every event cache MUST provide a coherent current answer for the state it currently retains.

An event cache MAY be:

- memory-only;
- bounded and evicting;
- persistent;
- remote; or
- deliberately null.

The selected profile MUST document:

- whether relay-observed events survive restart;
- whether provenance survives restart;
- whether deletion tombstones survive restart;
- whether expiry is repaired after restart;
- whether coverage/progress facts are persisted;
- eviction behavior; and
- resource bounds.

Fava MUST NOT advertise a stronger cache guarantee than the selected implementation provides.

**Acceptance:** an ephemeral profile restarts with no cached relay events and does not claim otherwise; a persistent profile passes its declared restart corpus.

## EVENT-005 — Cache eviction is coherent

A bounded cache MAY forget events and evidence according to its declared policy.

It MUST NOT selectively retain mutually inconsistent positive and negative facts. For example, it cannot retain a stale event while forgetting a retained tombstone that invalidates it, or retain a coverage claim for data it evicted where its profile promises exact coverage semantics.

Cache eviction or reset MAY cause event retraction from open queries when no other source retains the event.

**Acceptance:** every eviction mutation produces a coherent source snapshot and exact query changes.

## EVENT-006 — Deletion and expiry retract current state

When Fava ingests or locally publishes a valid kind:5 deletion, it MUST apply it only to targets the author is permitted to delete and retract affected event records from current queries.

Fava does not automatically widen unrelated application queries merely to fetch deletion events. If a deletion is ingested, it is applied.

Expiration MUST retract events when their expiry becomes due. A persistent cache profile that advertises expiry recovery MUST catch up expired records after restart.

**Acceptance:** a valid deletion or expiry retracts the event through the ordinary query update path; a different author's deletion cannot remove it.

## EVENT-007 — Event-cache progress never becomes global truth

A persistent event cache MAY retain exact source-scoped completion or coverage facts to avoid redundant acquisition.

Such facts MUST remain keyed to the exact relay, request shape, relay access, and interval they prove. They MUST NOT become a global sync claim or override application-authored windows.

**Acceptance:** cache coverage can suppress redundant work against one relay without claiming another relay or the whole network is complete.

## EVENT-008 — Unpublished local events belong to the write store

Accepted local event materializations MUST be supplied to queries by the write store rather than requiring insertion into the event cache.

The event cache therefore stores signed relay-observed events according to its contract, while the write store owns local publication state.

This separation MUST preserve optimistic visibility, cancellation, rematerialization, relay echo, and restart behavior without a compensating cache transaction.

**Acceptance:** accepting, cancelling, and rematerializing a local event changes the write-store query source only; the event cache remains unchanged until a relay-observed signed event is admitted.

## EVENT-009 — Same-event contributions merge without duplication

When the event cache and write store both contribute the same signed event id, the query result MUST contain one event record combining:

- relay provenance from the event cache; and
- local receipt/signing/delivery evidence from the write store.

A relay echo does not create a second record or erase local publication identity.

**Acceptance:** a locally published event later served by two relays remains one event record with one local receipt and two relay sources.

## EVENT-010 — Service data has service-owned cache semantics

NIP-05 resolutions, NIP-11 documents, DNS results, HTTP metadata, and similar fetched data MUST NOT be treated as Nostr event-cache records.

Each service owns:

- key normalization;
- validation;
- freshness and staleness;
- positive and negative caching;
- conditional refresh where applicable;
- last-good versus last-error behavior; and
- service-specific typed failures.

A selected service cache provider stores opaque entries and does not reinterpret them.

**Acceptance:** NIP-05 and NIP-11 may share a physical fetch-cache provider while retaining independent freshness and failure semantics.

## EVENT-011 — Persistent-format ownership is local to each provider

Each persistent provider owns validation, versioning, supported migration, reset, and corruption refusal for its own bytes.

Fava does not require:

- a global assembly identity in persisted state;
- one event-cache implementation to open another's format;
- one write-store implementation to migrate another's private representation; or
- continuity when an application deliberately replaces a provider without a migration.

Unsupported or corrupt state MUST be refused explicitly by the provider that owns it. It MUST NOT be silently reset or reinterpreted.

**Acceptance:** opening a store through the wrong provider produces a typed refusal rather than invented compatibility.

## EVENT-012 — Destructive reset is exact

When a product profile exposes destructive reset, the operation MUST clear every configured local authority covered by that profile:

- event-cache state;
- write-store obligations and receipts;
- session/account state;
- signer state owned by Fava;
- service-cache entries; and
- provider-owned local metadata.

Ordinary restart, account switch, and logout are not destructive reset.

Reset MUST either complete across the configured profile or report exact partial failure; it MUST NOT report success after silently leaving sensitive or user-specific state behind.

## EVENT-013 — Ordinary storage errors remain operation errors

A failed cache or store operation MUST fail the operation honestly and leave no success fact for an uncommitted mutation.

Fava does not promise recovery from an irreparably failing disk or build a product-wide degraded-disk state machine. Ordinary crash/restart semantics remain required for providers that advertise persistence.

## EVENT-014 — One admitted event becomes visible as one coherent cache mutation

For one admitted relay event or one cache-owned removal, the event cache MUST expose the event value, relay evidence, replacement/address consequences, deletion/expiry consequences, indexes, and emitted query-source change as one coherent mutation.

A query MUST NOT observe a new event without the relay evidence committed with it, a new replaceable winner while the predecessor is simultaneously still current, or a retained deletion/expiry fact without the corresponding source retraction.

The physical mechanism is provider-specific. The observable mutation is atomic.

**Acceptance:** inject failure at each provider-defined mutation boundary and verify that a reader observes either the complete previous state or the complete next state, never a mixed state.

---

# Part IV — Event construction, replaceable-event edits, and publication

## WRITE-001 — Fava has one event-construction primitive

Fava MUST expose one general event-construction primitive that supports:

- author, kind, content, and creation time;
- validated Nostr tags; and
- unsigned event output suitable for the one signing/publication path.

Protocol crates own protocol meaning, calculate their exact tags, and compose
this primitive. The event builder does not know about replies, reactions,
reposts, quotes, follows, bookmarks, groups, or other event-kind semantics.

**Acceptance:** reply, reaction, repost, quote, and custom-kind protocol crates use the same primitive; removing protocol-specific methods from the event builder does not prevent any of them from constructing valid events.

## WRITE-002 — A write has one of three accepted forms

The publication lifecycle MUST accept:

1. an unsigned event whose `pubkey` already identifies the author;
2. a `ReplaceableEventEdit` that can produce an unsigned replacement from the latest event at its coordinate; or
3. a complete pre-signed event.

The accepted form determines the remaining work. It does not create separate publication or receipt systems.

The `fava` facade MUST expose the one application publication door:

```rust
fava.publish(payload)
fava.by(author).publish(edit)
fava.to(relays)?.publish(payload)
fava.by(author).to(relays)?.publish(edit)
```

`by(...)` and `to(...)` are inert scopes until `publish(...)` is called.
Successful `publish(...)` returns only after synchronous durable acceptance and
returns a `Write`. `Write` exposes stable write and receipt identity, current
receipt inspection, and asynchronous settlement through `settled(all())` or
`settled(at_least(n))`. Applications do not construct `WriteIntent`, receive
`AcceptedWrite`, or call a separate facade wait function.

## WRITE-003 — Authorship is carried by the event or replaceable-event edit

For an unsigned or signed event, the author is the event's `pubkey`.

Before a `ReplaceableEventEdit` has produced an event, the accepted write carries its resolved author. Every materialization MUST produce an event with that author as `pubkey`.

Current-account convenience APIs resolve the selected account before the write is accepted, and the resolved author is committed with it. A later account switch MUST NOT retarget accepted work.

No parallel author field may contradict the event or edit.

**Acceptance:** accept an unsigned event for Alice, switch current account to Bob, and verify that only Alice's signer can satisfy it.

## WRITE-004 — Acceptance is a durable write-store fact

An assembly exposing the standard durable publication contract MUST NOT return
the application `Write` until the write store has atomically committed:

- stable write and receipt identity;
- the accepted unsigned event, replaceable-event edit, or pre-signed event;
- the current materialization when one exists;
- current signing state;
- cancellation/supersession state required for recovery; and
- enough information to resume after ordinary process restart.

The accepted local materialization MUST be visible through the write-store query source before `Accepted` is returned.

If commit fails, no receipt, local event record, signer request, route session, or delivery work may remain.

The neutral owner records this boundary as `AcceptedWrite`; the facade projects
that accepted identity into the application `Write` without weakening the
boundary.

**Acceptance:** crash immediately after the application receives `Write`;
restart recovers one write and the same receipt without resubmission.

## WRITE-005 — Optimistic visibility comes from the write store

Every accepted materialized event MUST appear immediately in matching open and newly opened queries through the write-store source, whether signed or unsigned and whether online or offline.

Relay refusal does not delete the local event merely because delivery failed. Delivery evidence changes on the same event record.

Cancellation or replacement by a newer current event retracts or replaces the write-store contribution through the ordinary query update path.

**Acceptance:** two matching queries show the accepted event before any relay is contacted, with local publication evidence and no invented relay source.

## WRITE-006 — Replaceable-event edits survive source changes

A protocol crate may produce a `ReplaceableEventEdit` before the final event body is known, for example `Follow(Bob)` by Alice.

The write store MUST retain the edit and its resolved author independently from the current materialization. The protocol crate that defines the edit applies it to the best qualified source state and may apply it again when a newer qualified source appears. If no prior source event exists, it applies the edit to its defined empty state and produces the first event for that coordinate.

Distinct edits accepted for the same author/kind/identifier coordinate while
its current generation remains unsigned MUST compose as one durable ordered
edit sequence. Each accepted edit is applied exactly once in acceptance order;
the complete sequence is replayed when qualified source state changes and after
restart. Composition keeps the original write and receipt identity, advances
the exact materialization generation, retires prior generation evidence, and
refuses atomically when that evidence bound is exhausted. Protocol-specific
queues or batching are not part of this lifecycle.

Any pre-provider reservation MUST be bound to one exact
author/kind/identifier coordinate. At most one reservation may exist per
coordinate; reserved inactive coordinates count against the global active
bound, while an active coordinate's one reservation cannot grow that bound.
Only matching-coordinate acceptance consumes it. Before installing a
source-driven successor, publication MUST refresh custody for that exact
materialization generation and prove that the complete durable edit sequence,
not merely its final edit, produced the successor.

On restart, publication MUST reconcile each recovered sequence against the
initial qualified source snapshot before the facade can admit another edit for
that coordinate. A recovered runner that observes a receipt generation newer
than its loaded sequence MUST refresh exact-generation custody before
materialization, signing, or routing begins.

Rematerialization MUST:

- preserve unrelated source changes;
- replace the previous local materialization atomically within write-store authority;
- keep the same accepted operation and receipt identity;
- invalidate stale signer and delivery work for the old event generation;
- never expose a half-applied operation; and
- remain bounded to the affected replaceable-event coordinate.

**Acceptance:** accept two distinct pre-signature edits for one coordinate,
verify one write and receipt with an ordered composed successor, restart and
replay both edits over newer source state, and prove the first generation's late
completion cannot mutate the current generation.

## WRITE-007 — Signing is exact and identity-bound

A signer request for an unsigned event MUST identify the exact event body/id and its `pubkey`.

A signer completion is accepted only if it:

- belongs to the current event generation;
- signs the exact event body;
- matches the event's `pubkey`;
- passes signature verification; and
- belongs to the current signer/provider operation.

Unavailable, rejected, invalid-output, cancelled, timed-out, and stale signer results remain distinct.

**Acceptance:** a late signature for a superseded materialization cannot promote or publish it.

## WRITE-008 — A missing signer parks the write for its exact pubkey

If an accepted unsigned event has no available signer for its pubkey, it remains awaiting that signer without elapsed-time abandonment.

A signer for another pubkey cannot satisfy it. Attaching the exact signer to the running
Fava instance wakes the same accepted write and receipt without rebuilding the engine.
Removing or replacing an attachment cancels or detaches its in-flight operation; every
completion is rechecked against the exact current attachment generation before it may
change signing state. Restart causes a fresh signer request when the correct provider
becomes available; no durable “signer is still working” flag is assumed to survive
process death.

Explicit cancellation or protocol expiry may terminate the write.

## WRITE-009 — Signing without publishing is a separate supporting operation

An application MUST be able to submit an unsigned event for signing without creating a write intent, receipt, route session, or relay delivery.

The operation uses the same signer contracts and exact body/identity validation as publication.

## WRITE-010 — Pre-signed events are verified and preserved verbatim

A complete signed event may enter the publication path without re-signing.

Before custody, Fava MUST verify its event id and signature. Publication MUST preserve the exact signed event bytes/identity; routing and delivery MUST NOT mutate it.

## WRITE-011 — Routing has exactly automatic and explicit modes

Every query or write that requires relays selects either:

- **Automatic:** use the configured ordered router chain; or
- **Explicit:** use the exact non-empty relay set supplied for that operation.

Explicit routing bypasses automatic routers and remains verbatim. An empty explicit route is refused before signing or relay work.

No protocol-named routing mode appears in the universal application surface.

## WRITE-012 — Automatic routing is composed from independent routers

An assembly MAY configure several routers. A router may contribute to reads, writes, or both.

Routers execute in configured order. Each router receives the accumulated current plan from earlier routers and emits its own complete current contribution.

Contributions are additive. The routing core deduplicates identical relays while retaining all contribution reasons and target coverage.

A router does not remove another router's contribution. Admission or exclusion policy, if selected, is a separate responsibility.

**Acceptance:** outbox, hint, app-relay, and fallback policies can be selected, omitted, reordered, or replaced independently.

## WRITE-013 — Router results are immediate and asynchronous

Starting automatic routing MUST NOT wait for network acquisition.

Each router supplies an immediate current contribution and may later replace it as its knowledge changes. A slow router does not block destinations already contributed by another router.

Known route destinations become usable immediately. Later destinations update the same write receipt or open query.

**Acceptance:** a write p-tagging three pubkeys starts delivery to known relays for two recipients plus app relays while the third recipient remains unresolved; the later relay is added under the same receipt.

A router update may add or withdraw desired destinations. For a query, withdrawn destinations close when no router still contributes them. For a write, a not-yet-handed-off lane may be retired when no router still contributes it, while any possible or completed handoff remains exact historical evidence.

## WRITE-014 — Routers may acquire their own inputs through Fava primitives

A router owns the meaning and acquisition needs intrinsic to its algorithm. It may request ordinary local reads and explicitly-routed queries through Fava-provided services.

A router MUST NOT open private sockets, bypass event admission, own generic subscription grouping, or recursively invoke automatic routing for its own acquisition.

**Acceptance:** an outbox router obtains NIP-65 events from configured indexers through explicit query machinery and no separate transport stack.

## WRITE-015 — Routing knowledge distinguishes known, absent, and unresolved

For each relevant route target, routing MUST distinguish:

- known destinations;
- settled absence under the router's declared source plan; and
- unresolved knowledge.

Elapsed time MUST NOT convert unresolved knowledge into settled absence.

A route plan may be partially usable while unresolved needs remain. Route settlement means the selected router chain currently owes no further answer for that route revision; it does not mean delivery succeeded.

## WRITE-016 — Route preview is side-effect free

The application MUST be able to inspect the automatic router chain's current answer for a prospective query or write without signing, accepting a durable write, creating a receipt, or sending relay traffic.

Preview uses the same routing derivation as the real operation over currently available router snapshots. Unresolved knowledge is reported as unresolved rather than triggering hidden publication or acquisition.

## WRITE-017 — Partial routing uses one receipt

A multi-target write MUST deliver to currently known destinations while waiting for unresolved targets, under one receipt.

Later route contributions append newly required delivery lanes to the same write. A relay already covered or delivered is not duplicated when learned again for another target or reason.

Routing completion and delivery completion remain distinct.

## WRITE-018 — Per-destination delivery facts remain exact

For each selected destination, the receipt MUST preserve exact observable outcomes such as:

- awaiting route;
- awaiting signer;
- queued;
- attempted;
- acknowledged, including the relay's message;
- rejected, including the relay's message;
- authentication denied;
- backing off;
- given up;
- cancelled before handoff; and
- outcome unknown after ambiguous handoff.

The application MUST also be able to await one terminal result for the whole write without implementing its own reducer. Mixed outcomes remain visible rather than collapsing into a misleading boolean. The receipt SHOULD expose derived counts such as acknowledged destinations over total destinations so every application need not reimplement that arithmetic.

## WRITE-019 — Delivery retry is evidence-based and bounded

Route acquisition, signer availability, transport connection, relay authentication, and durable delivery attempts have separate owners.

Time spent offline, awaiting routing, awaiting signing, or awaiting auth MUST NOT count as a failed delivery attempt.

Once a destination is known and actual attempts repeatedly fail, the selected delivery policy MUST eventually stop under a finite declared bound and produce a per-destination terminal outcome.

Several writes for one relay SHOULD share connection/backoff ownership rather than creating independent reconnect storms.

## WRITE-020 — Ambiguous handoff follows the selected delivery contract

If Fava cannot determine whether event bytes reached the destination, it MUST record `OutcomeUnknown` for that exact attempt.

A durable delivery policy MAY retry the exact same signed event bytes, relying on stable event identity and relay idempotence. An at-most-once delivery policy MUST terminate the lane as unknown and not retry automatically.

The selected product profile MUST document which policy applies. Neither policy may claim the opposite fact.

## WRITE-021 — Replaceable writes retire obsolete active delivery

When a newer accepted event supersedes an older local replaceable event at the same coordinate, the older active delivery obligation MUST be retired promptly.

If Fava can prove no bytes were handed off, obsolete delivery state may be removed. If handoff may have happened, bounded explanatory evidence remains.

Cancelling the newer write does not resurrect an obsolete prior local obligation. A cached remote predecessor may reappear only because it remains a valid contribution from another query source.

## WRITE-022 — Corrected replaceable generations preserve relevant destinations

When a replaceable-event edit produces a corrected successor generation, the successor's destination set MUST include:

- current automatic or explicit routing; and
- destinations that require correction because they may have received the predecessor.

Acknowledgement of one event generation cannot settle another generation.

## WRITE-023 — Cancellation and receipt removal are separate

An application may cancel an accepted write while Fava can still prove that no event bytes have been handed to transport for any destination whose obligation cancellation would erase.

Cancellation MUST:

- terminate current signer/route/delivery work;
- retract the current write-store event contribution where appropriate;
- preserve exact historical evidence that cannot be erased; and
- produce a cancelled receipt outcome.

Removing a retained terminal receipt is a separate operation. Removal while work is still active is refused.

The exact boundary for cancelling a write after partial handoff remains an explicit product decision; implementations MUST NOT invent silent “unsend” semantics.

## WRITE-024 — Write inspection is bounded

The application MUST be able to:

- reattach to a receipt by stable receipt identity;
- page through active and retained writes without loading all history;
- inspect active writes for an exact event id; and
- obtain one terminal result after restart.

Completed receipt retention MUST be bounded under one declared policy. Eviction removes only evidence exclusively owned by the evicted receipt and never active work.

## WRITE-025 — Relay echoes enrich rather than replace local state

When a relay serves an event already contributed by the write store, Fava MUST merge relay provenance and delivery evidence into the same event record.

The relay echo MUST NOT create a duplicate, erase the receipt, or require rewriting an unsigned placeholder in the event cache.

## WRITE-026 — Terminal delivery does not itself retract the local event

Acknowledgement, rejection, authentication denial, or give-up changes publication evidence. It does not by itself remove the locally accepted event from matching queries.

A local event is retracted only by cancellation where allowed, replacement by a newer current event, valid deletion, expiration, destructive reset, or a documented local-publication retention policy.

**Acceptance:** an event remains in the local query after every destination rejects it, with refusal evidence attached.

## WRITE-027 — Settled empty routing is explicit

If the selected automatic router chain settles with no destination, the write MUST expose a typed no-destination outcome naming the unresolved/absent route reasons that led there.

The write MUST NOT silently disappear, substitute an unconfigured relay, or treat an indexer/discovery relay as a generic destination.

If the route remains unresolved, it stays open rather than becoming no-destination merely because time passed.

## WRITE-028 — Automatic routes are re-evaluated while work remains open

Automatic routing remains a live strategy for the write while destination work is unresolved or new route knowledge can create required lanes. Relevant router changes, restart recovery, signer availability, and reconnect/queue drain MUST use current router snapshots rather than a relay list frozen at composition time.

Explicit routes remain fixed. Existing acknowledged destinations are not resent merely because routing was recomputed, except as required for a new corrected event generation.

## WRITE-029 — Write recovery is complete before new work is admitted

A persistent write store MUST recover its open obligations, receipts, current materializations, routes, and delivery state before the engine admits new commands that could conflict with them. For semantic writes, engine recovery includes applying the complete durable edit sequence to the initial qualified source snapshot; starting a background runner is not completion of this admission barrier.

Recovery work SHOULD scale with current open obligations and bounded retained evidence rather than the total historical number of completed attempts. Repeated superseding writes to one replaceable coordinate MUST recover as bounded current work rather than one active obligation per historical renewal. A live same-coordinate semantic edit sequence MUST recover in its exact accepted order under one stable write and receipt identity; its length is bounded by retained retired-generation evidence.

Every recovered runner MUST bind its loaded sequence to the exact current
materialization generation. If same-coordinate admission advances the receipt
before that runner initializes, it MUST reload the newer complete sequence
before opening signer or route work or reacting to later source state.

Unchanged recovered state MUST NOT require rewriting every record merely to reopen.

---

## WRITE-030 — Already-expired events are refused before custody

A complete event whose NIP-40 expiration is already in the past MUST be refused before durable acceptance, receipt allocation, optimistic query visibility, signing, routing, or relay work.

A replaceable-event edit that can produce only an already-expired event likewise cannot become an active publication obligation.

**Acceptance:** submit an already-expired unsigned event, pre-signed event, and event produced from a replaceable-event edit; verify zero write-store residue and zero provider work.

# Part V — Relay planning, transport, authentication, and protocol services

## RELAY-001 — Fava contacts only justified relays

Fava MUST open or retain a relay session only while current query, routing acquisition, authentication, publication, explicit application configuration, or selected service work requires it.

Every contacted relay MUST be explainable by current demand. Bystander relays receive no connection attempt.

## RELAY-002 — Subscription planning is separate from routing

Routing determines which relays should receive logical demand.

The selected subscription planner maps all logical demand for one relay session into semantically equivalent wire subscriptions. It may deduplicate or coalesce demand only when equivalence is proven.

The planner MUST preserve attribution from every wire request back to the logical queries it serves.

## RELAY-003 — Subscription grouping cannot change meaning

A planner MAY merge compatible filters that differ in one safely unionable dimension.

It MUST NOT merge across differences that would change meaning, including incompatible time windows, relay-side limits, relay access, physical sessions, or combinations whose local refiltering cannot reproduce the original result/evidence.

**Acceptance:** 300 compatible tag-value queries may share one wire request while each logical query retains exact matching and evidence.

## RELAY-004 — Relay limits produce exact plans or shortfall

When fresh NIP-11 information advertises read/write limits that Fava can interpret deterministically, planning and publication MUST either honor them or surface exact source-scoped shortfall.

This includes, where applicable:

- maximum subscriptions;
- maximum message length;
- subscription-id length;
- maximum/default filter limits;
- event size/tag/content constraints; and
- proof-of-work or write restrictions that can be evaluated locally.

Fava MUST NOT silently truncate, clamp, collide identifiers, or claim omitted work was completed.

Missing, stale, malformed, or unsupported claims remain unknown rather than becoming invented defaults.

## RELAY-005 — Transport owns sessions and byte handoff

Transport owns:

- connection establishment;
- physical session identity;
- reconnect/backoff;
- frame I/O;
- cancellation;
- shutdown;
- exact write-handoff facts; and
- connection-scoped errors.

Transport MUST NOT decide query meaning, route policy, durable retry, Nostr event state, or publication success beyond the facts it directly observes.

## RELAY-006 — Reconnect uses fresh generation identity

Every reconnect creates fresh session/request identity. Frames and completions from earlier generations cannot mutate current work.

Active logical demand is replayed automatically after reconnect. Work incomplete on the old session starts again under the new session rather than being spliced into one false lifecycle.

## RELAY-007 — NIP-42 authentication is explicit and isolated

Relay authentication context is distinct from:

- event author;
- current account;
- query authors;
- signer selection; and
- routing.

The application supplies an auth policy for exact relay access. Fava answers challenges, supports challenge timing before or after a request, and re-authenticates after reconnect.

If the application declines authentication for a publication, that destination terminates with an auth-denied outcome while unrelated accounts and destinations continue independently.

## RELAY-008 — Relay rejection text remains verbatim evidence

Relay `OK`, `CLOSED`, and other user-relevant messages MUST be preserved exactly enough for the application to report what the relay said. Fava MUST NOT replace relay text with an invented generic explanation when the exact message is available.

## RELAY-009 — NIP-11 is a service with independent cache semantics

Fava MUST provide a relay-information service capable of acquiring, validating, caching, and returning NIP-11 documents with exact freshness and acquisition status.

The service owns:

- HTTP acquisition;
- validation and typed projection;
- single-flight behavior;
- freshness/staleness;
- last-good document separately from last acquisition error; and
- use of the selected service cache.

NIP-11 documents are not event-cache entries.

## RELAY-010 — NIP-05 is an optional service with independent cache semantics

When selected, the NIP-05 service MUST resolve a NIP-05 identifier into its scoped result and expose exact freshness and failure status.

The service owns positive/negative cache policy, validation, refresh, and typed errors. A cached NIP-05 result is HTTP-derived resolution evidence, not a cryptographic proof or a Nostr event.

NIP-05 cache policy remains independent from NIP-11 and event-cache policy.

## RELAY-011 — Ordinary reads use explicit Nostr requests

Ordinary relay reads use standard Nostr request/subscription semantics.

Fava MUST NOT automatically introduce negentropy or another set-reconciliation protocol during open, restart, or reconnect. Such a protocol may exist only as an explicit application-selected protocol implementation with its own observable contract.

## RELAY-012 — Hostile relay behavior remains scoped

A relay may stall, never send EOSE, silently cap subscriptions, challenge authentication mid-stream, send EOSE then more events, send CLOSED mid-subscription, return off-filter events, truncate frames, inject bytes, acknowledge without later serving, or disconnect after handoff.

Fava MUST keep these outcomes scoped to the exact relay/session/request and must not wedge unrelated work or fabricate stronger facts.

## ROUTER-001 — The standard outbox router is an independent policy

When selected, the standard outbox router derives read and write destinations from NIP-65 relay-list events and its configured discovery/indexer sources.

It may contribute:

- author read/write relays according to the operation;
- p-tagged recipient inbox/read relays for publication;
- unresolved needs while relay-list acquisition remains open; and
- settled absence only after its exact configured source plan settles.

The router shares/coalesces identical discovery needs across queries and writes and releases acquisition when nothing needs it. Confirmed absence is not treated as a durable signed Nostr fact and is re-evaluated according to the router's declared restart policy.

With no configured discovery source and no retained relay-list event, the router reports unknown rather than inventing absence.

## ROUTER-002 — The standard hint router is an independent policy

When selected, the hint router contributes permitted relays from pointer-like event/address/pubkey references and compatible event evidence.

It does not define generic routing, fallback policy, NIP-65 discovery, or relay admission. A copied or malformed hint is not upgraded into stronger evidence than it carries.

## ROUTER-003 — The app-relay router is an independent policy

When selected, the app-relay router contributes its configured relays to the read/write operation classes it is configured to cover. It does so regardless of whether earlier routers already provide destinations.

## ROUTER-004 — The fallback-relay router is an independent reactive policy

When selected, the fallback router observes the accumulated live plan from earlier routers and contributes configured relays when its documented coverage rule is not satisfied.

Its policy defines whether coverage is measured per recipient, author, reference, or whole request; whether unresolved targets receive immediate fallback; and whether it applies to reads, writes, or both.

When upstream coverage changes, the fallback router recomputes its complete contribution. It remains independently selectable from the app-relay router.

---

# Part VI — Identity, sessions, signers, and cryptographic operations

## ID-001 — Session state and accepted-write state are separate

A session contains accounts, current-account selection, and attached signer/crypto provider configuration.

Signer attachment is mutable while Fava is running. Exactly one signer may be attached
per public key; add, explicit replace, and remove operations are bounded to 64 attached
public keys and refuse atomically. Builder-supplied signers seed this same runtime
session rather than a publication-owned signer map.

Accepted writes remain owned by the write store and are not rewritten when session state changes.

Removing an account or logging out does not delete cached public events, accepted writes, or receipts.

## ID-002 — Current account is a reactive input

Queries that explicitly depend on the current account MUST update when the current account changes.

Write convenience APIs that use the current account resolve it before producing the accepted unsigned event or replaceable-event edit. Accepted work does not follow later account changes.

## ID-003 — Missing identity is refused before acceptance

If a convenience publication operation requires a current account and none exists, and no explicit author public key is supplied, the operation MUST fail before creating a write or receipt.

A low-level unsigned event already carries its pubkey and does not require current-account resolution.

## ID-004 — Identity inputs are unambiguous

Internal boundaries use raw protocol identity values, not human-facing bech32 text.

An application may decode `npub`, `nprofile`, or other presentation forms at its input boundary. Fava MUST refuse the wrong identity shape rather than silently reinterpret it where a raw pubkey is required.

## ID-005 — Session restore is all-or-nothing

Restoring an application-saved session MUST reconstruct every account, signer-backed account, pubkey-only account, and current selection as one operation.

If one required provider configuration cannot be understood or restored, restore is refused and no partial session becomes active.

The application owns persistence of the opaque session representation unless the selected platform profile specifies another owner.

## ID-006 — Signer providers preserve key custody

Applications MUST be able to supply local, remote, hardware, extension, or other signer implementations without giving Fava raw private-key bytes when that provider does not require it.

Signer providers expose named pubkeys, availability, exact signing operations, cancellation, and typed outcomes. They do not own routing, event composition, receipt progression, or transport success.

## ID-007 — Signing, encryption, and decryption are separate operations

A provider may implement one or several cryptographic operations, but the contracts and outcomes remain distinct.

NIP-44 and legacy NIP-04 support, where selected, MUST preserve exact account, source, operation, and request identity. Unavailable, rejected, invalid ciphertext, malformed plaintext, cancellation, and stale completion remain distinct.

## ID-008 — Secret material does not enter generic state

Private keys, decrypted plaintext, authentication payloads, and equivalent secret material MUST NOT enter generic event records, diagnostics, logs, debug formatting, persistent caches, or unrelated callbacks.

Each secret owner defines its exact lifetime and cleanup boundary. A stale completion is discarded without exposing its payload.

---

# Part VII — Protocol crates and composition

## PROTO-001 — Event-kind meaning lives outside the core

The universal query, event-state, routing, publication, and runtime owners MUST NOT branch on event-kind meaning owned by protocol crates.

Adding protocol crate N+1 requires only that crate and a product assembly change, not edits to the Fava facade or universal owners.

## PROTO-002 — Protocol crates compose universal primitives

A protocol crate may provide:

- typed decoding;
- query fragments;
- event construction helpers;
- replaceable-event edits;
- validation;
- presentation-neutral parsed values; and
- edit application for replaceable events.

It uses the one event builder, query model, signer path, router chain, publisher, receipt lifecycle, and write store.

## PROTO-003 — Replaceable-event edits express the intended change

For replaceable events, a protocol crate SHOULD expose edits such as:

- follow / unfollow;
- bookmark / unbookmark;
- add relay / remove relay;
- add media server / remove media server.

The protocol crate owns how its edit applies to the event coordinate. The write store owns durable custody, materialization generations, signing, routing, delivery, and receipts.

For NIP-02, `contact_list(authors)` and `followers_of(subject)` are ordinary
kind-3 `Query` values, while `follows_of(snapshot)` is a typed snapshot
projection.
`ContactList` accounts for every `p` row in source order. Valid rows expose
typed pubkey, relay-hint, and UTF-8 petname fields; malformed, duplicate, or
uninterpreted `p` rows expose exact typed row evidence. Empty lists are valid.
Edits preserve first-occurrence order,
malformed rows, unknown rows, and extensions such as `t` tags while changing
only the targeted follow relationship. Invalid pubkeys or relay hints are
evidence, not a reason to discard the containing event.

## PROTO-004 — Raw Nostr remains expressible

Applications MUST be able to:

- query arbitrary and future event kinds;
- construct events with validated raw tags;
- publish pre-signed events;
- use explicit relay routes; and
- inspect undecoded event content.

Typed protocol crates add ergonomics and safety. They do not make unknown Nostr inexpressible.

## PROTO-005 — Protocol crates own reference-tag meaning

When constructing a pointer-like relationship to an event or address, the owning protocol crate MUST derive appropriate markers, author hints, and usable relay hints from the target's own thread position and trusted evidence, then supply validated tags to the general event builder.

Reply, reaction, repost, quote, and comment crates each own their exact protocol tagging. Non-pointer semantics such as list entries or deletion targets are not forced through pointer tagging.

## PROTO-006 — `fava-simple-groups` preserves multi-relay simple group truth

`fava-simple-groups` MUST expose a pure `SimpleGroup` value over one opaque
NIP-29 simple group id and an application-selected non-empty, bounded set of
host relays. One host is the ordinary case; several hosts are a required
application aggregation for independently authoritative relay-local forks.

Simple group content reads MUST add the exact `h` constraint and ask the
complete host set through an ordinary `Query`. Relay-authored
simple-group-record reads MUST add the exact `d` constraint, retain actual
per-host relay evidence, and expose record disagreement rather than
field-merging it or silently selecting a winner. The same event id served by
several selected hosts appears once with every actual serving-relay
contribution.

Simple group publication MUST prepare the exact simple group context without
restricting the carried event to a fixed set of event kinds. The application
then publishes the prepared payload through
`fava.to(simple_group.hosts()).publish(payload)`, which gives the universal
publication owner the complete selected host set as its exact explicit route.
Custom event kinds MUST use the same path. A pre-signed event is verified
unchanged and MUST already carry the exact simple group context; adding
routing cannot rewrite its tags.

The capability MUST return ordinary `Query`, event, or
`ReplaceableEventEdit` values and MUST NOT own a socket, observation, signer,
store, delivery, retry, or receipt lifecycle. It MUST provide typed bounded
parsing/projection for NIP-29 records and saved rows so ordinary applications do
not decode raw tags. Discovery remains declarative and makes no relay-global
completeness, existence, membership-absence, or canonical-fork claim.

## PROTO-007 — NIP-25 reaction construction refuses ambiguous content

The NIP-25 reaction crate MUST refuse content forms that would silently acquire a different protocol meaning than the caller intended, including empty reaction content and shortcode-shaped custom emoji when the required custom-emoji tagging is not supplied.

Refusal happens before write acceptance.

## PROTO-008 — NIP-09 deletion remains a protocol write

Publishing a deletion event creates a new signed/routed write intent. It is not the mechanism for cancelling an unsent local obligation.

Applying an ingested deletion and cancelling local work remain separate operations with separate evidence.

## PROTO-009 — Content parsing is shared but presentation-neutral

When selected, the content parser converts event content into structured blocks, inline spans, and unresolved references.

It MUST NOT render UI or automatically resolve references into fetched event/profile state. Rendering and resolution policy remain application-owned.

## PROTO-010 — Initial protocol inventory is explicit

The rewrite program SHOULD explicitly classify each intended protocol service or crate as required, optional, deferred, or application-owned rather than letting examples imply support.

The initial inventory to classify includes:

### Universal relay/protocol mechanisms

- NIP-01 event and relay messages;
- NIP-09 deletion application;
- NIP-11 relay information;
- NIP-19 presentation codecs where useful;
- NIP-40 expiration;
- NIP-42 relay authentication; and
- explicit opt-in reconciliation implementation, if retained.

### Supporting services

- NIP-05 resolution;
- content parsing;
- local signer;
- NIP-46 remote signer;
- NIP-44 encryption/decryption;
- legacy NIP-04 decryption where required by selected protocol crates; and
- Blossom/NIP-96-style asset services where selected.

### Independently selectable event-kind protocol crates

- NIP-02 follows;
- NIP-18 reposts;
- NIP-22 comments;
- NIP-25 reactions;
- NIP-29 groups;
- NIP-51 concepts as separate protocol crates rather than one list mega-crate;
- NIP-65 relay-list semantics;
- NIP-73 external identifiers;
- NIP-C7 chat; and
- product-specific protocol crates supplied by applications.

---

# Part VIII — Diagnostics, boundedness, testing, platforms, and lifecycle

## OPS-001 — Diagnostics report facts, not a health score

Diagnostics MUST expose bounded, queryable facts about:

- current relay sessions and their reasons;
- query demand and shortfall;
- routing contributions and unresolved needs;
- write obligations and per-destination state;
- authentication state;
- provider availability/failure;
- cache/profile status; and
- resource limits or explicit loss.

Diagnostics MUST NOT synthesize a global sync score, completeness percentage, or invented root-cause fact.

## OPS-002 — Diagnostics delivery is lazy and coalesced

Diagnostics current-state observations MAY coalesce bursts into one exact latest snapshot.

Opening diagnostics mid-burst returns current truth rather than replaying stale intermediates. With no diagnostics observer, Fava SHOULD avoid constructing expensive presentation snapshots.

## OPS-003 — Stalled writes are visible under one classification

The application MUST be able to inspect every currently stuck write independently of individual receipt streams.

A write may be classified, at minimum, as:

- unroutable/unresolved;
- unsignable; or
- undeliverable after routing/signing.

Elapsed stuck time is evidence for presentation and policy; it does not by itself convert unresolved routing or signer availability into failure.

## OPS-004 — Every externally influenced resource is bounded

Fava MUST define bounds or explicit backpressure/refusal for:

- query structure and derived values;
- router contributions and route fan-out;
- active relay sessions;
- wire subscriptions;
- frame and message sizes;
- event-cache memory where bounded;
- write-store active work and retained receipts;
- provider operations;
- observation delivery;
- diagnostics;
- fetched service entries; and
- platform bridge queues.

Exceeding a bound MUST produce refusal, backpressure, or exact shortfall. It MUST NOT silently discard work while claiming success.

## OPS-005 — Application-facing test infrastructure is part of the product

Fava MUST ship supported test facilities allowing consuming applications and provider authors to exercise:

- deterministic time and expiry;
- scripted relay frames and protocol misbehavior;
- connection failure and reconnect;
- EOSE, silence, CLOSED, auth, and relay limits;
- signer delay, refusal, invalid output, and human approval;
- event-cache and write-store restart behavior;
- cancellation races;
- exact route destinations and router updates;
- per-relay publication outcomes;
- provider substitution; and
- platform lifecycle behavior where feasible.

A test must be able to prove the mechanism it claims by disabling or mutating that mechanism and observing failure.

## OPS-006 — Rust, Swift, and Kotlin preserve behavior

For the same assembled product profile, direct Rust, Swift, and Kotlin applications MUST observe equivalent:

- event records and evidence;
- query changes;
- receipt facts and terminal outcomes;
- routing and shortfall;
- session behavior;
- typed errors;
- cancellation; and
- restart semantics.

Platform idioms may differ. Behavioral meaning may not.

## OPS-007 — Parity is structurally checked

Fava MUST maintain a real inventory of public operations and values for each supported platform and executable cross-SDK behavior tests.

Heuristic source-word matching is insufficient. Removing a supported operation or changing a reachable outcome on one platform must fail qualification.

## OPS-008 — Native artifacts are ordinary external dependencies

An iOS or Android application MUST consume its selected Fava artifact without repository-relative source paths, raw generated bindings, or direct native-library loading.

The artifact MUST document the selected profile, supported ABI/platform set, and public surface. Unselected protocol crates and providers MUST not appear as callable API.

This build metadata is not a durable store compatibility identity.

## OPS-009 — Lifecycle and teardown are deterministic

Opening, observing, cancelling, dropping, closing, backgrounding, foregrounding, and engine shutdown MUST each have one exact owner.

Pending work wakes promptly on cancellation or close. No event, receipt fact, callback, or provider completion may be delivered after terminal close.

Repeated close is harmless. Repeated open/observe/cancel/close cycles return resources to a stable baseline. Ordinary observation does not allocate one operating-system thread per query.

## OPS-010 — Mobile process and suspension claims require real proof

The iOS and Android profiles MUST be exercised in real platform processes through their public artifacts.

For an iOS profile claiming suspension transparency, foreground resume after sockets become unusable MUST reconnect, re-authenticate, and restore active demand without application scene-phase code or engine reconstruction.

For an Android profile claiming persistent local state, a fresh process MUST reopen the selected event-cache/write-store profile and reproduce its declared behavior offline.

Simulator, desktop-JVM, compile-only, and archive inspection do not prove physical-device lifecycle behavior.

## OPS-011 — Performance claims are profile-specific and measured

A product profile SHOULD publish measured bounds for:

- first local query result;
- first relay result;
- event ingest throughput;
- active/idle observation cost;
- thread growth;
- memory retention;
- write recovery; and
- teardown.

Performance qualification must use the production path and report tradeoffs of the selected providers rather than treating one backend's numbers as universal Fava behavior.

---

# Part IX — Product profiles and declared guarantees

## PROFILE-001 — Every profile declares its guarantees

A product profile MUST document which guarantees arise from its selected implementations.

At minimum it states:

- whether relay-observed events survive restart;
- whether provenance, tombstones, expiry, and coverage survive restart;
- event-cache eviction policy;
- write-store durability and recovery;
- service-cache persistence;
- selected routers and order;
- fallback/app-relay policy;
- delivery retry and ambiguity policy;
- supported protocol crates and services; and
- supported platform artifacts.

## PROFILE-002 — A persistent full-client profile provides offline reuse

A profile selecting a persistent event cache MAY advertise:

- cached relay-event reads after restart;
- persisted relay provenance;
- persistent deletion suppression;
- expiry repair after restart;
- source-scoped coverage/progress reuse; and
- indexed local query performance.

It MUST pass the declared restart and corruption/refusal corpus for its selected cache implementation.

## PROFILE-003 — An ephemeral event-cache profile remains valid

A profile selecting a memory, bounded, or null event cache MAY advertise only current-process cache behavior.

Such a profile:

- may open with no cached relay events;
- may reacquire relay data;
- may retract evicted records absent from other sources;
- starts after restart with no retained relay-event cache unless another selected source provides records; and
- MUST NOT claim persistent provenance, tombstones, coverage, or cold event reuse.

Accepted local writes remain visible through the write store independently of event-cache retention.

## PROFILE-004 — Durable write custody remains the standard write contract

A production profile exposing Fava publication MUST select a write store that
preserves accepted obligations and receipts across ordinary crash/restart.

Memory write stores may exist for deterministic tests or deliberately non-production profiles, but they do not satisfy the standard durable-write product claim.

## PROFILE-005 — Routing policy is selected by router composition

Fava supplies independently selectable router implementations, including standard outbox, hints, app-relay, and fallback-relay policies.

The application chooses which routers exist and their order. Fava does not centrally impose that app-relay and fallback-relay policy must be combined.

When both are selected, configured order and each router's documented contribution semantics define the result.

## PROFILE-006 — The standard provider distribution has no privilege

Fava MAY publish a recommended full-client assembly, but its implementations use the same contracts and conformance suites as external providers.

Applications can assemble smaller or different products without editing the universal core.

## PROFILE-007 — The recommended full-client assembly is explicit

A recommended full-client distribution SHOULD name its complete selected profile rather than relying on hidden facade defaults. A plausible initial profile includes:

- one persistent event cache selected and qualified for the target platform;
- one durable write store;
- one bounded or persistent service-cache provider;
- the standard query evaluator;
- outbox and hint routers;
- either app-relay or fallback-relay policy when configured by the application;
- the standard exact subscription planner;
- WebSocket transport;
- NIP-01 publisher;
- standard bounded delivery policy;
- NIP-11 service and, when selected, NIP-05 service; and
- selected signer and event-kind protocol crates.

This is a convenience assembly, not universal core behavior.

## PROFILE-008 — Standard terminal receipt retention is shared and oldest-first

The standard durable write-store profile applies one bounded terminal-receipt retention policy across acknowledged, rejected, cancelled, given-up, unknown, superseded, and no-destination outcomes.

When the bound is exceeded, the oldest terminal receipt is retired first. Active writes are never evicted by this policy, and retirement removes only facts exclusively owned by the retired receipt.

A custom write-store profile MAY choose another declared bounded policy while preserving active work and exact retained evidence.

---

# Part X — Explicit non-requirements

The rewrite does not promise the following:

1. **Application framework ownership.** Fava does not own product UI, navigation, ranking, or moderation policy.
2. **Global completeness.** No global synced/complete/authoritative-empty contract exists.
3. **Gap-free history plus live delivery.** Applications request the backfill they need.
4. **Automatic negentropy.** Set reconciliation is explicit opt-in protocol work, if supplied.
5. **A core catalog of event-kind meaning.** Event-kind meaning remains in independent protocol crates.
6. **Parallel primitive paths.** Protocol crates do not invent their own query, signer, publisher, receipt, or route lifecycle.
7. **Silent truncation.** Bounds become refusal, backpressure, or shortfall.
8. **A hardcoded public relay fallback.** Relay policy comes from selected routers and explicit application configuration.
9. **Runtime provider hot swapping.** Build-time composition is the required model.
10. **Persistence compatibility between unrelated provider implementations.** Each provider owns its bytes and migrations.
11. **A universal persistent event replica.** Event-cache restart guarantees are profile-specific.
12. **Copying unsigned local events into the event cache.** Local publication state is supplied by the write store.
13. **Deletion as cancellation.** A kind:5 event is a new write; local cancellation is separate.
14. **App-owned reconnect or hidden duplicate retry loops.** Fava/provider owners manage their lifecycles.
15. **Recovery from an irreparably failing disk.** Providers report failed operations and ordinary crash recovery honestly.
16. **UI scroll-position persistence.** Applications restore presentation state.
17. **Parity by documentation alone.** Supported platform claims require executable evidence.
18. **Service data treated as Nostr events.** NIP-05/NIP-11 and similar fetched data retain service-owned cache semantics.

---

# Part XI — Open product decisions

The following decisions remain explicit rather than being silently resolved by implementation:

## OPEN-001 — Public windowing API

The target mental model is a growable acquisition window on the existing live-query lifecycle, separate from UI presentation. The exact public API, cursor semantics, and restart-resume behavior require an owner decision before being promised.

## OPEN-002 — Cancellation after partial handoff

Pre-handoff cancellation is required. The exact application operation, if any, for stopping remaining unsent destinations after some destinations may already have received bytes must be decided explicitly. No implementation may claim to “unsend” bytes already handed off.

## OPEN-003 — Outage backfill scope

Reconnect must restore active demand. Whether a selected profile additionally promises retrieval of events published strictly during the disconnected interval depends on explicit backfill behavior and must not be inferred from reconnect alone.

## OPEN-004 — Full delivery-history retention

Receipts and current outcomes are required and bounded. Whether a production profile exposes or persists every historical attempt detail beyond what is needed for exact current outcomes and ambiguity evidence remains a product decision.

## OPEN-005 — Recommended persistent event-cache profile

The architecture supports persistent and ephemeral event caches. The project should explicitly choose which cache guarantee profile is recommended for the primary shipped client artifact rather than letting a provider default imply product scope.

---

# Part XII — Definition of a conforming rewrite

A rewrite is conforming only when all of the following are true:

- Fava remains an embeddable library with live queries and write intents as its primary workload model.
- Applications can select provider and protocol crates at build time without forking Fava.
- Every replaceable contract has a public conformance kit and no default-provider privilege.
- Live queries open atomically, return an immediate local view, and remain coherent across additions, removals, provenance changes, replaceable-event rematerialization, and routing changes.
- Local query state deterministically merges event-cache and write-store contributions without copying unsigned events into the event cache.
- Event-cache persistence and retention claims match the selected implementation/profile exactly.
- Accepted production writes are durable before `Accepted`, visible through the write-store query source, reattachable by receipt, and recoverable after ordinary restart.
- Event authorship comes from the event `pubkey` or the author resolved when a replaceable-event edit was accepted, with no contradictory parallel authority.
- Automatic routing composes independently selectable routers, yields immediate partial results, and expands asynchronously without blocking known work.
- Explicit routing bypasses automatic routers and remains exact.
- Routing, subscription planning, transport, publication attempts, and delivery policy remain separate responsibilities.
- Relay input is verified and attributed before it can influence local state, routing, or applications.
- Per-relay and per-destination facts remain exact; ignorance never becomes completion, success, failure, or absence.
- NIP-05, NIP-11, and other fetched services own their cache semantics independently from the event cache.
- Event-kind protocol crates compose universal primitives and adding one does not edit the universal core.
- All externally influenced queues and collections are bounded or explicitly backpressured/refused.
- Rust, Swift, and Kotlin preserve behavior for the same assembled product profile.
- Native artifacts are tested as external dependencies in real platform processes.
- Application-facing test infrastructure can falsify relay, routing, signer, cache, write-store, restart, cancellation, overload, and lifecycle claims through public APIs.
- The engine remains understandable as a set of focused owners rather than a monolith with hidden parallel authorities.

---

# Appendix A — Traceability to the archaeology requirement families

This specification consolidates the original requirement corpus as follows:

| New section | Original requirement families primarily covered |
|---|---|
| Product/composition goals | purpose/scope, design posture, QD-006, ID-007, NEW-N, NEW-O, provider-replaceability rulings |
| Live queries | LQ-001..007, QD-001..005, HIST-003..010, OFF-001..003, CANCEL-001..007/009, PARITY-007 |
| Event admission and caches | EVT-003/006, HIST-005..009, OFF-007/008, SYNC-003/005/006, restart cache requirements |
| Writes and publication | PUB-001..009, EVT-001/002/007/008, RESTART-001/003/004/007/008/009, NEW-C/E/H/I |
| Routing and relays | RELAY-001..013, SYNC-001..006, ID-008/009, NEW-D, NIP-11 limits |
| Identity and crypto | ID-001..009, awaiting-signer and NIP-46 recovery requirements, sign-without-publish |
| Protocol crates | EVT-004/005, NEW-J/K/L/M/N/O, protocol-module ownership requirements |
| Diagnostics/testing/platforms | NEW-A/B/C/D, PARITY-001..007, Android/iOS runtime requirements, boundedness and app test infrastructure |
| Non-requirements/open decisions | historical tombstones and owner rulings, automatic negentropy rejection, no global sync, cancellation boundary, pagination/windowing uncertainty |

The consolidation intentionally removes implementation status, stale test tags, crate/file paths, current symbol names, issue sequencing, and deleted-mechanism internals from the normative body.
