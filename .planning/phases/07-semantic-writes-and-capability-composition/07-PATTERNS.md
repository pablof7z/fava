# Phase 7: Semantic Writes and Capability Composition - Pattern Map

**Mapped:** 2026-08-21
**Baseline:** `309e421` (`feat: complete M6 automatic write routing`)
**Candidate files classified:** 43 across 19 change sets
**Complete analogs found:** 38 / 43; five M7 semantic-generation gaps have only partial structural analogs

## Approved Symbols and Scope Fence

The architecture slice already approved exactly these new public nouns:

- `ReplaceableEventEdit`
- `ReplaceableEventMaterializer`
- `MaterializationId`

Use those names verbatim. Internal module names below are proposed cohesion boundaries, not additional vocabulary. Do not introduce another public generation token, edit wrapper, registry noun, signer-operation noun, route-operation noun, or durable-record noun without a separate vocabulary approval.

The durable/live ownership split is fixed: `WriteStore` owns accepted edit custody, stable `WriteId`/`ReceiptId`, current `MaterializationId`, current event, receipt, query-source contribution, and every final currentness check. `fava-publication` owns live tasks and requests correlated effects. Protocol crates own event-kind meaning and edit application only.

## File Classification

| New/Modified File(s) | Role | Data Flow | Closest Current Analog | Match Quality |
|---|---|---|---|---|
| `crates/fava-write/src/lib.rs` | model facade | transform | same file, especially IDs, `WritePayload`, receipt values | exact role |
| `crates/fava-write/src/replaceable_event_edit.rs` (new) | model | transform / durable bytes | `crates/fava-write/src/builder.rs` plus `crates/fava-nip65/src/lib.rs` | partial; no semantic-edit value exists |
| `crates/fava-write/src/materialization.rs` (new) | model + provider contract | request-response / transform | `crates/fava-signer/src/lib.rs` plus `WriteId` in `fava-write` | partial; no materializer/generation contract exists |
| `crates/fava-write/BUILD.bazel` | config/test target | batch | `crates/fava-nip65/BUILD.bazel` | role-match |
| `crates/fava-write-store/src/lib.rs` | provider contract | CRUD + event-driven | same `WriteStore` trait | exact role |
| `crates/fava-write-store-memory/src/lib.rs`, `src/model.rs`, `src/semantic.rs` (new), `BUILD.bazel` | store/model/test | CRUD + event-driven + transform | current memory acceptance/currentness implementation and `crates/fava/tests/write_bounds.rs` | exact role; semantic state is new |
| `crates/fava-write-store-redb/src/lib.rs`, `src/ops.rs`, `src/schema.rs` (new) | durable store/model | CRUD + file-I/O | current redb immediate transactions, load, and update helper | exact role; versioned semantic schema is new |
| `crates/fava-write-store-redb/tests/process_kill.rs` | durability test | file-I/O + process-driven | same M5 SIGKILL harness | exact |
| `crates/fava-publication/src/lib.rs`, `src/run.rs`, `src/materialization.rs` (new) | service/orchestrator | event-driven + streaming | current receipt runner, signer/route/lane orchestration | exact role; source-driven rematerialization is new |
| `crates/fava-signer/src/lib.rs`, `crates/fava-routing/src/lib.rs`, `crates/fava-publisher/src/lib.rs` | provider contracts/models | request-response + streaming | current exact event/session/attempt values | exact role |
| `crates/fava-query/src/lib.rs` (conditional) | query contract | streaming | current `FilterSelection` and `QuerySource` snapshot stream | exact role; modify only if bounded exact-coordinate observation cannot be composed privately |
| `crates/fava/src/lib.rs`, `crates/fava/tests/semantic_writes.rs` (new), `crates/fava/Cargo.toml`, `crates/fava/BUILD.bazel` | facade/builder/integration test/config | request-response + event-driven | `FavaBuilder` and `tests/explicit_publication.rs` | exact role / test role-match |
| `crates/fava-nip02/{src/lib.rs,Cargo.toml,BUILD.bazel}` (new) | protocol model/materializer/config | transform | `crates/fava-nip65/*` | role-match |
| `crates/fava-bookmarks/{src/lib.rs,Cargo.toml,BUILD.bazel}` (new) | protocol model/materializer/config | transform | `crates/fava-nip65/*` | role-match |
| `Cargo.toml`, `Cargo.lock` | workspace config/generated lock | batch | current `fava-nip65` membership/dependency entries | exact role |
| `apps/canary/src/semantic_writes.rs` (new), `src/lib.rs`, `src/main.rs`, `scenarios.json`, `Cargo.toml`, `Cargo.lock` | canary/selected assembly/config | event-driven + process/file evidence | `automatic_publication.rs`, current registry/dispatcher | role-match |
| `falsifiers/external-protocol-capability/{Cargo.toml,src/lib.rs,Cargo.lock}` (new) | external provider/test workspace | request-response + transform | `falsifiers/external-null-cache/*` | role-match |
| `docs/internals/vocabulary.toml` | architecture config | batch validation | current approved M7 entries | exact |
| `docs/issues/0010-m7-semantic-writes-and-capability-composition.md` | evidence ledger | batch/manual deliberate break | current M7 issue exit-gate section | exact role |

Generated `Cargo.lock` files follow manifest changes; do not hand-edit their package graph. No `MODULE.bazel` change is implied by the current crate-universe setup.

