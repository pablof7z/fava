# Fava Architecture

**Status:** proposed target architecture for the rewrite  
**Audience:** Fava implementors, provider authors, protocol-crate authors, SDK authors, and application developers
**Authority:** `FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md` defines what Fava must do; this document defines where responsibilities, state, lifecycles, and replaceable interfaces belong. `FAVA_TDD_BDD_TESTING_GUIDE.md` defines how those claims are specified and proved.

## Purpose

Fava is an embeddable Nostr client engine assembled from focused crates. An application chooses the implementation crates that make up its Fava build: event cache, write store, routers, subscription planner, transport, publisher, delivery policy, signers, protocol services, and event-kind protocol crates.

The architecture has four goals:

1. **Small, explicit ownership.** Every mutable fact, lifecycle, queue, retry loop, and external resource has one owner.
2. **Build-time substitutability.** Different applications can select different implementation crates without forking Fava or modifying unrelated crates.
3. **One set of primitives.** Querying, event construction, signing, publishing, routing composition, and event-state rules each have one primitive contract. Protocol crates compose those primitives.
4. **Behavioral integrity across compositions.** Provider choice may change policy where the contract allows it; it does not change universal Nostr validity, evidence meaning, query continuity, write identity, or lifecycle correctness.

The architecture is deliberately organized around responsibility rather than crate size. A five-line routing policy is still a higher-level routing policy and belongs outside the primitive routing crate when keeping it separate is what makes it independently selectable.

---

## Architectural mental model

### Static application composition

Provider selection happens when the application assembles its Fava build.

```rust
let engine = Fava::builder()
    .event_cache(MyEventCache::open(cache_path)?)
    .write_store(MyWriteStore::open(write_path)?)
    .query_evaluator(StandardQueryEvaluator::new())
    .routers(vec![
        Box::new(OutboxRouter::new(indexer_relays)),
        Box::new(HintRouter::new()),
        Box::new(AppRelayRouter::new(app_relays)),
    ])
    .subscription_planner(StandardSubscriptionPlanner::new())
    .transport(WebSocketTransport::new())
    .publisher(Nip01Publisher::new())
    .delivery_policy(StandardDeliveryPolicy::new())
    .build()?;
```

Another application may compile a different combination:

```rust
let engine = Fava::builder()
    .event_cache(BoundedMemoryEventCache::new(50_000))
    .write_store(SqliteWriteStore::open("writes.sqlite")?)
    .query_evaluator(CompanyQueryEvaluator::new(...))
    .routers(vec![
        Box::new(CompanyDirectoryRouter::new(...)),
        Box::new(HintRouter::new()),
        Box::new(FallbackRelayRouter::new(fallback_relays)),
    ])
    .subscription_planner(NoGroupingPlanner::new())
    .transport(CompanyTransport::new(...))
    .publisher(GatewayPublisher::new(...))
    .delivery_policy(CompanyDeliveryPolicy::new(...))
    .build()?;
```

The exact Rust mechanism may use generics, trait objects, generated assembly code, or a combination. The architectural property is that the selected set is fixed for the engine instance and built into the application artifact. Swift and Kotlin products compile one selected assembly into their native library.

### One owner per responsibility

Fava distinguishes responsibilities even when one physical implementation serves several of them.

- The **event cache** owns cached relay-observed event state.
- The **write store** owns accepted local write obligations, current materializations, receipts, routes, and delivery facts.
- A **fetch cache** stores opaque cached service payloads for owners such as NIP-05 or NIP-11.
- The **query owner** merges query sources and owns observation continuity.
- The **routing chain** owns the current desired relay plan.
- The **publication owner** owns one accepted write lifecycle.
- The **transport** owns relay sessions and byte handoff.

There may be several physical databases, or one physical database exposed through several provider types. There is one authority for each responsibility.

### Contracts and implementations are separate crates

A replaceable subsystem has:

1. a neutral contract crate;
2. one or more implementation crates; and
3. a conformance suite owned by the contract.

For example:

```text
fava-routing
    ├── fava-router-outbox
    ├── fava-router-hints
    ├── fava-router-app-relays
    ├── fava-router-fallback-relays
    └── third-party router crates
```

The standard implementation uses the same interface available to an external crate. Universal owners depend on the contract, not on the standard implementation.

### Types live with their owner

Shared types are exported by the crate that defines their meaning:

- wire frames live in `fava-wire`;
- cached event state and relay evidence live in `fava-state`;
- query descriptions, `EventRecord`, snapshots, and query evidence live in `fava-query`;
- unsigned events, write intents, receipts, and delivery facts live in `fava-write`;
- route requests, contributions, plans, and route evidence live in `fava-routing`;
- signer requests and outcomes live in `fava-signer`.

A type used by several crates still has one owner. Other crates depend on that owner rather than moving the type into a common bucket.

### Primitive and higher-layer crates

Primitive crates define general machinery. Higher-layer crates encode one particular policy or protocol meaning.

Examples:

- `fava-routing` defines ordered asynchronous router composition.
- `fava-router-hints` defines one particular interpretation of relay hints.
- `fava-router-outbox` defines one particular NIP-65-based outbox/inbox algorithm.
- `fava-write` defines the event-construction and write-intent primitives.
- `fava-nip02` defines follow/unfollow semantics by composing those primitives.

A higher-layer policy remains a separate crate even when its implementation is very small. This preserves independent selection and prevents the primitive contract from silently privileging one policy.

### Reactive contributions and partial progress

Several subsystems produce knowledge over time. Their contracts are signal-like:

- an immediate complete current snapshot;
- later complete replacement snapshots when knowledge changes;
- explicit unresolved needs and shortfalls; and
- cancellation that releases exactly the work owned by that session.

A slow provider does not block other providers. Work begins from currently available facts and expands as new facts arrive.

This is especially load-bearing for routing. A write that p-tags three recipients may immediately have destinations for two recipients plus application relays. Publication begins to those destinations while the third recipient's route is still being acquired. A later router update adds the newly learned destination under the same receipt.

### Decisions, commits, effects, and facts

Fava uses a facts-before-effects flow:

```text
command or observed fact
        ↓
deterministic owner decision
        ↓
required durable commit
        ↓
committed fact
        ↓
external effect
        ↓
correlated completion fact
```

The durable owner defines the truth boundary for durable state. External I/O happens after the state authorizing it is committed. Completions carry exact operation and generation identity and are applied only while still current.

This is a mental model, not a requirement for one universal effect enum. Each owner may expose the smallest typed command/fact/effect vocabulary appropriate to its lifecycle.

### Event authorship

An unsigned or signed event carries its author in the event's `pubkey` field. Signer selection uses that pubkey.

Before an event exists, a replaceable-event edit carries its actor and event coordinate:

```rust
pub struct ReplaceableEventEdit {
    pub actor: PublicKey,
    pub coordinate: EventCoordinate,
    pub format: u32,
    pub change: Bytes,
}
```

Materialization creates an unsigned event whose `pubkey` is the actor. Current-account convenience APIs resolve the account before the write enters the accepted write lifecycle.

### Query results are merged source state

A live query's local result is the deterministic merge of several query sources, primarily:

- the event cache, containing signed relay-observed events and relay evidence; and
- the write store, containing current local materializations and publication evidence.

The write store supplies unpublished events directly to queries, while the event cache remains the source of relay-observed signed events.

### Cache guarantees belong to cache implementations and profiles

`EventCache` describes coherent cache behavior. An implementation may be memory-only, bounded, persistent, or backed by a remote service. Its documented retention and restart guarantees become guarantees of the application profile that selected it.

`fava-standard` may select a persistent event cache and advertise cold restart reads. A smaller assembly may select a bounded memory cache and advertise only current-process reuse.

The write-store profile selected by the standard product provides durable accepted-write recovery. Test and deliberately weaker profiles may use memory implementations.

---

## Top-level system map

```text
                                APPLICATION
                                     │
                                     ▼
                                fava facade
          ┌──────────────────────────┼──────────────────────────┐
          ▼                          ▼                          ▼
    fava-observe                fava-publication              fava-session
          │                          │                          │
          │                          │                          └── signers
          │                          │
          │                          ├── fava-write-store
          │                          ├── fava-routing
          │                          ├── fava-delivery
          │                          ├── fava-publisher
          │                          └── fava-signer
          │
          ├── fava-event-cache
          ├── fava-write-store as QuerySource
          ├── fava-query + selected QueryEvaluator
          ├── fava-routing
          ├── fava-subscriptions
          └── fava-transport

relay bytes ──► fava-wire ──► fava-ingest ──► fava-state ──► fava-event-cache
                                             │
                                             └──── committed cache changes
                                                          │
                                                          ▼
                                                      fava-observe
```

Routing is an ordered reactive chain:

```text
Auto route request
      │
      ▼
OutboxRouter ──► HintRouter ──► AppRelayRouter ──► FallbackRelayRouter
      │               │                 │                    │
      └──── asynchronous complete route contributions ──────┘
                              │
                              ▼
                         fava-routing
                              │
                              ▼
                         live RoutePlan
```

Explicit routes bypass this chain and produce an exact route plan directly.

---

## Crate families

| Family | Crates | Purpose |
|---|---|---|
| Protocol and domain rules | `fava-wire`, `fava-state`, `fava-query`, `fava-write` | Stable values, contracts, and deterministic universal rules |
| Storage contracts | `fava-event-cache`, `fava-write-store`, `fava-fetch-cache` | One contract per storage responsibility |
| Routing | `fava-routing` plus router implementation crates | Ordered asynchronous relay-plan composition |
| Relay execution | `fava-subscriptions`, `fava-transport`, `fava-publisher`, `fava-delivery` | Group demand, move bytes, perform attempts, schedule retries |
| Identity | `fava-signer`, signer implementation crates, `fava-session`, `fava-auth` | Account, signing, crypto, and relay-access lifecycles |
| Protocol services | `fava-nip11`, `fava-nip05` and later service crates | Non-event protocol acquisition and interpretation |
| Protocol crates | `fava-nip02`, `fava-nip29`, `fava-bookmarks`, etc. | Event-kind meaning, typed queries, event construction, and replaceable-event edits |
| Universal owners | `fava-ingest`, `fava-observe`, `fava-publication`, `fava-diagnostics`, `fava-runtime` | Fava-instance lifecycles and cross-subsystem ordering |
| Product assembly | `fava`, `fava-standard`, `fava-ffi`, Swift/Kotlin packages | Public facade, default profile, and platform artifacts |

# Part I — Protocol and domain-rule crates

## `fava-wire`

**Responsibility:** Nostr relay message encoding and decoding.

### Public contract

Illustrative shape:

```rust
pub enum ClientMessage {
    Req {
        subscription: SubscriptionId,
        filters: Vec<Filter>,
    },
    Close {
        subscription: SubscriptionId,
    },
    Event(Event),
    Auth(Event),
}

pub enum RelayMessage {
    Event {
        subscription: SubscriptionId,
        event: Event,
    },
    Eose {
        subscription: SubscriptionId,
    },
    Ok {
        event_id: EventId,
        accepted: bool,
        message: String,
    },
    Closed {
        subscription: SubscriptionId,
        message: String,
    },
    Auth {
        challenge: String,
    },
    Notice(String),
}

pub fn decode_relay_frame(bytes: &[u8])
    -> Result<RelayMessage, WireError>;

pub fn encode_client_message(message: &ClientMessage)
    -> Result<Bytes, WireError>;

pub fn encoded_len(message: &ClientMessage)
    -> Result<usize, WireError>;
```

### Owned meaning

- exact NIP-01 message shapes;
- exact client-message serialization;
- relay-message parsing;
- typed malformed-frame errors;
- exact byte length used for relay-advertised message limits;
- bounded preservation of relay-provided message text.

### Relationship to other crates

`fava-transport` moves bytes and owns sessions. `fava-wire` gives those bytes protocol meaning. `fava-ingest` determines whether a decoded relay event is attributable and valid.

`fava-wire` can therefore be used and tested independently of sockets and retry policy.

---

## `fava-state`

**Responsibility:** deterministic semantics for signed event state learned from relays.

`fava-state` defines the event-cache state model. It is independent of a particular cache implementation.

### Core values

