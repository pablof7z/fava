# Fava Rewrite Implementation Plan

**Status:** proposed delivery plan for the rewrite
**Behavioral authority:** `FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md`
**Architectural authority:** `ARCHITECTURE.md`
**Testing authority:** `FAVA_TDD_BDD_TESTING_GUIDE.md`
**Companion acceptance application:** `apps/canary/`

## 1. Purpose

This plan turns the Fava rewrite specification and target architecture into a sequence of complete vertical milestones.

The plan is not organized as "first create every trait, then fill in implementations." Each milestone must produce a usable end-to-end behavior through the public `fava` facade, add the canary scenario that proves it, and leave behind the narrowest contracts justified by at least one real implementation.

The rewrite is complete only when ordinary applications can:

- open coherent live queries over local and relay sources;
- observe exact source-scoped evidence without global-completeness claims;
- accept and recover writes through a durable write store;
- publish through explicit or asynchronously expanding automatic routes;
- replace event-cache, routing, subscription-planning, transport, publisher, delivery, signer, and service-cache implementations at build time;
- add protocol crates without editing the universal core;
- consume equivalent behavior through Rust, Swift, and Kotlin profiles; and
- diagnose and test the real work Fava performs without reaching into internal crates.

## 2. Delivery principles

### 2.1 Vertical slices before generalized frameworks

A contract is provisional until:

1. one complete application flow uses it;
2. its standard implementation works through the public facade;
3. its behavior is covered by a conformance corpus; and
4. a meaningfully different implementation has challenged the boundary.

Do not create a broad provider framework in advance of those proofs.

### 2.2 Behavior-first TDD, canary-backed acceptance

Every contract slice starts with an accurate behavior statement and a failing executable proof at the smallest responsible owner. Every milestone also has an end-to-end canary scenario. The canary scenario should land before or with the implementation and must fail causally while the owned mechanism is disabled. The canary is the public composition capstone, not a substitute for owner, property, model, crash, or protocol tests.

The canary is an ordinary downstream Rust application. It may use:

- the public `fava` facade;
- selected public protocol crates;
- public diagnostics;
- public provider/profile construction;
- real local signer providers;
- real OS processes, sockets, clocks, and files; and
- an independent relay lab for seeding and wire witnessing.

It must not use:

- Fava internal crates;
- test-only engine commands;
- direct database inspection to decide application correctness;
- private route/subscription/attempt state;
- an application-side reimplementation of Fava query, route, or receipt semantics; or
- a helper layer that conceals an awkward public Fava API.

### 2.3 One milestone, one coherent product increase

A milestone may contain several crates where one complete flow requires them, but each issue and PR should own one contract slice. Avoid both extremes:

- one PR per tiny symbol; and
- a milestone-sized mega-PR that makes failures impossible to attribute.

### 2.4 Facts before effects

Whenever a milestone introduces durable authority:

```text
command
  -> owner decision
  -> durable commit
  -> committed fact
  -> external effect
  -> correlated completion fact
```

The canary must include a crash boundary around every newly introduced durable transition.

### 2.5 Partial progress is a first-class outcome

Routing, relay acquisition, signing, and delivery often become ready independently. Known work begins immediately. Unknown work remains explicitly unresolved and may add later work to the same query or receipt.

No milestone may serialize independent progress merely to simplify implementation.

### 2.6 Performance follows ownership

Performance work may alter representation and algorithms. It must not blur ownership. Before optimizing, the milestone must have:

- an observable product workload;
- a phase attribution;
- a behavior oracle;
- a measured regression budget; and
- a canary or benchmark mutation that detects a false optimization.

### 2.7 TDD and BDD discipline

`FAVA_TDD_BDD_TESTING_GUIDE.md` defines the development loop and evidence placement. In summary:

1. correct product behavior text before implementation when meaning changes;
2. write and observe the smallest causal failing test;
3. implement the smallest complete owner change;
4. refactor while green;
5. disable the protection and confirm the evidence fails; and
6. add a public/canary capstone only when it proves additional cross-boundary behavior.

Feature files preserve durable product meaning; they are not an exhaustive test inventory or a mandatory runner.

## 3. Workstreams

The milestones advance six parallel workstreams.

### A. Event, query, and write rules

- `fava-wire`
- `fava-state`
- `fava-query`
- `fava-write`

### B. Local data and recovery

- `fava-event-cache`
- event-cache implementations
- `fava-write-store`
- write-store implementations
- `fava-fetch-cache`
- fetch-cache implementations

### C. Relay planning and execution

- `fava-routing`
- router implementation crates
- `fava-subscriptions`
- subscription planners
- `fava-transport`
- transports
- `fava-publisher`
- publishers
- `fava-delivery`
- delivery policies

### D. Universal lifecycle owners

- `fava-ingest`
- `fava-observe`
- `fava-publication`
- `fava-session`
- `fava-auth`
- `fava-diagnostics`
- `fava-runtime`
- `fava`

### E. Protocol services and event-kind crates

- `fava-nip11`
- `fava-nip05`
- `fava-nip65`
- `fava-nip02`
- second unrelated protocol crate
- later protocol crates selected by product scope

### F. Product qualification

- provider conformance kits
- Rust canary
- relay lab
- public-relay reconnaissance
- FFI inventory
- Swift SDK
- Kotlin SDK
- mobile process tests
- profile-specific performance evidence

## 4. Milestone summary

| Milestone | Product result | Primary canary gate |
|---|---|---|
| M0 | Evidence foundation and real-relay lab | A real signed event survives publish, query, relay kill, and relay restart through independent wire tooling. |
| M1 | Coherent local query state from independent sources | Event-cache and write-store contributions merge into one current event view without cache pollution. |
| M2 | Explicit live query against a real relay | One public live query receives stored events, EOSE, live events, cancellation, and exact source evidence. |
| M3 | Multi-relay reactivity and bounded observation | Cross-relay dedup/provenance, reconnect generations, removals, and slow-consumer latest-state delivery work. |
| M4 | Ordered asynchronous routing and exact subscription planning | Known relays are used immediately; delayed routers add work later; explicit routing bypasses routers; wire grouping preserves meaning. |
| M5 | Durable explicit-route publication | Accepted writes appear locally, sign, publish to real relays, return exact receipts, cancel pre-handoff, and survive process death. |
| M6 | Automatic routing and partial delivery | Outbox, hints, app-relay, and fallback routers compose; partial recipient routes deliver now and expand under one receipt. |
| M7 | Replaceable-event edits and protocol-crate composition | Follow/unfollow rematerializes over newer source state; protocol crate N+1 changes only its crate and assembly. |
| M7.1.1 | `fava-simple-groups` multi-relay NIP-29 capability | One public app combines relay-local forks without losing per-host truth and publishes arbitrary kinds through the exact selected host set. |
| M8 | Authentication, hostile relays, limits, and boundedness | NIP-42, malformed/off-filter input, silence, CLOSED, limits, ambiguous handoff, give-up, and resource bounds are exact. |
| M9 | Cache/service profiles and restart guarantees | Persistent and ephemeral event-cache profiles, durable write recovery, NIP-05, and NIP-11 cache semantics are truthful. |
| M10 | Provider substitution and profile qualification | External providers replace each major seam without core edits and pass the same application corpus. |
| M11 | Native products and release qualification | Rust, Swift, and Kotlin profiles produce equivalent behavior in real platform processes. |

