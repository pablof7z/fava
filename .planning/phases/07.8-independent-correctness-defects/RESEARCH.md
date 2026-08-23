# Phase 07.8 — Router engine access and the recursion guard

**Researched:** 2026-08-23
**Tree read:** `87c3688` (`main`, post-07.6 — `crates/fava/src/{relay,live,routes}.rs` no longer exist)
**Domain:** router input acquisition, automatic-routing recursion, engine construction order
**Confidence:** HIGH — every claim below is a file read this session with a line range and a verbatim quote.

---

## Verdict first

**Pablo's model tracks on substance and over-reaches on wording.**

What holds: one engine, one transport stack, one event cache, one registry, one admission window. No narrow purpose-built *read service*. Explicit-or-cached is the entire read-acquisition constraint, and it is sufficient **for reads**. `ARCHITECTURE.md`'s two-service noun split (`local_queries` / `explicit_queries`) is not load-bearing and can be dropped — and dropping it costs nothing against *authority 1*, because `GOALS:878` states the same thing without inventing service nouns:

> "It may request ordinary local reads and explicitly-routed queries through Fava-provided services."
> — `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:878` [VERIFIED: read this session]

What does not hold: **"the only constraint is explicit relays" is false if "the engine" means the `Fava` type.** There are exactly two other doors into the router chain that an explicit-relay constraint on a *query* says nothing about — route preview and the write path. Both are synchronous or asynchronous re-entry into `fava_routing::open`/`preview` reached without issuing a query at all. Details in Q1.

Nearest workable variant, and the recommendation: **full read access to the real engine, handed to the router at `open()`/`preview()` time, through a contract whose acquisition argument makes `QueryAcquisition::Automatic` unrepresentable.** That keeps everything Pablo wants (same machinery, no separate stack, no purpose-built read interface) and closes all three doors by construction rather than by refusal.

One correction to the brief's premise that changes the stakes: **the failure mode of an unguarded automatic router query is no longer unbounded task spawning. It is unbounded synchronous recursion.** 07.6 made `Observer::open` total and synchronous, and the router chain's panic isolation cannot contain a stack overflow. See Q1.

---

## Q1 — Does explicit-only suffice as the recursion guard?

### Every production entry point into the router chain

Complete enumeration, `grep 'fava_routing::open\|fava_routing::preview'` over `crates/` and `apps/`, production paths only:

| # | Call site | Trigger condition | Re-entry shape |
|---|-----------|-------------------|----------------|
| 1 | `crates/fava-observe/src/routes.rs:78` `fava_routing::open(routers, &request)` | read query, `Freshness::Live` **and** `QueryAcquisition::Automatic` | **synchronous** |
| 2 | `crates/fava/src/lib.rs:247` `fava_routing::preview(&self.routers, &request)` | `Fava::preview_routes` with `QueryAcquisition::Automatic` | **synchronous** |
| 3 | `crates/fava-publication/src/run.rs:337` `fava_routing::open(self.routers.as_slice(), &request)` | publication run loop, `WriteRouting::Automatic` | asynchronous |
| 4 | `crates/fava-publication/src/materialization.rs:344` `fava_routing::preview(self.routers.as_slice(), &request)` | write route preview, `WriteRouting::Automatic` | synchronous |

Verbatim guards, so the conditions above are checkable:

- `crates/fava-observe/src/observer.rs:163-166` — `let live = query.freshness() != Freshness::CacheOnly;` / `if live {` / `self.start_engine()?;` / `}` [VERIFIED: crates/fava-observe/src/observer.rs:163-166]
- `crates/fava-observe/src/observer.rs:188-198` — `let binding = if live {` / `match routes::bind(&query, &self.routers) {` … `} else {` / `None` / `};` [VERIFIED: crates/fava-observe/src/observer.rs:188-198]
- `crates/fava-observe/src/routes.rs:67-78` — `QueryAcquisition::Explicit(relays) => {` / `RoutePlan::explicit(relays.iter().cloned(), query.access(), &request.targets())` … `QueryAcquisition::Automatic => {` / `let session = fava_routing::open(routers, &request)` [VERIFIED: crates/fava-observe/src/routes.rs:67-78]
- `crates/fava-publication/src/run.rs:332-337` — `fn open_routes(&self, receipt: &Receipt) -> (Option<Box<dyn RouterSession>>, u64) {` / `let WriteRouting::Automatic = receipt.routing else {` / `return (None, receipt.route_revision);` / `};` / `let request = RouteRequest::Write(receipt.current.event.clone());` / `match fava_routing::open(self.routers.as_slice(), &request) {` [VERIFIED: crates/fava-publication/src/run.rs:332-337]
- `crates/fava-publication/src/materialization.rs:337-345` — `match routing {` / `WriteRouting::Explicit(relays) => RoutePlan::explicit(` … `WriteRouting::Automatic => fava_routing::preview(self.routers.as_slice(), &request)` [VERIFIED: crates/fava-publication/src/materialization.rs:337-345]

### Door-by-door

**Door 1 — read acquisition. Closed by `Explicit` or `CacheOnly`.**
`routes::bind` reaches the chain only on the `Automatic` arm, and `bind` itself is reached only when `live`. So the pair *(acquisition, freshness)* is the whole gate on this door, and either half suffices. `Explicit` alone closes it; `CacheOnly` alone closes it.