```rust
pub struct RelayObservation {
    pub relay: RelayUrl,
    pub access: RelayAccess,
    pub observed_at: Timestamp,
}

pub struct RelayEvidence {
    pub observations: BTreeMap<RelaySessionKey, RelayObservation>,
}

pub struct CachedEvent {
    pub event: Event,
    pub evidence: RelayEvidence,
}

pub enum CacheMutation {
    Insert(CachedEvent),
    Replace {
        coordinate: EventCoordinate,
        previous: EventId,
        current: CachedEvent,
    },
    MergeEvidence {
        event_id: EventId,
        evidence: RelayEvidence,
    },
    Retract {
        event_id: EventId,
        cause: RetractionCause,
    },
    RecordTombstone(Tombstone),
}

pub struct EventStateDecision {
    pub mutations: Vec<CacheMutation>,
    pub affected: AffectedEventState,
}
```

### Deterministic operations

```rust
pub fn admit_observation(
    current: &StateSlice,
    event: VerifiedRelayEvent,
) -> Result<EventStateDecision, StateError>;

pub fn apply_expiration(
    current: &StateSlice,
    now: Timestamp,
) -> EventStateDecision;

pub fn select_replaceable_winner<'a>(
    candidates: impl IntoIterator<Item = &'a Event>,
) -> Option<&'a Event>;
```

### Owned semantics

- event-id deduplication;
- relay-evidence merge;
- ordinary and replaceable event identity, including addressable coordinates;
- deterministic winner selection;
- NIP-09 deletion authorization and tombstones;
- NIP-40 expiration consequences;
- prevention of resurrection within the cache guarantees provided by the selected implementation;
- exact affected-state descriptions used to invalidate queries.

### State/cache boundary

`fava-state` decides the semantic mutation. `fava-event-cache` commits it.

A typical flow is:

```text
fava-ingest obtains the relevant cache state slice
        ↓
fava-state calculates one mutation batch
        ↓
fava-event-cache commits the batch atomically
        ↓
CommittedCacheChange is published
```

The Fava instance has one serialized event-state writer. This allows a pure read/decide/commit boundary without requiring cache implementations to duplicate Nostr rules.

---

## `fava-write`

**Responsibility:** event construction and immutable publication vocabulary.

### Event forms

```rust
pub struct UnsignedEvent {
    pub id: EventId,
    pub pubkey: PublicKey,
    pub created_at: Timestamp,
    pub kind: Kind,
    pub tags: Vec<Tag>,
    pub content: String,
}

pub enum EventValue {
    Unsigned(UnsignedEvent),
    Signed(Event),
}
```

`UnsignedEvent.id` is computed from the unsigned event body. A signature converts an exact `UnsignedEvent` into an `Event` without changing its body, id, or pubkey.

### Event construction

```rust
pub struct EventBuilder { /* private fields */ }

impl EventBuilder {
    pub fn new(author: PublicKey, kind: Kind) -> Self;
    pub fn created_at(self, timestamp: Timestamp) -> Self;
    pub fn content(self, content: EventContent) -> Self;
    pub fn tag(self, tag: Tag) -> Self;
    pub fn build(self) -> Result<UnsignedEvent, EventBuildError>;
}
```

`EventBuilder` understands generic Nostr event fields and validated tags. It
does not interpret reply, reaction, repost, quote, follow, bookmark, group, or
other event-kind meaning. Protocol crates calculate those tags and compose the
builder rather than constructing another signing or publication path.

### Write vocabulary

```rust
pub enum WritePayload {
    Event(UnsignedEvent),
    Edit(ReplaceableEventEdit),
    Presigned(Event),
}

pub enum WriteRouting {
    Auto,
    Explicit(NonEmptyVec<RelayUrl>),
}

pub struct WriteIntent {
    pub payload: WritePayload,
    pub routing: WriteRouting,
}

pub struct ReceiptId(/* opaque */);
pub struct WriteId(/* opaque */);
```

### Publication evidence

```rust
pub enum SignatureState {
    AwaitingEvent,
    Unsigned,
    Signing,
    Signed,
    Refused(SignatureRefusal),
}

pub enum RelayDeliveryOutcome {
    Pending,
    AwaitingAuthentication,
    Acknowledged { message: String },
    Rejected { message: String },
    GivenUp { reason: GiveUpReason },
    Unknown { reason: AmbiguityReason },
}

pub struct PublicationEvidence {
    pub receipt_id: ReceiptId,
    pub write_id: WriteId,
    pub signature: SignatureState,
    pub destinations: BTreeMap<RelaySessionKey, RelayDeliveryOutcome>,
}
```

### Owned meaning

- one event-construction primitive;
- unsigned and signed event identity;
- unsigned events, replaceable-event edits, and pre-signed events;
- automatic and explicit routing values;
- stable write and receipt identities;
- exact delivery-fact vocabulary;
- cancellation, retirement, and terminal receipt values.

Durable queues, signer registries, routing algorithms, retry scheduling, and transport are supplied by their owning contracts and lifecycle crates.

---

## `fava-query`

**Responsibility:** declarative query values, local-source observation contracts, result merging, ordering, and query evidence.

### Query language

```rust
pub struct Query {
    /* private fields */
}

pub enum QueryRouting {
    Auto,
    Explicit(NonEmptyVec<RelayUrl>),
}

pub enum Selection {
    Filter(FilterSelection),
    Union(Vec<Selection>),
    Intersection(Vec<Selection>),
    Difference {
        include: Box<Selection>,
        exclude: Box<Selection>,
    },
    Derived(DerivedSelection),
}

impl Query {
    pub fn events() -> Query;
    pub fn from_relays(
        self,
        relays: impl IntoIterator<Item = RelayUrl>,
    ) -> Result<Query, QueryError>;
    pub fn only_from_relays(
        self,
        relays: impl IntoIterator<Item = RelayUrl>,
    ) -> Result<Query, QueryError>;
    pub fn union(
        queries: impl IntoIterator<Item = Query>,
    ) -> Result<Query, QueryError>;
}
```

Every `Query` is valid. Construction rejects invalid inputs. Equality and
hashing give queries that mean the same thing the same identity, regardless of
insignificant construction order, while retaining source, access, freshness,
and acquisition distinctions.

### Query-source contract

```rust
pub trait QuerySource: Send + Sync {
    fn open(
        &self,
        query: &Query,
    ) -> Result<OpenedQuerySource, QuerySourceError>;
}

pub struct OpenedQuerySource {
    pub initial: SourceSnapshot,
    pub changes: Box<dyn SourceChanges>,
}

pub struct SourceSnapshot {
    pub revision: SourceRevision,
    pub events: Vec<SourceEvent>,
}

pub trait SourceChanges: Send {
    fn next_change(&mut self) -> SourceChangeFuture<'_>;
    fn close(&mut self);
}
```

Opening a query source establishes one continuous source observation: the initial snapshot and all later changes form one gapless local sequence for that source.


### Query evaluator contract

The query language and source protocol are stable; the local evaluation strategy is replaceable.

```rust
pub trait QueryEvaluator: Send + Sync {
    fn evaluate(
        &self,
        query: &Query,
        sources: &[SourceSnapshot],
    ) -> Result<QuerySnapshot, QueryEvaluationError>;

    fn update(
        &self,
        query: &Query,
        previous: &QuerySnapshot,
        sources: &[SourceSnapshot],
        changed: &SourceChangeSet,
    ) -> Result<QuerySnapshot, QueryEvaluationError>;
}
```

An evaluator owns matching, derived-selection evaluation, cross-source merge, ordering, and whole-query limits. A source implementation owns efficient access to its own retained facts. `fava-query-standard` provides the reference evaluator and behavior oracle used by source/provider conformance suites.

### Source contributions

```rust
pub enum SourceEvent {
    Cached(CachedEvent),
    Local(LocalWriteEvent),
}

pub struct LocalWriteEvent {
    pub event: EventValue,
    pub publication: PublicationEvidence,
}
```

### Application-facing event value

```rust
pub struct EventRecord {
    pub event: EventValue,
    pub relay_evidence: RelayEvidence,
    pub publication: Option<PublicationEvidence>,
}
```

`EventRecord` is the query-domain value delivered to applications. It combines relay evidence from the event cache with local publication evidence from the write store.

### Result and change vocabulary

```rust
pub struct QuerySnapshot {
    pub revision: QueryRevision,
    pub events: Vec<EventRecord>,
    pub evidence: QueryEvidence,
}

pub enum QueryChange {
    ReplaceSnapshot(QuerySnapshot),
    Delta(QueryDelta),
}
```

The implementation may use snapshots, deltas, or both. Every change is defined relative to the last revision actually delivered to the consumer.

### Owned semantics

- valid-by-construction query identity;
- literal and derived query algebra;
- deterministic ordering and limits;
- merging event-cache and write-store contributions;
- deduplicating the same event id across sources;
- selecting the visible replaceable-event value across sources;
- combining relay and publication evidence;
- source-scoped query evidence;
- complete initial result and current-state update values.

### Source merge rules

1. **Same event id:** one `EventRecord`; relay and publication evidence merge.
2. **Pending local replacement over cached predecessor:** the local materialization participates in ordinary deterministic winner selection and is visible while current.
3. **Local cancellation or rematerialization:** removal of the local source contribution causes the cached candidate to become visible naturally when it is the next winner.
4. **Relay echo:** the event cache adds relay evidence to the same event id while the write store retains receipt and delivery evidence.
5. **Source failure:** one source's failure becomes scoped evidence; it does not erase the other source's valid state.

---

## Replaceable-event edits

`ReplaceableEventEdit` is a durable change to a replaceable Nostr event. It
identifies the actor and event coordinate and carries the protocol-owned edit
value needed to apply that change again.

Applying an edit to the newest qualified event at its coordinate, or to the
protocol crate's defined empty state, produces an `UnsignedEvent`. A newer
qualified event causes the same accepted edit to be applied again under the
same write and receipt identity.

Concrete protocol APIs produce these values:

```rust
fava_nip02::follow(actor, bob) -> ReplaceableEventEdit
fava_nip02::unfollow(actor, bob) -> ReplaceableEventEdit

fava_bookmarks::add(actor, target) -> ReplaceableEventEdit
fava_bookmarks::remove(actor, target) -> ReplaceableEventEdit
```

Each protocol crate owns the meaning and durable format of its edit values.
The application assembly supplies the protocol crates required to recover its
accepted edits. The write store owns durable custody, generations, receipts,
and current materializations.


---

# Part II — Storage roles and local query sources

## Storage-role model

Fava assembles distinct storage roles:

```text
EventCache
    cached signed events observed from relays

WriteStore
    accepted local edits/events, current materializations,
    receipts, routes, attempts, and outcomes

FetchCache
    opaque cached service payloads for NIP-05, NIP-11, and similar services
```

Each role has one provider in an engine assembly. A provider may internally use several physical databases or tiers. One physical database may expose several provider types. Fava coordinates semantic roles rather than arbitrary database instances.

The local query system observes `EventCache` and `WriteStore` independently and merges them through `fava-query` semantics. Additional query-source roles are introduced together with an explicit contribution type and universal merge rule, so source composition remains semantic rather than precedence-by-convention.

---

## `fava-event-cache`

**Responsibility:** retain and query signed event state learned from relays.

### Baseline contract

```rust
pub trait EventCache: QuerySource + Send + Sync {
    fn state_slice(
        &self,
        key: StateLookup,
    ) -> Result<StateSlice, EventCacheError>;

    fn commit(
        &self,
        mutations: Vec<CacheMutation>,
    ) -> Result<CommittedCacheChange, EventCacheError>;

    fn event(
        &self,
        id: EventId,
    ) -> Result<Option<CachedEvent>, EventCacheError>;

    fn maintain(
        &self,
        request: CacheMaintenance,
    ) -> Result<CacheMaintenanceResult, EventCacheError>;
}
```

### Owned state

- signed relay-observed events;
- relay and relay-access evidence;
- replaceable-event indexes;
- deletion tombstones retained according to the implementation's guarantee;
- expiration indexes;
- query indexes;
- optional historical coverage records;
- cache eviction and maintenance state.

### Baseline behavior

Every conforming implementation provides:

- coherent current-process query snapshots and change observations;
- atomic application of one `fava-state` mutation batch;
- deterministic reads matching the universal state/query semantics;
- explicit eviction or capacity shortfall rather than silent semantic corruption;
- source revisions sufficient for continuous query-source observation.

### Implementation guarantee profiles

Implementations may additionally qualify for named profiles:

- **Persistent cache:** cached event state and declared evidence survive process restart.
- **Persistent provenance:** relay evidence survives restart.
- **Persistent tombstones:** deletion suppression survives restart.
- **Persistent coverage:** scoped acquisition records survive restart and are lowered when cache eviction invalidates them.
- **Bounded memory:** current-process retention remains within a declared limit.
- **Rebuildable indexes:** indexes may be reconstructed from retained event data.

These are implementation and product-profile guarantees. They are not inferred from the `EventCache` name alone.

### Standard implementations

Proposed crates:

```text
fava-event-cache-memory
fava-event-cache-redb
```

A later implementation may use Fjall, SQLite, LMDB, a remote event service, or another cache design while preserving the baseline contract and declaring the guarantee profiles it actually satisfies.

---

## `fava-write-store`

**Responsibility:** commit and recover accepted local publication obligations and expose their current event materializations as a query source.

### Contract

```rust
pub trait WriteStore: QuerySource + Send + Sync {
    fn accept(
        &self,
        request: WriteAcceptance,
    ) -> Result<AcceptedWrite, WriteStoreError>;

    fn install_materialization(
        &self,
        update: MaterializationUpdate,
    ) -> Result<CommittedWriteChange, WriteStoreError>;

    fn install_signature(
        &self,
        update: SignatureUpdate,
    ) -> Result<CommittedWriteChange, WriteStoreError>;

    fn record_route_revision(
        &self,
        revision: WriteRouteRevision,
    ) -> Result<CommittedWriteChange, WriteStoreError>;

    fn record_delivery_fact(
        &self,
        fact: DeliveryFact,
    ) -> Result<CommittedWriteChange, WriteStoreError>;

    fn cancel(
        &self,
        request: CancelWrite,
    ) -> Result<CommittedWriteChange, WriteStoreError>;

    fn recover_open(
        &self,
    ) -> Result<Vec<RecoveredWrite>, WriteStoreError>;

    fn receipt(
        &self,
        id: ReceiptId,
    ) -> Result<Option<Receipt>, WriteStoreError>;

    fn page_writes(
        &self,
        page: WritePageRequest,
    ) -> Result<WritePage, WriteStoreError>;
}
```

### Owned state

- write and receipt identity allocation;
- accepted unsigned-event, replaceable-event-edit, and pre-signed-event payloads;
- actor for replaceable-event edits before an event exists;
- current materialization and materialization generation;
- exact unsigned or signed event bytes;
- signature request and completion state;
- live route-plan revisions;
- per-destination delivery lanes;
- attempt, handoff, acknowledgment, refusal, ambiguity, and give-up facts;
- cancellation and supersession state;
- bounded retained terminal receipts;
- restart recovery of open obligations.

### Query-source role

The store exposes each current materialized event as `SourceEvent::Local` together with publication evidence.

```text
accept or rematerialize write
        ↓
WriteStore commits
        ↓
WriteStore source revision changes
        ↓
matching LiveQueries update
```

Unsigned events are visible through this source. They are not inserted into `EventCache`.

### Acceptance boundary

For a materialized write, one acceptance transaction commits at least:

- write identity;
- receipt identity;
- event or replaceable-event edit;
- current materialization generation;
- current unsigned/signed event, when available; and
- the query-source state needed to expose that materialization.

The resulting committed source change becomes observable before the application receives the accepted result.

### Durability profile

The standard Fava publication profile uses a durable write-store implementation whose accepted obligations and receipts survive process restart. Memory implementations support deterministic tests and explicitly volatile application profiles.

Proposed implementations:

```text
fava-write-store-memory
fava-write-store-redb
```

---

## `fava-fetch-cache`

**Responsibility:** opaque cache persistence for non-event protocol services.

### Contract

```rust
pub trait FetchCache: Send + Sync {
    fn get(
        &self,
        namespace: CacheNamespace,
        key: &[u8],
    ) -> Result<Option<CachedBytes>, FetchCacheError>;

    fn put(
        &self,
        namespace: CacheNamespace,
        key: &[u8],
        value: CachedBytes,
    ) -> Result<(), FetchCacheError>;

    fn remove(
        &self,
        namespace: CacheNamespace,
        key: &[u8],
    ) -> Result<(), FetchCacheError>;
}
```

`FetchCache` stores bytes and metadata supplied by the service owner. It gives no meaning to freshness, validation, negative caching, ETags, HTTP status, domain identity, or failure retention.

Those policies belong to service crates:

```text
fava-nip05 owns NIP-05 normalization, resolution, and cache policy
fava-nip11 owns NIP-11 acquisition, parsing, freshness, and cache policy
```

The storage classification is semantic:

| Data | Storage role |
|---|---|
| Signed Nostr events, including NIP-65 relay-list events | `EventCache` |
| Accepted local operations/events and delivery facts | `WriteStore` |
| NIP-05 resolutions and NIP-11 documents | Service-owned state backed optionally by `FetchCache` |
| Derived route plans and query results | Recomputed by their lifecycle owner |
| Relay connection and authentication state | Transport/auth lifecycle owner |

Proposed implementations:

```text
fava-fetch-cache-memory
fava-fetch-cache-redb
```

An application may supply separate cache instances to different services or share one implementation through namespaced handles.

---

## Query-source composition

`fava-observe` opens the event cache and write store as independent `QuerySource` instances.

### Continuous source opening

Each source returns an initial snapshot and a change stream belonging to one source revision sequence. The observation owner buffers source changes while all initial source snapshots are being acquired, then calculates one merged initial `QuerySnapshot`.

```text
open EventCache source ──────┐
                             ├── buffer source changes
open WriteStore source ──────┘
            │
            ▼
merge coherent initial source state
            │
            ▼
deliver one complete QuerySnapshot
            │
            ▼
apply later source changes in revision order
```

A failed source open causes the whole live-query open to fail or produces an explicitly configured degraded profile; the standard profile uses all-or-nothing opening.

### Merge authority

The universal merge algorithm lives in `fava-query`; cache and write-store providers contribute source facts to that one precedence model.

### Example: optimistic event

```text
EventCache:
    cached profile v1

WriteStore:
    local unsigned profile v2

Merged query:
    profile v2 with publication evidence
```

Cancellation retracts v2 from the write source. The next merge naturally reveals cached v1.

### Example: relay echo

```text
EventCache:
    signed event E, seen on relay A

WriteStore:
    signed event E, receipt 42

Merged query:
    one EventRecord E
    relay evidence: A
    publication evidence: receipt 42
```

### Example: replaceable-event rematerialization

```text
EventCache receives newer source base v3
        ↓
publication owner applies the accepted replaceable-event edit again
        ↓
WriteStore installs local materialization v4
        ↓
query merge selects v4
```

The event cache retains v3 as the best observed relay source. The write store retains v4 as the current local desired materialization.

---

## Persistent format ownership

Persistent compatibility is local to the owner of the bytes:

- an event-cache implementation owns its cache schema and migration policy;
- a write-store implementation owns its write/receipt schema and migration policy;
- a fetch-cache implementation owns its storage schema;
- each protocol crate owns the format of its persisted replaceable-event edits;
- signer providers own their persisted credential/session formats.

An application changing an implementation in a later release chooses the corresponding provider migration, parallel transition, or reset path as part of that application release.

# Part III — Routing

## `fava-routing`

**Responsibility:** compose an ordered set of asynchronous router contributors into one live relay plan.

`fava-routing` is the primitive routing crate. Its contents are route values, ordered asynchronous composition, attribution, revisions, settlement, and lifecycle. NIP-65, hint, app-relay, and fallback policies live in their router crates.

### Route request

One contract serves reads and writes:

```rust
pub enum RouteRequest {
    Read(ReadRouteRequest),
    Write(WriteRouteRequest),
}

pub struct ReadRouteRequest {
    pub query: Query,
    pub access: RelayAccess,
}

pub struct WriteRouteRequest {
    pub event: EventValue,
    pub receipt_id: ReceiptId,
    pub generation: MaterializationGeneration,
}
```

A router may contribute to reads, writes, or both.

### Route targets

Routing contributions describe what they cover:

```rust
pub enum RouteTarget {
    WholeRequest,
    Author(PublicKey),
    Recipient(PublicKey),
    ReferencedEvent(EventId),
    ReferencedAddress(EventAddress),
    Custom(RouteTargetKey),
}
```

This allows fallback policy to reason per recipient or per referenced object instead of using one whole-request relay count.

### Route contributions

```rust
pub struct RouteContribution {
    pub destinations: Vec<RouteDestination>,
    pub coverage: Vec<TargetCoverage>,
    pub unresolved: Vec<RouteNeed>,
    pub shortfalls: Vec<RouteShortfall>,
}

pub struct RouteDestination {
    pub relay: RelayUrl,
    pub access: RelayAccess,
    pub targets: BTreeSet<RouteTarget>,
    pub reason: NamespacedRouteReason,
}

pub struct TargetCoverage {
    pub target: RouteTarget,
    pub state: CoverageState,
}

pub enum CoverageState {
    Covered { destinations: BTreeSet<RelaySessionKey> },
    Unresolved { needs: BTreeSet<RouteNeed> },
    SettledAbsent,
}
```

A router emits complete replacement snapshots for its own contribution. A later snapshot replaces that router instance's prior destinations, coverage, unresolved needs, and shortfalls.

### Router contract

Conceptual signal-oriented shape:

```rust
pub trait Router: Send + Sync {
    fn open(
        &self,
        request: RouteRequest,
        upstream: UpstreamRouteSignal,
    ) -> Result<Box<dyn RouterSession>, RouterError>;
}

pub trait RouterSession: Send {
    fn current(&self) -> RouteContribution;
    fn next_change(&mut self) -> RouteChangeFuture<'_>;
    fn close(&mut self);
}
```

The implementation may use callbacks, channels, streams, or signals internally. The contract semantics are:

1. opening a router session produces an immediate complete current contribution;
2. the initial contribution never waits on network acquisition;
3. later knowledge produces a replacement contribution asynchronously;
4. closing the session releases all router-owned acquisition work;
5. one router's delay does not prevent other routers' contributions from entering the plan.

### Ordered composition

Routers are evaluated in application-configured order.

```text
request
  ↓
router 1 sees empty upstream plan
  ↓
router 2 sees router 1's current accumulated plan
  ↓
router 3 sees routers 1 + 2
  ↓
final current RoutePlan
```

When an upstream router changes, downstream routers receive the new upstream plan and recompute their current contribution. The chain is acyclic: a router observes only the accumulated output of earlier routers.

This is the mechanism used by fallback routing. A fallback router can inspect current upstream coverage and contribute only for targets it considers insufficiently covered.

### Final route plan

```rust
pub struct RoutePlan {
    pub revision: RouteRevision,
    pub destinations: BTreeMap<RelaySessionKey, PlannedRelay>,
    pub coverage: BTreeMap<RouteTarget, TargetCoverage>,
    pub unresolved: BTreeSet<RouteNeed>,
    pub shortfalls: Vec<RouteShortfall>,
    pub settlement: RouteSettlement,
}

pub struct PlannedRelay {
    pub session: RelaySessionKey,
    pub targets: BTreeSet<RouteTarget>,
    pub reasons: Vec<AttributedRouteReason>,
}
```

Identical relay sessions deduplicate into one destination while retaining every contributing router, target, and reason.

### Plan settlement

A plan may contain destinations and unresolved needs simultaneously. Settlement describes current knowledge; it does not gate progress.

```text
known destinations are usable immediately
unresolved targets remain visible
later contributions create a new plan revision
```

For reads, new destinations create relay demand and retracted destinations withdraw relay demand when no remaining router contributes them.

For writes, new destinations create new delivery lanes under the same receipt and current event generation. Retraction may retire work that has not crossed handoff. Existing delivery facts remain scoped to what actually happened.

### Explicit routing

An exact non-empty relay list produces a `RoutePlan` directly. No router session is opened, and no router-owned input acquisition runs.

### Route preview

Preview evaluates the same ordered router chain over currently available router snapshots without accepting a write, signing, creating a receipt, opening delivery lanes, or starting new router-owned relay acquisition. Unresolved needs remain visible as unresolved.

A caller may take a one-shot snapshot or keep a preview open to observe changes produced by knowledge already being maintained for other owned work. Preview never creates publication or relay-acquisition ownership merely because an application asks where an operation would currently go.

---

## Router input queries

Router implementations may acquire algorithm-specific inputs through ordinary
Fava query APIs supplied when the router is constructed.

### Local query service

Reads current merged local query state without starting automatic relay routing.