## Pattern Assignments

### `fava-write`: values, opaque edit, materializer contract, and generation identity

**Analogs:** `crates/fava-write/src/lib.rs`, `crates/fava-write/src/builder.rs`, `crates/fava-signer/src/lib.rs`

**Module/re-export pattern** (`crates/fava-write/src/lib.rs:12-17`):

```rust
mod attempt_map;
mod builder;
mod delivery_map;
mod session_set;

pub use builder::{EventBuildError, EventBuilder};
```

Keep the already 467-line `lib.rs` as the public facade. Put the new cohesive value and provider code in `replaceable_event_edit.rs` and `materialization.rs`, then re-export the three approved symbols from `lib.rs`.

**Provider-independent identity pattern** (`crates/fava-write/src/lib.rs:22-38`):

```rust
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct WriteId(u64);

impl WriteId {
    #[must_use]
    pub const fn from_u64(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}
```

`MaterializationId` should copy this opaque, serializable, ordered newtype pattern. The store allocates/persists it; publication and providers only carry it.

**Third payload form seam** (`crates/fava-write/src/lib.rs:44-67`):

```rust
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum WritePayload {
    Event(UnsignedEvent),
    Presigned(Event),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WriteIntent {
    payload: WritePayload,
    routing: WriteRouting,
}
```

Add `ReplaceableEventEdit` as the third authoritative accepted payload. The edit must contain its actor before materialization, exact coordinate, protocol-owned format/version discriminator, and bounded opaque bytes. Do not add follow/bookmark variants here.

**Validate before custody pattern** (`crates/fava-write/src/lib.rs:69-120`, `189-200`):

```rust
event.ensure_id();
event
    .verify_id()
    .map_err(|error| WriteIntentError::InvalidEvent(error.to_string()))?;
validate_routing(&routing)?;
validate_event_size(&event)?;
// ... expiry refusal ...
Ok(Self {
    payload: WritePayload::Event(event),
    routing,
})
```

Copy the typed constructor/refusal pattern for semantic edits: reject byte, coordinate, and routing bounds before store mutation. After materialization, repeat ordinary event-id/body, author, coordinate, size/tag, and expiry validation before atomic install.

**Kind-agnostic event construction** (`crates/fava-write/src/builder.rs:19-37`, `53-83`):

```rust
pub fn new(author: PublicKey, kind: Kind) -> Self { /* fields only */ }

pub const fn created_at(mut self, created_at: Timestamp) -> Self {
    self.created_at = created_at;
    self
}

pub fn build(self) -> Result<UnsignedEvent, EventBuildError> {
    // tag and byte bounds
    let mut event = UnsignedEvent::new(
        self.author,
        self.created_at,
        self.kind,
        self.tags,
        self.content,
    );
    event.ensure_id();
    // exact serialized-size check
    Ok(event)
}
```

Protocol materializers must use this general builder. Keep timestamp an injected exact input; protocol crates must not call wall-clock time internally. CAP-09's raw future-kind proof should construct an arbitrary `Kind` through this unchanged path.

**Replaceable provider shape** (`crates/fava-signer/src/lib.rs:19-32`):

```rust
pub trait Signer: Send + Sync {
    fn public_key(&self) -> PublicKey;
    fn availability(&self) -> SignerAvailability;
    fn sign_event(
        &self,
        event: UnsignedEvent,
        cancel: watch::Receiver<bool>,
    ) -> Pin<Box<dyn Future<Output = Result<Event, SignerError>> + Send + '_>>;
}
```

`ReplaceableEventMaterializer` should follow the small object-safe `Send + Sync` contract style and return a typed result. Its semantic input is `ReplaceableEventEdit` plus qualified signed source or `None` and exact materialization context. It returns an unsigned event value only; it cannot receive store, signer, router, publisher, transport, delivery, or receipt authority.

### `WriteStore`: durable authority and exact currentness

**Analog:** `crates/fava-write-store/src/lib.rs`

**Neutral contract and committed notification pattern** (`crates/fava-write-store/src/lib.rs:15-40`):

```rust
pub struct AcceptedWrite {
    pub write_id: WriteId,
    pub receipt_id: ReceiptId,
    pub current: LocalWriteEvent,
}

pub trait WriteStore: QuerySource + Send + Sync {
    fn receipt_changes(&self) -> broadcast::Receiver<(ReceiptId, Option<Receipt>)>;
    fn accept(&self, intent: WriteIntent) -> Result<AcceptedWrite, WriteStoreError>;
}
```

Extend this contract first, then make both providers conform. Semantic acceptance must not return `AcceptedWrite` until edit custody, stable IDs, generation 1, event, receipt, and query-source row are one committed mutation.

**Current M6 correlation gap** (`crates/fava-write-store/src/lib.rs:58-116`):

```rust
fn install_signed(&self, receipt_id: ReceiptId, event: Event) -> Result<Receipt, WriteStoreError>;
fn apply_route(&self, receipt_id: ReceiptId, plan: &RoutePlan) -> Result<Receipt, WriteStoreError>;
fn begin_attempt(
    &self,
    receipt_id: ReceiptId,
    session: &RelaySessionKey,
) -> Result<Receipt, WriteStoreError>;
fn record_outcome(
    &self,
    receipt_id: ReceiptId,
    session: &RelaySessionKey,
    outcome: RelayDeliveryOutcome,
) -> Result<Receipt, WriteStoreError>;
```