## 5. Milestone dependency graph

```text
M0
 |
 v
M1 -----> M5
 |          |
 v          v
M2 -----> M4 -----> M6
 |          |        |
 v          |        v
M3 ---------+------> M8
 |                   |
 +-------> M9 <------+
             |
             v
            M10
             |
             v
            M11

M7 depends on M1, M5, and enough of M6 to obtain source state and route writes.
M7.1.1 depends on M3, M5 through M7, and the completed literal tag-value query slice; it completes before M8.
```

The graph is a sequencing aid, not permission to maintain parallel half-built architectures. A milestone may begin preparatory pure-value work earlier, but its product claim does not exist until its canary gate passes.

# 6. Milestones in detail

## M0 — Evidence foundation and relay lab

### Goal

Create the independent evidence system used by every later milestone before Fava itself can accidentally become the witness for its own claims.

### Deliverables

- `apps/canary` Rust workspace/binary scaffold.
- Scenario registry with requirement IDs and milestone ownership.
- Deterministic run IDs and disposable test identities.
- JSONL evidence stream plus human-readable report.
- External process supervisor capable of:
  - launching a real third-party relay binary;
  - assigning isolated ports and data directories;
  - waiting for readiness;
  - graceful stop;
  - hard kill;
  - restart against the same relay data;
  - preserving stdout/stderr and configuration.
- Transparent WebSocket proxy recording client-to-relay and relay-to-client frames.
- Independent Nostr seeder/witness using real signatures and real WebSocket frames.
- Public-relay reconnaissance mode requiring explicit relay URLs.
- Run manifest recording:
  - Fava revision when available;
  - canary revision;
  - selected profile;
  - relay implementation/version/command;
  - platform/toolchain;
  - scenario seed;
  - process IDs;
  - start/end times; and
  - artifact hashes.

### Deterministic relay profile

At least one relay must be a real third-party implementation running as a separate process with real persistence. The default target is a pinned `strfry` build. A second independent relay implementation is added no later than M8 for interoperability.

A separate-process scriptable adversarial relay is allowed for exact protocol faults that a third-party relay cannot be configured to produce. It is a fixture, not the sole system-under-test relay.

### Canary scenario

`lab-real-relay-smoke`

1. Start a fresh real relay process.
2. Create and sign a real kind-1 event using a disposable key.
3. Publish over WebSocket.
4. Independently observe a matching `OK`.
5. Query the event through an exact `REQ`.
6. Observe the event and EOSE.
7. Hard-kill the relay.
8. Restart against the same data directory.
9. Query the event again.
10. Preserve the full wire transcript and process logs.

### Exit gates

- The scenario runs from one command on a clean machine after the documented relay prerequisite is installed.
- External relay failure is a failure, never a skipped success.
- The event survives a real relay process kill/restart.
- The evidence directory is sufficient to reconstruct every assertion.
- The canary itself has no dependency on Fava internal crates.

### Falsifier

Run the relay with a fresh data directory after the kill. The persistence assertion must fail.

---

## M1 — Local event state and merged query sources

### Goal

Prove the central local-state model before networking: live query state is the deterministic merge of independent `EventCache` and `WriteStore` sources.

### Crates/slices

- `fava-state`
- `fava-query`
- `fava-query-standard`
- `fava-event-cache`
- `fava-event-cache-memory`
- `fava-write-store`
- `fava-write-store-memory`
- local-only portion of `fava-observe`
- first thin `fava` facade path

### Required behavior

- Event identity, replaceable-event winner rules, deletion, expiry, and evidence merge are deterministic.
- Event cache accepts only admitted signed relay events.
- Write store exposes current local unsigned/signed materializations as a query source.
- Same-event source contributions merge into one `EventRecord`.
- A query-matching local pending replacement can shadow a matching cached predecessor without deleting it; an out-of-selection candidate cannot displace a selected event.
- Cancelling the local write retracts its source contribution and naturally reveals the cached predecessor.
- Query opening is all-or-nothing and returns one complete local snapshot.
- Equivalent query descriptions have stable identity.
- Current-state delivery is bounded and can coalesce intermediate states.

### Canary scenarios

`local-source-merge`

- Seed a signed event into the memory event cache.
- Accept a local materialization of the exact event into the memory write store.
- Open one query and observe one `EventRecord` containing merged relay/local evidence.

`local-replaceable-shadow-and-cancel`

- Cache profile v1.
- Accept local unsigned profile v2.
- Query shows v2.
- Cancel v2.
- Query shows v1 again.
- Event cache never stored v2.

`local-source-removal`

- Remove/expire/delete an event contribution.
- Every affected open query receives the current result without a parallel removal API.

### Exit gates

- No relay, transport, or runtime networking dependency exists in these crates.
- The same semantic corpus runs against memory cache and memory write store.
- The canary uses only the public facade for local queries and writes.
- Event-cache source data and write-store source data can be inspected through public event records without exposing storage internals.

### Falsifier

Disable the source merge and concatenate source results. The same-event scenario must detect a duplicate.

---

## M2 — Explicit one-relay live query

### Goal

Establish the complete read path against a real relay before automatic routing or subscription optimization exists.

### Crates/slices

- `fava-wire`
- `fava-subscriptions`
- `fava-subscriptions-no-grouping`
- `fava-transport`
- `fava-transport-websocket`
- `fava-ingest`
- relay-facing portion of `fava-observe`
- explicit query routing through `fava`
- minimum public diagnostics for relay/session/query facts

### Required behavior

- Explicit relay lists are exact and non-empty.
- Query opening immediately starts relay work for live freshness.
- Wire messages are exact NIP-01 messages.
- Every inbound event is attributed to an accepted subscription and verified before it can affect any source or query.
- Off-filter events are refused from the query result.
- EOSE is recorded only from the actual relay frame and scoped to the exact request.
- Empty + EOSE differs from silence, failure, auth-required, and CLOSED.
- Cancellation sends/causes exact withdrawal and wakes pending pulls.
- Query close is idempotent.