```rust
local_queries.open(query)
```

This allows a router to use locally cached Nostr events and accepted local writes as inputs. A locally accepted relay-list update can therefore influence routing immediately through the merged local query view, before a relay echoes it.

### Explicit query service

Opens ordinary query machinery against an exact relay set:

```rust
explicit_queries.open(query, exact_relays)
```

Router-owned acquisition is explicitly routed. This prevents automatic-routing recursion and reuses the same:

- wire protocol;
- subscription planner;
- transport;
- event verification;
- event cache;
- query-source observation; and
- cancellation semantics

used by application queries.

### Router-owned state

A router may retain bounded derived state intrinsic to its algorithm, such as:

- current relay-list resolutions;
- session-scoped settled absence;
- shared discovery needs;
- current hint candidates; and
- current upstream coverage.

The router owns that state and its lifecycle. Generic routing does not interpret it.

---

## `fava-router-outbox`

**Responsibility:** NIP-65-based author outbox, recipient inbox, and discovery routing.

### Inputs

- read or write route request;
- event author and p-tagged recipients;
- locally available kind-10002 events;
- configured indexer relays;
- explicit query service;
- current upstream route plan when composition policy needs it.

### Outputs

- author read/write relay destinations appropriate to the request;
- recipient read-relay destinations;
- target coverage per author/recipient;
- unresolved relay-list needs;
- session-scoped settled absence; and
- exact reasons for each contribution.

### Asynchronous behavior

The first contribution uses currently available relay-list facts immediately. Missing facts become unresolved needs. Shared explicit discovery queries acquire those facts asynchronously and emit replacement contributions as each author resolves.

Two route sessions needing the same author may share one discovery observation while retaining independent route-session ownership.

### Dependencies

```text
fava-routing
fava-query
fava-nip65
```

The crate contains the outbox algorithm. `fava-nip65` contains pure NIP-65 event vocabulary and parsing.

---

## `fava-router-hints`

**Responsibility:** contribute relays from event references, relay hints, and actual event evidence.

### Inputs

- query or write request;
- event references in tags/content structure;
- referenced `EventRecord` values available locally;
- relay hints carried by references;
- relay evidence recording where referenced events were actually observed;
- current relay-admission policy exposed through routing services.

### Outputs

- relay destinations for referenced events, addresses, or authors;
- target coverage and reasons distinguishing carried hints from observed evidence;
- unresolved needs when a referenced object must be resolved before a useful contribution can be made.

This is an independently selectable higher-layer policy crate regardless of implementation size.

---

## `fava-router-app-relays`

**Responsibility:** always contribute configured application relays to automatically routed operations.

Illustrative configuration:

```rust
AppRelayRouter::new(relays)
    .reads(true)
    .writes(true)
```

Its contribution covers `RouteTarget::WholeRequest`. It is immediate and normally has no unresolved work.

Applications select this router when app relays are an unconditional part of their read/write policy.

---

## `fava-router-fallback-relays`

**Responsibility:** contribute configured relays when the live upstream route plan does not provide the coverage required by the application's fallback policy.

### Inputs

- current upstream route plan signal;
- configured fallback relays;
- fallback coverage policy;
- read/write applicability.

### Policy examples

- contribute when a target has zero destinations;
- contribute until every recipient has at least two destinations;
- contribute only after upstream resolution settles absent;
- contribute immediately while a target is unresolved;
- apply to writes but not reads;
- cover each insufficient recipient independently.

### Reactive behavior

If an upstream router later supplies sufficient coverage, the fallback router emits a new snapshot retracting the fallback contribution for that target. Query routing follows the current plan. Publication preserves any delivery evidence already produced while applying the new desired plan to future work.

Applications typically select either `fava-router-app-relays` or `fava-router-fallback-relays`. When both are selected, configured order gives their composition precise meaning.

---

## Routing implementation testkit

`fava-routing` ships a conformance testkit that every router implementation can run.

It tests:

- immediate initial contribution;
- asynchronous updates;
- complete-snapshot replacement semantics;
- cancellation and resource release;
- deduplication of relay destinations;
- exact attribution of targets and reasons;
- upstream-plan reactivity;
- no automatic-routing recursion;
- deterministic behavior for equal inputs; and
- bounded unresolved and diagnostic state.

The routing-chain testkit additionally exercises arbitrary router order, delayed routers, routers that retract contributions, and provider failure isolation.


---

# Part IV — Relay execution contracts

## `fava-subscriptions`

**Responsibility:** map logical read demand assigned to one exact relay session into semantically equivalent wire subscriptions and attribution.

### Contract

```rust
pub trait SubscriptionPlanner: Send + Sync {
    fn plan(
        &self,
        relay: &RelaySessionKey,
        demand: &[RelayDemand],
        constraints: &RelayReadConstraints,
    ) -> Result<SubscriptionPlan, SubscriptionPlanError>;
}

pub struct RelayDemand {
    pub owner: ObservationId,
    pub branch: QueryBranchId,
    pub filter: Filter,
    pub bounds: QueryBounds,
}

pub struct SubscriptionPlan {
    pub wire: Vec<PlannedSubscription>,
    pub attribution: SubscriptionAttribution,
    pub shortfalls: Vec<SubscriptionShortfall>,
}
```

### Owned meaning

- planner input identity;
- exact relay-session scope;
- logical-to-wire attribution;
- plan diff values;
- relay-limit shortfalls;
- withdrawal identity;
- the conformance rules that define semantic equivalence.

### Planner boundary

Routing has already selected the relay. The planner decides only how the demand for that relay is represented on the wire.

The planner output does not open a socket and does not mutate observation state. `fava-observe` owns logical demand; `fava-transport` performs the plan.

---

## `fava-subscriptions-standard`

**Responsibility:** the standard exact subscription-grouping policy.

It may:

- deduplicate identical filters;
- combine filters that differ in one safely unionable dimension;
- preserve separate wire subscriptions when limits, time windows, relay access, or multiple differing dimensions make grouping non-equivalent;
- split oversized requests into exact subsets;
- account for NIP-11 message-size, subscription-count, subscription-id, default-limit, and result-limit constraints; and
- report typed shortfall when exact execution does not fit.

Example:

```text
three logical filters:
    kinds:[1], authors:[alice]
    kinds:[1], authors:[bob]
    kinds:[1], authors:[carol]

one exact wire filter:
    kinds:[1], authors:[alice,bob,carol]
```

Attribution still identifies which logical observations each returned event satisfies.

A custom planner may choose one wire subscription per logical demand. Both implementations must produce equivalent local results and evidence.

---

## `fava-transport`

**Responsibility:** own relay sessions and correlated byte handoff.

### Contract

```rust
pub trait Transport: Send + Sync {
    fn open_session(
        &self,
        request: OpenRelaySession,
    ) -> RelaySessionFuture;
}

pub trait RelaySession: Send {
    fn identity(&self) -> RelaySessionIdentity;

    fn send(
        &self,
        frame: Bytes,
        correlation: HandoffCorrelation,
    ) -> HandoffFuture;

    fn messages(&self) -> Box<dyn RelayMessageStream>;

    fn close(&self) -> CloseFuture;
}
```

### Owned state

- DNS/TCP/TLS/WebSocket resources;
- relay URL and relay-access session key;
- connection and reconnect generation;
- connection backoff;
- bounded inbound and outbound byte queues;
- exact byte-handoff outcomes;
- session health and transport errors;
- current and retiring session lifecycle;
- shutdown and resource joining.

### Handoff facts

```rust
pub enum HandoffOutcome {
    NotHandedOff { reason: TransportFailure },
    HandedOff,
    Ambiguous { reason: TransportAmbiguity },
}
```

This boundary is used by publishers and subscription execution. It lets higher layers distinguish bytes that never left Fava from bytes whose outcome is uncertain.

### Session identity

Every inbound frame and handoff completion carries exact session generation and relay-access identity. Reconnected sessions are new authorities.

---

## `fava-transport-websocket`

**Responsibility:** standard WebSocket implementation of `Transport`.

It owns the ordinary Nostr relay connection lifecycle:

- URL normalization and admission;
- DNS resolution;
- TCP/TLS/WebSocket handshake;
- bounded reads and writes;
- keepalive and dead-session detection;
- reconnect backoff;
- session generation;
- transport-level replay hooks for current subscription plans; and
- deterministic close.

It uses `fava-wire` only for optional framing diagnostics; wire semantics remain owned by `fava-wire` and higher owners.

---

## `fava-publisher`

**Responsibility:** perform one publication attempt for one exact signed event at one exact relay session.

### Contract

```rust
pub trait Publisher: Send + Sync {
    fn publish(
        &self,
        attempt: PublishAttempt,
        transport: &dyn TransportAccess,
    ) -> PublishFuture;
}

pub struct PublishAttempt {
    pub write_id: WriteId,
    pub receipt_id: ReceiptId,
    pub generation: MaterializationGeneration,
    pub session: RelaySessionKey,
    pub event: Event,
    pub deadline: Deadline,
}

pub enum PublishOutcome {
    Acknowledged { message: String },
    Rejected { message: String },
    AuthenticationRequired,
    NotHandedOff { reason: PublishFailure },
    OutcomeUnknown { reason: PublishAmbiguity },
}
```

### Attempt identity

A publication outcome is valid only for the exact write, materialization generation, event id, relay session, and attempt that caused it.

### Relationship to other owners

- routing selects the destination;
- delivery policy decides when an attempt is due;
- publisher performs the attempt;
- transport owns the session and byte handoff;
- auth owner handles NIP-42 policy;
- write store commits attempt and receipt facts;
- publication owner coordinates the lifecycle.

---

## `fava-publisher-nip01`

**Responsibility:** standard NIP-01 `EVENT`/`OK` publication attempt.

The implementation:

1. obtains the exact relay session;
2. encodes `ClientMessage::Event` with `fava-wire`;
3. hands the bytes to transport with attempt correlation;
4. waits for the matching `OK` or a terminal session fact;
5. preserves the relay's response message within bounds; and
6. returns one `PublishOutcome`.

It performs one attempt. It does not schedule its own repeated attempts.

---

## `fava-delivery`

**Responsibility:** decide when current durable delivery facts authorize another publication attempt or a terminal lane decision.

### Contract

```rust
pub trait DeliveryPolicy: Send + Sync {
    fn decide(
        &self,
        facts: DeliveryFacts<'_>,
    ) -> DeliveryDecision;
}

pub struct DeliveryFacts<'a> {
    pub write: &'a CurrentWrite,
    pub lane: &'a DeliveryLane,
    pub route_plan: &'a RoutePlan,
    pub transport: &'a TransportAvailability,
    pub authentication: &'a AuthenticationState,
    pub now: Timestamp,
}

pub enum DeliveryDecision {
    AttemptNow,
    WaitUntil(Timestamp),
    Park(ParkReason),
    GiveUp(GiveUpReason),
    Settled,
}
```

### Owned policy

- retry delay and backoff;
- fairness across writes and destinations;
- finite attempt/deadline ceilings;
- interpretation of retryable observed failure;
- lane give-up policy;
- scheduling priority;
- ambiguity policy where the contract permits a provider choice.

The policy consumes durable facts and returns decisions. The write store remains the authority for the facts.

---

## `fava-delivery-standard`

**Responsibility:** standard bounded, fair, evidence-based delivery policy.

The standard policy has these properties:

- unresolved routes park without consuming a delivery attempt;
- unavailable signers park without consuming a delivery attempt;
- disconnected transport does not count as an attempted delivery;
- an actual attempted failure advances the lane's finite budget;
- backoff is per relay/session rather than one independent reconnect storm per write;
- lanes settle independently;
- mixed terminal outcomes close the overall receipt while preserving every lane result;
- an ambiguous handoff produces an explicit unknown outcome according to the configured standard profile; and
- current route-plan additions create fair new work without restarting settled lanes.

The implementation is mostly deterministic policy over durable facts and the clock. It retains no private durable ledger.

---

## `fava-signer`

**Responsibility:** exact identity-bound signing and cryptographic contracts.

### Contract

```rust
pub trait Signer: Send + Sync {
    fn public_key(&self) -> PublicKey;

    fn availability(&self) -> SignerAvailability;

    fn sign_event(
        &self,
        event: UnsignedEvent,
    ) -> SignFuture;

    fn decrypt(
        &self,
        request: DecryptRequest,
    ) -> DecryptFuture;
}
```