These signatures are the delta, not the final M7 pattern. Every signer/refusal/route/attempt/outcome mutation must additionally carry and validate the exact current `MaterializationId`, exact event body/id, and operation-specific route revision, relay session, and durable attempt number as applicable. A mismatch returns typed stale/refused and mutates nothing. Cancellation remains advisory; store equality is authoritative.

**Shared bounded mutation helper pattern** (`crates/fava-write-store/src/lib.rs:229-323`):

```rust
pub fn apply_route_to_receipt(
    receipt: &mut Receipt,
    plan: &RoutePlan,
) -> Result<(), WriteStoreError> {
    if plan.revision <= receipt.route_revision {
        return Err(WriteStoreError::Refused(/* exact reason */));
    }
    // validate fan-out and bounded text before mutation
    // mutate complete replacement state
    receipt.route_revision = plan.revision;
    receipt.desired_destinations = desired;
    settle_route(receipt);
    Ok(())
}
```

Put provider-neutral generation/currentness validation beside the contract when both memory and redb must behave identically. Keep actual locking/transactions in provider crates.

### Memory store model and state-machine evidence

**Analogs:** `crates/fava-write-store-memory/src/lib.rs`, `crates/fava/tests/write_bounds.rs`

**Single owner state pattern** (`crates/fava-write-store-memory/src/lib.rs:29-50`):

```rust
pub struct MemoryWriteStore {
    capacity: NonZeroUsize,
    state: Mutex<WriteState>,
    latest: watch::Sender<Arc<SourceSnapshot>>,
    receipt_changes: broadcast::Sender<(ReceiptId, Option<Receipt>)>,
}

struct WriteState {
    revision: u64,
    next_identity: u64,
    writes: BTreeMap<ReceiptId, Receipt>,
}
```

Extend this one authoritative state with bounded coordinate/edit/current-generation/source-basis indexing. Do not create a second protocol-owned store. Put semantic compare/apply helpers in `semantic.rs` so `lib.rs` does not cross the 500-line soft limit.

**Commit-before-observe pattern** (`crates/fava-write-store-memory/src/lib.rs:110-155`):

```rust
let write_id = WriteId::from_u64(identity);
let receipt_id = ReceiptId::from_u64(identity);
// build complete receipt
guard.next_identity = next_identity;
guard.revision = next_revision;
guard.writes.insert(receipt_id, receipt.clone());
self.publish_snapshot(&guard);
let _ = self.receipt_changes.send((receipt_id, Some(receipt)));
Ok(AcceptedWrite { write_id, receipt_id, current })
```

For rematerialization, compute the candidate outside the mutex, then lock, recheck expected current `MaterializationId` and source-basis id, install the successor and direct old-local-to-new-local snapshot atomically, unlock, and only then notify.

**Exact immutable-body comparison pattern** (`crates/fava-write-store-memory/src/lib.rs:158-196`):

```rust
let EventValue::Unsigned(unsigned) = &receipt.current.event else {
    return Err(WriteStoreError::Refused("event is already signed".to_owned()));
};
if UnsignedEventView::from(unsigned) != UnsignedEventView::from(&event) {
    return Err(WriteStoreError::Refused(
        "signature does not match current unsigned event".to_owned(),
    ));
}
receipt.current.event = EventValue::Signed(event);
```

Retain the whole-body equality check and add `MaterializationId` currentness before it.

**Atomic refusal test pattern** (`crates/fava/tests/write_bounds.rs:157-197`):

```rust
let before = store.apply_route(accepted.receipt_id, &first).unwrap();
// construct an invalid successor
assert!(store.apply_route(accepted.receipt_id, &refused).is_err());
assert_eq!(store.receipt(accepted.receipt_id).unwrap(), Some(before));
```

The memory semantic corpus should assert both the typed result and complete state equality after wrong generation, wrong event/body, wrong signer operation, wrong route revision, wrong session, wrong attempt, materializer refusal/panic, capacity refusal, and unrelated-coordinate source change.

### Redb persistence, recovery, and process-kill proof

**Analogs:** `crates/fava-write-store-redb/src/lib.rs`, `src/ops.rs`, `tests/process_kill.rs`

**Immediate atomic transaction pattern** (`crates/fava-write-store-redb/src/lib.rs:92-108`):

```rust
let mut transaction = self.database.begin_write().map_err(refused)?;
transaction.set_durability(Durability::Immediate).map_err(refused)?;
{
    let mut receipts = transaction.open_table(RECEIPTS).map_err(refused)?;
    let bytes = serde_json::to_vec(receipt).map_err(refused)?;
    receipts.insert(receipt.receipt_id.as_u64(), bytes.as_slice()).map_err(refused)?;
}
{
    let mut meta = transaction.open_table(META).map_err(refused)?;
    meta.insert(NEXT_ID, next_identity).map_err(refused)?;
}
transaction.commit().map_err(refused)
```

The M7 acceptance/successor transaction must include the versioned durable write record, opaque edit bytes, stable IDs, `MaterializationId`, exact current event, source basis, receipt, current/correction destinations, attempt facts, and the query-visible replacement. Do not split those facts across commits.

**Persist, then publish pattern** (`crates/fava-write-store-redb/src/ops.rs:290-312`):