**Door 2 — route preview. NOT closed by explicit relays.**
`Fava::preview_routes(&Query)` takes a query but issues no acquisition. A router given the `Fava` type could call it, and a router whose `preview` calls `preview_routes` recurses synchronously into itself with no relay set involved anywhere. `OutboxRouter::preview` happens to be safe — `crates/fava-router-outbox/src/lib.rs:159-165` returns `Ok(self.contribution(request, &BTreeSet::new(), &Shortfalls::default()))` and touches no source [VERIFIED: crates/fava-router-outbox/src/lib.rs:159-165] — but that is the same *voluntary* safety the brief already identified as unacceptable in `from_relays`. A third-party router closes this door only if the door does not exist on its handle.

**Door 3 — the write path. NOT closed by explicit relays.**
`Fava::publish` with default routing produces `WriteRouting::Automatic`, which reaches `fava_routing::open` at `run.rs:337` from inside the publication run loop. A constraint stated over `QueryAcquisition` has no jurisdiction over `WriteRouting`; they are separate enums. `Fava::to(relays)` is the write-side explicit narrowing, and nothing ties the two. This recursion is asynchronous (the publication loop), so it degrades into unbounded write lifecycles rather than a stack overflow — slower, and harder to attribute.

**Door 4 — write route preview.** Same as door 2, via `materialization.rs:344`.

**Not doors (checked, negative claims):**

- *Derived queries in publication.* The semantic materializer opens `self.event_source` and `self.store` directly, never the engine: `crates/fava-publication/src/materialization.rs:269-274` — `let query = exact_query(edit, author);` / `let cache = self.event_source.open(&query)` / `let writes = match self.store.open(&query)`, and `exact_query` ends `.cache_only()` at `:404-410` [VERIFIED: crates/fava-publication/src/materialization.rs:269-274, 404-410]. No routing.
- *The route-revision loop.* `routes::follow` consumes `session.next_change()`, builds a `RoutePlan`, and calls `registry.assign` (`crates/fava-observe/src/routes.rs:137-161`). It never calls a router. Demand changes wake the reconciliation engine, which opens relay work, not routers.
- *Ingest.* `crates/fava-observe/src/ingest.rs:41-105` admits frames into the event cache. A cache change wakes the projection loop, which re-evaluates the query. No routing.
- *The outbox's own knowledge feedback.* `KnownLists::remember` bumps a watch revision only when the candidate supersedes (`crates/fava-router-outbox/src/lib.rs:126-141`), and `OutboxSession::contribution` is a pure function of `lists` + `request`. The loop is convergent and bounded by the missing-author set. Not recursion.
- *`Engine::start` re-entrancy.* `start_engine` runs at `observer.rs:164-166`, strictly **before** `routes::bind` at `:188`, so the `OnceLock::get_or_init` at `observer.rs:339-341` has already returned by the time any router runs. No `OnceLock` re-entrancy deadlock. This is order-dependent and fragile: moving `start_engine` after `bind` would introduce a re-entrant `get_or_init` — that is a deadlock or panic depending on std version.
- *Lock inversion.* `KnownLists::values()` clones under a short mutex and drops it (`lib.rs:117-123`); `remember` explicitly `drop(values)` before `send_replace` (`lib.rs:136-139`); `Registry::install` is called *after* `bind` (`observer.rs:206`). No lock crosses the router boundary. The brief's "no deadlock" finding still holds.

### The failure mode changed under 07.6

The brief says the failure of an unguarded automatic query is unbounded task spawning, because `tokio::spawn` breaks the stack unconditionally. That was true when `impl QuerySource for Fava` was the only door. It is no longer the general case:

- `Fava::observe` is `crates/fava/src/lib.rs:112` `pub async fn observe(&self, query: Query) -> Result<Observation, ObserveError>` whose body is `self.observer.open(query)` — documented `"opening is total and synchronous; the async signature is the public door and never awaits a provider"` [VERIFIED: crates/fava/src/lib.rs:105-113].
- So `Observer::open → routes::bind → fava_routing::open → Router::open → Observer::open` is a **synchronous** cycle with no yield point.

And the chain's isolation cannot contain it:

```rust
fn isolate<T>(
    action: &str,
    call: impl FnOnce() -> Result<T, RouterError>,
) -> Result<T, RouterError> {
    std::panic::catch_unwind(AssertUnwindSafe(call)).unwrap_or_else(|_| {
```
— `crates/fava-routing/src/chain.rs:141-146` [VERIFIED: crates/fava-routing/src/chain.rs:141-146]

`catch_unwind` catches unwinding panics. A stack overflow is not an unwinding panic; it aborts the process. Gate 4 (failure isolation) therefore does **not** hold for this class. That is the concrete reason "unrepresentable" is not a style preference here: the difference between the two enforcement options is a typed `RouterError::Refused` versus process abort.

Non-termination is real, not theoretical: the recursive query's targets are `RouteTarget::Author(missing…)` (`crates/fava-routing/src/lib.rs:33-53`), so each level re-derives the same missing set at `crates/fava-router-outbox/src/lib.rs:176-181` and opens the same query again.

