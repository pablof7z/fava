# Requirements: Fava

**Defined:** 2026-08-21
**Core Value:** Applications can rely on coherent live queries and durable writes with exact, bounded, failure-isolated lifecycle and evidence semantics across replaceable provider compositions.

## Validated Baseline

- ✓ **M0-01**: An ordinary canary can publish and query a genuinely signed event through real WebSocket frames against a third-party relay process.
- ✓ **M0-02**: The canary can hard-kill and restart the relay against the same data directory and independently prove the event remains queryable.
- ✓ **M0-03**: Every M0 assertion is reconstructable from bounded process, wire, manifest, report, and JSONL evidence under one run identity.
- ✓ **M0-04**: Enabled evidence scenarios fail on unavailable prerequisites or scenario errors and never silently skip.
- ✓ **M0-05**: The canary proves M0 without depending on Fava implementation crates.

M0 is complete and is a prerequisite baseline, not an active roadmap phase. M1-M6 are also complete; their 66 requirements are checked below from the focused milestone records, implementation commits, current validation, and retroactive phase verification reports. M7-M11 remain active.

## v1 Requirements

Requirements for the Fava release. Every requirement is normative; mechanisms recommended by research remain provisional until their owning vertical slice qualifies them.

### Local Semantic State

- [x] **LOCAL-01**: Applications observe deterministic event identity, replaceable/addressable winner selection, deletion, expiry, ordering, and evidence merge semantics.
- [x] **LOCAL-02**: An event-cache provider accepts only admitted signed relay events and cannot retain unpublished local materializations.
- [x] **LOCAL-03**: A write-store provider exposes current local unsigned and signed materializations as an independent query source.
- [x] **LOCAL-04**: Contributions for the same event from event cache and write store merge into one `EventRecord` with source-specific evidence.
- [x] **LOCAL-05**: A pending local replaceable event can shadow a cached predecessor without mutating or deleting that predecessor in the event cache.
- [x] **LOCAL-06**: Cancelling a local write retracts only its write-store contribution and naturally reveals any still-qualified cached predecessor.
- [x] **LOCAL-07**: Removal, deletion, expiry, or eviction of a source contribution revises every affected open query without a parallel removal API.
- [x] **LOCAL-08**: Opening a local query is all-or-nothing and returns one complete current snapshot without waiting for relay work.
- [x] **LOCAL-09**: Equivalent query descriptions, including access context, acquisition scope, and result authority, have stable semantic identity.
- [x] **LOCAL-10**: Current-state delivery is bounded, may coalesce intermediate states, and always rebases the consumer onto one exact latest result.
- [x] **LOCAL-11**: Applications can inspect cache and write-store evidence through public event records without seeing provider storage internals.
- [x] **LOCAL-12**: The same semantic corpus passes through memory event-cache and write-store providers and the public Fava facade without relay, transport, or runtime networking dependencies.

### Live Relay Queries