```rust
mutation(&mut receipt)?;
let removals = terminal_evictions(&state, &receipt, self.limits.terminal.get());
let next_revision = next_revision(&state)?;
self.commit_update(Some(&receipt), &removals)?;
// update in-memory mirror only after durable commit
state.receipts.insert(receipt_id, receipt.clone());
state.revision = next_revision;
self.publish_snapshot(&state);
self.publish_receipt(Some(receipt.clone()), receipt_id);
```

Copy this ordering exactly for generation install. Never call `ReplaceableEventMaterializer` while holding the mutex or a redb transaction.

**Current schema gap and hard-refusal seam** (`crates/fava-write-store-redb/src/lib.rs:193-214`):

```rust
for entry in table.iter().map_err(refused)? {
    let (_, value) = entry.map_err(refused)?;
    let receipt: Receipt = serde_json::from_slice(value.value()).map_err(refused)?;
    if receipts.insert(receipt.receipt_id, receipt).is_some() {
        return Err(WriteStoreError::Refused(
            "duplicate durable receipt identity".to_owned(),
        ));
    }
}
```

This unversioned whole-`Receipt` JSON is not an M7 pattern to copy. Add an explicit hard-cut schema envelope in `schema.rs`; preserve protocol bytes exactly; reject unknown durable/edit versions and missing selected materializers on open/build. Do not use `#[serde(default)]` to invent generation facts.

**SIGKILL harness pattern** (`crates/fava-write-store-redb/tests/process_kill.rs:71-108`):

```rust
for boundary in ["before-accept", "acceptance", "signature", "attempt", "outcome", "cancel"] {
    let mut child = Command::new(env::current_exe().expect("test executable"))
        .args(["--exact", "boundary_child", "--nocapture"])
        .env(CHILD_BOUNDARY, boundary)
        // exact database and marker paths
        .spawn()
        .expect("boundary child starts");
    wait_for(&marker, &mut child);
    child.kill().expect("SIGKILL succeeds");
    child.wait().expect("killed child reaped");
    let store = RedbWriteStore::open(&database).expect("store recovers after kill");
    assert_boundary(boundary, receipt.as_ref());
}
```

Extend this file with M7 boundaries: before semantic accept; after generation 1; source v2 before successor commit; after generation 2 before effects; predecessor attempt before successor correction; delayed retired completion; unknown edit version/missing materializer; bounded reopen after many supersessions.

### Publication orchestration and exact signer/route/publisher identity

**Analogs:** `crates/fava-publication/src/lib.rs`, `src/run.rs`, `fava-signer`, `fava-routing`, `fava-publisher`

**Registry assembly pattern** (`crates/fava-publication/src/lib.rs:36-59`):

```rust
let mut indexed = BTreeMap::new();
for signer in signers {
    let public_key = signer.public_key();
    if indexed.insert(public_key, signer).is_some() {
        return Err(PublicationError::DuplicateSigner(public_key));
    }
}
```

Index selected `ReplaceableEventMaterializer` implementations by their neutral claimed format/coordinate domain using the same duplicate-refusal pattern. Do not branch on NIP-02, bookmarks, kind 3, or kind 10003.

**Commit before effect and recover-current pattern** (`crates/fava-publication/src/lib.rs:62-87`):

```rust
let accepted = self.store.accept(intent)?;
self.start(accepted.receipt_id);
Ok(accepted)

let receipts = self.store.recover_open()?;
for receipt in receipts {
    self.start(receipt.receipt_id);
}
```

For semantic writes, resolve materializer and compute the first candidate before the short atomic acceptance commit; start signer/route work only from the committed current `MaterializationId`. On recovery, selected materializers must already exist before `recover_open`/reconciliation begins and before the builder admits new commands.

**Current task-key gap** (`crates/fava-publication/src/run.rs:17-30`):

```rust
pub(super) fn start(&self, receipt_id: ReceiptId) {
    // one cancellation slot keyed only by receipt
    cancellations.insert(receipt_id, cancel);
    tokio::spawn(async move { publication.run(receipt_id, cancel_rx).await });
}
```

M7 live work must be keyed by stable receipt plus current `MaterializationId`; starting a successor retires/cancels predecessor tasks, but delayed predecessor results still flow to store mutations carrying their old identity and are rejected there.

**Current signer correlation gap** (`crates/fava-publication/src/run.rs:134-164`):

```rust
let receipt_id = receipt.receipt_id;
tokio::spawn(async move {
    match signer.sign_event(unsigned, cancel).await {
        Ok(event) => {
            if publication.store.install_signed(receipt_id, event).is_err() {
                // refusal recorded only by receipt today
            }
        }
        // ...
    }
});
```

Capture the current `MaterializationId` and exact unsigned body before spawning. Pass both back to `install_signed`/refusal so a generation-1 signer result cannot sign or refuse generation 2.

**Route value seam** (`crates/fava-routing/src/lib.rs:18-25`, `197-212`):

```rust
pub enum RouteRequest {
    Read(Query),
    Write(EventValue),
}

pub struct RoutePlan {
    pub revision: u64,
    pub destinations: BTreeMap<RelaySessionKey, PlannedRelay>,
    // complete current coverage/shortfall state
    pub settled: bool,
}
```

Keep routers ignorant of semantic meaning. Thread `MaterializationId` and exact event through write-route acquisition/application without adding protocol variants. Successor destinations must union the current route with bounded predecessor destinations that may require correction; outcomes remain generation-scoped.