### Answer

`QueryAcquisition::Explicit` **is** sufficient for door 1 — the only door reachable by issuing a read query. It is **not** sufficient for doors 2, 3, 4, which are not queries. Explicit-only is a complete guard over *reads*; it is not a complete guard over *the engine*. The guard must therefore be located on the handle's shape, not only on the query's acquisition field.

---

## Q2 — Is the cache read expressible without triggering automatic routing?

Yes, and `Freshness::CacheOnly` is exactly that expression. No exemption clause is needed — the guard never fires because the code path never reaches routing.

`crates/fava-observe/src/observer.rs:163-166, 188-198` (quoted above) gate *both* `start_engine()` and `routes::bind` on `live`. With `CacheOnly`:

- no transport is required (`start_engine` is skipped, so the three `ok_or_else` refusals at `observer.rs:322-336` never run);
- no router is consulted;
- both local sources still open (`observer.rs:168-186`), so the merged event-cache + write-store view is delivered;
- `evaluate_initial` runs and `revision = QueryRevision(1)` carries real content (`observer.rs:246-255`).

Existing evidence, already green:

```rust
async fn a_cache_only_query_opens_no_relay_work() {
    let assembly = assemble();
    let observation = assembly
        .observer
        .open(Query::events().cache_only())
        .expect("the cache-only query opens");
    settle().await;
    assert!(assembly.planner.inputs().is_empty());
    assert!(observation.current().evidence.relays.is_empty());
```
— `crates/fava-observe/tests/open_sequence.rs:61-74` [VERIFIED: crates/fava-observe/tests/open_sequence.rs:61-74]

**Gap:** that test asserts no *relay* work. It does not assert no *router* work. The load-bearing claim for this phase is "`CacheOnly` never calls `Router::open`", and no test states it. Falsifier F2 below.

**Right expression for the outbox's warm-cache read:** `Query::events().kind(10002).authors(known_or_wanted).cache_only()`. Note that `cache_only()` leaves `acquisition` at its default `Automatic` (`crates/fava-query/src/lib.rs:59-64, 166-171`), and that is harmless *today* because `live` gates routing before acquisition is ever inspected. If the guard is implemented as "acquisition must be Explicit", a cache read would be wrongly rejected. The guard must be **`Explicit` OR `CacheOnly`**, expressed as two constructors, not one predicate.

**Second-order note the plan must not skip:** the outbox does not read the cache at all today. `grep 'cache_only' crates/fava-router-*` returns empty; its only relay-list inputs are `OutboxRouter::remember` (a public out-of-contract method, `lib.rs:94-97`) and `self.lists.ingest(&initial, …)` on the discovery query's initial snapshot (`lib.rs:196`). Giving it engine access does not by itself fix `router-source-fabricates-empty-initial`; the router must additionally issue the cache read. See Q5.

---

## Q3 — How is the constraint enforced?

Three candidates, plus a fourth that emerged from the dependency-direction gate.

### (a) Runtime refusal inside `impl QuerySource for Fava`

Reject `QueryAcquisition::Automatic` at `crates/fava/src/query_source.rs:15`.

- **Closes:** door 1 only.
- **Cost:** ~5 lines. No vocabulary change.
- **Room to get it wrong:** maximal. A third-party router discovers the constraint at runtime, on the unhappy path, as a `QuerySourceError::Refused` string. And the handle is still `Fava`, so doors 2–4 stay wide open. Contradicts `AGENTS.md:72` — "Make invalid use unrepresentable or refuse it before opening work" — only in its weaker half.
- **Verdict:** insufficient. It is what the codebase would get by accident; it should not be what it gets by choice.

### (b) A router-facing handle contract whose acquisition argument is a required relay set

A trait declared in `fava-routing` (the contract crate every router already depends on) and implemented by the engine:

```rust
pub trait RouterQueries: Send + Sync {
    fn cached(&self, query: &Query) -> Result<Box<dyn RouterObservation>, RouterQueryError>;
    fn from_relays(
        &self,
        query: &Query,
        relays: &BTreeSet<RelayUrl>,
    ) -> Result<Box<dyn RouterObservation>, RouterQueryError>;
}
```

The engine applies `query.cache_only()` / `query.from_relays(relays)` itself and ignores whatever acquisition the caller's `Query` carried. `Automatic + Live` is then **unrepresentable**: there is no method that produces it, and there is no third method. No new *query* type is required — the relay set being a required positional argument is the whole mechanism, which is also literally the shape `ARCHITECTURE.md:1312` sketches (`explicit_queries.open(query, exact_relays)`).

- **Closes:** doors 1–4. Doors 2–4 close because the handle has no preview method and no publish method — they are not refused, they are absent.
- **Cost:** one new trait + one new return nominal + one new error nominal in `fava-routing`. `AGENTS.md` classifies "a new … provider contract" and "cross-crate nominal type" as a **vocabulary change**, and "a feature change cannot approve its own new vocabulary." So this needs a separate Pablo-approved architecture change before 07.8 can land it. That is a real sequencing cost and the main argument against.
- **Room to get it wrong:** near zero. The only mistake left is a router opening an unbounded *number* of explicit queries — a boundedness problem, not a recursion problem (see "Residual risk").

### (c) `Query` type-state