### Canary scenarios

`explicit-read-eose`

- Seed real events into a real relay through the independent seeder.
- Open one public Fava query explicitly against that relay.
- Observe the stored events, exact relay evidence, and EOSE.
- Assert no global `synced` or complete fact exists.

`explicit-read-live-after-eose`

- Keep the query open after EOSE.
- Publish a new matching event independently.
- Observe it on the same query.

`explicit-read-cancel`

- Open a query.
- Confirm the proxy sees `REQ`.
- Cancel it.
- Confirm exact closure/withdrawal and no later application delivery.

### Exit gates

- One real relay path works without any automatic router.
- Fava's public diagnostics and the independent proxy agree on relay/session/subscription identity.
- No Fava internal types appear in the canary.
- The transport conformance kit includes handoff success, refusal, disconnect, and close.

### Falsifier

Bypass signature verification in the admission path and inject a forged event. The canary must fail because the forged event becomes visible.

---

## M3 — Multi-relay reactivity and bounded observation

### Goal

Prove that Fava's live query model remains correct as several relays, local sources, source evidence, reconnect generations, and slow consumers interact.

### Crates/slices

- completion of `fava-observe`
- multi-session support in transport/runtime
- exact request generation identity
- bounded current-state observation delivery
- per-source evidence and diagnostics

### Required behavior

- The same event from several relays appears once with all actual serving relays.
- A relay named in a plan but not serving the event is not credited.
- Reopened subscriptions cannot inherit stale frames from a prior generation.
- Reconnect automatically restores active demand using fresh identity.
- Reconnect does not itself claim backfill for the disconnected interval.
- Source/provenance-only changes can update an event record without duplicating it.
- Slow current-state observers receive an exact latest result under a bounded mailbox.
- Causal streams such as receipt facts are not conflated as current-state snapshots.
- Repeated cancel/retry of `next` does not build an update backlog.

### Canary scenarios

`multi-relay-dedup-provenance`

- Seed the same signed event into two real relay processes.
- Query both explicitly.
- Observe one event record naming both relays.
- Add a third relay to the query that never serves the event; it is not credited.

`reconnect-generation`

- Open a live query.
- Kill one relay.
- Restart it.
- Confirm a fresh `REQ` generation appears without application resubscription.
- Inject a late frame attributed to the old subscription through the adversarial relay/proxy; it cannot affect current state.

`slow-consumer-latest-state`

- Produce a burst larger than the configured delivery capacity.
- Delay the application reader.
- Observe one bounded latest state containing every committed change.
- Diagnostics report coalescing/loss facts accurately.

### Exit gates

- Observation resource usage is independent of one-thread-per-query design.
- At least 1,000 simultaneous idle observations remain bounded under a declared profile.
- The multi-relay scenario passes against two independent relay implementations by M8 at the latest.

### Falsifier

Remove request-generation checking. A late event from the dropped connection must cause the reconnect scenario to fail.

---

## M4 — Ordered asynchronous routing and subscription planning

### Goal

Introduce automatic read routing as an ordered chain of independently selectable router crates and prove that routing and per-relay wire planning are separate responsibilities.

### Crates/slices

- `fava-routing`
- `fava-router-app-relays`
- `fava-router-fallback-relays`
- a delayed test router in the routing testkit
- ordinary local queries and explicitly routed queries used by router implementations
- `fava-subscriptions-standard`
- route/session diagnostics

`fava-router-hints` and `fava-router-outbox` may begin here or in M6; the primitive must already support them without revision.

### Required behavior

- Automatic routing runs configured routers in order.
- Each router exposes an immediate complete current contribution and later complete replacement contributions.
- A slow router does not block known destinations from earlier/other routers.
- Downstream routers react to the live accumulated upstream plan.
- Contributions are additive; identical relays deduplicate while preserving all reasons and targets.
- Explicit routing starts zero automatic router sessions.
- Router-owned acquisition uses explicit sources and cannot recursively invoke automatic routing.
- Route preview uses the same derivation over currently available facts without accepting work or creating delivery lanes.
- Subscription planning receives logical demand already assigned to one relay session.
- Planner grouping may change wire shape but not query meaning, evidence, access isolation, or cancellation.
- Relay limits produce exact shortfall rather than silent dropped demand.

### Canary scenarios

`async-route-partial-read`

- Configure an immediate app-relay router and a delayed router.
- Open an automatic query.
- Proxy shows immediate `REQ` to app relay.
- Delayed router later contributes a second real relay.
- Proxy shows new work only for the second relay; first relay remains uninterrupted.

`explicit-route-bypass`

- Open the same query explicitly.
- Router diagnostics show no router sessions or acquisitions.

`fallback-reacts`

- Upstream plan initially has insufficient target coverage.
- Fallback router contributes fallback relay and query work begins.
- Upstream router later provides adequate coverage.
- Fallback contribution retracts; unrelated relay work is unchanged.

`subscription-grouping-equivalence`

- Open many logically separate compatible queries to one real relay through the standard planner.
- Independent proxy observes fewer wire `REQ`s.
- Application-visible results equal the no-grouping planner's results exactly.

### Exit gates

- `fava-routing` contains no NIP-65, hint, app-relay, or fallback meaning.
- Each higher-level routing policy is a separate crate.
- A router outside the workspace can be implemented against public contracts by M10.
- Planner substitution does not require router or observation changes.

### Falsifier

Make automatic query opening await settlement of every router. The immediate-route scenario must time out before the delayed router completes.

---

## M5 — Durable explicit-route publication

### Goal

Build one complete durable publication path before automatic write routing: accept, expose locally, sign, publish to exact relays, record outcomes, cancel before handoff, recover after crash.

### Crates/slices

- `fava-write`
- durable `fava-write-store` implementation, initially Redb or SQLite
- `fava-signer`
- `fava-signer-local`
- `fava-publisher`
- `fava-publisher-nip01`
- `fava-delivery`
- initial `fava-delivery-standard`
- `fava-publication`
- publication/runtime integration
- receipt and stalled-write diagnostics

### Required behavior

- M5 accepts unsigned and verified pre-signed events; `ReplaceableEventEdit` acceptance begins in M7 through the same write lifecycle.
- Applications call synchronous `publish(payload)`, optionally after inert
  `by(author)` and/or `to(relays)` scopes, and receive `Write` only after the
  write-store acceptance transaction commits. Receipt settlement is
  `write.settled(all())` or `write.settled(at_least(n))`.