**Exact attempt value seam** (`crates/fava-publisher/src/lib.rs:11-25`):

```rust
pub struct PublishAttempt {
    pub write_id: WriteId,
    pub receipt_id: ReceiptId,
    pub number: u32,
    pub session: RelaySessionKey,
    pub event: Event,
    pub timeout: Duration,
}
```

Add `MaterializationId`; retain exact signed event, relay session, and one-based durable attempt number. Store-side `record_outcome` must compare all of them, not trust the publisher task's currentness.

**Effect construction pattern** (`crates/fava-publication/src/run.rs:227-248`):

```rust
let Ok(receipt) = self.store.begin_attempt(receipt_id, session) else { return; };
let EventValue::Signed(event) = receipt.current.event.clone() else { return; };
let attempt = PublishAttempt {
    write_id: receipt.write_id,
    receipt_id,
    number: receipt.attempts.get(session).copied().unwrap_or(0),
    session: session.clone(),
    event,
    timeout: ATTEMPT_TIMEOUT,
};
let outcome = self.publisher.publish(attempt, self.transport.as_ref()).await;
```

Continue deriving effects from the receipt returned by the durable authorization mutation. In M7, that returned receipt provides the exact current `MaterializationId`, event, session, and attempt identity to echo on completion.

### Qualified source observation without self-feedback

**Analogs:** `crates/fava-query/src/lib.rs`, `crates/fava-query-standard/src/lib.rs`

**Current query axes** (`crates/fava-query/src/lib.rs:16-25`):

```rust
pub struct FilterSelection {
    pub ids: Option<BTreeSet<EventId>>,
    pub authors: Option<BTreeSet<PublicKey>>,
    pub kinds: Option<BTreeSet<Kind>>,
}
```

The two selected M7 capabilities are ordinary non-addressable replacements, so author+kind can form a bounded query. Modify this public contract only if the external N+1 proof requires addressable coordinates and no private exact-coordinate composition is possible; any new public selection noun needs vocabulary approval.

**Gapless source stream pattern** (`crates/fava-query/src/lib.rs:330-359`):

```rust
pub trait SourceChanges: Send {
    fn next_change(&mut self) -> SourceChangeFuture<'_>;
    fn close(&mut self);
}

pub struct OpenedQuerySource {
    pub initial: SourceSnapshot,
    pub changes: Box<dyn SourceChanges>,
}

pub trait QuerySource: Send + Sync {
    fn open(&self, query: &Query) -> Result<OpenedQuerySource, QuerySourceError>;
}
```

Publication should open one bounded coordinate-scoped cache/source observation per active coordinate, coalesce only by reloading exact current source, and close it when no live edits remain.

**Why the merged winner is unsafe** (`crates/fava-query-standard/src/lib.rs:23-49`, `91-110`):

```rust
for source in sources {
    for contribution in &source.events {
        merge_contribution(&mut by_id, contribution)?;
    }
}
// replacement winner is chosen after cached and local contributions merge
let mut by_coordinate = BTreeMap::<EventCoordinate, EventRecord>::new();

SourceEvent::Local(local) => {
    // local write-store materialization participates in the same merged record set
    record.publication = Some(local.publication.clone());
}
```

Do not feed `AnyLocal`'s merged winner back into the same edit. M7 qualified source is the newest signed relay-observed/cache event at the exact coordinate, or `None`; explicitly exclude the operation's own write-store contribution.

### `FavaBuilder` selected assembly and public facade tests

**Analogs:** `crates/fava/src/lib.rs`, `crates/fava/tests/explicit_publication.rs`

**Static selected-provider fields and fluent registration** (`crates/fava/src/lib.rs:230-263`, `312-337`):

```rust
#[derive(Default)]
pub struct FavaBuilder {
    event_cache: Option<Arc<dyn EventCache>>,
    write_store: Option<Arc<dyn WriteStore>>,
    // ...
    signers: Vec<Arc<dyn Signer>>,
    publisher: Option<Arc<dyn Publisher>>,
}

pub fn signer<T>(mut self, signer: Arc<T>) -> Self
where
    T: Signer + 'static,
{
    self.signers.push(signer);
    self
}
```

Add generic and erased `ReplaceableEventMaterializer` registration methods following `signer`/`signers`. The `fava` production dependency list must remain free of `fava-nip02` and `fava-bookmarks`; selected applications/tests may depend on them.

**Build and recovery ordering** (`crates/fava/src/lib.rs:355-400`):

```rust
let event_cache = self.event_cache.ok_or(BuildError::MissingEventCache)?;
let write_store = self.write_store.ok_or(BuildError::MissingWriteStore)?;
let evaluator = self.evaluator.ok_or(BuildError::MissingQueryEvaluator)?;
// validate complete publication provider set
let publication = Publication::new(/* selected providers */)?;
publication.recover()?;
// only then return Fava
```

Validate duplicate materializer claims and missing recovery materializers before constructing a usable `Fava`. Assemble the registry before publication recovery; fail the build rather than silently parking an undecodable accepted edit.

**Public optimistic/query/cache proof** (`crates/fava/tests/explicit_publication.rs:29-76`):

```rust
let mut observation = fava
    .observe(Query::events().kind(Kind::TextNote).cache_only())
    .await
    .expect("query opens");
let accepted = fava.publish(intent).expect("acceptance commits");
assert!(matches!(accepted.current.event, EventValue::Unsigned(_)));
let visible = observation.changed().await.expect("local write appears");
assert_eq!(visible.events.len(), 1);
assert!(visible.events[0].relay_evidence.is_empty());
assert!(cache.event(event_id).expect("cache readable").is_none());
```