`Query<Explicit>` / `Query<Automatic>`.

- **Cost:** `Query` is a spec'd public symbol (`docs/internals/vocabulary.toml:301`, `fava_query::QuerySourcePolicy`; `Query` itself is exported from `crates/fava/src/lib.rs:22-24`). It appears in `RouteRequest::Read(Query)`, `RelayDemand`, `QueryEvaluator::evaluate`, `QuerySource::open(&Query)`, every application call, and every test. `QuerySource` is object-safe today (`Arc<dyn QuerySource>` at `crates/fava-observe/src/observer.rs:31-32`); a generic `Query<S>` parameter breaks that unless the trait is also split.
- **Verdict:** reject. Enormous blast radius across the entire public surface for one predicate that (b) enforces with two method names.

### (d) The dependency-direction constraint that rules out the literal reading

`fava-router-outbox` currently depends on contracts only:

```toml
fava-nip65.workspace = true
fava-query.workspace = true
fava-routing.workspace = true
fava-state.workspace = true
fava-write.workspace = true
```
— `crates/fava-router-outbox/Cargo.toml:7-13` [VERIFIED: crates/fava-router-outbox/Cargo.toml:7-13]

Handing it the `Fava` type adds a dependency on the facade crate; handing it `Observer` adds a dependency on `fava-observe`. Both invert gate 2 — `AGENTS.md:47`: "**Dependency direction:** domain values -> neutral contracts -> providers; universal owners use contracts, not standard implementations." A provider depending on the universal owner is the inversion this gate exists to catch, and the `public-surface` audit records that this gate currently *holds* workspace-wide ("no universal owner or facade depends on any … implementation crate; no contract crate depends on an implementation; no dependency cycles").

**So "routers get full access to the engine" cannot be implemented by handing over a concrete engine type at all.** It has to be a contract the engine implements. Which is (b). The narrowing is not a design preference — it is forced by the gate that is currently green.

### Recommendation

**(b), with (d) as the reason it is the only shape available.** Escalate the vocabulary change as its own architecture slice before 07.8 plans against it.

---

## Q4 — Construction order

The circularity is real today: `OutboxRouter::new(name, indexers, queries: Arc<dyn QuerySource>)` at `crates/fava-router-outbox/src/lib.rs:71-75` needs a source before `FavaBuilder::router` (`crates/fava/src/builder.rs:100-107`) can take the router, and `build(self)` (`builder.rs:178`) consumes the builder.

A fact that decides three of the four options, and which the brief does not mention:

**A strong engine reference held by a `Router` is an unbreakable `Arc` cycle.** `Observer` holds `routers: Vec<Arc<dyn Router>>` (`crates/fava-observe/src/observer.rs:35`) and `Fava` holds `routers: Vec<Arc<dyn Router>>` (`crates/fava/src/lib.rs:85`). If the router stores a clone of either, the graph is engine → `Arc<dyn Router>` → engine, refcount never reaches zero, and the whole engine — registry, slots, transport leases, tasks — leaks for the process lifetime. This is not hypothetical: it is the direct consequence of every construction-time injection option.

| Option | Partially-initialised engine visible to application code? | Cycle? | Other cost |
|--------|-----------------------------------------------------------|--------|------------|
| `Arc::new_cyclic` | **Yes.** Inside the closure `Weak::upgrade()` returns `None`; a router that touches the engine during its own construction observes an engine that exists and cannot be used. | No (Weak) | Forces `Arc<Fava>` as the public built value; `build()` returns `Fava` by value today (`builder.rs:178`). Public-surface change. Every router call site pays an `upgrade().ok_or(...)` refusal that can fire during shutdown. |
| Late-bound setter (`router.attach(engine)`) | **Yes**, in the router: between `new()` and `attach()` the router is a public value whose `open()` cannot acquire, and nothing prevents `FavaBuilder::router()` accepting it in that state. | Yes, if strong | Needs interior mutability (`OnceLock`) on a `Send + Sync` router; a second `attach` is either a silent overwrite or a runtime refusal. |
| Lazy handle (`Arc<OnceLock<Handle>>` given at construction, filled by `build()`) | **Yes**, same window, relocated into a cell. The application holds the `Arc<OutboxRouter>` and can call `Router::open` on it before `build()`. | Yes, if strong | Adds a nominal type whose only purpose is the window it fails to eliminate. |
| **Handle passed at `open()` / `preview()` time** | **No.** The router has no engine-shaped field, so there is nothing to be partially initialised. The handle exists only inside a call the engine itself initiated, which is by construction after `build()`. | Bounded — only a `RouterSession` may retain it, and the session is dropped when `Registry::withdraw` releases the observation's tasks (`crates/fava-observe/src/registry.rs:150-159`). | Changes the spec-named `Router` / `RouterSession` trait shape (`docs/spec/ARCHITECTURE.md:1204-1216`, `vocabulary.toml:678 spec_symbols = ["Router", "RouterSession"]`). That is an approved-vocabulary edit. |

**Answer: passing the engine at `open()` time is the only one that does not introduce a partially-initialised engine visible to application code**, and it is also the only one that bounds the reference cycle without `Weak`.