- [x] **READ-01**: Applications can open a live query against an exact, non-empty explicit relay list without invoking automatic routers.
- [x] **READ-02**: Opening a live query starts relay work immediately when live freshness is requested.
- [x] **READ-03**: Product transport sends and receives exact bounded NIP-01 wire messages over real sockets.
- [x] **READ-04**: Every inbound event is attributed to an accepted relay session, request, generation, access context, and subscription before admission.
- [x] **READ-05**: Invalid-id, invalid-signature, malformed, off-filter, stale-generation, and post-terminal events cannot affect cache state or query results.
- [x] **READ-06**: EOSE evidence exists only after the actual relay frame and remains scoped to the exact relay request and generation.
- [x] **READ-07**: Applications can distinguish empty-plus-EOSE, silence, failure, authentication required, NOTICE, CLOSED, timeout, cancellation, and shortfall.
- [x] **READ-08**: A query remains live after EOSE and delivers later matching events without application resubscription.
- [x] **READ-09**: Query cancellation performs exact withdrawal, wakes pending pulls, and prevents later application delivery for the cancelled generation.
- [x] **READ-10**: Query close is idempotent and deterministically releases owned relay, task, queue, and subscription resources.
- [x] **READ-11**: The same event served by several relays appears once with evidence for every relay that actually served it.
- [x] **READ-12**: A relay that was planned or contacted but did not serve an event is never credited as provenance for that event.
- [x] **READ-13**: Reconnect restores active demand under fresh session and request generation identity without application resubscription.
- [x] **READ-14**: Reconnect does not imply that events missed during an outage were recovered or that history is complete.
- [x] **READ-15**: Source- or provenance-only changes revise an existing event record without duplicating the event.
- [x] **READ-16**: Slow current-state consumers receive an exact bounded latest result with truthful coalescing and loss diagnostics.
- [x] **READ-17**: Causal receipt and lifecycle facts use loss-honest delivery separate from coalescible current-state snapshots.
- [x] **READ-18**: Repeated cancellation and retry of pending pulls cannot accumulate an update backlog or retain stale waiters.
- [x] **READ-19**: Public diagnostics identify query, relay session, access context, request generation, logical demand, wire subscription, terminal reason, and source counts without private inspection.
- [x] **READ-20**: The declared standard profile keeps at least 1,000 simultaneous idle observations within explicit task, memory, descriptor, and queue bounds.

### Routing and Subscription Planning

- [x] **ROUTE-01**: Automatic routing evaluates the application-selected router chain in configured order.
- [x] **ROUTE-02**: Every router produces an immediate complete current contribution and may later replace that contribution as its facts change.
- [x] **ROUTE-03**: A slow or blocked router cannot delay destinations already known from other router contributions.
- [x] **ROUTE-04**: Downstream routers react to the live accumulated upstream plan without taking ownership of upstream facts.
- [x] **ROUTE-05**: Identical relay destinations deduplicate while preserving every contributing reason, target, and unresolved need.
- [x] **ROUTE-06**: Explicit routing creates no automatic router session or router-owned acquisition work.
- [x] **ROUTE-07**: Router-owned acquisition uses explicit sources and cannot recursively invoke automatic routing.
- [x] **ROUTE-08**: Route preview uses the same derivation as real routing while creating no write, receipt, signing, delivery lane, or router acquisition.
- [x] **ROUTE-09**: Subscription planning receives logical demand already assigned to one relay session and remains separate from routing policy.
- [x] **ROUTE-10**: Planner grouping may change wire shape but cannot change query meaning, evidence, access isolation, or cancellation.
- [x] **ROUTE-11**: Relay limits and bounded router contribution/fan-out budgets yield exact typed shortfall instead of silent dropped demand.

### Durable Publication

- [x] **WRITE-01**: Applications can accept unsigned events and verified pre-signed events through one durable write-intent lifecycle.
- [x] **WRITE-02**: An unsigned event's author identity selects the signer without conflating authorship with relay authentication identity.
- [x] **WRITE-03**: `Accepted` is returned only after the write obligation, current materialization, receipt identity, and recovery cursor are durably committed.
- [x] **WRITE-04**: Matching queries expose the accepted local materialization directly from the write store before relay acknowledgement.
- [x] **WRITE-05**: No unsigned or unpublished local event is copied into the event cache; only an admitted signed relay echo may enter it.
- [x] **WRITE-06**: Exact explicit publication routes bypass automatic routers.
- [x] **WRITE-07**: A publisher owns one transport handoff attempt while delivery policy alone decides retry, scheduling, and give-up.
- [x] **WRITE-08**: Every destination outcome preserves exact relay text, attempt identity, generation, acknowledgement, rejection, ambiguity, cancellation, and terminal reason.
- [x] **WRITE-09**: Proven pre-handoff cancellation produces zero `EVENT` frames, retracts the local query contribution, and records an exact idempotent terminal receipt state.
- [x] **WRITE-10**: Receipt removal is separate from write cancellation and obeys explicit retention and lifecycle rules.
- [x] **WRITE-11**: A hard process kill after acceptance recovers one obligation, the same write and receipt identities, and the current materialization without application resubmission.
- [x] **WRITE-12**: The application-selected router chain is the only automatic write-routing policy.
- [x] **WRITE-13**: Outbox routing acquires kind:10002 facts through explicit indexer queries owned by its router crate.
- [x] **WRITE-14**: Hint routing uses pointer-like hints and admitted relay evidence through its own independently selectable crate.
- [x] **WRITE-15**: App-relay routing always contributes configured relays according to its documented read/write scope.
- [x] **WRITE-16**: Fallback routing contributes and retracts independently as upstream target coverage changes.
- [x] **WRITE-17**: Known destinations begin delivery immediately while other recipient or route needs remain unresolved.
- [x] **WRITE-18**: Later route destinations create new delivery lanes under the same receipt and signed event without duplicate sends to existing destinations.
- [x] **WRITE-19**: Duplicate destination contributions cannot create duplicate publication handoffs.
- [x] **WRITE-20**: A removed desired route can retire only work proven not to have crossed a handoff boundary; historical delivery facts remain exact.
- [x] **WRITE-21**: Automatic routes continue to re-evaluate while work remains open, using exact route revision and lane generation identity.
- [x] **WRITE-22**: Route preview and initial real routing are identical when their input facts do not change.
- [x] **WRITE-23**: Route contributions, destinations, attempts, retries, receipt facts, and retained history have explicit bounds or typed refusal/shortfall.