`tests/semantic_writes.rs` should copy this public-facade arrangement: first value visible before relay effect, cache remains free of unpublished local materializations, source v2 enters through the real cache/ingest boundary, query swaps generation atomically, same write/receipt remains, inverse is ordinary publication, and raw future kind still works.

**Controlled delayed completion pattern** (`crates/fava/tests/explicit_publication.rs:302-341`):

```rust
struct GatedPublisher {
    outcome: PublishOutcome,
    calls: AtomicU64,
    gate: watch::Sender<bool>,
}

impl Publisher for GatedPublisher {
    fn publish<'a>(/* exact attempt */) -> Pin<Box<dyn Future<Output = PublishOutcome> + Send + 'a>> {
        let mut gate = self.gate.subscribe();
        Box::pin(async move {
            if !*gate.borrow() { let _ = gate.changed().await; }
            self.outcome.clone()
        })
    }
}
```

Use gates/channels, not sleeps, to hold generation-1 materializer/signer/route/publisher/delivery completions, install generation 2, release old completions, and assert unchanged current receipt/query state plus attributable predecessor evidence.

**Integration BUILD target pattern** (`crates/fava/BUILD.bazel:36-60`):

```python
rust_test(
    name = "explicit_publication",
    srcs = ["tests/explicit_publication.rs"],
    crate_name = "explicit_publication",
    aliases = aliases(normal_dev = True, proc_macro_dev = True),
    deps = all_crate_deps(normal = True, normal_dev = True) + [
        ":lib",
        # explicit first-party test dependencies
    ],
    proc_macro_deps = all_crate_deps(proc_macro = True, proc_macro_dev = True),
    compile_data = ["Cargo.toml"],
)
```

Add a peer target for `semantic_writes.rs`; protocol crates belong in dev/test dependencies only.

### Protocol crates and Cargo/Bazel metadata

**Analog:** `crates/fava-nip65/*`

**Pure protocol boundary** (`crates/fava-nip65/src/lib.rs:1-10`, `21-73`):

```rust
//! Pure NIP-65 relay-list vocabulary and parsing.

use fava_state::RelayUrl;
use fava_write::{EventId, EventValue, Kind, PublicKey, Timestamp};
use thiserror::Error;

const MAX_RELAYS: usize = 256;

pub fn from_event(event: &EventValue) -> Result<Self, RelayListError> {
    if event.kind() != Kind::from(10_002_u16) {
        return Err(RelayListError::WrongKind(event.kind().as_u16()));
    }
    // parse protocol tags, preserve exact source facts, enforce bound
}
```

Copy the narrow semantic-owner shape for NIP-02 and public NIP-51 bookmarks: protocol-specific kind/tag/codec logic, explicit bounds, typed errors, deterministic apply, `None` empty state, inverse, and unrelated-field preservation. Unlike NIP-65, implement the public neutral `ReplaceableEventMaterializer` contract and return `ReplaceableEventEdit` values. Do not import runtime, transport, store implementations, standard routers, signer, publisher, delivery, or receipts.

**Unit test placement** (`crates/fava-nip65/src/lib.rs:135-155`):

```rust
#[cfg(test)]
mod tests {
    use fava_write::EventBuilder;

    #[test]
    fn parses_read_write_and_both_markers() {
        // generic event construction, protocol parse, exact assertions
    }
}
```

Each new crate owns golden codec bytes, version refusal, empty-state apply, idempotence, inverse, duplicate normalization, unrelated-field preservation, bounds, and deterministic order tests. The cross-capability corpus remains in the selected facade/canary assembly.

**Cargo manifest pattern** (`crates/fava-nip65/Cargo.toml:1-16`):

```toml
[package]
name = "fava-nip65"
version.workspace = true
edition.workspace = true
rust-version.workspace = true

[dependencies]
fava-state.workspace = true
fava-write.workspace = true
thiserror.workspace = true

[lints]
workspace = true
```

Use only the dependencies actually needed by the pure codec/materializer. Add serde/serde_json only for the protocol-owned durable format if used; no universal owner or concrete provider dependency.

**Bazel target pattern** (`crates/fava-nip65/BUILD.bazel:1-31`):

```python
rust_library(
    name = "lib",
    crate_name = "fava_nip65",
    srcs = ["src/lib.rs"],
    deps = all_crate_deps(normal = True) + [
        "//crates/fava-state:lib",
        "//crates/fava-write:lib",
    ],
    visibility = ["//visibility:public"],
)

rust_test(
    name = "unit_tests",
    srcs = ["src/lib.rs"],
    deps = all_crate_deps(normal = True, normal_dev = True) + [/* same explicit deps */],
)
```

Create equivalent targets for both crates. Cargo and Bazel dependency edges must agree.

**Workspace registration pattern** (`Cargo.toml:3-38`, `45-78`):

```toml
members = [
    # ...
    "crates/fava-nip65",
    # ...
]

[workspace.dependencies]
fava-nip65 = { path = "crates/fava-nip65" }
```

Add `fava-nip02` and `fava-bookmarks` in both lists. Do not add the external N+1 falsifier as a root workspace member.

### Canary and selected capability assembly