It composes with Q3(b): `Router::open(request, upstream, queries: &dyn RouterQueries)` and `Router::preview(request, upstream, cached: &dyn CachedQueries)`. Preview gets the strictly weaker handle, and that split is forced independently by preview's own requirement —

> "Preview never creates publication or relay-acquisition ownership merely because an application asks where an operation would currently go."
> — `docs/spec/ARCHITECTURE.md:1290` [VERIFIED: read this session]

— not by the recursion guard. So the two capability levels are not `ARCHITECTURE.md`'s two injected services smuggled back in; they are one door observed from two lifecycles.

A retained handle must be `Send + Sync + 'static` (the `RouterSession` is moved into `observe.routes`, a `spawn_cancellable` future — `crates/fava-observe/src/routes.rs:142-147`), so the concrete type is `Arc`-backed and cheap to clone.

---

## Q5 — What this deletes

| Item | Disposition |
|------|-------------|
| Canary's second engine | **Deleted.** |
| `impl QuerySource for Fava` | **Deleted.** |
| `router-source-fabricates-empty-initial` | **Fixed, but only with a router change too.** |
| `outbox-does-not-coalesce-discovery` | **Mostly fixed by the engine; the finding needs restating, not just closing.** |
| `source-role-impersonation` | **This instance deleted; the general contract question survives and needs a decision in this phase.** |
| `OutboxRouter::remember` | **Should be deleted** — separate call. |

### The canary's second engine

`apps/canary/src/automatic_support.rs:68-79` builds a whole second `Fava` with its own `MemoryWriteStore` and its own `WebSocketTransport::default()`; `apps/canary/src/automatic_publication.rs:94-96` is its only consumer:

```rust
let queries: Arc<dyn QuerySource> = Arc::new(query_fava(Arc::clone(&cache))?);
let outbox = Arc::new(OutboxRouter::new("nip65", [urls[4].clone()], queries).map_err(error)?);
```
[VERIFIED: apps/canary/src/automatic_publication.rs:94-96; apps/canary/src/automatic_support.rs:68-79]

Under open()-time passing, `OutboxRouter::new` loses its third parameter entirely and `query_fava` has no callers. Both go. This is exactly WRITE-014's acceptance — "through explicit query machinery **and no separate transport stack**" (`GOALS:882`) — satisfied structurally rather than by inspection. `router-acquisition-starts-from-fabricated-empty-state` closes.

### `impl QuerySource for Fava`

Whole-workspace consumer count: **one**, `automatic_publication.rs:95` above. Test count: **zero** — `crates/fava/tests/source_contract.rs:1` is the "Shared query-source behavior corpus for the two M1 memory providers" and exercises `MemoryEventCache` and `MemoryWriteStore`, not `Fava`. Deleting the impl breaks nothing that survives the canary change.

Deleting it also removes the last bare `tokio::spawn` in the facade (`crates/fava/src/query_source.rs:30`), which 07.5 created `fava-runtime` to eliminate and 07.6 did not reach.

### `router-source-fabricates-empty-initial`

Two independent causes, and the engine change fixes only one:

1. `crates/fava/src/query_source.rs:21` — `let initial = SourceSnapshot::empty(SourceKind::EventCache);` returned before any observation exists, with the real state pushed later from the spawned task at `:39`. **Fixed by deletion.** The replacement handle returns a real snapshot because `Observer::open` is synchronous and `Observation::current()` (`crates/fava-observe/src/observation.rs:46-49`) is readable immediately.
2. The outbox never reads the cache. `crates/fava-router-outbox/src/lib.rs:176-181` computes `missing` from `self.lists` only, and `self.lists` is populated exclusively by `remember` and by the discovery query's own results. **Not fixed by the engine change.** The plan must add the warm-cache read (Q2) or the finding survives with a new cause.

Both must be in the same slice or the finding is closed on a false premise.

### `outbox-does-not-coalesce-discovery`

The requirement text is *identical* needs, not author-granular needs:

> "The router shares/coalesces identical discovery needs across queries and writes and releases acquisition when nothing needs it."
> — `GOALS:1161` (ROUTER-001) [VERIFIED: read this session]

`ARCHITECTURE.md:1369`'s author-granular phrasing is permissive ("**may** share one discovery observation"). See Q6 — the engine already delivers both, at the wire layer. What survives is that the *test oracle* in the audit (`counting_source.open_count() == 1`) becomes the wrong instrument: under full engine access there legitimately are two observations and one REQ. The finding must be re-specified against indexer wire frames.

### `source-role-impersonation`

Half of this finding is already stale on `87c3688`. `SourceKind` is no longer a closed two-variant enum:

```rust
pub enum SourceKind {
    /// Signed relay-observed cache state.
    EventCache,
    /// Current accepted local materializations.
    WriteStore,
    /// Verified live occurrences admitted from one relay session, retained by
    /// no store.
    LiveRelay {
        /// Relay session that served them.
        session: RelaySessionKey,
    },
}
```
— `crates/fava-query/src/evidence.rs:23-35` [VERIFIED: crates/fava-query/src/evidence.rs:23-35]

The live half is the nested-Fava instance specifically: `query_source.rs:21` and `:122` stamp `kind: SourceKind::EventCache` on a snapshot whose events include `SourceEvent::Local(LocalWriteEvent…)` produced at `:139-142`. Deleting the impl deletes that instance.