- An unsigned event's `pubkey` selects the signer.
- `Accepted` occurs only after write obligation, current materialization, and receipt are durably committed.
- The write store supplies the unpublished event directly to matching queries.
- No unsigned event is inserted into the event cache.
- Explicit routes are exact and bypass routers.
- One publication attempt is performed by the publisher over transport.
- Delivery policy decides when to attempt/retry/give up; publisher does not.
- Each relay outcome preserves exact text and exact ambiguity.
- Pre-handoff cancellation retracts write-store query contribution and creates exact terminal receipt state.
- Receipt removal is separate and only applies under the declared retention/lifecycle rules.
- A hard process kill after acceptance recovers one obligation and the same receipt.

### Canary scenarios

`explicit-publish-optimistic`

- Open a query matching a disposable author's kind-1 events.
- Publish explicitly to a real relay.
- Observe local unsigned/signed event from the write store before relay `OK`.
- Observe receipt transitions and later relay evidence merge after echo.
- Confirm event cache receives only the admitted signed relay echo.

`mixed-relay-outcomes`

- Publish one event explicitly to:
  - accepting relay;
  - rejecting relay; and
  - unreachable relay.
- Observe exact per-relay facts and one aggregate terminal result according to policy.

`cancel-pre-handoff`

- Block signer or transport before any handoff.
- Cancel.
- Confirm zero `EVENT` frame, query retraction, exact cancellation receipt, and idempotent retry/remove behavior.

`crash-after-acceptance`

- Call `publish(payload)` and receive `Write`.
- Supervisor SIGKILLs the canary child before delivery.
- Restart against the same write store.
- Same receipt and event materialization reappear; delivery resumes without app resubmission.

### Exit gates

- The standard write-store profile has process-kill tests at every commit/effect boundary.
- `fava-publication` owns the write lifecycle but not router, signer, publisher, transport, or delivery policy state.
- The canary sees no internal attempt/lane types.
- A memory write store remains usable for deterministic lower tests but is not presented as the standard durable profile.

### Falsifier

Return `Accepted` before the write-store commit completes. Kill at that boundary; the recovery scenario must fail to find the receipt.

---

## M6 — Automatic write routing and partial delivery

### Goal

Compose real routing policies for reads and writes, with immediate partial delivery and asynchronous route expansion under one receipt.

### Crates/slices

- `fava-nip65`
- `fava-router-outbox`
- `fava-router-hints`
- production `fava-router-app-relays`
- production `fava-router-fallback-relays`
- route revision integration in publication/write store
- complete standard delivery policy

### Required behavior

- The configured router chain is the application's automatic routing policy.
- Outbox router acquires kind:10002 information through explicit indexer queries.
- Hint router uses pointer-like relay hints and admitted event evidence through its own crate.
- App-relay router always contributes configured relays according to its documented read/write scope.
- Fallback router observes upstream coverage and contributes/retracts independently.
- Known route destinations begin delivery immediately while needs remain unresolved.
- Later destinations become new lanes under the same receipt.
- Duplicate destination contributions do not create duplicate sends.
- Removed desired routes may retire only work that has not crossed a handoff boundary; historical delivery facts remain exact.
- Automatic routes are re-evaluated while work remains open.
- Route preview and real routing use one derivation.

### Canary scenarios

`async-recipient-routing`

- Publish an event p-tagging three disposable pubkeys.
- Outbox router already knows two recipient relay lists.
- App-relay router immediately contributes its relay.
- Third recipient is unresolved.
- Known relays receive the event immediately.
- Seed/serve the third recipient's kind:10002 later through real indexer relays.
- Newly discovered relay receives the same signed event under the same receipt.
- Existing destinations receive no duplicate send.

`hint-routing`

- Query/ingest a target event from one relay with usable pointer evidence.
- Compose a reply/reaction through ordinary event construction.
- Hint router contributes the justified relay independently of outbox routing.

`route-preview-parity`

- Preview current automatic route.
- Publish without changing route facts.
- Initial real route plan matches preview exactly.
- Preview creates no receipt, signing, write-store entry, or router-owned relay acquisition.

`app-relay-versus-fallback-profile`

- Run the same publication under one profile selecting app-relay policy and another selecting fallback policy.
- Each produces its documented distinct plan without core changes.

### Exit gates

- No central routing crate names or depends on the router implementations.
- Router contribution count and route fan-out are bounded with exact shortfall.
- The async recipient scenario passes through real relay processes and independent wire transcripts.

### Falsifier

Wait for all recipients to settle before sending to known destinations. The canary must detect delayed first handoff.

---

## M7 — Replaceable-event edits and protocol-crate composition

### Goal

Prove that protocol crates own event-kind meaning while reusing one write lifecycle, and that accepted replaceable-event edits survive changing source state.

### Crates/slices

- `fava-nip02`
- `fava-bookmarks` as the second unrelated protocol crate
- replaceable-event-edit storage in write store
- materialization/rematerialization lifecycle in publication
- replaceable-event-edit conformance corpus

### Required behavior

- Protocol crates expose replaceable-event edits, such as follow and unfollow.
- Protocol-crate code produces replaceable-event edits or ordinary event values; it does not sign, route, publish, or own receipts.
- `fava-nip02` exposes fallible `contact_list(authors)` and
  `followers_of(subject)` builders for ordinary queries, plus
  `follows_of(snapshot)` as a typed pure projection. Query construction returns
  the neutral `QueryError` on bounded-input refusal.
- Contact-list parsing treats an empty kind-3 event as valid and retains ordered
  typed evidence for malformed pubkeys and relay hints. Petnames preserve UTF-8
  bytes without normalization.
- Follow-list edits preserve content, unknown and extension tags, malformed
  unrelated rows, unrelated valid rows, and first-occurrence order while
  changing only the targeted relationship.
- The author is resolved when the write is accepted, before materialization; the resulting unsigned event carries it in `pubkey`.
- First-value operation materializes against no prior event.
- A newer qualified source event rematerializes still-live operations while preserving unrelated source changes.
- One receipt remains stable across materialization generations.
- Signer, route, and delivery completions for retired generations are rejected as stale.
- Deterministic memory and durable-restart barriers advance the receipt during
  custody loading and router-session opening; stale custody is never
  materialized, stale sessions close, and only the complete current generation
  reaches signing and route effects, including after process kill.
- A second protocol crate proves the edit contract is not secretly NIP-02-shaped.
- Adding protocol crate N+1 edits only its crate and selected assembly/artifact metadata.

### Canary scenarios

`replaceable-edit-first-value`

- Alice has no kind-3 event.
- `follow(Bob)` accepts one `ReplaceableEventEdit`.
- Matching queries immediately show the local materialized kind-3.
- Publication uses the ordinary write receipt.