**Analog:** `apps/canary/src/automatic_publication.rs`

**Scenario runner/evidence pattern** (`apps/canary/src/automatic_publication.rs:28-74`):

```rust
pub async fn run_automatic_publication_scenario(
    id: &str,
    options: SmokeOptions,
) -> CanaryResult<PathBuf> {
    let count = match id { /* exact supported ids */ };
    let mut artifacts = RunArtifacts::create(&options.runs_directory, id, &options.seed)?;
    artifacts.record("scenario_started", json!({ "scenario": id, "seed": options.seed }))?;
    let result = match id { /* one executor per id */ };
    let completed = result?;
    finish(/* independently witnessed evidence */)
}
```

Add `semantic_writes.rs` and route these exact identifiers through it:

```text
replaceable-edit-first-value
replaceable-edit-rematerialization
replaceable-edit-inverse
protocol-crate-n-plus-one
```

The first three must use public capability + `Fava` + query + receipt calls. Source v2 must enter through the canonical cache/ingest boundary. The N+1 executor must run the separate falsifier workspace, not an in-workspace helper.

**Stable receipt and independent wire evidence** (`apps/canary/src/automatic_publication.rs:137-170`):

```rust
let accepted = fava.publish(intent).map_err(error)?;
// independently wait for observable relay/query evidence
let receipt = wait_terminal(&fava, accepted.receipt_id).await?;
if receipt.receipt_id != accepted.receipt_id || receipt.outcome != ReceiptOutcome::Complete {
    return Err(CanaryError::new("partial route did not expand under one receipt"));
}
```

M7 adds equality of stable `WriteId`/`ReceiptId` across differing `MaterializationId` values and asserts no stale completion changes generation 2.

**Registry and dispatcher pattern** (`apps/canary/src/lib.rs:112-136`, `461-475`; `apps/canary/src/main.rs:87-130`):

The registry record carries an id, milestone, requirement list, and status. The
registry loader decodes `scenarios.json` into those existing canary records, and
the invariant test checks that every enabled record id has a dispatcher branch.
Keep the record and loader names exactly as they already exist in the canary;
this pattern does not propose a new public type.

Update `scenarios.json`, `has_executor`, exports, CLI dispatch, canary manifest, and lock together. Keep protocol crates in this selected application assembly, not the universal `fava` production dependencies.

### External N+1 and dependency-negative falsifiers

**Analogs:** `falsifiers/external-null-cache/*`, `crates/fava-routing/src/chain.rs`

**Separate workspace pattern** (`falsifiers/external-null-cache/Cargo.toml:1-17`):

The analog declares its own workspace, marks its task-local package unpublished,
and keeps `fava` as the sole normal dependency. Existing provider/testkit crates
needed to instantiate integration actors belong under dev-dependencies. The
pattern is the dependency boundary and workspace isolation, not the analog's
package identifier.

`falsifiers/external-protocol-capability` must remain its own workspace. Depend on public neutral contracts and only the providers needed to assemble/run the proof. Its capability name must not appear in universal core manifests or source.

**Public-contract implementation and assembly** (`falsifiers/external-null-cache/src/lib.rs:11-54`, `65-79`):

The analog defines a local cache implementation of the existing `EventCache`
and `QuerySource` contracts, then passes that implementation, a memory write
store, and the standard evaluator through public `FavaBuilder` methods. The
important falsifier is that public contracts suffice for external assembly;
the analog's concrete type name and declaration syntax are not vocabulary for
M7.

The N+1 falsifier should define an unrelated external edit codec/materializer, cover empty/current/inverse, register it through `FavaBuilder`, and verify ordinary query/receipt behavior without crate-private access or core edits.

**Source/manifest dependency-negative pattern** (`crates/fava-routing/src/chain.rs:445-461`):

```rust
#[test]
fn routing_core_does_not_name_concrete_router_crates_or_types() {
    let cargo = include_str!("../Cargo.toml");
    let public_source = include_str!("lib.rs");
    for forbidden in [/* concrete provider crate/type names */] {
        assert!(!cargo.contains(forbidden));
        assert!(!public_source.contains(forbidden));
    }
}
```

Copy this check across universal owners (`fava`, publication, routing, stores, query/state, transport, signer, publisher, delivery) for `fava-nip02`, `fava-bookmarks`, their concrete types, and the external N+1 name. Also assert the external crate is absent from root workspace members.

### Vocabulary registry and checks

**Analog:** `docs/internals/vocabulary.toml`, `tools/check_vocabulary.py`

The registry already contains the three approved M7 terms, each owned by the
write-value crate and each linked to its specification symbol. Those entries
currently have no implementation symbol or crate claim because Plan 01 begins
from the behavior-first RED state.

When the public symbols/crates exist, populate the existing entries' `symbols`/`crates`; do not create aliases or renamed duplicate terms. Likewise move `fava-nip02` and `fava-bookmarks` from `spec_crates`-only to actual `crates` under their existing `ContactList`/`BookmarkList` entries (`vocabulary.toml:108-126`).

**Gate behavior** (`tools/check_vocabulary.py:196-237`):

```python
public_symbols, package_names, source_problems = collect_public_symbols(root)
spec_symbols, spec_crates, spec_problems = collect_spec_vocabulary(root)
for symbol in sorted(public_symbols - registry.symbols):
    problems.append(f"undocumented public architectural symbol: {symbol}")
for package in sorted(package_names - registry.crates):
    problems.append(f"undocumented architectural crate: {package}")
# specified symbols/crates are checked in both directions too
```

