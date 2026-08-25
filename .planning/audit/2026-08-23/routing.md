# Routing audit

Area slug: `routing`
Mode: read-only. No production source, test, or spec file was modified.

## Scope checked

Implementation (read in full):

- `crates/fava-routing/src/lib.rs` (381 lines)
- `crates/fava-routing/src/chain.rs` (463 lines)
- `crates/fava-router-outbox/src/lib.rs` (314 lines), `tests/outbox.rs`
- `crates/fava-router-hints/src/lib.rs` (154 lines), `tests/hints.rs`
- `crates/fava-router-app-relays/src/lib.rs` (120 lines)
- `crates/fava-router-fallback-relays/src/lib.rs` (178 lines)
- `crates/fava-router-testkit/src/lib.rs` (91 lines)
- `crates/fava-nip65/src/lib.rs` (156 lines)
- `crates/fava/src/routes.rs` (174 lines) — facade edge
- Supporting reads for boundary tracing: `crates/fava/src/lib.rs`, `crates/fava/src/live.rs`,
  `crates/fava/src/query_source.rs`, `crates/fava-observe/src/lib.rs` + `Cargo.toml`,
  `crates/fava-diagnostics/src/lib.rs`, `crates/fava-query/src/lib.rs`,
  `crates/fava-query/src/selection.rs`, `crates/fava-publication/src/run.rs` (routing call sites only),
  `crates/fava/tests/automatic_routes.rs`, `apps/canary/src/automatic_publication.rs`,
  `apps/canary/src/automatic_support.rs`, all seven `Cargo.toml` files in scope.

Authority (read):

- `docs/spec/ARCHITECTURE.md` — Part III routing (1119–1470), router input queries (1296–1341),
  `fava-observe` owned state and modules (2058–2110), `fava-publication` routing (2160–2200),
  ownership ledger (2960–3010), dependency direction (3060–3095), Falsifiers D/M/N/O
  (3186–3210, 3371–3420), crate responsibility tables (3590–3650).
- `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md` — GOAL-004/006/008/009/010,
  QUERY-004/013/014, WRITE-011..017, WRITE-027/028, RELAY-001/002/012,
  ROUTER-001..004, OPS-004/005, PROFILE-005.
- `docs/internals/vocabulary.toml` — `RouteRequest`, `RouteTarget`, `Router`, `RoutePlan` terms
  (lines 587–668).
- `AGENTS.md` gate list (via the shared brief).

## Findings

---

### route-session-owned-by-facade — critical — ownership

**authority**

> `- route session for automatic queries;`
> — `docs/spec/ARCHITECTURE.md:2070` (inside `## fava-observe` → `### Owned state`)

and the suggested `fava-observe` internal module list:

> `routes.rs           RoutePlan binding`
> — `docs/spec/ARCHITECTURE.md:2105`

and the ownership ledger:

> `| Merged automatic route plan | fava-routing session | observe/publication owner |`
> — `docs/spec/ARCHITECTURE.md:2976`

**implementation**

The entire route session for a live automatic query lives in the thin facade, in a file with the
exact name the spec assigns to a `fava-observe` module:

- `crates/fava/src/routes.rs:24` — `let routes = fava_routing::open(&fava.routers, &request)`
- `crates/fava/src/routes.rs:29` — `RoutePlan::from_contribution(1, &routes.current())`
- `crates/fava/src/routes.rs:74-80` — the facade-private `run(query, providers, routes, active, cancel, revision)`
  task owns the live `Box<dyn RouterSession>`, the desired-vs-active relay reconciliation map
  (`active: BTreeMap<RelaySessionKey, watch::Sender<bool>>`), and the route revision counter.
- `crates/fava-observe/Cargo.toml` has no `fava-routing` dependency at all; `crates/fava-observe/src/lib.rs`
  contains zero occurrences of `Route`/`routing` (verified by grep). `Observation`
  (`crates/fava-observe/src/lib.rs:81`) exposes no route plan, no route revision, no per-relay demand.

Every route-derived mutable fact for a live query — current `RoutePlan`, route revision, desired
destination set, active relay-session cancellation handles, and route shortfall reporting — is owned by
`crates/fava/src/routes.rs`, not by `fava-observe`.

**observable distinction**

An application holding an `Observation` cannot read the route plan that produced its relay work; the
only route evidence is a global, coalesced `Diagnostics::routes` ring buffer keyed by a
facade-fabricated `u64` revision (`crates/fava-diagnostics/src/lib.rs:111`), not attributed to any
observation. Two concurrently open observations produce interleaved, indistinguishable route
revisions in the same buffer. A competing `Observer` implementation cannot receive route-plan changes
at all, because the contract that would carry them does not exist in `fava-observe`.

**proposed falsifier**

```rust
// crates/fava-observe/tests/route_session.rs
#[tokio::test]
async fn observation_owns_and_exposes_its_route_session() {
    let observation = observer.open(Query::events().authors([alice])).unwrap();
    let plan = observation.route_plan();                 // does not exist today
    assert_eq!(plan.revision, 1);
    delayed_router.replace(covering(alice, relay_b));
    let next = observation.route_changed().await.unwrap(); // does not exist today
    assert!(next.destinations.contains_key(&relay_b_session));
}
```

**confidence** confirmed

---

### router-open-failure-kills-whole-query — critical — failure isolation

**authority**

> `5. one router's delay does not prevent other routers' contributions from entering the plan.`
> — `docs/spec/ARCHITECTURE.md:1225`

> Deliberately make providers: block; return late; panic; exceed declared output bounds; ignore
> cancellation; fail during shutdown. **Unrelated queries, relays, writes, signers, and services must
> retain bounded progress.**
> — `docs/spec/ARCHITECTURE.md:3371-3383` (Falsifier M)

> The initial query value MUST be produced from the configured local query sources without waiting for
> any relay response. … **Acceptance:** with every relay unreachable, opening a query returns its local
> view or a local-source error, never hangs waiting for the network.
> — `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:311-325` (QUERY-004)

**implementation**

`crates/fava-routing/src/chain.rs:62-76` — during `open`, a refusal from any single router aborts the
whole chain:

```rust
let mut session = match router.open(request.clone(), upstream_rx) {
    Ok(session) => session,
    Err(error) => { close_sessions(&mut sessions); return Err(error); }
};
let contribution = match attribute(session.current(), router.name()) {
    Ok(contribution) => contribution,
    Err(error) => { session.close(); close_sessions(&mut sessions); return Err(error); }
};
```

`crates/fava/src/routes.rs:24-25` converts that into a hard failure of the observation:

```rust
let routes = fava_routing::open(&fava.routers, &request)
    .map_err(|error| ObserveError::Relay(error.to_string()))?;
```

So one misconfigured or over-bound router makes `Fava::observe` (`crates/fava/src/lib.rs:108`) return
`Err` and yields **no local view at all** — even though every other router had a usable immediate
contribution and the local EventCache/WriteStore sources were never consulted.