### Semantic Writes and Capabilities

- [ ] **CAP-01**: Protocol capability crates expose ordinary event values or semantic replaceable-event edits and their inverses without signing, routing, publishing, or owning receipts.
- [ ] **CAP-02**: Actor identity exists on a semantic edit before materialization and becomes the author of every resulting event generation.
- [ ] **CAP-03**: A first-value semantic operation can materialize when no prior replaceable event exists.
- [ ] **CAP-04**: A newer qualified source event rematerializes still-live operations while preserving unrelated source changes.
- [ ] **CAP-05**: One write and receipt identity remains stable across materialization generations.
- [ ] **CAP-06**: Signer, route, publisher, and delivery completions for retired materialization generations are attributable and inert.
- [ ] **CAP-07**: At least two unrelated protocol capability crates prove the semantic-edit contract is not shaped around one NIP.
- [ ] **CAP-08**: Adding capability N+1 changes only its crate and selected assembly/artifact metadata, with zero universal-core behavior changes.
- [ ] **CAP-09**: Raw arbitrary and future Nostr event kinds remain usable without adding universal-core switches over event-kind meaning.

### Authentication, Hostility, and Bounds

- [ ] **HARD-01**: Relay NIP-42 authentication is explicit, generation-scoped, and separate from event authorship and query filter identity.
- [ ] **HARD-02**: Denial or failure of one account's authentication policy terminates only the exact affected operation and cannot block another account.
- [ ] **HARD-03**: Invalid, malformed, oversized, off-filter, stale, post-CLOSED, never-EOSE, truncated, silent-limit, and disconnected relay behavior remains scoped and attributable.
- [ ] **HARD-04**: NIP-11 limits produce a valid plan or exact shortfall before knowingly invalid work is sent.
- [ ] **HARD-05**: Offline or unreachable time is distinct from a failed delivery attempt and does not consume the attempt budget.
- [ ] **HARD-06**: Real retryable attempts reach the configured terminal give-up policy within declared ceilings.
- [ ] **HARD-07**: A completed handoff without a received relay outcome remains ambiguous and is never rewritten as acknowledged, rejected, or never sent.
- [ ] **HARD-08**: Every externally influenced input, queue, set, fan-out, retained history, diagnostic stream, and artifact has an explicit bound, backpressure rule, refusal, or shortfall.
- [ ] **HARD-09**: Provider panic, blocking, late result, malformed result, or ignored cancellation cannot block unrelated queries, relays, writes, or shutdown.
- [ ] **HARD-10**: Deterministic hostile scenarios use real sockets and separate processes, and publish resource envelopes and failure evidence for every run.

### Profiles and Services