Signing completion returns a signed event for the exact unsigned body. The publication owner verifies exact body, id, pubkey, and signature before installation.

Encryption and decryption use distinct request and outcome values even when one provider implements both.

### Runtime lifecycle

Signer instances are attached to accounts at runtime because login, hardware availability, remote signer connectivity, and human approval are application lifecycles.

### Standard implementations

```text
fava-signer-local
fava-signer-nip46
```

Additional applications may provide hardware, extension, enclave, or remote signer crates through the same contract.

# Part V — Protocol services and event-kind crates

## `fava-nip11`

**Responsibility:** define NIP-11 values, validation, freshness vocabulary, and the relay-information service contract.

It owns:

- relay-information document values;
- limitation values consumed by subscription and publication planning;
- document parsing and validation;
- freshness, staleness, and last-good-document vocabulary;
- service requests, snapshots, and typed errors.

Illustrative contract:

```rust
pub trait RelayInformationFetcher: Send + Sync {
    async fn get(
        &self,
        relay: RelayUrl,
        policy: RelayInformationCachePolicy,
    ) -> Result<RelayInformationSnapshot, RelayInformationError>;
}
```

`fava-nip11-http` is the standard HTTP implementation. It owns HTTP acquisition, conditional requests, bounded single-flight fetches, and NIP-11-specific cache policy over an optional `FetchCache` namespace.

## `fava-nip05`

**Responsibility:** define NIP-05 identifier values, validation, resolution vocabulary, and the resolver contract.

It owns:

- identifier parsing and normalization;
- exact name-to-pubkey matching semantics;
- optional relay-list projection;
- resolution snapshots, freshness values, and typed errors.

Illustrative contract:

```rust
pub trait Nip05Resolver: Send + Sync {
    async fn resolve(
        &self,
        identifier: Nip05Identifier,
        policy: Nip05CachePolicy,
    ) -> Result<Nip05Resolution, Nip05Error>;
}
```

`fava-nip05-http` is the standard HTTP implementation. It owns `.well-known/nostr.json` acquisition, bounded per-identifier single flight, positive and negative cache policy, and optional persistence through a `FetchCache` namespace.

NIP-05 and NIP-11 results are service data. They remain in their owning services and fetch-cache namespaces rather than entering the event cache.

## Event-kind protocol crates

Each independently selectable protocol crate owns one coherent Nostr concept.

Examples:

```text
fava-nip02          follow-list decoding and follow/unfollow operations
fava-nip29          group-scoped values and operations
fava-nip65          relay-list event semantics
fava-bookmarks      public bookmark-list semantics
fava-nip18          repost semantics
fava-nip22          comment semantics
fava-nip25          reaction semantics
fava-content        structured content parsing
```

A protocol crate may expose:

```rust
pub fn decode(record: &EventRecord)
    -> Result<TypedValue, DecodeError>;

pub fn query(...)
    -> Query;

pub fn compose(...)
    -> Result<UnsignedEvent, ComposeError>;

pub fn edit(...)
    -> ReplaceableEventEdit;
```

Protocol crates use:

- `fava-query` for local and remote state acquisition;
- `fava-write::EventBuilder` for event construction;
- `fava-write::ReplaceableEventEdit` for durable edits to replaceable events;
- the one publication lifecycle for acceptance, signing, routing, delivery, and receipts.

A new protocol crate is selected by the application profile and contributes through these ordinary contracts. The facade and universal owners remain unchanged.

# Part VI — Universal engine owners

## `fava-ingest`

**Responsibility:** turn untrusted relay frames into committed event-cache facts.

### Inputs

- decoded `RelayMessage` values;
- exact relay-session and subscription attribution;
- current subscription attribution plan;
- event-cache state slices;
- current clock.

### Owned lifecycle

1. validate relay-frame shape and bounds;
2. attribute an event to an accepted wire subscription and logical demand;
3. verify event id and Schnorr signature;
4. verify the event matches at least one attributed logical filter;
5. construct `VerifiedRelayEvent`;
6. ask `fava-state` for the cache decision;
7. commit the decision through `EventCache`;
8. emit `CommittedCacheChange` and per-relay evidence;
9. emit typed rejected-input diagnostics when validation fails.

### State ownership

`fava-ingest` owns current ingress operation identity and serialized admission order. Event state belongs to `EventCache`; universal state semantics belong to `fava-state`; relay sessions belong to transport.

---

## `fava-observe`

**Responsibility:** own live-query handles, dependency lifecycles, source merging, relay demand, and bounded delivery to applications.

### Owned state

- observation identity and open/close lifecycle;
- query;
- source observations over EventCache and WriteStore;
- derived-query dependency graph;
- current merged `QuerySnapshot`;
- route session for automatic queries;
- logical per-relay demand;
- ownership/refcounts for shared work;
- source-scoped evidence;
- bounded application delivery state;
- pending consumer request and cancellation state.

### Open sequence

1. validate the query through `fava-query`;
2. open continuous EventCache and WriteStore sources;
3. establish derived dependencies;
4. create explicit plan or open the router chain;
5. compile current per-relay logical demand;
6. open the source snapshots and buffer changes;
7. calculate one complete initial `QuerySnapshot`;
8. install the observation owner;
9. return the handle and make the initial snapshot readable;
10. start or continue relay work for the current route plan.

### Current-state propagation

Committed source changes, route-plan changes, subscription-plan changes, session facts, and dependency changes are routed to the exact affected observations. The observation owner recalculates only affected branches and delivers one new revision.

### Delivery model

Application-facing query delivery is a bounded latest-state stream. Intermediate internal states may coalesce; every delivered update is sufficient to derive the exact current result from the last delivered revision.

### Suggested internal modules

```text
open.rs             all-or-nothing observation installation
registry.rs         observation identity and shared-work ownership
sources.rs          QuerySource opening and source revision merge
dependencies.rs     derived query graph
routes.rs           RoutePlan binding
subscriptions.rs    logical demand and planner integration
projection.rs       query evaluation and EventRecord merge
delivery.rs         bounded current-state delivery
close.rs            cancellation and teardown
```

---

## `fava-publication`

**Responsibility:** own each accepted write from intent through materialization, signature, live routing, delivery, receipt settlement, cancellation, and recovery.

### Owned state

The durable state is in `WriteStore`. The publication owner owns the live orchestration and current operation generations:

- accepted write identity and receipt;
- current replaceable-event edit or exact event;
- current materialization generation;
- current unsigned or signed event;
- current signer operation;
- current route session and route revision;
- current delivery lanes and due work;
- exact cancellation eligibility;
- terminal settlement and retained receipt observation.

### Acceptance paths

#### Unsigned event

The event already contains its pubkey. `WriteStore` commits the event and receipt. The event becomes visible through the write-store query source. Signing then targets that pubkey.

#### Replaceable-event edit

The edit contains its actor, coordinate, durable protocol-owned change, and format version. Its protocol crate applies it to the current source event or defined empty state. The write store commits the edit, receipt, and current materialization together. If materialization is temporarily unavailable, the accepted edit remains content-pending according to the selected publication profile.

#### Pre-signed event

The event is verified before acceptance, committed verbatim, exposed through the write-store query source, and routed without a signing step.

### Materialization changes

When a newer relevant source event arrives:

1. the owning protocol crate applies the accepted edit again;
2. a new unsigned event and materialization generation are produced;
3. `WriteStore` atomically replaces the current local materialization;
4. query observations receive the write-source change;
5. stale signing and route completions are rejected;
6. a new route session is opened for the current event when event-dependent routing changed; and
7. prior delivery facts remain scoped to their exact predecessor event id.

### Signing and routing progress independently

Once an unsigned event is committed, signer acquisition and route acquisition begin independently. Routing uses the unsigned event's pubkey, tags, references, and route-relevant protocol facts; it does not wait for a signature. Known destinations may become durable lanes while signing is still pending. Delivery begins when both the exact current signature and an eligible lane are ready.

### Routing behavior

Automatic writes open one live `RoutePlan`. Currently known destinations become delivery lanes immediately. Later router contributions add lanes under the same receipt and current event generation.

Explicit writes create one settled exact plan and open no router sessions.

### Delivery behavior

For each due lane:

1. delivery policy returns a decision;
2. an authorized attempt is recorded as required by the write-store contract;
3. the selected publisher performs one attempt;
4. the result is correlated to exact write, event generation, relay session, and attempt;
5. `WriteStore` commits the result; and
6. receipt observers receive the committed fact.

### Cancellation

Cancellation is decided from current materialization, signature, and handoff facts. A successful cancellation commits the cancelled state and retracts the local write-source contribution. Cached relay events remain untouched.

### Recovery

At engine start, the owner loads open writes, reconstructs current edit or event state from durable facts, restores replaceable-event edits through their selected protocol crates, reopens route sessions, resumes required signing, and schedules current lanes. Query and relay execution begin after required write-store reconciliation is complete.

### Suggested internal modules

```text
accept.rs           write acceptance paths
edits.rs            replaceable-event edit application and rematerialization
sign.rs             signer selection and exact completion validation
route.rs            live RoutePlan binding and lane creation
lanes.rs            current delivery-lane state
attempt.rs          publisher invocation and result correlation
receipt.rs          receipt projection and terminal reduction
cancel.rs           cancellation eligibility and commit
recover.rs          open-write reconstruction
retention.rs        terminal receipt retention
```

---

## `fava-session`

**Responsibility:** own the application-visible account set and current-account input.

### Owned state

- account identities;
- current account;
- signer and crypto provider attachment to accounts;
- account addition, removal, and replacement;
- all-or-nothing session import/export;
- provider-specific session reconstruction data;
- reactive current-account changes used by queries and write-construction conveniences.

### Write relationship

A convenience API may resolve `CurrentAccount` through `fava-session`. Before acceptance, it constructs either:

- an `UnsignedEvent` whose `pubkey` is the resolved account; or
- a `ReplaceableEventEdit` whose `actor` is the resolved account.

Accepted write state is then self-identifying and independent of later current-account changes.

---

## `fava-auth`

**Responsibility:** own NIP-42 challenge and authorization lifecycles for exact relay access.

### Owned state

- relay-access identity;
- current relay challenge;
- application authentication-policy operation;
- signer operation for the AUTH event;
- current session generation;
- accepted/refused/failed authentication facts;
- re-authentication after reconnect;
- exact attribution of authentication outcomes to query and publication work.

Authentication identity is explicit in the route/session configuration. It is independent of query filters and event authorship.

### Contract

```rust
pub struct RelayChallenge { /* relay session key, transport generation, challenge */ }

pub enum AuthorizationDecision {
    Authorize(PublicKey),
    Decline(String),
}

pub trait AuthenticationPolicy: Send + Sync {
    async fn authorize(&self, challenge: &RelayChallenge) -> AuthorizationDecision;
}

pub enum AuthenticationOutcome {
    Accepted { identity: PublicKey, message: String },
    Refused { message: String },
    Declined { reason: String },
    Failed { reason: String },
}

pub struct Authentication { /* policy, signers, bounded deadline */ }
```

The application supplies the policy and the signers it may authorize. `Authentication` correlates
one exact challenge, the policy decision, the selected `Signer`, and the relay's `OK` into one
session-scoped outcome. It owns no connection: the caller supplies the exact session whose
generation carried the challenge, so an answer for a retired generation is refused by identity.

A reader that also carries query traffic uses `prepare` and settles the relay `OK` in its own loop,
so authentication never drains frames another owner is responsible for.

---

## `fava-diagnostics`

**Responsibility:** expose bounded, current, typed facts from Fava owners without making policy decisions.

### Inputs

Each owner publishes structured diagnostic facts:

- open observation and route ownership;
- relay-session state and reason;
- source shortfalls;
- router unresolved needs;
- subscription-plan limits;
- stalled write reasons;
- signer and auth availability;
- cache and write-store failures;
- bounded counts and high-water facts.

### Output

```rust
pub struct DiagnosticsSnapshot {
    pub relays: Vec<RelayDiagnostic>,
    pub queries: Vec<QueryDiagnostic>,
    pub writes: Vec<WriteDiagnostic>,
    pub providers: Vec<ProviderDiagnostic>,
    pub limits: Vec<LimitDiagnostic>,
}
```

The output is a bounded latest-state stream. Diagnostics reports facts and scoped reasons. Applications decide presentation and product policy.

---

## `fava-runtime`

**Responsibility:** execute asynchronous resources and deliver correlated completions.

### Owned resources