The identical failure on the *write* path is handled correctly, which proves this is an inconsistency
rather than a design intent: `crates/fava-publication/src/run.rs:372` converts a `fava_routing::open`
error into `RoutePlan::shortfall(revision, request, error.to_string())` and keeps going.
`RoutePlan::shortfall` (`crates/fava-routing/src/lib.rs:217`) has **zero** callers on the read path.

The same code shape also means a *panicking* third-party router panics the application's own
`Fava::observe` future (the `router.open` call at `chain.rs:63` is a direct provider call on the
caller's task, with no `catch_unwind` and no runtime isolation), and a *blocking* `Router::open`
blocks it indefinitely — `chain::open` is a synchronous `for` loop over all routers.

**observable distinction**

Assembly = `[AppRelayRouter("app", [wss://a])], [RefusingRouter]`. `fava.observe(Query::events())`
returns `Err(ObserveError::Relay("router refused work: …"))` and no relay is contacted at all. With
only the app-relay router configured, the same query returns a handle and contacts `wss://a`.
An application cannot distinguish "one routing policy is broken" from "queries do not work".

**proposed falsifier**

```rust
// crates/fava/tests/automatic_routes.rs
#[tokio::test]
async fn refusing_router_is_attributed_shortfall_not_a_failed_observation() {
    let fava = assembly(transport.clone())
        .router(Arc::new(AppRelayRouter::new("app", [app_relay.clone()])))
        .router(Arc::new(RefusingRouter::new("broken")))   // Router::open -> Err(Refused)
        .build().unwrap();
    let observation = fava.observe(Query::events()).await.expect("other routers still route");
    assert_eq!(transport.open_count(&app_relay), 1);
    assert!(fava.diagnostics().route_shortfalls.iter().any(|(_, m)| m.contains("broken")));
}
```

**confidence** confirmed

---

### chain-collapse-tears-down-all-relay-demand — critical — failure isolation

**authority**

> `4. closing the session releases all router-owned acquisition work;`
> `5. one router's delay does not prevent other routers' contributions from entering the plan.`
> — `docs/spec/ARCHITECTURE.md:1224-1225`

> Later router contributions MAY add relay work to the same query. A route contribution that
> disappears MAY withdraw relay work when no other router still contributes that destination.
> **Unchanged destinations and unrelated query branches MUST remain running.**
> — `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:473-481` (QUERY-014)

`RouterError::Closed` is documented in the implementation itself as a *legitimate* terminal state:
`/// Router session ended after a coherent contribution.` — `crates/fava-routing/src/lib.rs:370-373`.

**implementation**

Three independent conditions collapse the whole chain and then silently cancel **every** relay session
of the open observation, while the `Observation` handle stays open and reports nothing:

1. `crates/fava-routing/src/chain.rs:152-181` — `monitor_router` breaks and drops its `updates_tx`
   clone when a router's `next_change()` returns `Err`. When the last monitor exits,
   `compose_updates` (`chain.rs:187-224`) sees `updates.recv() == None`, breaks, and drops
   `latest_tx`.
2. `crates/fava-routing/src/chain.rs:121-124` — `OpenedChain::next_change` then returns
   `Err(RouterError::Closed)`.
3. `crates/fava/src/routes.rs:93-98` — the facade's `run` loop treats that as terminal:
   `route_shortfall(...)` then `break`, falling through to `routes.rs:130-133`, which sends
   `true` on **every** entry of `active`, closing all relay sessions for that live query.

The same `break`-and-teardown happens at `crates/fava/src/routes.rs:101-108` when
`RoutePlan::from_contribution` returns a bounds refusal for a later contribution.

Additionally `crates/fava-routing/src/chain.rs:217` is an unconditional
`.expect("validated router contributions remain bounded when combined")` on a `Result` that is
genuinely reachable: per-router coverage is validated against `MAX_COVERAGE * 1 = 256`
(`chain.rs:277-279`) but the combined plan is validated against `MAX_COVERAGE * MAX_ROUTERS = 8192`
(`chain.rs:281-283`), and `complete_targets` (`chain.rs:255-272`) then adds one coverage entry per
`RouteRequest::targets()` on top of the router union. `RouteRequest::targets()`
(`crates/fava-routing/src/lib.rs:31-73`) is derived directly from `Query::authors`/`Query::ids`, which
are unbounded (`crates/fava-query/src/selection.rs:41-52` — no cap, verified by grep for
`MAX_AUTHORS`/`authors.len()` across `crates/`). A read query with 8192 authors opens successfully
(8192 ≤ 8192), and the first router update that adds a single out-of-request coverage target makes
8193 > 8192 → the spawned `compose_updates` task panics → `latest_tx` dropped → path (2)/(3) above →
all relays closed. A panic inside a spawned task is neither attributable nor observable.

**observable distinction**

Assembly = `[SettlingRouter]` (a router whose `next_change` returns `Ok` once with destinations, then
`Err(RouterError::Closed)`). The observation opens, contacts the contributed relay, and then — with no
application action and no error surfaced on the handle — the relay session is closed and the query
receives no further relay events. `observation.changed()` never errors; `observation.current()` keeps
returning the last snapshot. The application sees a live query that has silently stopped being live.

**proposed falsifier**

```rust
// crates/fava/tests/automatic_routes.rs
#[tokio::test]
async fn router_that_settles_and_closes_does_not_withdraw_relay_demand() {
    let fava = assembly(transport.clone()).router(Arc::new(SettlingRouter::new("nip65", stable.clone()))).build().unwrap();
    let observation = fava.observe(Query::events()).await.unwrap();
    wait_until(|| transport.open_count(&stable) == 1).await;
    settling.finish();                                  // next_change -> Err(RouterError::Closed)
    tokio::task::yield_now().await;
    assert!(!transport.close_seen(&stable), "settled routing must keep contributed demand");
}
```

**confidence** confirmed

---

### outbox-fabricates-settled-absence-from-source-close — critical — behavioral proof

**authority**

> Elapsed time MUST NOT convert unresolved knowledge into settled absence.
> — `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:878-888` (WRITE-015)

> …and **settled absence only after its exact configured source plan settles**. … With no configured
> discovery source and no retained relay-list event, the router reports unknown rather than inventing
> absence.
> — `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1144-1157` (ROUTER-001)

> A fact established by a specific source **completing** a specific request, such as a relay sending
> EOSE for an exact subscription.
> — `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:141-147` (definition of *settled source fact*)

> Fava MUST keep these outcomes scoped to the exact relay/session/request and **must not wedge
> unrelated work or fabricate stronger facts**.
> — `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1138-1142` (RELAY-012)

**implementation**

`crates/fava-router-outbox/src/lib.rs:216-228`:

```rust
changed = next_source(&mut self.changes) => {
    if let Ok(snapshot) = changed {
        self.lists.ingest(&snapshot, &mut self.shortfalls);
    } else {
        self.settled_absent.extend(self.queried.iter().copied());   // <-- close treated as settlement
        self.changes = None;
    }
    ...
}
```

`self.changes` is a `Box<dyn SourceChanges>`; `next_change()` returns `Err(QuerySourceClosed)` when the
source **closes**, which is not the same event as the source **completing**. The outbox router never
observes EOSE or `SourceStatus` at all — grep shows no `SourceStatus` reference in the crate, and the
only `SourceSnapshot` field it reads is `.events` (`lib.rs:112`).

Worse, closure is exactly what happens on *failure*. When the router's query source is the engine
(`impl QuerySource for Fava`, `crates/fava/src/query_source.rs:14`), the spawned task does:

```rust
let Ok(mut observation) = fava.observe(query).await else { return; };   // query_source.rs:29
```

A failed indexer query (no transport, unreachable indexer, refusing router in the inner assembly,
`ObserveError::Relay`) returns early, drops the `watch::Sender`, and the outbox promotes **every**
queried author to `CoverageState::SettledAbsent` (`lib.rs:264-270`). Combined with WRITE-027 that
turns a network/config failure into a typed "settled empty routing" outcome — a positive fact
manufactured from an error.

**observable distinction**

Configure `OutboxRouter("nip65", [unreachable_indexer], fava_as_source)` for author `alice` whose
relay list is unknown. `Fava::preview_routes` / the live plan report
`CoverageState::SettledAbsent` for `RouteTarget::Author(alice)` and `settled == true`, i.e. Fava
asserts alice has published no relay list — an assertion no relay ever made. The correct observable is
`CoverageState::Unresolved` and `settled == false` for as long as the indexer never answered.

**proposed falsifier**

```rust
// crates/fava-router-outbox/tests/outbox.rs
#[tokio::test]
async fn closed_discovery_source_stays_unresolved_and_never_becomes_settled_absent() {
    let source = Arc::new(ClosingSource::default());       // open() ok, next_change() -> Err immediately
    let router = OutboxRouter::new("nip65", [indexer()], source).unwrap();
    let mut session = router.open(RouteRequest::Read(Query::events().authors([alice])), upstream).unwrap();
    let plan = RoutePlan::from_contribution(2, &session.next_change().await.unwrap()).unwrap();
    assert_eq!(plan.coverage[&RouteTarget::Author(alice)], CoverageState::Unresolved);
    assert!(!plan.settled);
}
```

**confidence** confirmed

---

### router-acquisition-starts-from-fabricated-empty-state — critical — behavioral proof

(New consequence, in this area, of the known-good baseline item
"`impl QuerySource for Fava` starts a recursive `Fava::observe` from a fabricated empty EventCache snapshot".)

**authority**

> - locally available kind-10002 events;
> — `docs/spec/ARCHITECTURE.md:1351` (`fava-router-outbox` → Inputs)

> **The first contribution uses currently available relay-list facts immediately.** Missing facts
> become unresolved needs.
> — `docs/spec/ARCHITECTURE.md:1367`

> A locally accepted relay-list update can therefore influence routing **immediately** through the
> merged local query view, before a relay echoes it.
> — `docs/spec/ARCHITECTURE.md:1305-1307` (Router input queries → Local query service)

**implementation**

- `crates/fava-router-outbox/src/lib.rs:145-165` — the *only* source of relay-list facts inside
  `Router::open` is `known = self.lists.values()` (the in-process `KnownLists` map, populated solely by
  the out-of-contract `OutboxRouter::remember` public method, `lib.rs:71`) plus
  `self.lists.ingest(&initial, &mut shortfalls)` at `lib.rs:164`.
- The router is constructed with a single `Arc<dyn QuerySource>` (`lib.rs:28`) and uses it only via
  `Query::…from_relays(self.indexers…)` (`lib.rs:154-158`) — i.e. only the **explicit** query service.
  There is no local query service wired anywhere: grep for `Freshness::CacheOnly` across the seven
  routing crates returns nothing.
- The `initial` it ingests is fabricated: `crates/fava/src/query_source.rs:20` —
  `let initial = SourceSnapshot::empty(SourceKind::EventCache);` — returned before any observation
  exists.

Net effect: relay-list events already present in the application's `EventCache` are **never** consulted
by the outbox router, and its first contribution is computed against an empty snapshot.

The canary is direct evidence of the resulting shape: `apps/canary/src/automatic_publication.rs:94-106`
must build a **second, separate `Fava` engine** (`query_fava`, `apps/canary/src/automatic_support.rs:68-79`,
with its own `MemoryWriteStore` and its own `WebSocketTransport`) purely to hand the router a query
source, and then hand-feed relay lists through `outbox.remember(...)` at
`automatic_publication.rs:97-107`. That directly contradicts:

> Router-owned acquisition is explicitly routed. This prevents automatic-routing recursion and **reuses
> the same**: wire protocol; subscription planner; **transport**; event verification; event cache;
> query-source observation; and cancellation semantics used by application queries.
> — `docs/spec/ARCHITECTURE.md:1317-1327`

The router's acquisition in the shipped example runs on a *different* transport stack, which is exactly
what WRITE-014's acceptance forbids ("…through explicit query machinery **and no separate transport
stack**", `FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:876`).

There is no deadlock (the recursion is broken by `from_relays` selecting the explicit path at
`crates/fava/src/live.rs:13-15`, and `QuerySource::open` returns without awaiting), but the
construction order is circular — `OutboxRouter::new` needs a `QuerySource` before
`FavaBuilder::router` can accept the router — so there is no supported way to give a router the engine
it is part of.

**observable distinction**

Put alice's kind-10002 event in the `EventCache` before building the assembly. Open an automatic query
for alice with an `OutboxRouter` configured. Today: alice's write relay is *not* contacted at open;
`preview_routes` reports `Unresolved` for `RouteTarget::Author(alice)`, and an indexer REQ is issued
for a fact Fava already holds. Correct behavior: alice's write relay is in the first contribution and
no indexer query is opened.

**proposed falsifier**

```rust
// crates/fava/tests/automatic_routes.rs
#[tokio::test]
async fn cached_relay_list_routes_immediately_without_indexer_traffic() {
    cache.commit(alice_kind_10002());                            // before build
    let fava = assembly(transport.clone()).router(outbox_router()).build().unwrap();
    let plan = fava.preview_routes(&Query::events().authors([alice])).unwrap();
    assert!(plan.destinations.contains_key(&alice_write_session));
    let _obs = fava.observe(Query::events().authors([alice])).await.unwrap();
    assert_eq!(transport.open_count(&indexer), 0);
}
```

**confidence** confirmed

---

### no-router-conformance-testkit — major — behavioral proof

**authority**

> `fava-routing` **ships a conformance testkit that every router implementation can run.**
> It tests: immediate initial contribution; asynchronous updates; complete-snapshot replacement
> semantics; cancellation and resource release; deduplication of relay destinations; exact attribution
> of targets and reasons; upstream-plan reactivity; no automatic-routing recursion; deterministic
> behavior for equal inputs; and bounded unresolved and diagnostic state.
> The routing-chain testkit additionally exercises **arbitrary router order, delayed routers, routers
> that retract contributions, and provider failure isolation.**
> — `docs/spec/ARCHITECTURE.md:1452-1469`

> Each replaceable contract MUST ship a public conformance kit covering: ordinary behavior; refusal and
> malformed input; cancellation and close; late completion; boundedness and overload; … negative tests
> proving it cannot bypass universal invariants.
> — `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:234-248` (GOAL-009; `Router` is a
> replaceable contract per GOAL-010's table, line 264)

**implementation**

`crates/fava-router-testkit/src/lib.rs` is 91 lines containing exactly one type, `DelayedRouter`
(line 14) — a controllable test double. It exports **no** conformance function, **no** assertions, and
**no** chain harness. `crates/fava-routing/` has no `tests/` directory (verified: `ls crates/fava-routing/`
= `BUILD.bazel  Cargo.toml  src`), so `fava-routing` ships no testkit at all, conformance or otherwise.
The only chain-level evidence is four hand-written tests in `crates/fava/tests/automatic_routes.rs`;
none of them exercises a refusing router, a panicking router, a blocking router, router reordering, or
bound overflow.

**observable distinction**

An author of a third-party router has no public way to prove their implementation satisfies the five
`Router`/`RouterSession` contract semantics at `ARCHITECTURE.md:1221-1225`. Concretely: a router whose
`preview()` disagrees with `open().current()` — legal today, since `Router::preview`
(`crates/fava-routing/src/lib.rs:326`) is a separate method with nothing tying it to `open` — makes
`Fava::preview_routes` report destinations that the real query never uses, and nothing in the
repository detects it. (`Router::preview` is itself an addition: the spec's `Router` trait at
`ARCHITECTURE.md:1204-1210` has only `open`.)

**proposed falsifier**

```rust
// crates/fava-routing/tests/conformance.rs   (crate + file do not exist)
#[tokio::test]
async fn conformance_suite_rejects_a_router_whose_preview_disagrees_with_open() {
    let router = Arc::new(LyingRouter::new("liar"));   // preview() != open().current()
    let report = fava_routing::testkit::run_router_conformance(router, sample_requests()).await;
    assert!(report.failures().iter().any(|f| f.contains("preview must equal initial contribution")));
}
```

**confidence** confirmed

---

### stringly-typed-route-shortfall-and-needs — major — boundedness

**authority**

> ```rust
> pub struct RouteContribution {
>     pub destinations: Vec<RouteDestination>,
>     pub coverage: Vec<TargetCoverage>,
>     pub unresolved: Vec<RouteNeed>,
>     pub shortfalls: Vec<RouteShortfall>,
> }
> pub struct RouteDestination { …, pub reason: NamespacedRouteReason }
> pub enum CoverageState { Covered { … }, Unresolved { needs: BTreeSet<RouteNeed> }, SettledAbsent }
> ```
> — `docs/spec/ARCHITECTURE.md:1170-1195`
>
> ```rust
> pub struct RoutePlan { pub revision: RouteRevision, …, pub unresolved: BTreeSet<RouteNeed>,
>                        pub shortfalls: Vec<RouteShortfall>, pub settlement: RouteSettlement }
> pub struct PlannedRelay { …, pub reasons: Vec<AttributedRouteReason> }
> ```
> — `docs/spec/ARCHITECTURE.md:1249-1265`

> If the selected automatic router chain settles with no destination, the write MUST expose a **typed**
> no-destination outcome **naming the unresolved/absent route reasons** that led there.
> — `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:997-1004` (WRITE-027)

> Exceeding a bound MUST produce refusal, backpressure, or exact shortfall. It MUST NOT silently
> discard work while claiming success.
> — `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1420-1438` (OPS-004, which explicitly
> names "router contributions and route fan-out")

**implementation**

Every typed noun in the spec is collapsed to a primitive:

| Spec | `crates/fava-routing/src/lib.rs` |
|---|---|
| `Vec<RouteShortfall>` | `pub shortfalls: Vec<String>` — lines 183, 209 |
| `BTreeSet<RouteNeed>` | `pub unresolved: BTreeSet<RouteTarget>` — lines 181, 207 |
| `NamespacedRouteReason` | `pub reason: String` — line 143 |
| `Vec<AttributedRouteReason>` | `reasons: BTreeSet<(String, String)>` — line 194 |
| `RouteSettlement` | `pub settled: bool` — line 211 |
| `RouteRevision` | `pub revision: u64` — line 201 |
| `Unresolved { needs }` | `Unresolved` (no payload) — line 124 |

`RouterError` (`lib.rs:363-374`) likewise has exactly two variants, both `Refused(String)`/`Closed`, so
a bound refusal ("route destinations exceed bound: 257 > 256"), a malformed indexer response, and an
unreachable indexer are all the same type carrying only prose. `Diagnostics::route_shortfall`
(`crates/fava-diagnostics/src/lib.rs:117`) is `(u64, String)`.

And the "no silent discard" clause is violated directly at
`crates/fava-router-outbox/src/lib.rs:117-122`:

```rust
if let Err(error) = self.remember(&event)
    && shortfalls.len() < MAX_SHORTFALLS
{ shortfalls.push(error.to_string()); }
```

Past 256, malformed relay-list events are dropped with no counter, no marker, and no shortfall — while
the contribution is still reported as a normal successful snapshot.

**observable distinction**

An application cannot programmatically branch on *why* routing produced no destination — WRITE-027's
required typed outcome does not exist; the only signal is substring matching on `Vec<String>`. A
`RelayListError::TooManyRelays { actual, maximum }` (a typed error that `fava-nip65` already produces,
`crates/fava-nip65/src/lib.rs:127-133`) is stringified and, past the 257th malformed event, discarded
entirely, so the plan claims settled/covered state with no record that input was dropped.

**proposed falsifier**

```rust
// crates/fava-router-outbox/tests/outbox.rs
#[tokio::test]
async fn dropped_relay_list_parse_failures_are_reported_as_typed_overflow_shortfall() {
    let source = source_yielding_malformed_relay_lists(300);
    let mut session = OutboxRouter::new("nip65", [indexer()], source).unwrap().open(req, upstream).unwrap();
    let plan = RoutePlan::from_contribution(2, &session.next_change().await.unwrap()).unwrap();
    assert!(plan.shortfalls.iter().any(|s| matches!(s, RouteShortfall::ShortfallsTruncated { dropped: 44, .. })));
}
```

**confidence** confirmed

---

### unbounded-router-owned-relay-list-state — major — boundedness

**authority**

> A router may retain **bounded** derived state intrinsic to its algorithm, such as: current relay-list
> resolutions; …
> — `docs/spec/ARCHITECTURE.md:1331-1339`

> Fava MUST define bounds or explicit backpressure/refusal for: query structure and derived values;
> **router contributions and route fan-out**; …
> — `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1420-1436` (OPS-004)

> Fava MUST keep these outcomes scoped to the exact relay/session/request and must not wedge unrelated
> work or fabricate stronger facts.
> — `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1138-1142` (RELAY-012)

**implementation**

Three unbounded route-derived collections:

1. `crates/fava-router-outbox/src/lib.rs:31-34` — `KnownLists { values: Mutex<BTreeMap<PublicKey, RelayList>>, … }`
   with no capacity, no eviction, and no lifetime scoping. It is `Arc`-shared by the router itself
   (`lib.rs:29`), so it lives for the process, not the route session. `KnownLists::remember`
   (`lib.rs:93-108`) inserts unconditionally on supersede. Each entry can hold up to 256 relay URLs
   (`crates/fava-nip65/src/lib.rs:9`), so the *per-entry* size is bounded but the *entry count* is not.
2. `crates/fava-router-hints/src/lib.rs:21` — `evidence: Arc<Mutex<BTreeMap<EventId, RelayEvidence>>>`,
   same shape, no bound, no eviction.
3. `crates/fava-routing/src/lib.rs:31-73` — `RouteRequest::targets()` is derived one-to-one from
   `Query::authors` / `Query::ids`, and `crates/fava-query/src/selection.rs:41-52` imposes no bound on
   either (grep for `MAX_AUTHORS`, `authors.len()` across `crates/` returns nothing). This is the
   "query structure and derived values" bullet of OPS-004, and it is the input that makes
   `chain.rs:217`'s `.expect` reachable (see `chain-collapse-tears-down-all-relay-demand`).

**observable distinction**

Routing 100 000 distinct authors over a process lifetime permanently retains 100 000 `RelayList`
values; nothing in the public surface exposes, bounds, or evicts them, and `Diagnostics` reports no
router-state size. Memory grows monotonically with the number of authors ever routed, with no typed
refusal at any threshold — the OPS-004 requirement is "bounds or explicit backpressure/refusal", and
neither exists.

**proposed falsifier**

```rust
// crates/fava-router-outbox/tests/outbox.rs
#[test]
fn retained_relay_list_state_is_bounded_and_reports_eviction() {
    let router = OutboxRouter::new("nip65", [indexer()], source).unwrap().with_capacity(1_024);
    for author in 2_000_distinct_authors() { router.remember(&relay_list(author)).unwrap(); }
    assert_eq!(router.retained_relay_lists(), 1_024);   // accessor does not exist today
}
```

**confidence** confirmed

---

### outbox-does-not-coalesce-discovery — major — replaceability

**authority**

> The router **shares/coalesces identical discovery needs across queries and writes** and releases
> acquisition when nothing needs it.
> — `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1144-1157` (ROUTER-001)

> Two route sessions needing the same author may share one discovery observation while retaining
> independent route-session ownership.
> — `docs/spec/ARCHITECTURE.md:1369`

> Shared explicit discovery queries acquire those facts asynchronously and emit replacement
> contributions as each author resolves.
> — `docs/spec/ARCHITECTURE.md:1367`

**implementation**

`crates/fava-router-outbox/src/lib.rs:141-172` — `Router::open` computes `missing` **per session** and
unconditionally opens a fresh query per session:

```rust
let missing: BTreeSet<_> = authors.into_iter().filter(|a| !known.contains_key(a)).collect();
…
let OpenedQuerySource { initial, changes } = self.queries.open(&query)?;
```

There is no in-flight-need registry, no refcount, and no reuse. `OutboxRouter` holds only `KnownLists`
(results), never in-flight needs. Two concurrent route sessions for the same unknown author each build
their own `Query::events().kind(10002).authors([author]).from_relays(indexers)` and each open a
separate observation.

**observable distinction**

Open two live queries for the same unknown author, or one query plus one write p-tagging that author.
The indexer relay receives **two** independent `REQ` frames for the identical kind-10002 filter, and
two independent relay sessions/subscriptions are created. RELAY-001's "every contacted relay MUST be
explainable by current demand" holds, but the duplicated wire work is directly observable in the
transport frame log used by `crates/fava/tests/automatic_routes.rs`.

**proposed falsifier**

```rust
// crates/fava-router-outbox/tests/outbox.rs
#[tokio::test]
async fn two_route_sessions_needing_the_same_author_share_one_discovery_query() {
    let router = OutboxRouter::new("nip65", [indexer()], counting_source.clone()).unwrap();
    let _a = router.open(RouteRequest::Read(Query::events().authors([alice])), up_a.clone()).unwrap();
    let _b = router.open(RouteRequest::Write(event_p_tagging(alice)), up_b.clone()).unwrap();
    assert_eq!(counting_source.open_count(), 1, "identical discovery need must coalesce");
}
```

**confidence** confirmed

---

### fallback-policy-space-not-expressible — major — replaceability

**authority**

> **Its policy defines** whether coverage is measured per recipient, author, reference, or whole
> request; **whether unresolved targets receive immediate fallback**; and whether it applies to reads,
> writes, or both.
> — `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1169-1179` (ROUTER-004)

> ### Policy examples
> - contribute when a target has zero destinations;
> - contribute until every recipient has at least two destinations;
> - **contribute only after upstream resolution settles absent;**
> - **contribute immediately while a target is unresolved;**
> - apply to writes but not reads;
> - cover each insufficient recipient independently.
> — `docs/spec/ARCHITECTURE.md:1424-1431`

**implementation**

`crates/fava-router-fallback-relays/src/lib.rs:17-23` — the whole policy surface is
`{ relays, minimum: NonZeroUsize, reads: bool, writes: bool }`. The coverage test
(`lib.rs:173-178`) is:

```rust
fn covered(plan: &RoutePlan, target: &RouteTarget) -> usize {
    match plan.coverage.get(target) {
        Some(CoverageState::Covered(relays)) => relays.len(),
        Some(CoverageState::Unresolved | CoverageState::SettledAbsent) | None => 0,
    }
}
```

`Unresolved` and `SettledAbsent` are folded into the same value, so the two spec'd policies
"contribute only after upstream resolution settles absent" and "contribute immediately while a target
is unresolved" are **the same** behavior and neither is selectable. Only "contribute immediately while
unresolved" exists. Coverage scope is also hard-coded per-`RouteTarget` (`lib.rs:60-64`); "whole
request" is not selectable.

**observable distinction**

An application whose policy is "only fall back once outbox discovery has genuinely settled absent"
gets its fallback relay **connected while discovery is merely pending**, then disconnected when the
real relay arrives. That connect/close churn is observable in the transport open/close log — and the
existing test `fallback_retracts_when_upstream_coverage_arrives_without_restarting_other_relays`
(`crates/fava/tests/automatic_routes.rs:218-248`) asserts exactly that churn as correct. This also
contacts a relay that the application's policy never justified (RELAY-001,
`FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1031-1035`).

**proposed falsifier**

```rust
// crates/fava/tests/automatic_routes.rs
#[tokio::test]
async fn settled_absent_only_fallback_does_not_connect_while_upstream_is_unresolved() {
    let fallback = FallbackRelayRouter::new("fb", [fb_relay.clone()], one())
        .on_settled_absence_only(true);            // policy knob does not exist today
    let fava = assembly(transport.clone()).router(unresolved_router()).router(Arc::new(fallback)).build().unwrap();
    let _obs = fava.observe(Query::events().authors([alice])).await.unwrap();
    tokio::task::yield_now().await;
    assert_eq!(transport.open_count(&fb_relay), 0);
}
```

**confidence** confirmed

---

### route-target-and-write-request-shape-incomplete — major — dependency direction

**authority**

> ```rust
> pub enum RouteTarget {
>     WholeRequest, Author(PublicKey), Recipient(PublicKey),
>     ReferencedEvent(EventId), ReferencedAddress(EventAddress), Custom(RouteTargetKey),
> }
> ```
> **This allows fallback policy to reason per recipient or per referenced object instead of using one
> whole-request relay count.**
> — `docs/spec/ARCHITECTURE.md:1155-1168`
>
> ```rust
> pub struct WriteRouteRequest { pub event: EventValue, pub receipt_id: ReceiptId,
>                                pub generation: MaterializationId }
> ```
> — `docs/spec/ARCHITECTURE.md:1141-1145`

> - relay destinations for referenced events, **addresses, or authors**;
> — `docs/spec/ARCHITECTURE.md:1409` (`fava-router-hints` → Outputs)

**implementation**

- `crates/fava-routing/src/lib.rs:111-119` — `RouteTarget` has four variants. `ReferencedAddress` and
  `Custom(RouteTargetKey)` are absent, and `RouteTargetKey` does not exist anywhere in the repo.
  `docs/internals/vocabulary.toml:601` even records the intended meaning ("One author, recipient,
  referenced event, **coordinate**, or whole request"), so the coordinate variant is a recorded,
  unimplemented obligation.
- `crates/fava-routing/src/lib.rs:24` — `Write(EventValue)`. No `receipt_id`, no `generation`. A router
  therefore cannot correlate a contribution to a receipt or a materialization generation, which is
  precisely what `ARCHITECTURE.md:2153` ("stale signing and route completions are rejected") requires.
- `crates/fava-router-hints/src/lib.rs:84-104` — the hint router only ever emits
  `RouteTarget::ReferencedEvent`; addresses and authors are impossible.
- `crates/fava-routing/src/lib.rs:31-53` — for a read, `targets()` returns author targets *or* id
  targets, never both, and never inspects `#p` tag selections. A query filtering by both authors and
  ids produces only `Author` targets, so `HintRouter` (which handles only `ReferencedEvent`) can
  contribute nothing to it.

**observable distinction**

A replaceable-event (kind 3xxxx) reply that `a`-tags a coordinate cannot be route-covered per address:
`preview_routes` reports coverage keyed only by `WholeRequest`/`Author`, so a fallback router cannot
apply a per-address minimum, and a third-party router has no `Custom` key to express its own coverage
unit — it must abuse `WholeRequest`, which then collides with the app-relay router's coverage in
`merge_coverage` (`crates/fava-routing/src/lib.rs:376-388`).

**proposed falsifier**

```rust
// crates/fava-router-hints/tests/hints.rs
#[test]
fn address_reference_hint_is_covered_as_referenced_address() {
    let reply = EventBuilder::new(author, Kind::TextNote)
        .tag(Tag::parse(["a", "30023:<pk>:slug", hinted.as_str()]).unwrap()).build().unwrap();
    let plan = RoutePlan::from_contribution(1, &router.preview(&RouteRequest::Write(reply.into()), &RoutePlan::default()).unwrap()).unwrap();
    assert!(plan.coverage.contains_key(&RouteTarget::ReferencedAddress(address)));  // variant absent today
}
```

**confidence** confirmed

---

### route-revision-not-owned-by-routing — major — ownership

**authority**

> `| Merged automatic route plan | fava-routing session | observe/publication owner |`
> — `docs/spec/ARCHITECTURE.md:2976`

> ```rust
> pub struct RoutePlan { pub revision: RouteRevision, … }
> ```
> — `docs/spec/ARCHITECTURE.md:1250`

> later contributions create a new plan revision
> — `docs/spec/ARCHITECTURE.md:1278`

**implementation**

`fava_routing::open` returns `Box<dyn RouterSession>` (`crates/fava-routing/src/chain.rs:47-51`), not a
route-plan session: `RouterSession::current()` / `next_change()` yield a `RouteContribution`
(`crates/fava-routing/src/lib.rs:345-357`), which has no revision field. The chain *does* maintain a
revision internally (`chain.rs:191`, `chain.rs:207`) but never exposes it.

Every consumer therefore invents its own:

- `crates/fava/src/routes.rs:29` — `RoutePlan::from_contribution(1, &routes.current())` (hard-coded 1)
- `crates/fava/src/routes.rs:99-101` — `revision = revision.saturating_add(1);` then
  `RoutePlan::from_contribution(revision, &contribution)`
- `crates/fava-publication/src/run.rs:319` — `let revision = receipt.route_revision.saturating_add(1);`
  (a *different* counter, correctly owned by `WriteStore` per `ARCHITECTURE.md:2977`)

So the same `fava-routing` chain, consumed by a query and by a write, stamps two unrelated revision
sequences onto the same underlying contribution, and the chain's own internal revision is discarded.

**observable distinction**

`Diagnostics::routes` (`crates/fava-diagnostics/src/lib.rs:23`) is `Vec<(u64, Vec<RelaySessionKey>)>`
shared across all queries. Two concurrent automatic observations both start at revision 1 and both
increment independently, so the diagnostic stream contains duplicate revision numbers with different
destination sets and no way to attribute either. An application cannot tell which route revision
belongs to which observation, nor whether revision 3 for query A precedes or follows revision 3 for
query B.

**proposed falsifier**

```rust
// crates/fava-routing/tests/revision.rs   (file does not exist)
#[tokio::test]
async fn the_chain_owns_and_advances_the_route_revision() {
    let mut chain = fava_routing::open(&routers, &request).unwrap();
    assert_eq!(chain.current_plan().revision, 1);       // RouterSession yields contributions today
    delayed.replace(other_contribution());
    assert_eq!(chain.next_plan().await.unwrap().revision, 2);
}
```

**confidence** confirmed

---

### outbox-shortfalls-accumulate-not-replaced — minor — ownership

**authority**

> A router emits **complete replacement snapshots** for its own contribution. A later snapshot replaces
> that router instance's prior destinations, coverage, unresolved needs, **and shortfalls**.
> — `docs/spec/ARCHITECTURE.md:1197`

**implementation**

`crates/fava-router-outbox/src/lib.rs:180` — `OutboxSession { …, shortfalls: Vec<String>, … }` is never
cleared. `next_change` at `lib.rs:219` calls `self.lists.ingest(&snapshot, &mut self.shortfalls)`,
which appends (`lib.rs:120`), and `OutboxSession::contribution` at `lib.rs:190-197` clones the whole
accumulated vector into every snapshot.

**observable distinction**

A single malformed relay-list event at revision 3 remains present in `plan.shortfalls` at revision 300,
long after the author resolved successfully. The application cannot distinguish "this contribution has
a current problem" from "this contribution had a problem once", and the vector grows to its 256 cap
over the life of a long-running route session.

**proposed falsifier**

```rust
// crates/fava-router-outbox/tests/outbox.rs
#[tokio::test]
async fn a_later_snapshot_replaces_prior_shortfalls() {
    source.replace(malformed_relay_list());
    assert_eq!(session.next_change().await.unwrap().shortfalls.len(), 1);
    source.replace(valid_relay_list(&alice, Some(&relay), None, 2));
    assert!(session.next_change().await.unwrap().shortfalls.is_empty());
}
```

**confidence** confirmed

---

### unapproved-private-lifecycle-owners — minor — vocabulary

**authority**

> A new crate, public or cross-crate nominal type, provider contract, persisted entity, configuration
> concept, or **lifecycle owner** is a vocabulary change — and so is a synonym, wrapper, alternate
> representation, or adjective-qualified variant of an existing noun. Note: `tools/check_vocabulary.py`
> only scans `pub struct|enum|trait|type`, so it is blind to `pub(crate)`, `pub(super)`, and private
> lifecycle nouns.
> — shared brief, "Vocabulary policy (AGENTS.md)"

`docs/internals/vocabulary.toml:645-668` records the `RoutePlan` term with
`symbols = [CoverageState, RouteContribution, RouteDestination, PlannedRelay, RoutePlan]`.

**implementation**

- `crates/fava-routing/src/chain.rs:110-114` — `struct OpenedChain { latest, cancel, closed }`. It is
  the lifecycle owner of the whole router chain: it holds the cancellation sender for two
  `tokio::spawn`ed tasks (`chain.rs:86`, `chain.rs:95`), implements `RouterSession`, and implements
  `Drop` to cancel them (`chain.rs:135-139`). This is structurally identical to `fava::OpenedRelay`,
  already flagged in the known-good baseline as an unapproved private lifecycle owner — same
  `Opened*` naming, same cancel-channel-plus-Drop shape.
- `crates/fava-router-outbox/src/lib.rs:31-34` — `struct KnownLists { values, revision }` owns the
  router's derived relay-list state *and* its own change-notification lifecycle
  (`watch::Sender<u64>`), shared by `Arc` across every route session of that router.

Neither appears in `docs/internals/vocabulary.toml` (verified by grep for `OpenedChain` and
`KnownLists`). Also absent from the file entirely, despite being spec nouns: `RouteNeed`,
`RouteShortfall`, `RouteSettlement`, `RouteRevision`, `NamespacedRouteReason`, `AttributedRouteReason`,
`RouteTargetKey` — only `TargetCoverage` is recorded (line 665) as an unimplemented `spec_symbol`.

**observable distinction**

Weak by construction — this is a governance gate, not an API promise. The observable consequence is
that the concept "the thing that owns a chain's spawned work and cancellation" has no name in the
ownership ledger, so `chain-collapse-tears-down-all-relay-demand` above had no registered owner to be
audited against (Falsifier N, `ARCHITECTURE.md:3390-3400`, requires every mutable field in a state-owner
crate to record an owner).

**proposed falsifier**

```python
# tools/check_vocabulary.py — extend the scanner
def test_private_lifecycle_owners_are_declared():
    owners = scan_structs_with_drop_or_cancel(Path("crates"))   # not scanned today
    assert "fava_routing::chain::OpenedChain" in vocabulary_symbols()
    assert "fava_router_outbox::KnownLists" in vocabulary_symbols()
```

**confidence** confirmed

---

## Conforming (verified, not merely unexamined)

Each of these was checked against the cited authority and found to match.

- **`fava-routing` contains no router-implementation semantics** (Falsifier D,
  `ARCHITECTURE.md:3196-3198`). `crates/fava-routing/Cargo.toml` depends only on
  `fava-query`, `fava-state`, `fava-write`, `thiserror`, `tokio` — no `fava-router-*`. There is even an
  enforcing test, `routing_core_does_not_name_concrete_router_crates_or_types`
  (`crates/fava-routing/src/chain.rs:443-463`), asserting the Cargo manifest and `lib.rs` never name
  `fava-router-outbox`/`OutboxRouter`/etc. Falsifier O's first bullet (`ARCHITECTURE.md:3414`) holds.
- **Router order is determined only by assembly** (PROFILE-005,
  `FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1574-1580`). `FavaBuilder::router` /
  `FavaBuilder::routers` (`crates/fava/src/lib.rs:318-330`) append in call order into a plain
  `Vec<Arc<dyn Router>>`; nothing sorts, prioritizes, or special-cases any concrete router type
  anywhere in `crates/fava/src/`.
- **Ordered composition is acyclic and upstream-only** (`ARCHITECTURE.md:1229-1247`). In
  `chain::open` router `i` receives a plan built from `contributions[..i]` only
  (`crates/fava-routing/src/chain.rs:57-60`), and `compose_updates` pushes a new upstream plan strictly
  to indices `> update.index` (`chain.rs:203-211`). No downstream contribution can reach an upstream
  router; the update cascade terminates.
- **Destination deduplication retains every contributing router, target, and reason**
  (`ARCHITECTURE.md:1267`). `RoutePlan::from_contribution` (`crates/fava-routing/src/lib.rs:238-256`)
  keys `PlannedRelay` by `RelaySessionKey` and unions both `targets` and `reasons`. Proved by
  `identical_relay_contributions_deduplicate_and_retain_both_reasons`
  (`crates/fava/tests/automatic_routes.rs:167-181`), which asserts `reasons.len() == 2`.
- **Router attribution cannot be forged by a router.** `RouteDestination.router` is a private field
  (`crates/fava-routing/src/lib.rs:141`); `RouteDestination::new` (`lib.rs:147-157`) hard-codes it to
  `String::new()`, and only the chain's `attribute` (`chain.rs:236-244`) stamps it via the
  `pub(crate)` `set_router`. A third-party router genuinely cannot claim another router's name.
- **Explicit routing bypasses the automatic chain entirely** (WRITE-011,
  `FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:835-844`; `ARCHITECTURE.md:1284`).
  `crates/fava/src/live.rs:13-15` dispatches `QueryAcquisition::Explicit` to `open_explicit`, which
  never touches `fava.routers`; `Fava::preview_routes` (`crates/fava/src/lib.rs:237-245`) uses
  `RoutePlan::explicit` for the explicit branch. Proved by
  `explicit_query_bypasses_every_automatic_router` (`crates/fava/tests/automatic_routes.rs:218-232`),
  which asserts `delayed.open_count() == 0` and `diagnostics().router_sessions.is_empty()`.
  Empty explicit relay sets are refused upstream at `Query::from_relays`
  (`crates/fava-query/src/lib.rs:125-135`, via `non_empty_relays`).
- **Route preview opens no router session and sends no relay traffic** (WRITE-016,
  `FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:890-895`). `chain::preview`
  (`crates/fava-routing/src/chain.rs:26-41`) calls only `Router::preview`; `OutboxRouter::preview`
  (`crates/fava-router-outbox/src/lib.rs:132-138`) reads `self.lists` and never calls
  `self.queries.open`. Proved by
  `immediate_route_starts_before_delayed_router_and_preview_opens_nothing`
  (`crates/fava/tests/automatic_routes.rs:150-153`) and by the canary's explicit
  "route preview performed publication work" checks
  (`apps/canary/src/automatic_publication.rs:126-135`).
- **`fava-nip65` is genuinely pure.** `crates/fava-nip65/src/lib.rs` has no `tokio`, no I/O, no
  routing types; it depends only on `fava-state`, `fava-write`, `thiserror`. `RelayList::supersedes`
  (`lib.rs:107-111`) implements the NIP-01 replaceable tie-break correctly (newer `created_at` wins;
  on equal timestamps the **lowest** event id wins). Relay count is bounded at 256 with a typed
  `RelayListError::TooManyRelays { actual, maximum }` (`lib.rs:127-133`) — a typed shortfall, exactly
  what the rest of the routing stack lacks.
- **`AppRelayRouter` matches ROUTER-003** (`FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1165-1168`):
  it contributes unconditionally regardless of upstream (`crates/fava-router-app-relays/src/lib.rs:47-76`
  never reads `upstream`), and its read/write applicability is selectable (`lib.rs:35-45`).
- **Chain-level bounds exist and produce refusal, not truncation.** `chain.rs:13-20` declares
  `MAX_ROUTERS=32`, `MAX_DESTINATIONS=256`, `MAX_TARGETS=256`, `MAX_COVERAGE=256`,
  `MAX_COVERED_SESSIONS=256`, `MAX_SHORTFALLS=256`, `MAX_TEXT_BYTES=4096`; `validate_contribution`
  (`chain.rs:293-336`) checks all of them and returns `RouterError::Refused` with exact numbers. Router
  names are validated non-empty, length-bounded, and unique (`chain.rs:375-392`). Two unit tests cover
  the destination and router-count bounds (`chain.rs:411-441`). The *typing* of the refusal and the
  *unbounded target derivation* are findings above, but the fan-out bounds themselves are real.
- **Later per-router failures are isolated and attributed.** `live_contribution`
  (`chain.rs:226-234`) converts a post-open router error into a shortfall stamped
  `format!("{router}: {error}")` via `bounded_error` (`chain.rs:236-243`), leaving other routers'
  contributions in the plan. This is the correct behavior — it is precisely what makes
  `router-open-failure-kills-whole-query` an inconsistency rather than a uniform design.
- **No routing deadlock and no automatic-routing recursion.** Traced the full acquisition boundary:
  `OutboxRouter::open` → `QuerySource::open` → (`impl QuerySource for Fava`) spawns and returns
  immediately (`crates/fava/src/query_source.rs:26-51`) → the spawned task calls `Fava::observe` →
  `live::open` → `QueryAcquisition::Explicit` (because the router always uses `from_relays`) →
  `open_explicit`, which never touches `fava.routers` (`crates/fava/src/live.rs:18-60`). The recursion
  is therefore broken and no lock is held across the boundary (`KnownLists::values()` clones under a
  short `Mutex` and drops it before any await, `crates/fava-router-outbox/src/lib.rs:85-91`;
  `remember` drops the guard explicitly before `send_replace`, `lib.rs:104-106`). The *fabricated empty
  initial state* is a separate finding above, but there is no deadlock.
- **`fava-router-testkit` and `fava-nip65` are recorded in `docs/internals/vocabulary.toml`**
  (lines 624/632/640 and 219-224 respectively) and appear in the crate responsibility tables
  (`ARCHITECTURE.md:3641`, `3650`).

## Open questions

1. **Is the second `Fava` engine in `apps/canary` the intended composition pattern?** The circular
   construction (`OutboxRouter::new` needs a `QuerySource`; `FavaBuilder::router` needs the router) has
   no documented resolution. `ARCHITECTURE.md:1296-1327` describes `local_queries` and
   `explicit_queries` as *services* supplied to the router, which suggests a late-binding injection
   point on `FavaBuilder` that does not exist. Settling this decides whether
   `router-acquisition-starts-from-fabricated-empty-state` is fixed at the facade or at the router.
2. **Where should the read-path route shortfall live?** The write path already has
   `RoutePlan::shortfall` (`crates/fava-routing/src/lib.rs:217`, used at
   `crates/fava-publication/src/run.rs:324/372/383`). Reads have no equivalent, and once
   `fava-observe` owns the route session (finding 1) the shortfall needs a home on `Observation`.
   Requires coordination with the `observe` audit area.
3. **Is there an intended bound on `Query::authors` / `Query::ids`?** OPS-004 names "query structure
   and derived values" but no number appears in any spec file. Without one, `RouteRequest::targets()`
   is unbounded and the routing bounds are the de-facto cap — enforced today as a hard observation
   failure rather than a shortfall. This belongs to the `query` audit area to settle.
4. **Bound on concurrent relay sessions created by one route plan.** `crates/fava/src/routes.rs:136-165`
   opens one `OpenedRelay` per destination with no cap; the plan permits 8192. OPS-004 names "active
   relay sessions" as requiring a bound. I did not find one anywhere; this likely belongs to the
   transport or observe audit area rather than routing.
5. **Should `Router::preview` exist at all?** It is not in the spec's `Router` trait
   (`ARCHITECTURE.md:1204-1210`), and nothing constrains it to agree with `open().current()`. Either
   remove it in favor of a non-acquiring `open` mode, or make the (missing) conformance kit enforce
   equality.
> Historical audit record. Superseded by STATE-ARCH-1; not current implementation guidance.