**What survives is the contract question, and this phase must answer it:** what provenance does a merged engine view carry when handed to a router? There is no correct `SourceKind` for "the merged result of two sources" — any choice impersonates one of them. The clean answer is to **not return a `SourceSnapshot` at all.** If the router handle returns an `Observation`, the router receives a `QuerySnapshot` whose `evidence.sources` retains per-role evidence (`crates/fava-observe/src/sources.rs:172-185` decorates, `crates/fava-query/src/evidence.rs:385` `fn source(&self, kind: &SourceKind)` reads it back), and nothing has to invent a role. This is a design fork the planner must decide explicitly:

- `Box<dyn RouterObservation>` wrapping `Observation` — role-preserving, real initial snapshot, refcount released on drop. **Recommended.**
- `OpenedQuerySource` — source-shaped, forces a fabricated `SourceKind`, re-creates the impersonation under a new name.

The outbox consumes only `.events` today (`lib.rs:143-155` `KnownLists::ingest`), so the migration from `SourceSnapshot`/`SourceEvent` to `QuerySnapshot`/`EventRecord` is mechanical.

### `OutboxRouter::remember`

Public, out-of-contract, and the only reason the canary can hand-feed relay lists at `automatic_publication.rs:97-107`. Once the cache read exists it has no legitimate caller. Removing it is a public-API deletion in an approved-vocabulary crate — flag it, do not fold it in silently.

---

## Q6 — Coalescing

**Where it lives: the engine, not the router. It is already built.** The unit is not "the author" or "the query" — it is the `RelayDemand` filter at one `RelaySessionKey`, which for the outbox's discovery query is `{kind: 10002, authors: […]}` at the configured indexer. That is author-granular in practice, because authors is the only axis that varies.

Four mechanisms, all present on `87c3688`:

**1. The registry keeps every demand distinct and hands all of them to the planner.**
> "It never merges two observations' demand: two equivalent queries are two `DemandId`s with their own bounds, route origin, and evidence (`GOALS:296`, QUERY-002 — sharing is permitted, erasing distinct evidence is not). Merging is the planner's decision, made later, with every logical demand still visible to it."
> — `crates/fava-observe/src/registry.rs:5-9` [VERIFIED: crates/fava-observe/src/registry.rs:5-9]

`Registry::desired()` (`registry.rs:163-180`) returns `BTreeMap<RelaySessionKey, Vec<RelayDemand>>` — the aggregate the planner sees.

**2. The admission window batches unsent demand into one cohort.**
`pub(crate) const ADMISSION_WINDOW: Duration = Duration::from_millis(10);` — `crates/fava-observe/src/admission.rs:28`, "anchored at the first uncovered demand and never slides" [VERIFIED: crates/fava-observe/src/admission.rs:22-28].

**3. The grouping planner merges on the sole differing axis and folds duplicates.**
`crates/fava-subscriptions-standard/src/grouping.rs:22-47` — "bucket byte-identical filters together … two demands asking the relay for exactly the same bytes are one request"; "merge to a **fixed point**"; "fold byte-identical survivors". `merge_authors` at `:273-278` unions the authors axis. So `{10002, [alice]}` + `{10002, [bob]}` in one cohort become one REQ `{10002, [alice, bob]}`, and `{10002,[alice]}` twice become one REQ.

**4. Late joiners attach to a running request by containment, refcounted.**
`Engine::attach` (`crates/fava-observe/src/engine.rs:215-260`) finds an installed subscription whose filter `admission::covers` the joiner's, inserts the joiner's `DemandId` into `entry.serves`, and does no wire work —
> "This is a refcount edit, not wire work: the subscription keeps its exact id and its exact filters, and no plan is computed."
> — `crates/fava-observe/src/engine.rs:209-213`

and release is the same refcount inverted:
> "The refcount that decides withdrawal is the attribution fan-out on each live request: it closes when, and only when, the last demand it serves goes away."
> — `crates/fava-observe/src/engine.rs:15-17`

That is ROUTER-001's "releases acquisition when nothing needs it", delivered by the engine, for free, the moment the router's discovery query runs on the real registry.

### What the router still does not get

- **Coalescing is order- and window-dependent.** `covers` (`admission.rs:36-53`) is containment, so an incumbent `{[alice, bob]}` absorbs a later `{[alice]}`, but an incumbent `{[alice]}` does **not** absorb a later `{[alice, bob]}` — that opens a second REQ beside it. Outside the 10 ms window, two route sessions for disjoint authors produce two REQs. This is a deliberate design choice ("Rewriting a running subscription costs the relay a full re-serve … It is never taken", `admission.rs:9-12`), not a defect, but the acceptance for ROUTER-001 must be written against *identical* needs, which always coalesce, and not against arbitrary author overlap, which sometimes will not.
- **Duplicate observation cost.** Two route sessions needing alice still install two `ObservationId`s, two projection tasks, two evaluator runs, two `KnownLists::ingest` passes. Cheap and local; no relay sees it. If Pablo wants literal single-observation sharing per `ARCHITECTURE.md:1369`, that is router-owned in-flight-need bookkeeping and is *additional* work on top of everything above. Recommendation: do not build it. The engine already meets the requirement text.
- **Router demand is indistinguishable from application demand in diagnostics.** Router observations become ordinary `ObservationId`s in `Registry::open_observations()` and in `QueryDiagnostic`. Arguably correct against RELAY-001 ("every contacted relay MUST be explainable by current demand"), but OPS consumers will see indexer traffic attributed to no application query. Small, and worth a `RouteOrigin`-carried attribution note in the plan.