- [ ] **PROF-01**: The baseline event-cache contract remains coherent without implying persistence, retention, coverage, or restart guarantees it does not own.
- [ ] **PROF-02**: A persistent profile provides its declared cold-cache reuse, provenance, deletion/expiry, coverage, and restart behavior without global completeness claims.
- [ ] **PROF-03**: An ephemeral event-cache profile restarts without cached relay events while accepted writes recover when its selected write store is durable.
- [ ] **PROF-04**: Event-cache eviction revises current queries coherently and adjusts any cache-owned coverage evidence.
- [ ] **PROF-05**: NIP-05 and NIP-11 independently own validation, freshness, negative caching, stale results, and failure semantics.
- [ ] **PROF-06**: A generic fetch cache stores opaque service payloads and may be physically shared without semantic leakage between NIP-05 and NIP-11.
- [ ] **PROF-07**: Every persistent provider owns and validates its private schema, version, migration, corruption, and refusal behavior.
- [ ] **PROF-08**: Destructive reset clears exactly the selected profile's cache, write, session, and service state according to its public contract.
- [ ] **PROF-09**: Profile guarantees are generated or checked from explicit assembly, and the same application source proves persistent and ephemeral behavior by changing provider selection only.

### Provider Substitution

- [ ] **SUB-01**: Standard providers expose no privileged constructor, facade path, internal-state access, or test-only semantic capability unavailable to external providers.
- [ ] **SUB-02**: Public conformance kits execute unchanged against standard and materially different alternative implementations for every claimed replaceable seam.
- [ ] **SUB-03**: Applications select provider profiles by changing assembly and dependencies without editing universal core source.
- [ ] **SUB-04**: Replacing one provider requires no changes to unrelated providers or their owned behavior.
- [ ] **SUB-05**: Alternative router, event cache, durable write store, planner, transport, publisher, delivery policy, signer, and fetch cache implementations use only public contracts.
- [ ] **SUB-06**: Provider failures remain isolated and private persisted-format incompatibility remains owned by that provider rather than a global assembly identity.
- [ ] **SUB-07**: Dependency-negative tests reject forbidden semantic-owner, contract, provider, runtime, facade, and capability edges.
- [ ] **SUB-08**: Every architecture falsifier passes, provider change amplification remains narrow, and contract stabilization occurs only after the provider matrix succeeds.

### Native Products and Release

- [ ] **NATIVE-01**: Rust, Swift, Kotlin/JVM, Android, and iOS applications consume declared release artifacts without repository-relative sources or raw generated bindings.
- [ ] **NATIVE-02**: Native artifacts expose only the providers, profiles, and protocol capabilities selected by their assembly.
- [ ] **NATIVE-03**: Live-query open, current value, next/update, cancellation, close, and terminal behavior match Rust semantics in Swift and Kotlin.
- [ ] **NATIVE-04**: Event records, source evidence, route shortfall, receipts, errors, ambiguity, and restart outcomes map without semantic flattening across languages.
- [ ] **NATIVE-05**: Android fresh-process tests prove the declared persistent-profile recovery behavior through ordinary application artifacts.
- [ ] **NATIVE-06**: Any iOS profile claiming suspension transparency proves suspension and resume behavior on a physical device.
- [ ] **NATIVE-07**: Repeated native lifecycle cycles return tasks, handles, descriptors, Rust memory, and native heap to declared baseline envelopes.
- [ ] **NATIVE-08**: The release candidate passes the shared Rust/Swift/Kotlin parity corpus, parity mutations, real-process evidence, two-relay interoperability subset, hostile corpus, provider matrix, and declared release-build resource budgets.

## Definition of Done

Every v1 requirement is complete only when:

1. Its owning behavior and responsibility are explicit in the authoritative specifications or a focused approved change.
2. The smallest executable proof failed before implementation for the intended reason.
3. The owning component proof passes through public contracts.
4. Its named deliberate break makes the proof fail causally.
5. A public Fava capstone proves any additional facade, process, relay, persistence, or platform boundary.
6. Independent wire, process, relay, storage, or native evidence exists wherever Fava cannot be its own witness.
7. Failure, cancellation, late completion, teardown, diagnostics, and resource bounds are exact and attributable.
8. Contract, provider-conformance, profile, canary, and ownership documentation is current.
9. The complete scoped validation set passes and the implementation is committed.