Run both required commands for every public/architectural task:

```text
python3 tools/check_vocabulary.py
python3 -m unittest tools.tests.test_vocabulary_check
```

The existing behavior test already proves the approved specification noun is accepted (`tools/tests/test_vocabulary_check.py:35-53`).

## Shared Patterns

### Commit Before Effect or Observation

**Sources:** `fava-publication/src/lib.rs:62-72`, `fava-write-store-redb/src/ops.rs:290-312`, `fava-write-store-memory/src/lib.rs:144-155`

All mutable provider facts commit before receipt/query notifications and before signer/router/publisher effects. Materializer work is candidate computation, not authority: run it outside the store lock/transaction, then compare-and-install under exact current generation/source basis.

### Stable Operation, Changing Materialization

`WriteId` and `ReceiptId` are allocated once at acceptance. `MaterializationId` changes for each immutable materialization. Every downstream effect carries all three stable/current layers plus exact event and provider-specific identity. Only the store decides whether a completion is current.

### Typed, Bounded Refusal

**Sources:** `fava-write/src/lib.rs:141-172`, `fava-write-store/src/lib.rs:178-208`, `fava-nip65/src/lib.rs:113-133`

Use `thiserror` typed enums and validate external/provider text, bytes, tags, fan-out, observations, retained evidence, and active state before mutation. Provider panic/block/failure must remain scoped to its operation and cannot hold durable authority.

### Protocol Purity

Protocol crates may parse/preserve/apply protocol state and return neutral values. They may not allocate receipts, insert cache rows, sign, route, publish, deliver, recover stores, or depend on concrete implementations. Inverses are ordinary `ReplaceableEventEdit` values through the same lifecycle.

### Query-Source Separation

**Sources:** `fava-query/src/lib.rs:272-304`, `fava-query-standard/src/lib.rs:69-112`

Relay-observed signed state remains `SourceKind::EventCache`; unpublished local materializations remain `SourceKind::WriteStore`. Atomic successor install replaces only the write-store contribution. Never copy it into the event cache.

### Test Layering

- Protocol crates: codec golden bytes/version rejection, empty/current/inverse algebra, preservation, determinism, bounds.
- Memory model/store: exact state-machine matrix and atomic refusal/no-mutation.
- Redb: real SIGKILL/reopen boundaries and unsupported-format refusal.
- Publication/facade: controlled delayed completions, canonical source v2, query/receipt observability.
- Canary: four named public scenarios with independently witnessed evidence.
- External workspace: N+1 public-contract assembly and universal-core allowlist.

Compilation is structural evidence only; phase completion requires the behavioral materialization, rematerialization, stale-completion, receipt, query, restart, canary, and external-falsifier results.

## No Complete Analog Found

These files have structural analogs above but no current code implements their M7 invariant. The planner must use `07-RESEARCH.md` and the authoritative specs for behavior rather than extrapolating current M6 behavior.

| File | Role | Data Flow | Missing Precedent |
|---|---|---|---|
| `crates/fava-write/src/replaceable_event_edit.rs` | model | transform/durable bytes | No accepted opaque edit exists; `WritePayload` has only finalized event forms. |
| `crates/fava-write/src/materialization.rs` | provider contract/model | request-response/transform | No public materializer contract or `MaterializationId` exists. |
| `crates/fava-write-store-memory/src/semantic.rs` | state model | CRUD/event-driven | No coordinate-local retained edit composition or generation/source-basis CAS exists. |
| `crates/fava-write-store-redb/src/schema.rs` | durable model | file-I/O | Current redb rows are unversioned whole-receipt JSON; no supported-version envelope exists. |
| `crates/fava-publication/src/materialization.rs` | orchestrator | streaming/event-driven | Publication currently observes receipts/routes only; no qualified-source reconciliation loop exists. |

## Planner Warnings

1. Lock the remaining authority decisions before schema implementation: multiple live edits at one coordinate, earlier-receipt observable state, deterministic materialization timestamp/winner rule, and public-only NIP-51 scope.
2. Do not treat cancellation as stale-completion safety. Preserve the named deliberate break: removing one store-side `MaterializationId` equality guard must make `retired_generation_completions_are_inert` fail through public receipt/query state.
3. Do not use the ordinary merged `AnyLocal` winner as rematerialization source; it contains the operation's own local output.
4. Do not add protocol crates to `fava` production dependencies. They belong to selected assembly/test/canary metadata.
5. Keep every code file below 800 lines and justify any file over 500. Split the currently 467-line `fava-write/src/lib.rs`, 482-line memory store, and growing publication/facade logic by cohesion.
6. Record failing-first and deliberate-break evidence in `docs/issues/0010-m7-semantic-writes-and-capability-composition.md`; do not claim M7 from compilation alone.

## Metadata

**Analog search scope:** `crates/fava-write*`, `crates/fava-publication`, `crates/fava-{signer,routing,publisher,query,query-standard,fava,nip65}`, `apps/canary`, `falsifiers`, `tools`, `docs/internals`

**Repository files indexed:** 199

**Strong analog families used:** current M5/M6 write lifecycle; `fava-nip65`; M6 automatic-publication canary; external null-cache falsifier; vocabulary checker

**Pattern extraction date:** 2026-08-21