---

## Implementation shape

Assumes the Q3(b)/Q4 vocabulary slice is approved first.

**1. New contract in `fava-routing`** (`crates/fava-routing/src/queries.rs`)
- `trait CachedQueries: Send + Sync` — `fn cached(&self, query: &Query) -> Result<Box<dyn RouterObservation>, RouterQueryError>`.
- `trait RouterQueries: CachedQueries` — `fn from_relays(&self, query: &Query, relays: &BTreeSet<RelayUrl>) -> Result<Box<dyn RouterObservation>, RouterQueryError>`.
- `trait RouterObservation: Send` — `fn current(&self) -> Arc<QuerySnapshot>`, `fn changed(&mut self) -> …`, `fn close(&mut self)`. Object-safe, `'static`.
- Implementations must apply `query.cache_only()` / `query.from_relays(relays)` themselves and discard the caller's acquisition field. No method produces `Automatic + Live`.

**2. `Router` trait shape** (`crates/fava-routing/src/lib.rs:317-342`)
- `fn preview(&self, request: &RouteRequest, upstream: &RoutePlan, cached: &dyn CachedQueries) -> …`
- `fn open(&self, request: RouteRequest, upstream: watch::Receiver<Arc<RoutePlan>>, queries: &dyn RouterQueries) -> …`
- `chain::open` / `chain::preview` gain the handle parameter and forward it inside `isolate`.

**3. Engine side**
- `impl CachedQueries + RouterQueries for Observer` in `fava-observe`, wrapping `Observer::open`.
- `crates/fava-observe/src/routes.rs:78` passes `self` (an `Observer` clone or a thin `&dyn` view) into `fava_routing::open`. `crates/fava/src/lib.rs:247` and the two `fava-publication` sites pass a cache-only view.
- **Do not** give the engine handle to the router at construction. Q4.

**4. Delete**
- `crates/fava/src/query_source.rs` entirely, and its `mod query_source;` at `crates/fava/src/lib.rs:5`.
- `apps/canary/src/automatic_support.rs::query_fava` and `automatic_publication.rs:94-96`.
- `OutboxRouter::new`'s `queries` parameter and the `queries` field (`crates/fava-router-outbox/src/lib.rs:55, 74`).
- `OutboxRouter::remember` (separate approval).

**5. Outbox changes**
- In `open`: first `queries.cached(&Query::events().kind(10002).authors(all_requested))` and `ingest` its `current()`, *then* compute `missing`, *then* `queries.from_relays(&discovery_query, &self.indexers)` only if `missing` is non-empty.
- In `preview`: the same cache read via `cached`, no acquisition.
- `KnownLists::ingest` migrates from `SourceSnapshot`/`SourceEvent` to `QuerySnapshot`/`EventRecord`.

**6. Order the phase must respect**
- `start_engine()` must stay before `routes::bind` in `Observer::open` (`:164` before `:188`) or `OnceLock::get_or_init` becomes re-entrant. Add a comment; the current ordering is load-bearing and unremarked.

---

## Falsifiers

Each must fail before the change and pass after.

- **F1 — automatic is unrepresentable.** A `compile_fail` doctest on `RouterQueries` proving no method call can produce a `Query` with `QueryAcquisition::Automatic` and `Freshness::Live`. Home: `crates/fava-routing/src/queries.rs`.
- **F2 — a cache-only query consults no router.** `crates/fava-observe/tests/open_sequence.rs`: assemble with a counting router; `observer.open(Query::events().cache_only())`; assert `router.open_count() == 0` **and** `router.preview_count() == 0`. Closes the untested half of `a_cache_only_query_opens_no_relay_work`.
- **F3 — one transport stack.** `crates/fava/tests/automatic_routes.rs`: one `Fava` with one transport, an `OutboxRouter` and an unknown author; assert the indexer REQ appears on **that** transport's frame log. Deliberate break: reintroduce a second transport in the assembly and assert the test goes red.
- **F4 — warm cache routes immediately with no indexer traffic.** Commit alice's kind-10002 to the event cache before `build()`; `preview_routes` reports alice's write relay covered; `observe` opens; `transport.open_count(&indexer) == 0`. This is the audit's proposed falsifier for `router-source-fabricates-empty-initial`, now runnable against one engine.
- **F5 — identical discovery needs are one REQ.** Two route sessions (one read, one write p-tagging) needing the same unknown author against the same indexer; assert exactly one `REQ` frame reaches the indexer and its filter is `{kinds:[10002], authors:[alice]}`. Replaces the audit's `open_count() == 1` oracle, which is wrong under this model.
- **F6 — releasing the last session closes the discovery subscription.** Close both sessions from F5; assert one `CLOSE` frame and that it is emitted only after the second close.
- **F7 — the write door is absent.** `compile_fail`: a `Router` implementation attempting `queries.publish(...)` or `queries.preview_routes(...)` does not compile. This is the falsifier for the two doors that explicit-only does not close.
- **F8 — a router refusal is still isolated.** Existing `crates/fava-routing/tests/failure_isolation.rs` must stay green with the new parameter; add a case where the router's `from_relays` returns `Err` and assert the chain returns `Ok` with an attributed shortfall (interacts with `router-open-failure-kills-whole-query`, same phase).