## v2 Requirements

None. M1-M11 together define the first Fava release; normative work is not silently deferred to an unspecified later version.

## Decisions Deferred Within v1

These remain explicitly unpromised unless resolved in their owning milestones:

- Public growable-window/query API and resume-token model.
- Cancellation semantics after partial relay handoff.
- Whether any profile promises outage-interval backfill.
- Retention of full historical attempt detail beyond exact current receipt evidence.
- Which persistent event-cache guarantee profile is recommended for the primary shipped artifact.

## Out of Scope

| Feature | Reason |
|---------|--------|
| Previous NMP implementation code or compatibility paths | Fava is a clean-room rewrite from authoritative documents |
| Application framework, UI state, navigation, ranking, moderation, and account UX | These remain application-owned product concerns |
| Runtime plugin discovery or hot-swappable providers | Provider selection is explicit static composition for an engine instance |
| Global synced, complete, authoritative-empty, percentage, or end-of-history claims | Relay evidence is exact and source-scoped, never global truth |
| Automatic negentropy or a parallel history workload | Ordinary reads remain declarative NIP-01 live queries unless a future explicit protocol scope is approved |
| Unsigned or unpublished local events in the event cache | The write store is their independent query authority |
| Silent truncation, fallback, compatibility, clamping, or hidden feature flags | Bounds, refusal, policy, and shortfall must remain explicit |
| Provider-specific private facade bypasses | Replaceability requires identical public contracts and conformance paths |
| Cross-provider persisted-format compatibility by default | Each provider owns its private bytes and migration/refusal behavior |
| Public-relay availability as a deterministic release gate | Controlled real third-party relay processes own repeatable pass/fail evidence |

## Traceability

Every v1 requirement maps to exactly one active phase. M0 remains a completed prerequisite baseline outside the active roadmap. `Complete` denotes an evidence-backed owning milestone verdict; Phases 1-6 were executed before GSD phase artifacts existed and are reconciled without retrospective PLAN.md or SUMMARY.md files.