- task execution;
- timers and clocks;
- bounded command/completion channels;
- router sessions and their asynchronous input queries;
- source-observation polling;
- transport sessions;
- publisher futures;
- signer and auth provider operations;
- provider panic/failure isolation;
- cancellation propagation;
- resource joining and shutdown deadlines.

### Owner relationship

Universal owners decide what work is authorized. The runtime performs the work and returns typed completions. It does not interpret event-kind meaning, choose routes, calculate query results, or update durable state directly.

### Provider isolation

Potentially blocking or application-supplied provider calls run outside owner locks and store transactions. A stalled provider has bounded influence and cannot block unrelated owner progress or Fava shutdown indefinitely.

---

## `fava`

**Responsibility:** provide the thin Rust application facade, builder, lifecycle, and ordering between universal owners.

The facade owns Fava-instance identity, top-level command admission, startup
and shutdown order, and handles to the selected owners and providers. It owns
no event-kind dispatch, routing policy, query evaluation, retry algorithm,
socket state, or storage schema.

Required ordering includes:

- `WriteStore` commit before an accepted response or publication effect;
- `EventCache` commit before query-source invalidation;
- account resolution before construction of an unsigned event or replaceable-event edit;
- route-plan commit before a new durable lane attempts delivery; and
- stop of new commands before observations, publications, routers, transports, and stores close.

### Public surface

- engine construction from selected providers and protocol crates;
- open live query;
- publish unsigned events, replaceable-event edits, or pre-signed events;
- route preview;
- receipt reattachment and write inspection;
- session/account operations;
- sign without publish;
- NIP-42 auth policy attachment;
- NIP-05/NIP-11 services when selected;
- diagnostics;
- deterministic close and destructive reset.

### Builder

```rust
Fava::builder()
    .event_cache(...)
    .write_store(...)
    .fetch_cache(...)
    .query_evaluator(...)
    .routers(...)
    .subscription_planner(...)
    .transport(...)
    .publisher(...)
    .delivery_policy(...)
    .services(...)
    .build()
```

`fava` depends on contracts and universal owners. It does not depend on event-kind protocol crates or standard provider crates. Product assemblies compile concrete protocol crates and compose their ordinary query, event-building, and write values.

---

## `fava-standard`

**Responsibility:** assemble the recommended Fava product profile.

A plausible standard profile selects:

```text
persistent event cache
persistent write store
persistent or bounded fetch cache
fava-query-standard
fava-router-outbox
fava-router-hints
one of app-relay or fallback-relay policy, as configured
fava-subscriptions-standard
fava-transport-websocket
fava-publisher-nip01
fava-delivery-standard
local signer support
NIP-11 service
commonly selected protocol crates
```

The profile documents the behavior implied by its implementations, including cache persistence, write durability, retry policy, route policy, and platform packaging.

An application may use `fava-standard` or assemble `fava` directly.


---

# Part VII — Application assembly and product profiles

## Direct Rust assembly

A direct Rust application may assemble providers explicitly:

```rust
let engine = Engine::builder()
    .query_evaluator(StandardQueryEvaluator::new())
    .event_cache(MemoryEventCache::bounded(100_000))
    .write_store(RedbWriteStore::open("writes.redb")?)
    .fetch_cache(MemoryFetchCache::bounded(2_000))
    .routers([
        Box::new(OutboxRouter::new(indexer_relays)),
        Box::new(HintRouter::new()),
        Box::new(FallbackRelayRouter::new(fallback_relays)),
    ])
    .subscription_planner(StandardSubscriptionPlanner::new())
    .transport(WebSocketTransport::new())
    .publisher(Nip01Publisher::new())
    .delivery_policy(StandardDeliveryPolicy::new())
    .materializers([
        Box::new(nip02.materializer()),
        Box::new(bookmarks.materializer()),
    ])
    .build()?;
```

The application may use a different provider for any named responsibility without editing Fava or unrelated providers.

---

## Standard persistent client profile

A conventional offline-capable client may select:

```text
persistent EventCache
persistent WriteStore
persistent FetchCache
OutboxRouter
HintRouter
AppRelayRouter or FallbackRelayRouter
`fava-query-standard` evaluator
standard subscription planner
WebSocket transport
NIP-01 publisher
standard delivery policy
local and/or remote signers
NIP-05 and NIP-11 HTTP services
```

This profile can advertise:

- cached event reads after restart according to its event-cache contract;
- retained provenance/deletion/expiration behavior provided by that cache;
- durable accepted writes and receipts;
- persistent fetched documents according to service cache policies.

Those guarantees arise from the selected providers rather than from the mere existence of an `EventCache` trait.

---

## Ephemeral client profile

A smaller application may select:

```text
bounded memory EventCache
persistent or volatile WriteStore
memory FetchCache
AppRelayRouter
no outbox discovery
standard subscription planner
WebSocket transport
NIP-01 publisher
simple delivery policy
```

An ephemeral event cache means:

- live observations remain correct for facts currently known in the process;
- a newly opened query may have no cached initial rows;
- relay data may be reacquired;
- cache eviction may retract events absent from other sources;
- restart starts with no cached relay events.

Accepted local publications still appear through the write-store source independently of event-cache retention.

---

## Custom routing application

An application may replace or reorder routing policy:

```rust
.routers([
    Box::new(CompanyDirectoryRouter::new(company_service)),
    Box::new(HintRouter::new()),
    Box::new(AppRelayRouter::new(company_relays)),
])
```

or:

```rust
.routers([
    Box::new(OutboxRouter::new(indexers)),
    Box::new(HintRouter::new()),
    Box::new(FallbackRelayRouter::new(fallbacks)),
])
```

`fava-routing` supplies the same asynchronous composition rules to both.

---

## Native Swift and Kotlin products

Swift and Kotlin products compile selected providers and protocol crates into their Fava native artifact.

The artifact exposes the same public workload model:

- live-query values and handles;
- event records;
- write intents and receipts;
- session and signer operations;
- diagnostics and supporting services.

Platform wrappers translate cancellation, async iteration, errors, and ownership into native idioms while retaining Rust-side state ownership.

Provider selection remains a build/product decision. Native applications do not load arbitrary Rust provider plugins at runtime.

---

## Provider-owned persistence compatibility

Every persistent provider owns:

- schema/version identification for its own bytes;
- validation at open;
- migrations it supports;
- typed refusal for unsupported or corrupt state;
- reset behavior for its own partition.

Examples:

- `RedbEventCache` owns compatibility of its event-cache data;
- `RedbWriteStore` owns compatibility of accepted write and receipt data;
- NIP-05 and NIP-11 services own interpretation of cached service entries;
- each protocol crate owns compatibility of its persisted replaceable-event edit format.

An application changing providers between releases explicitly chooses migration, parallel transition, reset, or continued use of the previous provider.

---

# Part VIII — End-to-end flows

## Opening an automatic live query

```text
application calls open(query)
        ↓
application constructs Query
        ↓
fava-observe creates provisional observation identity
        ↓
EventCache QuerySource opens initial snapshot + changes
        ↓
WriteStore QuerySource opens initial snapshot + changes
        ↓
derived query dependencies open
        ↓
fava-routing opens configured router chain
        ↓
each router contributes immediately
        ↓
current RoutePlan assigns logical demand to known relay sessions
        ↓
subscription planner compiles per-relay wire plans
        ↓
transport executes current wire deltas
        ↓
fava-query merges source snapshots into one QuerySnapshot
        ↓
observation owner commits open
        ↓
application receives handle and complete initial snapshot
```

Router and relay work may continue while local source snapshots are assembled. No relay result is required to produce the local initial snapshot.

Later router contributions add or withdraw relay demand without reopening the application query.

---

## Opening an explicit live query

```text
application calls open(query, Explicit([relay-a, relay-b]))
        ↓
query validation and local source opening
        ↓
exact RoutePlan is created directly
        ↓
router chain is not opened
        ↓
subscription planner and transport execute exact demand
```

The exact relay set remains the route authority for the query's lifetime.

---

## Asynchronous route expansion

A write p-tags Alice, Bob, and Carol.

At route-session open:

```text
OutboxRouter:
    Alice -> relay-a
    Bob   -> relay-b
    Carol -> unresolved(kind:10002)

AppRelayRouter:
    WholeRequest -> app-relay
```

The current plan is immediately:

```text
destinations:
    relay-a
    relay-b
    app-relay

unresolved:
    Carol's relay list
```

Publication creates lanes and begins eligible attempts for all three known destinations.

Later:

```text
OutboxRouter explicit discovery receives Carol's kind:10002
    Carol -> relay-c
```

The router emits a replacement contribution. `fava-routing` produces a new route revision containing `relay-c`. `fava-publication` commits the route revision and creates one new lane under the same receipt and current event generation.

No existing acknowledged lane is resent merely because another destination was added.

---

## Receiving a relay event

```text
transport receives bytes
        ↓
fava-wire decodes RelayMessage
        ↓
fava-ingest verifies exact session/subscription attribution
        ↓
event id and signature are verified
        ↓
event is checked against attributed logical filters
        ↓
fava-state calculates CacheMutation batch
        ↓
EventCache commits
        ↓
CommittedCacheChange is emitted
        ↓
fava-observe updates affected query-source projections
        ↓
routers observing explicit/local queries may update contributions
        ↓
publication owners may recognize relay echo or a newer relevant source event
        ↓
diagnostics update
```

The same committed cache fact can drive query projection, routing knowledge, and publication reconciliation without any of those consumers acquiring ownership of the event cache.

---

## Accepting and publishing an unsigned event

```text
application or protocol crate constructs UnsignedEvent
        ↓
pubkey is already part of the event
        ↓
fava-publication validates event and WriteIntent
        ↓
WriteStore atomically commits:
    WriteId
    ReceiptId
    UnsignedEvent
    signature state
    QuerySource contribution
        ↓
write-source change is visible to fava-observe
        ↓
application receives Accepted + ReceiptId
        ↓
┌───────────────────────────────┴───────────────────────────────┐
│ signer for event.pubkey is requested                         │
│ Auto opens live RoutePlan; Explicit creates exact plan       │
└───────────────────────────────┬───────────────────────────────┘
        ↓
known destinations become lanes immediately
        ↓
exact Event is verified and installed in WriteStore
        ↓
delivery policy schedules attempts for lanes whose route and signature are ready
        ↓
publisher and transport perform one attempt per due lane
        ↓
WriteStore commits each result
        ↓
receipt and query evidence update
```

The event cache is not part of acceptance. A later relay echo enters the cache as a signed relay observation and merges into the same query `EventRecord`.

---

## Accepting a replaceable-event edit

```text
application calls follow(actor=alice, target=bob)
        ↓
fava-nip02 creates ReplaceableEventEdit
        ↓
publication owner reads current relevant EventRecord
        ↓
fava-nip02 materializes UnsignedEvent(kind:3, pubkey:alice)
        ↓
WriteStore commits operation, receipt, and materialization
        ↓
materialization appears through WriteStore QuerySource
        ↓
normal signing, routing, and delivery continue
```

If a newer source kind-3 arrives before settlement:

```text
CommittedCacheChange identifies affected coordinate
        ↓
publication owner reloads current source record
        ↓
fava-nip02 applies the accepted edit again
        ↓
WriteStore installs successor materialization generation
        ↓
query result changes directly from old local materialization to new
        ↓
stale signer/route/attempt completions cannot affect current generation
```

---

## Cancelling an unpublished write

```text
application requests cancel(receipt)
        ↓
publication owner evaluates current signature/handoff facts
        ↓
WriteStore commits cancelled state and removes current local contribution
        ↓
WriteStore QuerySource emits retraction
        ↓
query merge reveals any remaining cached predecessor
        ↓
router, signer, and delivery sessions for the write close
        ↓
receipt reports Cancelled
```

The event cache requires no compensation because it never stored the local unpublished materialization.

---

## Relay echo of a local publication

```text
WriteStore source:
    signed event E
    receipt 42
    relay A pending

relay A returns E through a subscription
        ↓
EventCache records E seen on relay A
        ↓
query merger combines both contributions
        ↓
EventRecord E now has:
    relay evidence: A
    publication evidence: receipt 42
```

A matching event id remains one application-facing event.

---

## Reconnection

```text
transport session generation N closes
        ↓
transport emits exact retired-session fact
        ↓
subscription and publication work attributed to N updates honestly
        ↓
transport opens generation N+1 according to transport policy
        ↓
current subscription plan is executed on N+1
        ↓
auth owner handles new challenge if configured
        ↓
publisher/delivery owners resume only work authorized by durable facts
```