`replaceable-edit-rematerialization`

- Accept offline `follow(Bob)` over source v1.
- Later ingest source v2 containing unrelated Carol changes.
- Local current materialization preserves Carol and Bob.
- Receipt remains the same; stale signature/delivery for old materialization is inert.

`replaceable-edit-opposing-operations`

- Follow then unfollow, or bookmark then unbookmark.
- Operations normalize to the correct desired state without accumulating obsolete active delivery.

`protocol-crate-n-plus-one`

- Add the second protocol crate to canary assembly.
- Core crates and the `fava` facade require no changes to their owned behavior.

### Exit gates

- `fava` has no switch over NIP-02 or the second protocol crate.
- Protocol-crate dependency direction excludes runtime, transport, store implementation, and standard routers.
- The canary depends on each protocol crate explicitly, making the selected product visible in Cargo metadata.

### Falsifier

Have the protocol crate call a signer or publisher directly. A dependency-negative compile test must fail.

---

## M7.1.1 — `fava-simple-groups` multi-relay NIP-29 capability

### Goal

As a Fava application developer, I can use the README-shaped
`fava-simple-groups` capability to query content and relay-generated state,
decode individual NIP-29 values, publish prepared content, and maintain my
kind-10009 list without creating a second query/publication lifecycle.

### Crates/slices

- `fava-simple-groups`
- `SimpleGroup` and `SimpleGroupStateEventKind` query lowering
- event-local kinds 39000 through 39005 decoders
- one-event `SavedGroupList` decoding
- ordinary query combinators over literal tag-value filters
- kind-10009 replaceable-edit materializer
- compiler-derived complete README public catalog
- controlled two-relay canary flow

### Required behavior

- Construction accepts a finite owned `Vec<RelayUrl>`:
  `SimpleGroup::new(id, relays)`. It returns the public attributable
  `SimpleGroupConstructionError` for exactly an empty id or empty relay vector,
  preserves every non-empty opaque id, and deduplicates relays by first
  occurrence without accepting arbitrary iterators.
- Content queries preserve unrelated selection, constrain lowercase `h` to
  the exact group id without broadening an existing `h` axis, and ask every
  selected relay. They delegate exact narrowing to query-owned
  `Query::intersect_tag_values`; a disjoint axis stays present-empty and
  matches nothing. Exact `QueryError` refusals pass through unchanged.
- State queries delegate kind input to the query owner, add the exact `d`
  value, and require actual evidence from selected relays without a
  capability-private result limit. Exact `QueryError` refusals pass through.
- State decoders check kind plus the first `d` value, ignore unknown and unused
  material, preserve repetitions and order, and retain malformed semantic entries
  as local typed errors.
- Publication composition is kind-blind. `SimpleGroupEventBuilder` keeps the
  concrete `EventBuilder` fluent while adding each distinct exact two-cell `h`
  tag in selection order and accumulating the first-occurrence-deduplicated
  relay union as bounded neutral routing.
- `fava.publish(builder)` consumes that route through the ordinary write
  lifecycle and bypasses automatic routers only when the builder carries an
  explicit route. The route is local, not serialized or signed. Supplying both
  a builder route and `fava.to(...)` refuses before signing or custody.
- Event-only construction refuses an attached explicit route. Pre-signed
  validation requires the selected exact group context, tolerates sibling
  contexts, returns the byte-exact event, and receives its route through the
  ordinary explicit facade scope. Management wrappers are absent.
- Kind-10009 queries are ordinary exact-author queries. One event decodes to one
  `SavedGroupList` whose group and relay entries retain order, repetitions, and
  entry-local failures.
- Saved-group and relay changes use crate-root pure edit functions and one
  materializer through the ordinary durable write lifecycle, preserving
  unrelated source material.
- The crate owns no observation, store, signer, routing session, publisher,
  delivery, retry, receipt, runtime, transport, verification, generic bound,
  projection, disagreement, management, or discovery policy.

### Canary scenarios

`simple-group-one-host`

- Parse one string with `RelayUrl::parse`, then construct
  `SimpleGroup::new(id, vec![relay])` and require success.
- Observe kind-9 content and typed metadata/members through public Fava.
- Publish an arbitrary custom kind through the exact host and inspect the
  ordinary receipt.

`simple-group-multi-relay-fork`

- Run two controlled real relays with the same simple group id and divergent
  metadata, admins, and content.
- Parse both relay strings with `RelayUrl::parse`, then construct
  `SimpleGroup::new(id, vec![relay_a, relay_b])` and require success.
- Open one multi-relay simple group feed; duplicate one event across both
  relays.
- Observe one event record with both serving-relay contributions, unique events
  from each relay, and decode each relay-generated state event locally.
- Publish through the multi-relay simple group and prove one exact handoff per
  selected relay under one receipt.
- Select relay-local values through ordinary `EventRecord::relay_evidence`.

`simple-group-saved-list`

- Parse several saved-simple-group and relay-in-use entries from one kind-10009
  event.
- Preserve the event author, relay identity, repetitions, malformed siblings,
  unused extra values, and source order.
- Materialize save, rename, remove, and relay edits through ordinary Fava.

`simple-group-context-preparation`

- Preserve an unsigned event with matching repeated or extended `h` tags.
- Append one matching tag when only missing or contradictory `h` tags exist.
- Prove preparation is pure and opens no Fava work.

### Exit gates

- The crate README remains the public API North Star and every executable
  example is covered by a compile test or explicitly marked prospective until
  its owning plan lands.
- `fava-simple-groups` depends only on neutral query/state/write contracts;
  `nostr` is used only for the typed relay-parser error.
- Generic owners retain verification, bounds, provenance, projection, and
  lifecycle policy.
- The compiler-derived README inventory contains exactly the public surface and
  a non-empty evidence-based description for every symbol.
- Pure parsers/construction, public facade, cancellation/close, deliberate
  breaks, and controlled two-relay wire evidence all pass.
- Adding or removing the selected capability changes only its crate and
  application/artifact assembly metadata.

### Falsifier

Drop a semantic sibling after one malformed entry, reorder interleaved pins, add
a private state-query limit, or route publication to one selected relay. The
crate and multi-relay canary must fail on exact decode, query, or wire evidence.

---

## M8 — Authentication, hostile relays, limits, and boundedness

### Goal

Harden all network and provider boundaries under deterministic real-process and adversarial-wire scenarios.

### Crates/slices

- `fava-auth`
- complete NIP-42 integration
- relay-limit/NIP-11 integration into planning and publication
- bounded transport queues and session pool
- bounded provider execution
- exact ambiguous-handoff handling
- attempt ceilings/give-up
- hostile relay admission and diagnostics
- resource diagnostics