| Requirement | Phase | Status |
|-------------|-------|--------|
| LOCAL-01 | Phase 1 | Complete |
| LOCAL-02 | Phase 1 | Complete |
| LOCAL-03 | Phase 1 | Complete |
| LOCAL-04 | Phase 1 | Complete |
| LOCAL-05 | Phase 1 | Complete |
| LOCAL-06 | Phase 1 | Complete |
| LOCAL-07 | Phase 1 | Complete |
| LOCAL-08 | Phase 1 | Complete |
| LOCAL-09 | Phase 1 | Complete |
| LOCAL-10 | Phase 1 | Complete |
| LOCAL-11 | Phase 1 | Complete |
| LOCAL-12 | Phase 1 | Complete |
| READ-01 | Phase 2 | Complete |
| READ-02 | Phase 2 | Complete |
| READ-03 | Phase 2 | Complete |
| READ-04 | Phase 2 | Complete |
| READ-05 | Phase 2 | Complete |
| READ-06 | Phase 2 | Complete |
| READ-07 | Phase 2 | Complete |
| READ-08 | Phase 2 | Complete |
| READ-09 | Phase 2 | Complete |
| READ-10 | Phase 2 | Complete |
| READ-11 | Phase 3 | Complete |
| READ-12 | Phase 3 | Complete |
| READ-13 | Phase 3 | Complete |
| READ-14 | Phase 3 | Complete |
| READ-15 | Phase 3 | Complete |
| READ-16 | Phase 3 | Complete |
| READ-17 | Phase 3 | Complete |
| READ-18 | Phase 3 | Complete |
| READ-19 | Phase 3 | Complete |
| READ-20 | Phase 3 | Complete |
| ROUTE-01 | Phase 4 | Complete |
| ROUTE-02 | Phase 4 | Complete |
| ROUTE-03 | Phase 4 | Complete |
| ROUTE-04 | Phase 4 | Complete |
| ROUTE-05 | Phase 4 | Complete |
| ROUTE-06 | Phase 4 | Complete |
| ROUTE-07 | Phase 4 | Complete |
| ROUTE-08 | Phase 4 | Complete |
| ROUTE-09 | Phase 4 | Complete |
| ROUTE-10 | Phase 4 | Complete |
| ROUTE-11 | Phase 4 | Complete |
| WRITE-01 | Phase 5 | Complete |
| WRITE-02 | Phase 5 | Complete |
| WRITE-03 | Phase 5 | Complete |
| WRITE-04 | Phase 5 | Complete |
| WRITE-05 | Phase 5 | Complete |
| WRITE-06 | Phase 5 | Complete |
| WRITE-07 | Phase 5 | Complete |
| WRITE-08 | Phase 5 | Complete |
| WRITE-09 | Phase 5 | Complete |
| WRITE-10 | Phase 5 | Complete |
| WRITE-11 | Phase 5 | Complete |
| WRITE-12 | Phase 6 | Complete |
| WRITE-13 | Phase 6 | Complete |
| WRITE-14 | Phase 6 | Complete |
| WRITE-15 | Phase 6 | Complete |
| WRITE-16 | Phase 6 | Complete |
| WRITE-17 | Phase 6 | Complete |
| WRITE-18 | Phase 6 | Complete |
| WRITE-19 | Phase 6 | Complete |
| WRITE-20 | Phase 6 | Complete |
| WRITE-21 | Phase 6 | Complete |
| WRITE-22 | Phase 6 | Complete |
| WRITE-23 | Phase 6 | Complete |
| CAP-01 | Phase 7 | Pending |
| CAP-02 | Phase 7 | Pending |
| CAP-03 | Phase 7 | Pending |
| CAP-04 | Phase 7 | Pending |
| CAP-05 | Phase 7 | Pending |
| CAP-06 | Phase 7 | Pending |
| CAP-07 | Phase 7 | Pending |
| CAP-08 | Phase 7 | Pending |
| CAP-09 | Phase 7 | Pending |
| HARD-01 | Phase 8 | Pending |
| HARD-02 | Phase 8 | Pending |
| HARD-03 | Phase 8 | Pending |
| HARD-04 | Phase 8 | Pending |
| HARD-05 | Phase 8 | Pending |
| HARD-06 | Phase 8 | Pending |
| HARD-07 | Phase 8 | Pending |
| HARD-08 | Phase 8 | Pending |
| HARD-09 | Phase 8 | Pending |
| HARD-10 | Phase 8 | Pending |
| PROF-01 | Phase 9 | Pending |
| PROF-02 | Phase 9 | Pending |
| PROF-03 | Phase 9 | Pending |
| PROF-04 | Phase 9 | Pending |
| PROF-05 | Phase 9 | Pending |
| PROF-06 | Phase 9 | Pending |
| PROF-07 | Phase 9 | Pending |
| PROF-08 | Phase 9 | Pending |
| PROF-09 | Phase 9 | Pending |
| SUB-01 | Phase 10 | Pending |
| SUB-02 | Phase 10 | Pending |
| SUB-03 | Phase 10 | Pending |
| SUB-04 | Phase 10 | Pending |
| SUB-05 | Phase 10 | Pending |
| SUB-06 | Phase 10 | Pending |
| SUB-07 | Phase 10 | Pending |
| SUB-08 | Phase 10 | Pending |
| NATIVE-01 | Phase 11 | Pending |
| NATIVE-02 | Phase 11 | Pending |
| NATIVE-03 | Phase 11 | Pending |
| NATIVE-04 | Phase 11 | Pending |
| NATIVE-05 | Phase 11 | Pending |
| NATIVE-06 | Phase 11 | Pending |
| NATIVE-07 | Phase 11 | Pending |
| NATIVE-08 | Phase 11 | Pending |

**Coverage:**
- v1 requirements: 110 total
- Mapped to phases: 110 ✓
- Unmapped: 0
- Duplicate mappings: 0

---
*Requirements defined: 2026-08-21*
*Last updated: 2026-08-21 after roadmap generation*