Late frames and completions from generation N retain historical diagnostic value but cannot mutate current work.

---

## Restart

```text
application constructs selected assembly
        ↓
providers open their own storage formats
        ↓
WriteStore recovers open obligations and retained receipts
        ↓
selected protocol crates decode required replaceable-event edits
        ↓
publication owners reconstruct current operations/generations
        ↓
WriteStore QuerySource is ready
        ↓
EventCache opens with the guarantees of its implementation
        ↓
required source/write reconciliation completes
        ↓
facade enters Running
        ↓
application reopens desired queries
        ↓
open writes reopen signer and router sessions automatically
```

A memory event cache starts empty after restart. A persistent event cache may serve cached relay events immediately. The durable standard write store still supplies open local writes through queries once the application reopens them.

---

## Shutdown

```text
facade enters Closing
        ↓
new application work is refused
        ↓
pending facade calls receive terminal lifecycle facts
        ↓
observations close and withdraw logical demand
        ↓
publication owners stop admitting new effects
        ↓
router sessions and router-owned explicit queries close
        ↓
publisher/signer/auth operations are cancelled or detached by contract
        ↓
transport sessions close and join
        ↓
stores flush/close according to provider contract
        ↓
runtime joins owned resources
        ↓
facade enters Closed
```

Each resource is closed by its owner. The facade owns shutdown ordering.

---

# Part IX — Ownership ledger

## Single-owner map

| Fact or lifecycle | Authoritative owner | Derived/consumer owners |
|---|---|---|
| Wire message grammar | `fava-wire` | transport, publisher, ingest, test tools |
| Event-id/signature admission | `fava-ingest` | observations, cache, publication reconciliation |
| Nostr event-state rules | `fava-state` | evaluator, cache implementations, conformance suites |
| Retained relay-observed events | selected `EventCache` | query evaluator, routers using local event reader |
| Accepted replaceable-event edit | selected `WriteStore` | publication owner, owning protocol crate |
| Current local event materialization | selected `WriteStore` | query evaluator, signer, routing, publication |
| Receipt identity and durable receipt facts | selected `WriteStore` | publication owner, receipt observers, diagnostics |
| Open live-query handle | `fava-observe` | facade/SDK handle |
| Current merged query snapshot | `fava-observe` | application observer, diagnostics |
| Reactive dependency node | `fava-observe` | parent/child observations |
| One router's inputs and contribution | that router instance | `fava-routing` session |
| Merged automatic route plan | `fava-routing` session | observe/publication owner |
| Write route revision admitted for delivery | selected `WriteStore` | publication owner, delivery policy |
| Query demand for one relay | `fava-observe` | subscription planner |
| Wire subscription plan | `fava-observe` owns desired plan; planner computes it | transport executes it |
| Relay connection generation | selected `Transport` | auth, ingest, publisher, observe |
| NIP-42 challenge lifecycle | `fava-auth` | query/publication owners |
| Signer registration and availability | `fava-session` plus signer provider | publication/auth owners |
| One signing operation | publication/auth owner | runtime executes provider call |
| One publication attempt | publication owner | publisher performs; transport hands off |
| Delivery retry decision | selected `DeliveryPolicy` | publication owner persists/executes decision |
| NIP-05 cache policy | NIP-05 service | selected `FetchCache` stores entries |
| NIP-11 cache policy | NIP-11 service | selected `FetchCache` stores entries |
| Generic fetched cache bytes | selected `FetchCache` | owning protocol service |
| Current diagnostic snapshot | `fava-diagnostics` | facade/SDK observers |
| Execution resources and joins | `fava-runtime` | all state owners |
| Public engine lifecycle | `fava` | application/SDK |

The ledger should remain a maintained architecture artifact. Adding mutable state requires naming its owner and consumers.

---

## Ordering belongs to the lifecycle owner

Cross-subsystem ordering is owned by the subsystem whose lifecycle advances.

Examples:

### Query opening

`fava-observe` owns:

```text
source boundary -> initial evaluation -> handle release -> later updates
```

### Write acceptance

`fava-publication` owns:

```text
write-store commit -> query-source visibility -> Accepted result -> external work
```

### New route destination

`fava-publication` owns:

```text
route contribution -> write-store route revision -> delivery eligibility -> handoff
```

### Relay ingest

`fava-ingest` owns:

```text
wire attribution -> event verification -> admitted occurrence
```

The runtime transports messages between these owners. It does not replace their ordering contracts with one global semantic state machine.

---

# Part X — Dependency and packaging rules

## Dependency direction

The target direction is:

```text
domain values and pure rules
            ↑
neutral contracts
            ↑
provider implementations

domain values + contracts
            ↑
universal lifecycle owners
            ↑
runtime / facade
```

Contract and implementation edges run one way:

```text
fava-event-cache   <- event-cache implementations
fava-write-store   <- write-store implementations
fava-fetch-cache   <- fetch-cache implementations
fava-routing       <- router implementations
fava-subscriptions <- subscription planners
fava-transport     <- transport implementations
fava-publisher     <- publisher implementations
fava-delivery      <- delivery-policy implementations
fava-signer        <- signer/crypto implementations
```

The contract crate never imports its standard implementation. Universal owners depend on contracts, not on standard providers. Product-profile crates assemble the chosen implementations.

## Protocol-crate direction

Protocol crates depend on the Nostr values and Fava primitives they compose. They remain independent of:

```text
fava facade
fava-runtime
concrete event-cache and write-store implementations
concrete router implementations
concrete transport/publisher/delivery implementations
```

## Router direction

Router implementations depend on `fava-routing` and the domain values they understand. Algorithm-specific acquisition uses ordinary local or explicitly routed queries; it does not import concrete transport or storage implementations.

## Storage direction

A physical backend package may support several semantic provider adapters, but each adapter implements one named contract. Sharing one database handle is an implementation choice rather than a merged semantic authority.

# Part XI — Public contract summaries

## Contract matrix

| Contract | Consumes | Produces | Owns |
|---|---|---|---|
| `QueryEvaluator` | query plus current source contributions | exact query snapshot/change | query meaning and merge calculation |
| `EventCache` | admitted signed relay events and event queries | retained candidates, source snapshots/changes | cache retention and its guarantees |
| `WriteStore` | accepted write mutations and event queries | durable write facts, receipts, local event source | accepted obligation authority |
| `FetchCache` | opaque partition/key/value operations | cached bytes | generic cache storage only |
| `Router` | route request, upstream plan, router services | current async route contribution | one routing algorithm and its inputs |
| `SubscriptionPlanner` | logical demand for one session plus limits | exact wire plan and attribution | grouping/planning calculation |
| `Transport` | session specs and bytes | connection/handoff/inbound facts | physical relay resource |
| `Publisher` | one signed event and one session | one protocol attempt outcome | attempt protocol |
| `DeliveryPolicy` | durable lane facts and time | attempt/wait/park/give-up decision | delivery policy |
| `Signer` | exact unsigned event or crypto request | signed/crypto result | key custody and crypto execution |

---

## Contract design rules

Every replaceable contract should satisfy these rules:

1. **Domain values at the boundary.** No Redb transactions, Tokio handles, WebSocket objects, or provider-private state crosses a neutral contract unless that object is the responsibility itself.
2. **One responsibility.** The contract can be described in one sentence without “and also.”
3. **Current facts are explicit.** Providers do not read unrelated mutable globals.
4. **Late results are attributable.** Operations carry exact owner and generation identity.
5. **Bounds are explicit.** Provider outputs and retained state are bounded or produce a typed limit outcome.
6. **Failure is scoped.** Provider failure remains a provider fact and does not masquerade as relay, store, or application truth.
7. **Default and external parity.** The default implementation uses the same contract and conformance kit as external providers.
8. **Construction is sufficient.** The contract does not include runtime registration machinery unless its responsibility has a genuine runtime lifecycle.

---

# Part XII — Conformance and architectural falsification

## The architecture is a hypothesis

The crate map is not accepted merely because the responsibilities sound coherent. Each claimed boundary must survive attempts to replace, isolate, overload, and compose it.

A boundary is healthy when an external implementation can use it without private access and when changing one provider does not force unrelated providers to change.

---

## Falsifier A — external-provider proof

For every replaceable contract, build at least one implementation in a crate outside the Fava workspace.

The external crate must:

- depend only on public contracts and ordinary third-party dependencies;
- require zero edits to `fava`, runtime, or unrelated providers;
- run through an ordinary application assembly;
- pass the same conformance kit as the standard provider.

Required early examples:

- a static-table router;
- a no-grouping subscription planner;
- a scripted transport;
- a no-retry delivery policy;
- a memory event cache;
- an independent persistent write store.

---

## Falsifier B — asynchronous routing makes partial progress

Scenario:

1. A publication p-tags three pubkeys.
2. Outbox information is locally known for two.
3. The third requires an indexer query.
4. `AppRelayRouter` contributes one relay immediately.
5. The publication begins eligible work to all immediately known relays.
6. The unresolved lookup does not block signing or known destinations.
7. The third relay arrives later under the same receipt.
8. A relay contributed twice produces one physical lane with multiple reasons.
9. Explicit routing creates no router sessions.

Disabling asynchronous router updates must make the scenario fail.

---

## Falsifier C — ordered fallback reaction

Scenario:

1. `FallbackRelayRouter` initially sees one uncovered recipient and contributes fallback.
2. An upstream router later contributes sufficient recipient coverage.
3. The fallback router receives the new upstream revision.
4. Its current contribution retracts fallback for that recipient.
5. Unrelated destinations remain unchanged.
6. Any prior write handoff to fallback remains visible as historical evidence.

The test must also reverse the transition: loss of upstream coverage makes fallback appear according to policy.

---

## Falsifier D — routing policies remain separate crates

Dependency tests prove:

- `fava-routing` contains no NIP-65, hint, app-relay, or fallback semantics;
- each `fava-router-*` crate can be omitted independently;
- a custom hint router replaces `fava-router-hints` without editing routing primitive code;
- router order is determined only by assembly.

Adding a new router means adding one crate and one assembly entry.

---

## Falsifier E — event cache and write store remain independent sources

Scenario:

1. Accept an unsigned local event.
2. The write-store source makes it visible to a matching live query.
3. The event cache contains no unsigned/incomplete record.
4. Cancel the write; the local event retracts and any cached predecessor naturally reappears.
5. Rematerialize the write; the query swaps to the successor without cache compensation.
6. Sign it; the same event record updates signature state.
7. Receive a relay echo; cache/live ingress adds provenance to one merged event record.
8. Restart with a persistent write store and empty event cache; the accepted local event still appears when the query is reopened.

Any implementation that requires copying the unpublished event into the event cache fails this falsifier.

---

## Falsifier F — event-cache guarantee profiles

Run the same baseline query semantics against:

- bounded memory cache;
- persistent cache;
- aggressively evicting cache.

All must:

- retain only admitted signed events;
- return no fabricated events or provenance;
- emit source retractions when retained data disappears;
- preserve exact query merge semantics for data they currently expose.

Additional persistent-profile tests prove whichever restart, provenance, deletion, expiration, and coverage guarantees that implementation advertises.

The baseline contract must remain useful without forcing every cache to implement the persistent profile.

---

## Falsifier G — write-store durability and recovery

For the standard durable write store, crash after each boundary:

- before acceptance commit;
- after acceptance commit before `Accepted` reaches the application;
- after materialization install;
- after signature install;
- after route revision commit;
- after transport handoff before attempt outcome commit;
- during cancellation;
- during rematerialization.

After restart:

- accepted writes have the same receipt identity;
- no unaccepted write exists;
- current materialization is exact;
- stale signer/router/publisher operations do not apply;
- open lanes are reconstructed once;
- query-source visibility matches write-store truth.

---

## Falsifier H — protocol crate N+1

Add a new event-kind protocol crate outside the workspace.

It must provide:

- typed decode/validation;
- one query fragment;
- one replaceable-event edit and its inverse;
- edit application to current and empty source state;
- optional route context.

It must require zero edits to:

```text
fava
fava-runtime
fava-observe
fava-publication
fava-routing
fava-state
fava-query-standard
store implementations
transport implementations
```

Only application assembly changes.

---

## Falsifier I — query evaluator substitution

Run one fixed source corpus through:

- the standard evaluator;
- a deliberately full-reevaluation evaluator;
- an optimized incremental evaluator.

They must produce identical:

- event identities;
- replaceable winners;
- deletion/expiration effects;
- relay/publication evidence;
- ordering;
- derived-query growth and retraction;
- source-removal behavior.

The simple evaluator is the oracle for optimization.

---

## Falsifier J — subscription planner substitution

Run the same routed logical demand through:

- no grouping;
- standard grouping;
- an alternative exact grouping implementation.

Wire shapes may differ. Applications must observe identical query results, source evidence, EOSE attribution, cancellation, access isolation, and limit shortfalls.

A planner that silently drops demand to fit relay limits fails.

---

## Falsifier K — transport, publisher, and delivery separation

Replace each independently:

- WebSocket transport with scripted transport;
- NIP-01 publisher with a gateway publisher;
- standard delivery policy with no-retry and alternative-fairness policies.

Changing one must not require replacing or forking the other two.

A scripted transport must be able to produce:

- definite pre-handoff refusal;
- definite handoff;
- ambiguous loss;
- reconnect generation change;
- malformed inbound bytes.

The publisher interprets protocol outcomes. The delivery policy decides the next action from committed facts.

---

## Falsifier L — fetched-data cache separation

Use one `FetchCache` implementation with both NIP-05 and NIP-11 services.

Tests prove:

- each service has an independent namespace;
- each service applies its own freshness and negative-cache rules;
- an NIP-05 entry cannot be interpreted as NIP-11 data;
- event-cache reset does not erase fetched documents unless the application resets that cache too;
- fetch-cache eviction causes refetching rather than event-state mutation.

---

## Falsifier M — provider failure isolation

Deliberately make providers:

- block;
- return late;
- panic;
- exceed declared output bounds;
- ignore cancellation;
- fail during shutdown.

Unrelated queries, relays, writes, signers, and services must retain bounded progress.

No provider call may run inside another owner's durable transaction or authoritative lock. Late results must be rejected by exact owner/generation identity.

---

## Falsifier N — ownership audit

For every mutable field in state-owner and provider crates, record:

- domain fact;
- owner;
- creation boundary;
- mutation boundaries;
- terminal boundary;
- consumers;
- persistence location if any.

The audit fails when:

- two crates can mutate the same fact;
- a derived cache is treated as authority;
- an owner cannot explain how its state terminates;
- a runtime queue carries policy not owned by its producer/consumer;
- receipt, route, or connection identity can be reconstructed ambiguously.

---

## Falsifier O — dependency-negative tests

Compiler-level tests prove that:

- `fava-routing` cannot import `fava-router-outbox` or any other router implementation;
- protocol crates cannot import runtime/facade internals;
- transport implementations cannot import write stores or routing algorithms;
- `fava-publication` cannot import standard delivery or publisher implementations;
- `fava` cannot name event-kind protocol crates;
- standard providers have no private bypass unavailable to external providers.

---

## Falsifier P — change amplification

Track architectural cost for ordinary changes:

- crates/files changed by adding protocol crate N+1;
- crates/files changed by adding a router;
- crates rebuilt after editing one protocol crate;
- adapter code needed to expose one protocol crate in a selected native artifact;
- public contracts affected by changing a standard provider's internal algorithm.

Expected results:

```text
new protocol support: protocol crate + assembly/artifact selection
new router: router crate + assembly
new event cache: provider crate + assembly
new subscription planner: provider crate + assembly
```

Broad edits falsify the claimed boundary.

---

## Application capstones

The architecture is accepted only after ordinary external applications prove:

1. a live query combining persistent cached events, local unpublished events, and live relay ingress;
2. a replaceable-event edit applied again over a newer relay event;
3. a partial asynchronous route that adds a later recipient under one receipt;
4. a custom router crate outside the workspace;
5. a custom event-cache implementation;
6. a custom subscription planner;
7. process restart with durable accepted writes;
8. NIP-05 and NIP-11 services sharing a fetch cache without semantic leakage;
9. Swift and Kotlin products built from explicit provider and protocol-crate selections.

---

# Part XIII — Implementation sequence

## Build vertical slices, not an empty provider framework

Contracts should be stabilized only after a complete slice and a competing implementation have challenged them.

### Slice 1 — local source merge

Build:

- `fava-state`;
- `fava-query`;
- standard evaluator;
- memory event cache;
- memory write store;
- `fava-observe` local-only path.

Prove:

- one `EventRecord` merge;
- unsigned local visibility;
- exact relay-event dedup;
- cancellation retraction;
- replaceable winner changes;
- no event-cache pollution by local writes.

### Slice 2 — explicit one-relay query

Add:

- `fava-wire`;
- subscription contract plus no-grouping planner;
- scripted transport;
- `fava-ingest`;
- explicit query routing.

Prove one live query end to end before automatic routing.

### Slice 3 — asynchronous router primitive

Add:

- `fava-routing`;
- static test router;
- delayed test router;
- app-relay router;
- fallback router.

Prove immediate partial plans, later additions, upstream fallback reaction, and explicit bypass.

### Slice 4 — explicit-route publication

Add:

- write-store durable implementation;
- `fava-publication`;
- signer contract and local signer;
- publisher and delivery contracts;
- scripted publisher/transport.

Prove acceptance, query visibility, signing, one attempt, receipt, cancellation, and restart.

### Slice 5 — standard routing and wire planning

Add:

- NIP-65 values;
- outbox router;
- hints router;
- standard subscription planner;
- WebSocket transport;
- NIP-01 publisher;
- standard delivery policy.

Prove partial recipient routing under real router-owned explicit queries.

### Slice 6 — replaceable-event edits

Add one protocol crate such as NIP-02:

- typed edit and inverse;
- edit application;
- source-state change and rematerialization;
- stale signer/delivery result rejection;
- same receipt across generations.

Then add a second unrelated protocol crate to challenge the contract.

### Slice 7 — fetched services

Add:

- fetch-cache contract and memory implementation;
- NIP-05 service;
- NIP-11 service;
- persistent fetch-cache implementation.

Prove service-specific freshness over one generic cache.

### Slice 8 — native products

Build explicit Swift and Kotlin artifacts from selected profiles. Run the same behavioral corpus through direct Rust and each public SDK.

---

# Part XIV — Concise crate inventory

## Foundational rules

| Crate | One-sentence responsibility |
|---|---|
| `fava-wire` | Nostr relay/client message grammar and bytes. |
| `fava-state` | Deterministic event-set identity, replacement, deletion, expiry, and evidence semantics. |
| `fava-write` | Event construction, write-intent, receipt, and publication values. |
| `fava-query` | Live-query language, `EventRecord`, source observations, and evaluator contract. |
| `fava-query-standard` | Recommended evaluator and behavior oracle over merged event sources. |

## Local data contracts and providers

| Crate | One-sentence responsibility |
|---|---|
| `fava-event-cache` | Contract for retaining and querying admitted signed relay events. |
| `fava-event-cache-memory` | Bounded current-process event cache. |
| `fava-event-cache-redb` | Persistent indexed event-cache implementation. |
| `fava-write-store` | Contract for accepted writes, receipts, current materializations, and recovery. |
| `fava-write-store-redb` | Standard durable write-store implementation. |
| `fava-write-store-memory` | Volatile reference/test write store. |
| `fava-fetch-cache` | Opaque partitioned cache contract for service-owned fetched data. |
| `fava-fetch-cache-memory` | Bounded memory fetched-data cache. |
| `fava-fetch-cache-redb` | Persistent fetched-data cache. |

## Routing and relay work

| Crate | One-sentence responsibility |
|---|---|
| `fava-routing` | Ordered asynchronous router composition and merged route plans. |
| `fava-router-outbox` | NIP-65 author-outbox and recipient-inbox routing. |
| `fava-router-hints` | Relay-hint and observed-reference routing. |
| `fava-router-app-relays` | Always contribute configured app relays for selected operations. |
| `fava-router-fallback-relays` | Contribute fallback relays from upstream coverage policy. |
| `fava-subscriptions` | Per-relay logical-demand to wire-plan contract. |
| `fava-subscriptions-standard` | Recommended exact grouping/coalescing planner. |
| `fava-transport` | Relay-session, byte-handoff, and inbound-byte contracts. |
| `fava-transport-websocket` | Standard WebSocket relay transport. |

## Publication and identity

| Crate | One-sentence responsibility |
|---|---|
| `fava-publisher` | One publication-attempt contract. |
| `fava-publisher-nip01` | One NIP-01 EVENT/OK publication attempt. |
| `fava-delivery` | Delivery-policy decision contract over durable lane facts. |
| `fava-delivery-standard` | Standard bounded retry, fairness, and ambiguity policy. |
| `fava-signer` | Identity-keyed signing and crypto provider contracts. |
| `fava-signer-local` | Local in-process signer provider. |
| `fava-signer-nip46` | Remote NIP-46 signer provider. |

## State and lifecycle owners

| Crate | One-sentence responsibility |
|---|---|
| `fava-ingest` | Admission of attributed, verified relay events. |
| `fava-observe` | Live-query lifecycle, source merge, routing, relay demand, and bounded result delivery. |
| `fava-publication` | Accepted-write lifecycle across materialization, signing, routing, delivery, and receipts. |
| `fava-session` | Accounts, current-account input, signer registrations, and session restore. |
| `fava-auth` | NIP-42 challenge and authentication lifecycle. |
| `fava-diagnostics` | Bounded current diagnostic facts and explanations. |
| `fava-runtime` | Execution resources, provider isolation, cancellation, timers, and shutdown joins. |
| `fava` | Thin public facade and assembly builder. |
| `fava-standard` | Recommended provider assembly and its documented profile guarantees. |

## Protocol services and event-kind crates

| Crate | One-sentence responsibility |
|---|---|
| `fava-nip05` | NIP-05 values, validation, and resolution semantics. |
| `fava-nip05-http` | HTTP NIP-05 acquisition and service-owned caching policy. |
| `fava-nip11` | NIP-11 values, parsing, validation, and freshness vocabulary. |
| `fava-nip11-http` | HTTP NIP-11 acquisition and service-owned caching policy. |
| `fava-nip65` | Pure NIP-65 relay-list event semantics. |
| `fava-nip02`, `fava-nip29`, `fava-bookmarks`, ... | Independent event-kind protocol crates built from query, event-building, and write primitives. |
| `fava-content` | Pure content parsing into structured values without rendering or acquisition. |

## Testing packages

Recommended public test tooling:

```text
fava-router-testkit
fava-event-cache-testkit
fava-write-store-testkit
fava-subscriptions-testkit
fava-transport-testkit
fava-publisher-testkit
fava-signer-testkit
```

The relay-lab role — real third-party relay process fixtures, wire evidence, and reconstructable run artifacts — is owned by the external `apps/canary` application, not a workspace crate. The canary is the independent relay-lab; no `fava-relay-lab` crate is created.

Each conformance kit is versioned with its contract and can be used by external provider crates.

---

# Part XV — Final architecture statement

## One-paragraph north star

Fava is a thin facade over independently owned query, publication, session, auth, cache, routing, subscription, transport, publisher, delivery, and signer responsibilities. Live queries merge relay-event cache state, accepted-write materializations, reactive dependencies, and admitted live events into one `EventRecord` view. Automatic routing is an ordered asynchronous chain of independent router crates whose immediately known destinations are useful before every route need settles. Accepted local writes remain authoritative in a dedicated write store and become visible through that store's query source rather than by inserting incomplete events into the event cache. Fetched NIP-05, NIP-11, and similar data use service-owned caches. Protocol crates own event-kind meaning and compose ordinary query, event-building, and write values. Applications select providers at build/construction time, defaults receive no privileged authority, and every claimed boundary is validated by an external implementation and an adversarial falsifier.

## Test for every architectural decision

For any proposed crate, interface, or state field, answer:

1. What single responsibility does it own?
2. What mutable state and lifecycle belong to it?
3. Which facts does it consume, and who owns them?
4. Which facts does it produce, and who may act on them?
5. Can another crate implement the contract without private access?
6. Can it fail or block without stopping unrelated work?
7. Can its current state be reconstructed or terminated exactly?
8. Does adding another protocol crate or provider leave unrelated crates unchanged?
9. Does the boundary remove policy from a primitive rather than merely move files?
10. What executable scenario would prove this boundary wrong?

A boundary that cannot answer these questions is not ready to stabilize.