### Required behavior

- NIP-42 relay access is explicit and separate from event authorship/filter identity.
- Auth challenges are generation-scoped; reconnect obtains fresh auth state.
- App auth policy denial terminates only the exact affected destination/session operation.
- Invalid id/signature, off-filter event, malformed frame, oversized frame, never-EOSE, CLOSED, NOTICE, silent limit, mid-frame truncation, and connection loss remain relay/request scoped.
- NIP-11 limits produce exact plan or shortfall before knowingly invalid work is sent.
- Offline/unreachable time is not a failed delivery attempt.
- Real attempted failures eventually reach the configured terminal give-up policy.
- Ambiguous handoff is never converted into acknowledged, rejected, or never-sent.
- All externally influenced queues/sets have bounds or explicit backpressure/refusal.
- Provider panic, block, late result, malformed result, or ignored cancellation cannot block unrelated progress or shutdown.

### Canary scenarios

`nip42-write-and-reconnect`

- Real relay requires NIP-42 for writes.
- App policy authorizes one account.
- Publish succeeds.
- Relay is restarted/reconnected.
- Fresh challenge is answered without app lifecycle code.

`auth-account-isolation`

- Two accounts publish to the same relay/access setup.
- Deny one account's auth policy.
- The other account's operation continues unaffected.

`hostile-relay-ingress`

- Adversarial relay emits invalid signature, wrong id, off-filter event, malformed JSON, event-after-CLOSED, and stale-subscription frames.
- None enters current state.
- Healthy relay/query remains live.

`relay-limit-shortfall`

- Relay advertises a strict subscription/message limit.
- Standard planner stays within it or reports exact shortfall.
- No silent query omission.

`ambiguous-handoff`

- Proxy confirms full EVENT frame crossed from Fava.
- Connection is cut before relay OK reaches Fava.
- Receipt records the selected ambiguity outcome exactly.

`attempt-ceiling`

- Relay repeatedly produces a retryable failure.
- Delivery retries under one owner and reaches `GaveUp` within the declared policy.
- A merely offline relay does not spend attempts until a real attempt occurs.

`provider-failure-isolation`

- Deliberately blocking/panicking custom provider occupies its own boundary.
- Unrelated query, relay, and shutdown progress remain bounded.

### Exit gates

- Deterministic hostile scenarios run through real sockets and a separate process.
- At least one real third-party relay proves NIP-42 and persistence behavior.
- A second relay implementation passes the core read/publish subset.
- Resource envelopes and failure evidence are published for every run.

### Falsifier

Route malformed relay input directly to event-cache mutation, bypassing admission. The hostile scenario must fail.

---

## M9 — Cache/service profiles and restart guarantees

### Goal

Make provider/profile guarantees explicit and prove the distinction between disposable event reuse, durable write custody, and service-owned cached data.

### Crates/slices

- persistent `fava-event-cache` implementation
- bounded memory and null/ephemeral cache implementations
- persistent write-store qualification
- `fava-fetch-cache`
- memory and persistent fetch-cache implementations
- `fava-nip05`
- `fava-nip11`
- standard persistent profile
- explicit ephemeral profile
- destructive reset lifecycle

### Required behavior

- Baseline event-cache contract remains coherent without implying persistence.
- Persistent profile provides its declared cold cache reuse, provenance, deletion/expiry, and coverage behavior.
- Ephemeral profile starts with no cached relay events after process restart while durable accepted writes may still recover if its write store is durable.
- Cache eviction retracts/changes current query results coherently and adjusts any cache-owned coverage claim.
- NIP-05 and NIP-11 own validation, freshness, negative caching, stale results, and failure semantics.
- Generic fetch cache stores opaque service payloads and does not interpret them.
- NIP-05 and NIP-11 may share one physical fetch-cache provider without semantic leakage.
- Each provider owns its private persisted schema/version/migration.
- Destructive reset clears exactly the selected profile's cache/write/session/service state according to its contract.

### Canary scenarios

`persistent-cache-restart`

- Read real events through Fava.
- Stop process cleanly.
- Stop all relays.
- Restart the app with the persistent profile.
- Query returns cached events and exact profile evidence without claiming relay completeness.

`ephemeral-cache-restart`

- Run the same flow using memory event cache and durable write store.
- Relay-observed cache rows disappear after restart.
- Accepted pending local writes recover through write store.

`nip11-cache-freshness`

- Fetch a real/local relay NIP-11 document over HTTP.
- Serve a changed/failing response.
- Observe service-owned fresh/stale/last-good/error behavior.
- Subscription planner consumes applicable limits without owning HTTP state.

`nip05-cache-isolation`

- Resolve controlled NIP-05 identities through a real HTTP server.
- Exercise positive, negative, stale, malformed, and changed responses.
- NIP-11 cache entries cannot alias or affect NIP-05 semantics.

`destructive-reset`

- Populate event cache, write store, service cache, and session.
- Invoke explicit reset through public facade.
- Reopen and prove exact emptiness/retention according to profile.

### Exit gates

- Profile documentation is generated/checked from assembly configuration.
- Persistent and ephemeral scenarios run from the same application source with only provider selection changed.
- Event-cache persistence is never inferred from the baseline trait.

### Falsifier

Run the ephemeral profile but reuse event-cache bytes from the prior process. The ephemeral restart scenario must fail.

---

## M10 — Provider substitution and architecture qualification

### Goal

Attempt to falsify every replaceability claim with implementations outside the core/default-provider crates.

### Required alternative implementations

At minimum:

- a third-party/external router crate with its own asynchronous input;
- a second event-cache implementation with materially different persistence/retention;
- a second durable write-store implementation or a deliberately independent qualification prototype;
- a no-grouping subscription planner beside the standard planner;
- an alternative transport or transport wrapper;
- an alternative publisher or gateway publisher;
- a different delivery policy;
- a delayed/remote signer provider; and
- a shared fetch-cache provider used by both NIP-05 and NIP-11.

These may live in a separate `examples/providers` workspace or an external fixture repository. They must depend only on public contract crates and the public assembly API.

### Required behavior

- Standard providers have no privileged constructors or internal state access.
- Provider conformance kits execute against standard and alternative implementations.
- The application can select profiles by changing assembly/dependencies, not core source.
- Changing one provider does not require changing unrelated providers.
- Provider failures remain isolated.
- Persistent-format incompatibility is owned by the provider, not a global assembly identity.
- Dependency-negative tests prove forbidden edges.
- Change-amplification metrics remain narrow.

### Canary scenarios

`provider-matrix`

Run the core scenario subset across a matrix such as:

```text
memory cache + durable redb write store + standard routers + standard planner
persistent cache A + durable write store A + standard routers + standard planner
persistent cache B + durable write store B + custom router + no-grouping planner
```

`external-router`

- Build a router outside the main Fava workspace.
- It contributes immediate and delayed routes.
- No core/runtime/facade source changes.

`planner-substitution`

- Same application workload through no-grouping and standard planner.
- Wire transcripts differ.
- Event records/evidence remain identical.

`publisher-delivery-separation`

- Swap publisher while retaining delivery policy.
- Swap delivery policy while retaining publisher.
- Only the owned behavior changes.

### Exit gates

- All architecture falsifiers in `ARCHITECTURE.md` have executable owners.
- A protocol crate N+1 and router N+1 change zero universal core lines.
- Provider contract versions are ready for stabilization only after the matrix passes.

### Falsifier

Give the standard provider a private facade door unavailable to the external provider. A source/dependency gate and conformance comparison must fail.

---

## M11 — Native products and release qualification

### Goal

Ship selected, ordinary external artifacts and prove behavioral equivalence across Rust, Swift, and Kotlin in real platform processes.

### Crates/artifacts

- FFI value/lifecycle projection
- Swift package/artifact
- Kotlin/JVM package
- Android AAR
- iOS XCFramework or selected package form
- profile and protocol-crate selection tooling
- selected `fava-simple-groups` Rust capability and native value/lifecycle projection
- SDK parity inventory
- shared cross-language scenario corpus

### Required behavior

- Native artifacts expose only selected providers and protocol crates.
- Applications consume artifacts without repository-relative paths or raw generated bindings.
- Live-query open/cancel/close behavior matches Rust.
- Event records, evidence, route shortfall, receipts, errors, and restart semantics match Rust for the same profile.
- Android fresh-process persistent-profile behavior is proven.
- iOS suspension/resume behavior is proven on a physical device for any profile that claims transparency.
- Resource use returns to baseline after repeated lifecycle cycles.
- Public operation inventory is structural, not heuristic word matching.
- Native products selected with `fava-simple-groups` preserve multi-host SimpleGroup construction, per-host record disagreement, kind-blind exact-host publication, and ordinary query/write lifecycles.

### Canary relationship

The Rust canary is the reference application for behavior. Native capstones reproduce selected scenario scripts through their public SDK idioms; they do not call the Rust canary internally.

Required parity subset:

- explicit read + EOSE;
- multi-relay dedup/provenance;
- `fava-simple-groups` multi-host fork projection and exact-host publication;
- optimistic local write visibility;
- receipt mixed outcomes;
- query cancellation race;
- restart recovery;
- automatic asynchronous route expansion;
- typed error/shortfall mapping; and
- deterministic close.

### Exit gates

- Release artifacts are built from a declared selected profile.
- The parity mutation corpus detects an intentionally removed operation/outcome on each SDK.
- Real mobile process evidence is committed/published with the release candidate.

# 7. Cross-cutting implementation rules

## 7.1 Public diagnostics are part of each milestone

Do not postpone diagnostics until the end. Every new owner must expose enough bounded facts for the canary to explain its real work without private inspection.

Minimum diagnostic dimensions:

- query identity and current source count;
- relay session URL/access/generation/reason;
- logical demand to wire-subscription mapping;
- router contributions, unresolved needs, and route revisions;
- write receipt and current materialization identity;
- per-destination delivery state;
- signer/provider availability;
- cache/write-store profile and bounded counts;
- explicit shortfalls/backpressure; and
- coalesced/lost diagnostic-update counts.

Diagnostics report current facts. The independent proxy remains the wire witness.

## 7.2 Test layers

Each behavior should be proven at its narrowest owner and at selected application capstones.

| Layer | Owns proof of |
|---|---|
| Pure/property tests | state semantics, query identity, merge algebra, planner equivalence, policy decisions |
| Provider conformance | public provider contract and lifecycle |
| Owner integration | state transitions across store/runtime boundaries |
| Scripted relay | hostile frames, exact races, malformed protocol behavior |
| Real relay lab | interoperability, persistence, real sockets/processes, NIP-42/NIP-11 where supported |
| Rust canary | public facade, ordinary app flow, process/restart/resource behavior |
| Public-relay mode | reconnaissance and unexpected ecosystem behavior, not deterministic pass/fail |
| Native capstones | SDK/parity/platform process behavior |

## 7.3 Mutation expectations

Every milestone names at least one mechanism-disable mutation. Examples:

- bypass admission verification;
- suppress EOSE attribution;
- remove route-generation checks;
- make router opening wait for settlement;
- insert local unsigned events into event cache;
- return Accepted before write-store commit;
- ignore later route contributions;
- allow stale signer completion after rematerialization;
- silently drop planner overflow;
- map ambiguous handoff to failure;
- share NIP-05 and NIP-11 freshness policy;
- remove one SDK operation.

A scenario that remains green under its mutation does not prove its claim.

## 7.4 Performance gates

Performance budgets become release gates only after representative behavior exists. Track from the first milestone anyway:

- time to initial local view;
- time to first relay result;
- route first-known latency and settlement latency separately;
- write acceptance latency;
- first handoff latency;
- open-write recovery time versus open obligation count;
- event ingest throughput;
- query update cost versus affected observations;
- active sessions/subscriptions;
- threads/tasks/file descriptors;
- RSS/native heap;
- wire bytes and request count; and
- write amplification by storage responsibility.

Do not optimize away evidence or lifecycle correctness to hit a number.

## 7.5 Documentation gates

Each merged contract slice updates:

- its contract crate documentation;
- provider conformance documentation;
- selected profile guarantees;
- canary scenario roster;
- architecture ownership ledger if ownership changed; and
- specification only when product behavior changes.

Implementation status and benchmark numbers do not belong in the normative specification.

# 8. Canary application design

## 8.1 Purpose

The canary is one small real Rust application whose job is to keep Fava honest as a product.

It answers:

> Can an ordinary downstream application read, compose, publish, route, recover, reconnect, inspect evidence, and shut down correctly through the supported Fava facade while Fava performs real work against real relays?

It is not a generic framework or internal debugger.

## 8.2 Application shape

The canary should be a small social client with enough ordinary product behavior to cross Fava's boundaries:

- multiple disposable accounts;
- profiles;
- follow/unfollow;
- follows feed;
- note/reply/reaction;
- bookmarks or a second replaceable-event-edit protocol crate;
- one `fava-simple-groups` multi-relay room/simple-group flow with visible per-host disagreement;
- route preview;
- outbox/receipt inspection;
- NIP-05 resolution;
- NIP-11 inspection;
- diagnostics view; and
- restart/crash supervisor modes.