**What would refute Pablo's model:** F1 passing while F7 fails would mean the handle is still too wide. F3 failing would mean the single-stack claim is unimplementable at open() time. F5 failing after F3 passes would mean the engine's admission/grouping does not in fact coalesce router demand and the router needs its own registry after all — the one outcome that would justify reopening `outbox-does-not-coalesce-discovery` as router work.

---

## Residual risk the guard does not cover

`Explicit` bounds *recursion*, not *fan-out*. A router may open one explicit query per author per route session, unboundedly. The chain bounds contributions (`MAX_DESTINATIONS = 256`, `MAX_TARGETS = 256`, `crates/fava-routing/src/chain.rs:13-20`) but nothing bounds the number of router-issued observations. Gate 5 (boundedness) requires "explicit bounds or typed refusal/shortfall". Recommend a per-router installed-observation cap enforced by the engine's handle implementation, with a typed refusal. This is new scope; name it rather than let it ride along.

Related, out of this brief's scope but adjacent and unresolved: the LEDGER's open cross-owner question on WRITE-027 (total router refusal making an automatic write terminal) is scheduled for this phase or 07.9 and touches the same `run.rs:337` call site.

---

## Assumptions Log

| # | Claim | Section | Risk if wrong |
|---|-------|---------|---------------|
| A1 | A Rust stack overflow aborts rather than unwinding, so `catch_unwind` at `chain.rs:141` cannot contain the recursion. | Q1 | If a future toolchain made this recoverable, option (a) would be merely bad rather than unsafe. Does not change the recommendation. |
| A2 | Introducing `RouterQueries` / `RouterObservation` / `RouterQueryError` requires a separate Pablo-approved vocabulary change under `AGENTS.md`. Read from the rule text; not confirmed against a precedent in `docs/internals/vocabulary.toml`'s change history. | Q3 | If it can ride inside 07.8, the sequencing cost disappears and (b) gets cheaper. |
| A3 | No application currently depends on `impl QuerySource for Fava` outside this repository. Verified inside the repo (one consumer, zero tests); the crate is unpublished and has no Git remote (`AGENTS.md:38`), so external breakage is assumed impossible. | Q5 | None, given no remote. |
| A4 | `Query::cache_only()` leaving `acquisition` at `Automatic` is safe *only because* `Observer::open` gates on `freshness` first. This is implementation, not contract — no test states it. | Q2 | F2 converts this from assumption to evidence; until then a reordering in `Observer::open` silently reopens door 1. |

---

## Sources

All primary, all read this session at `87c3688`.

**Source**
`crates/fava/src/lib.rs`, `crates/fava/src/builder.rs`, `crates/fava/src/query_source.rs`,
`crates/fava-observe/src/{observer,routes,sources,registry,engine,admission,ingest,observation,plan}.rs`,
`crates/fava-query/src/{lib,evidence}.rs`,
`crates/fava-routing/src/{lib,chain}.rs`,
`crates/fava-router-outbox/src/lib.rs`, `crates/fava-router-outbox/Cargo.toml`, `crates/fava-router-outbox/tests/outbox.rs`,
`crates/fava-publication/src/{run,materialization}.rs`,
`crates/fava-subscriptions-standard/src/grouping.rs`,
`crates/fava-observe/tests/open_sequence.rs`, `crates/fava/tests/source_contract.rs`,
`apps/canary/src/{automatic_support,automatic_publication}.rs`

**Authority**
`docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:876-882` (WRITE-014), `:1150-1163` (ROUTER-001)
`docs/spec/ARCHITECTURE.md:1204-1216` (Router contract), `:1290` (preview), `:1296-1327` (router input queries), `:1341-1371` (`fava-router-outbox`)
`AGENTS.md:44-49` (gates), `:72` (unrepresentable), `:56-63` (vocabulary)
`docs/internals/vocabulary.toml:320-338, 393-404, 654-678`

**Prior findings re-checked, not re-derived**
`.planning/audit/2026-08-23/LEDGER.md`, `routing.md:342-420, 625-680, 1030-1060`, `observe-facade.md:211-230, 303, 585-587, 727`, `public-surface.md:152`

---

## Metadata

**Confidence breakdown**
- Re-entry path enumeration: HIGH — exhaustive grep plus read of all four call sites and their guards.
- `CacheOnly` behaviour: HIGH — guard read verbatim, existing green test cited, gap in that test named.
- Enforcement comparison: HIGH on mechanism, MEDIUM on cost — A2 is unconfirmed.
- Construction order: HIGH — the `Arc` cycle is read off the two `routers:` fields.
- Coalescing: HIGH — four mechanisms read in source with their own doc comments.

**Research date:** 2026-08-23
**Valid until:** invalidated by any change to `Observer::open`, the `Router` trait, or `fava_routing::chain`.