Scenarios drive these app operations; they do not invoke engine internals.

## 8.3 Evidence environments

### Deterministic real-relay lab

Authoritative pass/fail evidence.

- third-party relay binaries as child processes;
- isolated data directories;
- real signed Nostr events;
- real TCP/WebSocket traffic;
- independent transparent proxy transcript;
- real kill/restart;
- no skipped scenarios on environmental failure.

### Adversarial relay process

Used only for exact faults not deterministically configurable in third-party relays:

- malformed JSON;
- forged or wrong-id events;
- off-filter events;
- never EOSE;
- EOSE then more stored-like events;
- stale subscription IDs;
- CLOSED/NOTICE sequences;
- mid-frame truncation;
- silent subscription caps; and
- controlled disconnect after handoff.

It runs as a separate process over real sockets. It is not accepted as the sole interoperability relay.

### Public-relay reconnaissance

The same application profile can be pointed at explicit public relays.

- read-only by default;
- public writes require an explicit allow flag and relay allowlist;
- uses disposable keys and clearly marked canary content;
- external failure is reported as evidence;
- no deterministic scenario is considered passed merely because public infrastructure was unavailable.

## 8.4 Artifact layout

Each run writes:

```text
runs/<run-id>/
  manifest.json
  report.md
  evidence.jsonl
  app.stdout.log
  app.stderr.log
  resources.csv
  relays/
    <relay>/config
    <relay>/stdout.log
    <relay>/stderr.log
    <relay>/process.json
  wire/
    <proxy>.jsonl
  children/
    <child-run>/...
```

## 8.5 Evidence rules

Every assertion should cite observable evidence from at least one of:

- application-visible Fava result;
- public Fava diagnostics;
- independent wire transcript;
- relay process log;
- child-process exit/restart fact; or
- external process resource sample.

Fava diagnostics alone do not prove that a frame crossed the socket. The proxy alone does not prove what the application observed. Important claims compare both.

## 8.6 Scenario status

A scenario has one of these states:

- `planned`: named and mapped, implementation milestone not complete;
- `enabled`: expected to run and pass in the current profile;
- `reconnaissance`: evidence-only public-relay scenario;
- `retired`: behavior removed from product scope, retained only in history outside the active registry.

An enabled scenario may never silently skip.

# 9. Canary scenario roster

| ID | Milestone | Primary requirements |
|---|---:|---|
| `lab-real-relay-smoke` | M0 | test infrastructure prerequisite |
| `local-source-merge` | M1 | QUERY-005, EVENT-008, EVENT-009, WRITE-005 |
| `local-replaceable-shadow-and-cancel` | M1 | EVENT-002, WRITE-005, WRITE-021, WRITE-023 |
| `explicit-read-eose` | M2 | QUERY-003/004/009/010/013, RELAY-005 |
| `explicit-read-live-after-eose` | M2 | QUERY-006/010/013 |
| `explicit-read-cancel` | M2 | QUERY-012, OPS-009 |
| `multi-relay-dedup-provenance` | M3 | EVENT-003/009, QUERY-008 |
| `reconnect-generation` | M3 | QUERY-015, RELAY-006 |
| `slow-consumer-latest-state` | M3 | QUERY-011, OPS-004 |
| `async-route-partial-read` | M4 | QUERY-014, WRITE-012/013 |
| `explicit-route-bypass` | M4 | WRITE-011, ROUTER contracts |
| `fallback-reacts` | M4 | ROUTER-004, WRITE-013 |
| `subscription-grouping-equivalence` | M4 | RELAY-002/003/004 |
| `explicit-publish-optimistic` | M5 | WRITE-003/004/005/007/018/025 |
| `mixed-relay-outcomes` | M5 | WRITE-018/019/020, RELAY-008 |
| `cancel-pre-handoff` | M5 | WRITE-023, OPS-009 |
| `crash-after-acceptance` | M5 | WRITE-004/029, PROFILE-004 |
| `async-recipient-routing` | M6 | WRITE-012/013/015/017/028, ROUTER-001/003 |
| `hint-routing` | M6 | ROUTER-002, PROTO-005 |
| `route-preview-parity` | M6 | WRITE-016 |
| `replaceable-edit-first-value` | M7 | PROTO-002/003, WRITE-002/006 |
| `replaceable-edit-rematerialization` | M7 | WRITE-006/007, PROTO-003 |
| `same-coordinate-edit-composition` | M7 | WRITE-006/007/029, PROTO-003 |
| `protocol-crate-n-plus-one` | M7 | GOAL-006, PROTO-001/002 |
| `nip42-write-and-reconnect` | M8 | RELAY-006/007, ID-006/007 |
| `auth-account-isolation` | M8 | RELAY-007, ID-001 |
| `hostile-relay-ingress` | M8 | EVENT-001, RELAY-012 |
| `relay-limit-shortfall` | M8 | RELAY-004/009, OPS-004 |
| `ambiguous-handoff` | M8 | WRITE-020, RELAY-005 |
| `attempt-ceiling` | M8 | WRITE-019, OPS-003 |
| `provider-failure-isolation` | M8 | GOAL-008, OPS-004/009 |
| `persistent-cache-restart` | M9 | EVENT-004/005/011, PROFILE-002 |
| `ephemeral-cache-restart` | M9 | EVENT-004, PROFILE-003/004 |
| `nip11-cache-freshness` | M9 | EVENT-010, RELAY-009 |
| `nip05-cache-isolation` | M9 | EVENT-010, RELAY-010 |
| `destructive-reset` | M9 | EVENT-012 |
| `provider-matrix` | M10 | GOAL-003/005/009/010 |
| `external-router` | M10 | ROUTER contracts, architecture falsifier A |
| `planner-substitution` | M10 | RELAY-002/003, architecture falsifier J |
| `teardown-resource-baseline` | M10 | OPS-004/009/011 |
| `public-relay-recon` | M0+ | reconnaissance only |

# 10. Release definition

The rewrite is release-candidate-ready when:

1. M0–M10 deterministic Rust gates pass against the declared standard profile.
2. All enabled canary scenarios produce complete evidence bundles.
3. At least two real third-party relay implementations pass the core interoperability subset.
4. The adversarial relay corpus proves hostile-input isolation.
5. Provider conformance kits include at least one non-standard implementation for every claimed replaceable seam.
6. No default provider uses privileged internal access.
7. Architecture dependency-negative tests pass.
8. Open product decisions remain explicitly unpromised rather than silently implemented as product guarantees.
9. M11 platform artifacts pass their selected parity subset in real processes.
10. Profile-specific resource/performance budgets pass on release builds.
