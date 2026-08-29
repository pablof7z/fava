# Observe / facade audit

Area slug: `observe-facade`
Mode: read-only. No production source, test, or spec file was modified.

## Scope checked

Implementation (read in full):

- `crates/fava-observe/src/lib.rs` (447 lines, incl. unit tests)
- `crates/fava-observe/Cargo.toml`
- `crates/fava/src/lib.rs` (475)
- `crates/fava/src/live.rs` (62)
- `crates/fava/src/relay.rs` (344)
- `crates/fava/src/routes.rs` (174)
- `crates/fava/src/query_source.rs` (109)
- `crates/fava/src/publication.rs` (317)
- `crates/fava/Cargo.toml`

Contract crates read for signature adequacy:

- `crates/fava-query/src/lib.rs`, `crates/fava-query/src/selection.rs`
- `crates/fava-subscriptions/src/lib.rs`
- `crates/fava-transport/src/lib.rs`
- `crates/fava-routing/src/lib.rs`, `crates/fava-routing/src/chain.rs` (signatures)
- `crates/fava-diagnostics/src/lib.rs`

Consumers read to settle the router-acquisition boundary and the publication-cancel boundary:

- `crates/fava-router-outbox/src/lib.rs`
- `crates/fava-publication/src/lib.rs`, `src/run.rs`, `src/delivery.rs`
- `crates/fava-write-store/src/lib.rs` (contract doc for `cancel`)
- `crates/fava-transport-websocket/src/lib.rs` (session/generation ownership)
- `apps/canary/src/grouping.rs`, `apps/canary/src/automatic_publication.rs`
- `crates/fava/tests/*` (inventory of test function names, `explicit_live.rs`, `automatic_routes.rs`, `multi_relay.rs`, `observation_bounds.rs`)

Authority read:

- `docs/spec/ARCHITECTURE.md` — `## fava-observe` (2059-2115), `## fava-routing` (1121-1295), `## Router input queries` (1296-1340), `## fava-router-outbox` (1341-1380), `## fava-subscriptions` (1476-1522), `## fava-transport` (1555-1610), `## fava-session` (2204-2280), `## fava-diagnostics` (2305-2337), `## fava-runtime` (2339-2366), `## fava` (2367-2420), query-source composition (1020-1100), live-query flows (2600-2790), shutdown (2930-2955), ownership ledger (2960-3010), crate table (3620-3640)
- `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md` — QUERY-001..QUERY-017 (273-513), WRITE-014 (870), WRITE-016 (890), WRITE-023 (957)
- `docs/internals/vocabulary.toml` — `Observation` term (355-367), `Subscription plan` symbols (677-687)

---

# Item 1 — `fava-observe` owned-state ledger

Authority: `docs/spec/ARCHITECTURE.md:2065-2075`.

| # | Owned state (ARCHITECTURE:2065-2075) | Status | Where it actually lives |
|---|---|---|---|
| 1a | observation **identity** | **ABSENT** | No `ObservationId` or equivalent exists in any crate (`grep -rn 'ObservationId'` across `crates/` → 0 hits; only `docs/spec/ARCHITECTURE.md:1493`). `Observation` (`crates/fava-observe/src/lib.rs:78-84`) has no id field, no `Eq`, no derivation from `Query`. QUERY-002 equivalence has no representation. |
| 1b | observation **open/close lifecycle** | **SPLIT** | Local half PRESENT: `Observer::open` `crates/fava-observe/src/lib.rs:53-73`; `Observation::close` `:207-212`; `Drop` `:222-226`. Relay half OWNED-ELSEWHERE: `crates/fava/src/live.rs:30-61` (explicit) and `crates/fava/src/routes.rs:33-61` (automatic) decide when relay lifecycle begins and ends. |
| 2 | query | **PRESENT-BUT-DUPLICATED** | Moved into `Observation::start` at `crates/fava-observe/src/lib.rs:87` and captured only by the spawned task closure; `Observation` exposes no accessor. A second copy is cloned into the facade's relay path (`crates/fava/src/live.rs:30,33,35`, `crates/fava/src/routes.rs:33,53`) and a third into each `OpenedRelay` (`crates/fava/src/relay.rs:19,54`). |
| 3 | source observations over EventCache and WriteStore | **PRESENT** (fixed arity) | `crates/fava-observe/src/lib.rs:15-16` (fields), `:53-73` (open), `:108-171` (change loop). Hardcoded to exactly two `Arc<dyn QuerySource>`; not an open, configured source set as ARCHITECTURE:1034 implies. |
| 4 | derived-query dependency graph | **ABSENT** | No dependency node type exists. `FilterSelection` (`crates/fava-query/src/selection.rs:9-21`) carries only literal `ids`/`authors`/`kinds`/`tag_values`; no `Query`-valued projection, union/intersection/difference, or current-account input. QUERY-001 (`:279-282`) and QUERY-007 (`:370-383`) are unrepresentable. |
| 5 | current merged `QuerySnapshot` | **PRESENT** | `crates/fava-observe/src/lib.rs:104` (`watch::channel`), `:79` (`latest`), `:186-188` (`current()`). |
| 6 | route session for automatic queries | **OWNED-ELSEWHERE** | `crates/fava/src/routes.rs:24` opens the chain; the session lives in the facade's detached task `crates/fava/src/routes.rs:72-134`, closed at `:130`. `fava-observe` cannot own it: `crates/fava-observe/Cargo.toml` depends only on `fava-query`, `thiserror`, `tokio` — no `fava-routing`. |
| 7 | logical per-relay demand | **OWNED-ELSEWHERE, NEVER AGGREGATED** | `crates/fava/src/relay.rs:180-181` builds exactly one `RelayDemand` per `(observation, relay)` via `fava_subscriptions::demand_for_query`. There is no demand set, no per-relay union across observations, no branch identity. |
| 8 | ownership/refcounts for shared work | **ABSENT** | No registry or refcount in either crate. `crates/fava/src/relay.rs:184` calls `transport.open_session(...)` unconditionally for every `(observation, relay)` pair; `crates/fava-transport-websocket/src/lib.rs:44-78` opens a fresh socket and increments `next_generation` on every call, with no per-key session map. |
| 9 | source-scoped evidence | **SPLIT** | Source-revision evidence PRESENT: `QueryEvidence`/`SourceEvidence` built in `crates/fava-query/src/lib.rs:419-437`, marked closed at `crates/fava-observe/src/lib.rs:238-242`. Relay-scoped evidence (EOSE / CLOSED / AUTH / failure) OWNED-ELSEWHERE and **unattributed**: written from `crates/fava/src/relay.rs:281-302` straight into the global `fava_diagnostics::Diagnostics` with no observation dimension. |
| 10 | bounded application delivery state | **PRESENT** | Single-slot `watch` (`crates/fava-observe/src/lib.rs:104`), `delivered_revision` (`:82`), coalescing report (`:174-183`). |
| 11 | pending consumer request and cancellation state | **PARTIAL / INVERTED** | Single-pull PRESENT structurally: `changed(&mut self)` (`crates/fava-observe/src/lib.rs:191`) makes a concurrent second pull a compile error. Own cancellation PRESENT: `cancel: watch::Sender<bool>` (`:80`). But `additional_cancel: Vec<watch::Sender<bool>>` (`:81`) is populated from outside via `pub fn attach_cancellation` (`:204-206`), called at `crates/fava/src/live.rs:58` and `crates/fava/src/routes.rs:52` — the observation owner holds cancellation handles it did not create, cannot name, and cannot join. |

Summary: of the 11 named owned facts, **3 are absent outright** (observation identity, dependency graph, shared-work refcount), **3 are owned by the `fava` facade** (route session, per-relay logical demand, relay-scoped evidence), **1 is inverted** (cancellation injected inward), and 4 are present.

---

# Item 2 — the 10-step open sequence, as actually executed

Authority: `docs/spec/ARCHITECTURE.md:2079-2088`.

Entry point for a `Freshness::Live` query: `Fava::observe` (`crates/fava/src/lib.rs:108-114`) → `live::open` (`crates/fava/src/live.rs:11-16`) → `routes::open` (automatic) or `live::open_explicit` (explicit).

| Spec step | Performed? | Exact code | Real position |
|---|---|---|---|
| 1. validate the query through `fava-query` | **NOT PERFORMED** | No validation call exists in either open path. `fava-query` exposes no `validate`; refusal is builder-side only (`crates/fava-query/src/lib.rs:166` `limit`, `:125` `from_relays`). | — |
| 2. open continuous EventCache and WriteStore sources | yes | `crates/fava-observe/src/lib.rs:54` (cache), `:59` (writes), via `crates/fava/src/routes.rs:33` / `crates/fava/src/live.rs:30` | **2nd** (automatic) — after step 4 |
| 3. establish derived dependencies | **NOT PERFORMED** | no code | — |
| 4. create explicit plan or open the router chain | **partially, in the facade** | automatic: `crates/fava/src/routes.rs:23-30`. explicit: **no plan is created at all** — `crates/fava/src/live.rs:33` fabricates a bare `RelaySessionKey` per relay; `RoutePlan::explicit` (`crates/fava-routing/src/lib.rs:287`) is never called on this path. | **1st** — before step 2 |
| 5. compile current per-relay logical demand | yes, per relay, unaggregated | `crates/fava/src/relay.rs:179-181` inside `establish` | **4th**, fused into step 10 |
| 6. open the source snapshots and buffer changes | **buffering NOT PERFORMED** | `crates/fava-observe/src/lib.rs:54` and `:59` read the two initial snapshots at two different instants; the change loop only starts at `:108` after `evaluate` at `:96`. No buffer exists between source open and loop start. | fused into step 2 |
| 7. calculate one complete initial `QuerySnapshot` | yes | `crates/fava-observe/src/lib.rs:96-103` | **3rd** |
| 8. install the observation owner | **partial** | `crates/fava-observe/src/lib.rs:104-171` installs a watch + one task. There is no registry to install into (item 1 #8). | fused into step 7 |
| 9. return the handle and make the initial snapshot readable | yes, **last** | `crates/fava/src/routes.rs:61`, `crates/fava/src/live.rs:61` | **6th — after step 10** |
| 10. start or continue relay work for the current route plan | yes, **awaited before the handle returns** | automatic: `crates/fava/src/routes.rs:42-49` `add_relays(...).await` → `crates/fava/src/relay.rs:184` `transport.open_session(...).await` and `:196` `session.send(frame).await`. explicit: `crates/fava/src/live.rs:32-54` awaits `OpenedRelay::open` per relay in a serial loop. | **5th — before step 9** |

**Real order:** `4 → 2 → (6-partial, 7, 8) → 5 → 10 → 9`, with steps **1 and 3 absent** and step 6's buffering absent.

**Divergences, ranked by consequence:**

1. **9 and 10 are inverted.** The handle is not returned until every relay socket is opened and every `REQ` frame is handed off. This is the QUERY-004 violation (`docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:313`) already in the baseline. Worse for explicit: `crates/fava/src/live.rs:32` awaits relays **serially**, so open latency is the sum of all relay handshakes.
2. **4 precedes 2.** The router chain is opened, and its first contribution computed, before either local source is opened. If `Observer::open` then fails (`crates/fava/src/routes.rs:33`), the already-open `RouterSession` is dropped without `close()` — `routes.rs` returns via `?` at `:33` while `routes` is still a live `Box<dyn RouterSession>` that is never `close()`d. `RouterSession::close` is contractually "releases all router-owned acquisition work" (`docs/spec/ARCHITECTURE.md:1224`); `Drop` is not that contract. This is a *new* leak, distinct from the baseline's partial-open relay-session leak.
3. **Step 1 has no owner.** Nothing refuses a malformed query at open. QUERY-001 (`:288`) requires refusal "before opening relay work".
4. **Step 5 is fused into step 10 per relay.** Demand is compiled inside `establish` (`crates/fava/src/relay.rs:179`) at the moment of connecting, so there is never a *plan* — only per-connection improvisation. Consequently the "aggregate desired subscription plan" of ARCHITECTURE:2979 never exists as a value.
5. **Step 6's buffering window is absent**, so gaplessness between initial snapshot and first change depends entirely on each provider's own stream, with no contract requiring it.

---

# Item 3 — required contract shape

Legend: **flows through** = the neutral contract crate the fact must cross; **adequate?** = whether that crate's *current* signature can carry it.

### 3.1 Observation identity

- Owner: `fava-observe`. Must be a public nominal type (`ObservationId`) derived from the canonical `Query` (`docs/spec/ARCHITECTURE.md:1493`, QUERY-002 `:294-298`).
- Flows through: `fava-subscriptions` (as `RelayDemand.owner`), `fava-diagnostics` (as the attribution key for every relay fact).
- **Adequate? NO.** Change required in `fava-subscriptions`:

```rust
// crates/fava-subscriptions/src/lib.rs:13-18 — current
pub struct RelayDemand {
    pub subscription_id: SubscriptionId,
    pub filter: Filter,
}
```

must become the spec shape (`docs/spec/ARCHITECTURE.md:1492-1497`) carrying `owner: ObservationId`, `branch: QueryBranchId`, `bounds: QueryBounds`. Note `RelayDemand` already ships a *wire* `SubscriptionId` as its logical identity — the logical/wire distinction the planner exists to make is collapsed at the input.
- Vocabulary: `ObservationId` and `QueryBranchId` are new cross-crate nominal types → `docs/internals/vocabulary.toml` change (the `Observation` term at `:355-367` lists no identity symbol today).

### 3.2 Observation registry

- Owner: `fava-observe`, internal (`registry.rs` in the suggested module list, `docs/spec/ARCHITECTURE.md:2100`). Not a cross-crate contract.
- Flows through: nothing; it is `fava-observe`-private state keyed by `ObservationId`.
- **Adequate? N/A — absent.** The blocking constraint is that `Observer` is `#[derive(Clone)]` with only `Arc<dyn ...>` provider handles (`crates/fava-observe/src/lib.rs:11-18`); a registry requires shared interior state, so `Observer` must gain an `Arc<Inner>`.

### 3.3 Logical per-relay demand

- Owner: `fava-observe` (`docs/spec/ARCHITECTURE.md:2978`, `:1520`).
- Flows through: `fava-subscriptions`.
- **Adequate? Signature yes, use no.** `SubscriptionPlanner::plan(&self, relay, demand: &[RelayDemand])` (`crates/fava-subscriptions/src/lib.rs:60-64`) already accepts a slice; the caller (`crates/fava/src/relay.rs:181`) always passes exactly one element. The missing piece is the `constraints: &RelayReadConstraints` third parameter from `docs/spec/ARCHITECTURE.md:1485-1489` — without it the planner cannot apply the NIP-11 subscription-count / message-size limits the spec assigns to it (`:1540-1546`), and `SubscriptionPlanError::TooManySubscriptions` / `FrameTooLarge` (`crates/fava-subscriptions/src/lib.rs:78-91`) are unreachable through the public path.

### 3.4 Aggregate desired subscription plan + diff

- Owner: `fava-observe` owns the desired plan; the planner computes it; transport executes it (`docs/spec/ARCHITECTURE.md:2979`).
- Flows through: `fava-subscriptions` (plan + diff values + withdrawal identity, `docs/spec/ARCHITECTURE.md:1508-1512`), then `fava-transport` (execution).
- **Adequate? NO.**

```rust
// crates/fava-subscriptions/src/lib.rs:33-42 — current
pub struct SubscriptionPlan {
    pub relay: RelaySessionKey,
    pub messages: Vec<ClientMessage<'static>>,
    pub attribution: BTreeMap<SubscriptionId, Filter>,
    pub demand: BTreeMap<SubscriptionId, Vec<SubscriptionId>>,
}
```

There are no `shortfalls`, no diff value, and no withdrawal identity. Today the facade re-derives the whole plan on every reconnect (`crates/fava/src/relay.rs:137-144`) and withdraws by hand-encoding `ClientMessage::close` per attribution key (`crates/fava/src/relay.rs:317`) — i.e. the facade owns the diff *and* the withdrawal vocabulary. The spec shape adds `wire: Vec<PlannedSubscription>` and `shortfalls: Vec<SubscriptionShortfall>` (`docs/spec/ARCHITECTURE.md:1499-1503`); a `SubscriptionPlanDelta` (add/keep/withdraw per wire subscription) is the missing value.

### 3.5 Shared-work refcount

- Owner: `fava-observe` (`docs/spec/ARCHITECTURE.md:2072`), keyed by `ObservationId` + `RelaySessionKey`.
- Flows through: `fava-transport` — because sharing is only possible if one relay session can be *acquired* rather than *opened*.
- **Adequate? NO — this is the hard blocker.**

```rust
// crates/fava-transport/src/lib.rs:28-35 — current
pub trait Transport: Send + Sync {
    fn open_session(
        &self,
        key: RelaySessionKey,
    ) -> Pin<Box<dyn Future<Output = Result<Arc<dyn RelaySession>, TransportError>> + Send + '_>>;
}

// crates/fava-transport/src/lib.rs:48-51 — current
fn next_message(
    &self,
) -> Pin<Box<dyn Future<Output = Result<String, TransportError>> + Send + '_>>;
```

Two problems. (a) `open_session` is unconditionally *open*, never *acquire*; `crates/fava-transport-websocket/src/lib.rs:44-78` therefore builds a new socket and a new generation per call with no per-key map. (b) `next_message(&self)` on a shared `Arc<dyn RelaySession>` means whichever holder polls first consumes the frame — two observations physically cannot both read one session. The spec shape is `fn messages(&self) -> Box<dyn RelayMessageStream>` (`docs/spec/ARCHITECTURE.md:1573`), i.e. the session hands out independent reader handles, plus `open_session(request: OpenRelaySession)` (`:1560-1563`) so the caller can pass an establishment deadline and access constraints. Both must change before any refcount is meaningful.

### 3.6 Relay-session binding

- Owner: session lifecycle and generation belong to `Transport` (`docs/spec/ARCHITECTURE.md:2980`, `:1583-1592`); the *binding* of an observation's demand to a session generation belongs to `fava-observe`.
- Flows through: `fava-transport` (`RelaySessionIdentity` — currently `key()` + `generation()`, `crates/fava-transport/src/lib.rs:39-43`, adequate as data) and `fava-diagnostics`.
- **Adequate? Partly.** The identity data exists. What is missing is that today the *binding* is a private facade struct: `OpenedRelay { session_key, query, transport, planner, cache, diagnostics, next_subscription, session, attribution }` (`crates/fava/src/relay.rs:17-27`), which also privately owns reconnect (`:126-168`) and backoff (`:135`, hardcoded 50 ms) — both assigned to transport by `docs/spec/ARCHITECTURE.md:1585-1586`. On reconnect the facade *replaces* `self.session` (`crates/fava/src/relay.rs:157`), so from the observation's point of view the generation change is invisible; nothing propagates a session-generation change to the observation (`docs/spec/ARCHITECTURE.md:2092` requires "session facts" to reach the exact affected observations).

### 3.7 Provider-operation generation with late-completion rejection

- Owner: the operation's owner — here `fava-observe` for read operations (analogous to `fava-publication`'s "current revision generation", `docs/spec/ARCHITECTURE.md:2126`).
- Flows through: `fava-transport` (session generation) and `fava-subscriptions` (wire subscription identity).
- **Adequate? Partly present, by accident, at the wrong owner.** `crates/fava/src/relay.rs:265` rejects an unattributed `EVENT` and `:281-299` reject unattributed `EOSE`/`CLOSED` — that is late-completion rejection, and it is correct behavior, but the attribution map is a facade-private `BTreeMap<SubscriptionId, Filter>` (`crates/fava/src/relay.rs:26`) replaced wholesale on reconnect (`:158`). There is no generation *value* an owner can compare; correctness relies on `SubscriptionId` being freshly allocated (`crates/fava/src/relay.rs:214-222`). No contract crate carries an operation generation for reads. The publication analogue (`RevisionId`) has a named type; the read side has none.

### 3.8 Route session

- Owner: `fava-observe` owns the *binding* (`docs/spec/ARCHITECTURE.md:2070`); `fava-routing` owns the merged plan (`:2976`).
- Flows through: `fava-routing`.
- **Adequate? Contract yes; dependency graph no.** `fava_routing::open(&routers, &request) -> Box<dyn RouterSession>` (`crates/fava-routing/src/chain.rs:49`) and `RouterSession` (`crates/fava-routing/src/lib.rs:341-353`) are exactly what an observe-side owner needs. The blocker is that `crates/fava-observe/Cargo.toml` does not depend on `fava-routing`, `fava-subscriptions`, `fava-transport`, `fava-ingest`, or `fava-diagnostics` — it depends only on `fava-query`, `thiserror`, `tokio`. **Every** relay-side owned fact in item 1 is unreachable from the crate the architecture assigns it to. Adding those four dependencies is dependency-direction-legal (all are neutral contracts, `docs/spec/ARCHITECTURE.md:3072-3082`).
- Second blocker: `Observer::open(&self, query: Query) -> Result<Observation, ObserveError>` is **synchronous** (`crates/fava-observe/src/lib.rs:52`). Steps 4/5/10 require awaiting router open and transport handoff. The correct shape is an `async fn open` that returns the handle **after** local steps 2/6/7/8 and **spawns** step 10, so 9-before-10 becomes structurally enforced rather than a discipline.

### 3.9 Cancellation

- Owner: `fava-observe` (`docs/spec/ARCHITECTURE.md:2075`); propagation belongs to `fava-runtime` (`:2358`).
- Flows through: nothing today; it must not be an injectable public method.
- **Adequate? NO — inverted.** `pub fn attach_cancellation(&mut self, cancel: watch::Sender<bool>)` (`crates/fava-observe/src/lib.rs:204-206`) is a public door letting any caller graft a lifecycle onto the observation owner. Once `fava-observe` owns route session, demand, and relay binding, this method has no caller and must be deleted; a competing `fava-observe` that omitted it would break the current `fava` crate, which is the definition of a private bypass (gate 3).

### 3.10 Shutdown join

- Owner: `fava-runtime` performs joins (`docs/spec/ARCHITECTURE.md:2990`); `fava` owns shutdown *ordering* (`:2955`, `:2371-2372`, `:2382`).
- Flows through: `fava-runtime` (does not exist — baseline).
- **Adequate? NO — nothing to be adequate.** `Observation::close()` (`crates/fava-observe/src/lib.rs:207-212`) sends three `watch` flags and returns; every consumer of those flags is a detached `tokio::spawn` (`crates/fava-observe/src/lib.rs:107`, `crates/fava/src/live.rs:59`, `crates/fava/src/routes.rs:53`, `:158`). There is no `JoinHandle` retained anywhere in either crate. `Fava` has no `close()` at all. Minimum shape: `Observation::close(self) -> impl Future<Output = ()>` (or a `closed()` awaitable), each spawn's `JoinHandle` retained by its owner, and `Fava::close(&self) -> Result<(), CloseError>` driving the `docs/spec/ARCHITECTURE.md:2930-2953` order.

### 3.11 Route session for diagnostics attribution

- Owner: `fava-diagnostics` owns the bounded snapshot (`docs/spec/ARCHITECTURE.md:2989`); each owner publishes attributed facts (`:2311-2321`).
- **Adequate? NO.** Every `Diagnostics` writer method is keyed by `(RelaySessionKey, generation)` or by a bare `u64` route revision (`crates/fava-diagnostics/src/lib.rs:105-200`); none accepts an observation dimension, and `DiagnosticsSnapshot` (`:17-40`) has no `queries` field despite `docs/spec/ARCHITECTURE.md:2329`. See finding `diagnostics-route-facts-are-unattributed`.

---

# Item 4 — `impl QuerySource for Fava` vs the router-acquisition boundary

**Is a recursive observe the intended shape?** Partly yes, mostly no.

The architecture *does* intend routers to reuse the real query machinery: "Router-owned acquisition is explicitly routed. This prevents automatic-routing recursion and reuses the same: wire protocol; subscription planner; transport; event verification; event cache; query-source observation; and cancellation semantics used by application queries" (`docs/spec/ARCHITECTURE.md:1317-1327`). So the *mechanism* is right.

But it specifies **two distinct, narrow services** supplied at router construction, not one `QuerySource`:

- `local_queries.open(query)` — "Reads current merged local query state **without starting automatic relay routing**" (`docs/spec/ARCHITECTURE.md:1300-1306`)
- `explicit_queries.open(query, exact_relays)` — `:1308-1314`

And WRITE-014 makes the constraint a MUST NOT: "A router MUST NOT open private sockets, bypass event admission, own generic subscription grouping, or **recursively invoke automatic routing for its own acquisition**" (`docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:874`).

What exists instead: `OutboxRouter::new(name, indexers, queries: Arc<dyn QuerySource>)` (`crates/fava-router-outbox/src/lib.rs:43-47`) takes **one** untyped source, and `impl QuerySource for Fava` (`crates/fava/src/query_source.rs:13-55`) satisfies it by spawning `Fava::observe(query)` (`:26`). Three concrete defects follow, two of them behavioral:

1. **The service permits the recursion WRITE-014 forbids.** `Fava::observe` dispatches on `query.source().acquisition()` (`crates/fava/src/live.rs:12-15`); an `Automatic` query re-enters `fava_routing::open(&fava.routers, ...)` (`crates/fava/src/routes.rs:24`) and calls the same router's `open` again. `OutboxRouter` happens to use `from_relays` today (`crates/fava-router-outbox/src/lib.rs:158`), so no live recursion — but nothing in the contract or the type prevents the next router from recursing. Finding `router-query-service-permits-recursion`.
2. **The initial snapshot is fabricated empty, destroying warm-cache routing.** `crates/fava/src/query_source.rs:20` returns `SourceSnapshot::empty(SourceKind::EventCache)` synchronously; the real merged snapshot is only pushed later from the spawned task (`:29`). `OutboxRouter::open` calls `self.lists.ingest(&initial, ...)` (`crates/fava-router-outbox/src/lib.rs:164`) on exactly that empty snapshot, so the router's first contribution reports every author unresolved even when the event cache already holds their kind-10002 events. Contradicts `docs/spec/ARCHITECTURE.md:1306` ("A locally accepted relay-list update can therefore influence routing immediately through the merged local query view, before a relay echoes it") and `:1222` ("the initial contribution never waits on network acquisition"). Finding `router-source-fabricates-empty-initial`.
3. **Open refusal is swallowed.** `let Ok(mut observation) = fava.observe(query).await else { return; };` (`crates/fava/src/query_source.rs:26-28`). The router's `queries.open()` already returned `Ok`, so a failed observation appears as a source that simply never changes. QUERY-003's typed refusal (`docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:302-305`) is erased for router acquisition. Also `SourceKind::EventCache` is stamped on a snapshot that carries `SourceEvent::Local` write-store records (`crates/fava/src/query_source.rs:88,98`) — a mislabeled source role.

**Verdict:** the router should not consume `Arc<dyn QuerySource>` at all, and `Fava` should not implement `QuerySource`. The router should receive two narrow, non-recursive services owned by `fava-observe`, each returning a real merged initial snapshot and a typed refusal — a `LocalQueryService` that never touches routers, and an `ExplicitQueryService` whose signature makes the exact relay set a required argument so `Automatic` is unrepresentable. `impl QuerySource for Fava` is additionally the reason the canary must construct a **second whole `Fava` engine** purely to feed the outbox router (`apps/canary/src/automatic_publication.rs:94-95`: `let queries: Arc<dyn QuerySource> = Arc::new(query_fava(Arc::clone(&cache))?);`) — an engine-per-router assembly the architecture never describes.

---

# Item 5 — facade thinness

Authority: "The facade owns Fava-instance identity, top-level command admission, startup and shutdown order, and handles to the selected owners and providers. It owns no event-kind dispatch, routing policy, query evaluation, retry algorithm, socket state, or storage schema." (`docs/spec/ARCHITECTURE.md:2371-2374`)

### Mutable facts and lifecycles the facade owns today

| Fact / lifecycle | Where | Architecture assigns it to | Verdict |
|---|---|---|---|
| `next_subscription: Arc<AtomicU64>` — wire subscription identity allocation | `crates/fava/src/lib.rs:90`, `:444`; allocated `crates/fava/src/relay.rs:214-222` | `fava-observe` ("Query demand for one relay", `:2978`) | **deviation** (finding) |
| Router chain session for a live query | `crates/fava/src/routes.rs:24`, task `:72-134` | `fava-observe` (`:2070`) | baseline |
| Freshness → relay-work decision | `crates/fava/src/lib.rs:109-113` | `fava-observe` open step 4 (`:2082`); QUERY-013A | **deviation** (finding) |
| Per-relay logical demand + `SubscriptionPlan` computation and validation | `crates/fava/src/relay.rs:179-183`, `:224-248` | `fava-observe` desired plan; planner computes (`:2979`) | baseline |
| Relay session establishment, `REQ` handoff, `CLOSE` withdrawal, `session.close()` | `crates/fava/src/relay.rs:184-211`, `:311-343` | `fava-transport` + `fava-observe` | baseline |
| Reconnect loop and 50 ms backoff | `crates/fava/src/relay.rs:126-168`, `:135` | `fava-transport` ("connection backoff", `:1585`) | baseline |
| Inbound frame decode + ingest attribution + admission | `crates/fava/src/relay.rs:106` (`decode_relay`), `:250-309` | `fava-ingest` (`:2040-2048`) | **deviation** (finding) |
| Relay/EOSE/CLOSED/AUTH/failure diagnostic emission | `crates/fava/src/relay.rs:89,109,193,209,266,283,295,300,302,330,333,339`; `crates/fava/src/routes.rs:27,95,111,161,168,172` | each owner publishes its own facts (`:2311`) | **deviation** (finding) |
| Route revision counter per observation | `crates/fava/src/routes.rs:29` (starts at 1), `:100` | `fava-routing` session (`:2976`) | **deviation** (finding) |
| A second, divergent route derivation for preview | `crates/fava/src/lib.rs:235-247` | `fava-routing`; WRITE-016 requires one derivation | **deviation** (finding) |
| Write cancellation eligibility | `crates/fava/src/lib.rs:121-125` (`cancel_write` → raw `WriteStore::cancel`) | `fava-publication` ("exact cancellation eligibility", `:2129`) | **deviation** (finding, critical) |
| Engine shutdown / command admission | **not owned by anyone** — `Fava` has no `close`, no state | `fava` (`:2371-2372`, `:2382`, `:2401`, `:2955`, `:2991`) | **deviation** (finding) |

### `publication.rs`

`crates/fava/src/publication.rs` is, by contrast, **thin and conforming**. It owns no mutable state:

- `Write` (`:19-24`) holds `WriteId`, `ReceiptId`, and a cloned `Publication` handle; every method delegates (`receipt` `:44-48`, `settled` `:57-70`).
- `PublishAs` / `PublishTo` (`:110-114`, `:145-148`) are inert borrow-scoped builders; `#[must_use = "a signer scope is inert until publish is called"]` (`:109`) plus the compile_fail doctests (`:88-108`) prove inertness. `publication_scopes_are_inert_before_valid_payload` is the named falsifier in `docs/internals/vocabulary.toml:505`.
- `PublishPayload` (`:227-233`) is `pub(crate)`, so the neutral `WriteIntent` door is closed to applications — matching the `compile_fail` block at `crates/fava/src/lib.rs:51-57`.
- `all()` / `at_least(n)` (`:203-225`) are pure predicates over `Receipt`.

The single facade-thinness defect on the publication side is not in `publication.rs` at all — it is `Fava::cancel_write` in `lib.rs`, which routes around this module and around `Publication` entirely.

---

# Findings

### cancel-write-bypasses-publication-owner — critical — ownership

**authority** — "exact cancellation eligibility" is listed among `fava-publication`'s owned state, `docs/spec/ARCHITECTURE.md:2129`; "Cancellation is decided from current revision, signature, and handoff facts." `docs/spec/ARCHITECTURE.md:2181`; WRITE-023: "An application may cancel an accepted write while Fava can still prove that no event bytes have been handed to transport for any destination whose obligation cancellation would erase. Cancellation MUST: terminate current signer/route/delivery work" `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:959-963`.

**implementation** — `crates/fava/src/lib.rs:121-125`:

```rust
pub fn cancel_write(&self, receipt_id: ReceiptId) -> Result<bool, WriteStoreError> {
    self.write_store.cancel(receipt_id).map(|receipt| receipt.is_some())
}
```

This calls the raw provider primitive whose own contract is "Cancel one accepted local contribution **before publication work exists**" (`crates/fava-write-store/src/lib.rs:285`), unconditionally and without consulting `Publication`. The eligible door, `Publication::cancel` (`crates/fava-publication/src/lib.rs:203-217`), additionally signals the receipt's cancellation watch (`:209-214`) which is the only thing that stops the running publication loop (`crates/fava-publication/src/run.rs:26,40,84-86`) and, through it, the in-flight signer operation (`crates/fava-publication/src/run.rs:150`, `:438`). `cancel_write` skips that signal entirely. Two public doors (`cancel_write` at `lib.rs:121`, `cancel_publication` at `lib.rs:202`) mutate the same lifecycle fact with different semantics and different error types. `cancel_write` is exercised as the real path in `crates/fava/tests/local_source_merge.rs:128`, `crates/fava/tests/semantic_write_failures/reservation.rs:39`, and `apps/canary/src/local.rs:161`.

**observable distinction** — Accept a write for which a slow `Signer` is in flight, then call `fava.cancel_write(receipt_id)`. It returns `Ok(true)`; the signer call is never cancelled and completes afterwards. The same receipt through `fava.cancel_publication(receipt_id)` cancels the signer operation. An application therefore gets different post-cancel provider behavior depending on which public door it used, for the same fact.

**proposed falsifier**

```rust
#[tokio::test]
async fn cancelling_a_write_terminates_its_in_flight_signer_operation() {
    let (fava, signer) = assembly_with_blocking_signer();   // signer records cancel-watch trips
    let write = fava.publish(unsigned_event()).expect("accepted");
    wait_until(|| signer.calls() == 1).await;
    assert!(fava.cancel_write(write.receipt_id()).expect("cancel commits"));
    wait_until(|| signer.cancelled() == 1).await;           // fails today: never cancelled
}
```

**confidence** — confirmed.

---

### router-source-fabricates-empty-initial — critical — behavioral proof

**authority** — "A locally accepted relay-list update can therefore influence routing immediately through the merged local query view, before a relay echoes it." `docs/spec/ARCHITECTURE.md:1306`; "the initial contribution never waits on network acquisition" `docs/spec/ARCHITECTURE.md:1222`; WRITE-015: "Elapsed time MUST NOT convert unresolved knowledge into settled absence" and route knowledge must distinguish known from unresolved, `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:880-886`.

**implementation** — `crates/fava/src/query_source.rs:20-21`:

```rust
let initial = SourceSnapshot::empty(SourceKind::EventCache);
let (latest, receiver) = watch::channel(Arc::new(initial.clone()));
```

The real merged state only arrives later, from the spawned task at `:29`. `OutboxRouter::open` consumes exactly that empty initial: `crates/fava-router-outbox/src/lib.rs:160-165` destructures `OpenedQuerySource { initial, changes }` and calls `self.lists.ingest(&initial, &mut shortfalls)`. The session's first contribution (`crates/fava-router-outbox/src/lib.rs:191-197` → `contribution()` over `self.lists.values()`) therefore sees zero relay lists from the cache. `crates/fava/src/routes.rs:29` turns that into `RoutePlan::from_contribution(1, ...)`, so route revision 1 has no destinations.

**observable distinction** — Warm the event cache with a valid kind-10002 relay list for author A, build a `Fava` whose only router is an `OutboxRouter` fed `Arc<dyn QuerySource>` = a `Fava`, then open a live query for author A. The first `fava.diagnostics().routes` entry is `(1, [])` and A's relay is contacted only after an indexer round-trip, even though the answer was already in the local cache. With the router's `remember()` pre-seeded instead, revision 1 contains the relay — proving the path exists but the local read is not wired.

**proposed falsifier**

```rust
#[tokio::test]
async fn warm_cache_relay_list_reaches_the_first_route_revision() {
    let cache = warm_cache_with_relay_list(&author, &relay_b);   // kind 10002, no remember()
    let fava = assembly_with_outbox_router(cache);
    let _observation = fava.observe(Query::events().authors([author])).await.unwrap();
    let routes = fava.diagnostics().routes;
    assert_eq!(routes[0].0, 1);
    assert!(routes[0].1.iter().any(|s| s.relay() == &relay_b));   // fails today: revision 1 is empty
}
```

**confidence** — confirmed.

---

### explicit-open-produces-no-route-plan — major — ownership

**authority** — "An exact non-empty relay list produces a `RoutePlan` directly. No router session is opened, and no router-owned input acquisition runs." `docs/spec/ARCHITECTURE.md:1284`; WRITE-016: "Preview uses the same routing derivation as the real operation over currently available router snapshots." `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:894`; ARCHITECTURE's explicit-open flow puts "exact `RoutePlan` is created directly" in the sequence, `docs/spec/ARCHITECTURE.md:2657`.

**implementation** — `crates/fava/src/live.rs:31-33` builds a bare `RelaySessionKey::new(relay.clone(), query.access().clone())` per relay and never constructs a `RoutePlan`. `RoutePlan::explicit` (`crates/fava-routing/src/lib.rs:287-311`) exists and *is* called — but only from `Fava::preview_routes` (`crates/fava/src/lib.rs:239`). So preview and the real open use two different derivations for the same explicit query, and the real path emits no `diagnostics.route(..)` fact (contrast `crates/fava/src/routes.rs:31` for automatic).

**observable distinction** — For one explicit query, `fava.preview_routes(&query)` returns a `RoutePlan` with `revision == 1`, populated `destinations`, and `settled == true`; opening the very same query leaves `fava.diagnostics().routes` empty. An application cannot see, for an explicit live query, which destinations the engine actually adopted, while it can for an automatic one.

**proposed falsifier**

```rust
#[tokio::test]
async fn explicit_open_records_the_same_route_plan_preview_reports() {
    let query = Query::events().from_relays([relay_a.clone()]).unwrap();
    let previewed = fava.preview_routes(&query).unwrap();
    let _observation = fava.observe(query).await.unwrap();
    let routes = fava.diagnostics().routes;
    assert_eq!(routes.len(), 1);                                   // fails today: 0
    assert_eq!(routes[0].0, previewed.revision);
}
```

**confidence** — confirmed.

---

### facade-owns-subscription-identity — major — ownership

**authority** — "| Query demand for one relay | `fava-observe` | subscription planner |" `docs/spec/ARCHITECTURE.md:2978`; "`fava-observe` owns logical demand; `fava-transport` performs the plan." `docs/spec/ARCHITECTURE.md:1520`; the spec's `RelayDemand` carries `owner: ObservationId` / `branch: QueryBranchId`, `docs/spec/ARCHITECTURE.md:1492-1497`.

**implementation** — `crates/fava/src/lib.rs:90` (`next_subscription: Arc<AtomicU64>`), initialized `crates/fava/src/lib.rs:444`, threaded through `crates/fava/src/live.rs:41`, `crates/fava/src/routes.rs:39,151`, and consumed by `crates/fava/src/relay.rs:214-222` (`allocate_subscription`, minting `fava-{n}`). The facade is the authority for the identity that correlates every inbound EVENT/EOSE/CLOSED, and it is a facade-global counter shared by all observations. `fava-observe` has no subscription-identity concept at all.

**observable distinction** — A replacement `fava-observe` implementation cannot allocate or reuse demand identity; a second `Fava` clone shares the counter through `Arc`, so demand identity is engine-global rather than observation-scoped, and there is no public way to correlate a `fava.diagnostics().subscriptions` entry back to the `Observation` that requested it.

**proposed falsifier**

```rust
#[tokio::test]
async fn subscription_identity_is_attributable_to_its_observation() {
    let first = fava.observe(live_query(author_a)).await.unwrap();
    let second = fava.observe(live_query(author_b)).await.unwrap();
    let facts = fava.diagnostics().subscriptions;
    assert_ne!(observation_of(&facts[0]), observation_of(&facts[1]));  // no such accessor today
}
```

**confidence** — confirmed (the observable distinction requires the new attribution surface; the ownership contradiction itself is direct).

---

### facade-decides-freshness-policy — major — ownership

**authority** — open step 4: "create explicit plan or open the router chain" is a `fava-observe` step, `docs/spec/ARCHITECTURE.md:2082`; QUERY-013: "Opening a live-freshness query MUST contribute relay demand immediately." `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:459`; QUERY-013A: "A query's declared freshness policy is evaluated when the query opens", `:467`.

**implementation** — `crates/fava/src/lib.rs:109-113`:

```rust
if query.freshness() == Freshness::CacheOnly {
    self.observer.open(query)
} else {
    live::open(self, query).await
}
```

`Observer::open` (`crates/fava-observe/src/lib.rs:52-73`) ignores `Freshness` entirely and opens zero relay work for any query. So `fava-observe`'s own public entry point violates QUERY-013 when used directly: a `Freshness::Live` query opened through the owning crate contributes no relay demand and reports no shortfall.

**observable distinction** — An application (or `fava-standard`, or a competing facade) that holds an `Observer` and calls `observer.open(live_query)` gets a silently cache-only observation with no error. Only routing the call through the concrete `fava::Fava` produces relay work — a private bypass in the facade rather than a contract in the owner.

**proposed falsifier** (in `crates/fava-observe/tests/`, at the owning component)

```rust
#[tokio::test]
async fn live_freshness_through_the_owner_contributes_relay_demand() {
    let observer = Observer::new(cache, writes, evaluator).with_transport(transport.clone());
    let _observation = observer.open(Query::events().kind(TEXT_NOTE)).await.unwrap();
    wait_until(|| transport.open_count(&relay_a) == 1).await;    // fails today: Observer cannot
}
```

**confidence** — confirmed.

---

### single-demand-per-relay-defeats-the-planner-contract — major — replaceability

**authority** — `fava-subscriptions` owns "the conformance rules that define semantic equivalence" and "plan diff values", `docs/spec/ARCHITECTURE.md:1508-1512`; `fava-subscriptions-standard` "may deduplicate identical filters; combine filters that differ in one safely unionable dimension", `docs/spec/ARCHITECTURE.md:1530-1536`; gate 6 requires evidence "at the owning component, through the real public path" (`AGENTS.md`, restated in the brief).

**implementation** — `crates/fava/src/relay.rs:180-181`:

```rust
let plan = planner
    .plan(session_key, &[demand_for_query(subscription, query)])
```

The slice is always length 1, at every call site (`crates/fava/src/relay.rs:181` is the only caller of `SubscriptionPlanner::plan` in the shipping engine). No aggregation across observations, branches, or route revisions exists. The grouping behaviour is proven only by invoking the planner directly, off the public path: `apps/canary/src/grouping.rs:80-92` builds a hand-made `demand` vector and calls `StandardSubscriptionPlanner::default()` itself (`:84`, `:139`), and `crates/fava/Cargo.toml` dev-depends on `fava-subscriptions-no-grouping` only.

**observable distinction** — Open three equivalent-relay live queries differing only in `authors` against the same relay with `StandardSubscriptionPlanner` selected. The relay receives three `REQ` frames; the planner's documented single-grouped-`REQ` behaviour is unreachable. Swapping to `fava-subscriptions-no-grouping` produces identical wire output — i.e. the planner slot is not actually replaceable through the public API.

**proposed falsifier**

```rust
#[tokio::test]
async fn three_author_queries_at_one_relay_group_into_one_req() {
    let fava = assembly(StandardSubscriptionPlanner::default(), transport.clone());
    let _a = fava.observe(explicit(relay_a.clone(), author_a)).await.unwrap();
    let _b = fava.observe(explicit(relay_a.clone(), author_b)).await.unwrap();
    let _c = fava.observe(explicit(relay_a.clone(), author_c)).await.unwrap();
    assert_eq!(transport.reqs(&relay_a).len(), 1);   // fails today: 3
}
```

**confidence** — confirmed.

---

### subscription-plan-has-no-diff-or-withdrawal-identity — major — ownership

**authority** — `fava-subscriptions` owned meaning includes "plan diff values" and "withdrawal identity", `docs/spec/ARCHITECTURE.md:1511-1512`; "| Wire subscription plan | `fava-observe` owns desired plan; planner computes it | transport executes it |", `docs/spec/ARCHITECTURE.md:2979`.

**implementation** — `SubscriptionPlan` (`crates/fava-subscriptions/src/lib.rs:33-42`) carries `relay`, `messages`, `attribution`, `demand` — no diff, no withdrawal identity, no shortfalls. Because the value does not exist, the facade improvises both: on reconnect it recomputes and re-sends the whole plan (`crates/fava/src/relay.rs:137-158`), and withdrawal is hand-encoded `ClientMessage::close(id)` per attribution key in the facade (`crates/fava/src/relay.rs:311-336`). A replacement planner cannot express "keep subscription X, withdraw Y, add Z" and cannot report a relay-limit shortfall alongside a partially satisfiable plan (`SubscriptionPlanError::TooManySubscriptions` at `crates/fava-subscriptions/src/lib.rs:78-83` is all-or-nothing).

**observable distinction** — When a route revision withdraws one of several relays for a query, the facade cancels the whole `OpenedRelay` (`crates/fava/src/routes.rs:119-123`) and closes the session. There is no path by which a planner can express partial withdrawal within a still-open session, so a shared session (once sharing exists) would necessarily be torn down for an unrelated observation's withdrawal.

**proposed falsifier**

```rust
#[tokio::test]
async fn withdrawing_one_branch_leaves_the_other_wire_subscription_open() {
    let mut a = fava.observe(explicit(relay_a.clone(), author_a)).await.unwrap();
    let _b = fava.observe(explicit(relay_a.clone(), author_b)).await.unwrap();
    a.close();
    wait_until(|| transport.closes(&relay_a) == 1).await;
    assert_eq!(transport.session_count(&relay_a), 1);   // fails today: two sessions, one torn down
}
```

**confidence** — confirmed.

---

### observation-close-does-not-join — major — failure isolation

**authority** — "Each resource is closed by its owner. The facade owns shutdown ordering." `docs/spec/ARCHITECTURE.md:2955`; `fava-runtime` owns "cancellation propagation; resource joining and shutdown deadlines", `docs/spec/ARCHITECTURE.md:2357-2358`; QUERY-012: "closing wakes pending pulls promptly; repeated close is harmless; and shutdown ends all pending pulls without hanging", `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:450-452`.

**implementation** — `Observation::close(&self)` (`crates/fava-observe/src/lib.rs:207-212`) sets three `watch` flags and returns. Every consumer is a detached task with no retained `JoinHandle`: `crates/fava-observe/src/lib.rs:107`, `crates/fava/src/live.rs:59`, `crates/fava/src/routes.rs:53` and `:158`. The actual `CLOSE` frame and `session.close()` happen inside `OpenedRelay::run`'s cancel arm (`crates/fava/src/relay.rs:77-82` → `withdraw` at `:311-343`), arbitrarily later. `Drop for Observation` (`crates/fava-observe/src/lib.rs:222-226`) makes this worse: dropping the handle at the end of a `#[tokio::test]` fires cancellation into tasks that the test runtime then destroys, so the `CLOSE` may never be written at all.

**observable distinction** — Immediately after `observation.close()` returns, the relay has received no `CLOSE` and the session is still open; an application that closes a query and immediately asserts on relay state sees stale demand. If the runtime is shut down right after `close()`, the `CLOSE` is never sent — the relay keeps the subscription until it times out.

**proposed falsifier**

```rust
#[tokio::test]
async fn close_returns_only_after_relay_demand_is_withdrawn() {
    let observation = fava.observe(explicit(relay_a.clone(), author_a)).await.unwrap();
    observation.close().await;                       // no awaitable close today
    assert!(script.sent().iter().any(|f| f.starts_with(r#"["CLOSE""#)));
    assert_eq!(script.open_sessions(), 0);
}
```

**confidence** — confirmed.

---

### post-open-evaluation-failure-is-silent — major — failure isolation

**authority** — QUERY-011: "Causal streams, including receipt transitions, cancellation, signer completion, and **lifecycle termination**, MUST NOT silently lose facts. Any bounded loss MUST be explicit and typed." `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:434`; QUERY-003: "Engine shutdown refusal and inability to read the initial local sources MUST remain distinguishable." `:307`.

**implementation** — `crates/fava-observe/src/lib.rs:159-162`:

```rust
let Ok(mut snapshot) = evaluator.evaluate(&query, &sources) else {
    break;
};
```

A `QueryEvaluationError` after open drops the error, breaks the loop, closes both sources (`:169-170`), and drops the `watch::Sender`. The application's next `changed()` (`:191-202`) returns `Err(ObservationClosed)` — the identical value produced by explicit `close()`, by engine teardown, and by revision overflow (`:163-166`). `ObserveError` (`:250-267`) has no shutdown variant, so the QUERY-003 distinction has no representation even at open time. No diagnostic is emitted on any of these paths (`Diagnostics` is not a dependency of `fava-observe`).

**observable distinction** — An application whose evaluator refuses one mid-stream snapshot sees its query terminate with `ObservationClosed` and cannot distinguish that from its own `close()` or from engine shutdown; `fava.diagnostics()` records nothing.

**proposed falsifier**

```rust
#[tokio::test]
async fn mid_stream_evaluation_failure_is_typed_and_distinguishable() {
    let mut observation = observer.open(query).await.unwrap();
    evaluator.fail_next();
    push_source_change();
    let error = observation.changed().await.unwrap_err();
    assert!(matches!(error, ObservationEnded::Evaluation(_)));   // today: ObservationClosed only
}
```

**confidence** — confirmed.

---

### diagnostics-route-facts-are-unattributed — major — boundedness

**authority** — `fava-diagnostics` inputs include "open observation and route ownership", `docs/spec/ARCHITECTURE.md:2311`; the output shape is `DiagnosticsSnapshot { relays, queries, writes, providers, limits }`, `docs/spec/ARCHITECTURE.md:2327-2333`.

**implementation** — `DiagnosticsSnapshot` (`crates/fava-diagnostics/src/lib.rs:17-40`) has no `queries` field and no observation dimension on any variant. `routes: Vec<(u64, Vec<RelaySessionKey>)>` (`:23`) is keyed by a route revision that is **per-observation and always starts at 1**: `crates/fava/src/routes.rs:29` (`RoutePlan::from_contribution(1, ...)`) and `:100` (`revision.saturating_add(1)` on a task-local variable). Two concurrent automatic queries therefore both publish `(1, ...)` into one flat bounded list, and `route_shortfall(revision, ...)` (`crates/fava/src/routes.rs:96,105,162,172`) is likewise unattributable.

**observable distinction** — Open two automatic live queries with disjoint relay sets. `fava.diagnostics().routes` contains two entries both labelled revision 1 with no field distinguishing which observation they belong to; there is no public way to attribute a shortfall to the query it degraded.

**proposed falsifier**

```rust
#[tokio::test]
async fn concurrent_automatic_queries_have_distinguishable_route_facts() {
    let _a = fava.observe(automatic(author_a)).await.unwrap();
    let _b = fava.observe(automatic(author_b)).await.unwrap();
    let queries = fava.diagnostics().queries;        // no such field today
    assert_eq!(queries.len(), 2);
    assert_ne!(queries[0].observation, queries[1].observation);
}
```

**confidence** — confirmed.

---

### facade-owns-ingest-pipeline — major — dependency direction

**authority** — `fava-ingest`'s owned lifecycle is "validate relay-frame shape and bounds; attribute an event to an accepted wire subscription and logical demand; verify event id and Schnorr signature; ..." `docs/spec/ARCHITECTURE.md:2040-2048`; the facade "owns no event-kind dispatch, routing policy, query evaluation, retry algorithm, socket state, or storage schema", `docs/spec/ARCHITECTURE.md:2373-2374`; the relay-event flow is `transport → fava-wire → fava-ingest → ... → fava-observe`, `docs/spec/ARCHITECTURE.md:2705-2720`.

**implementation** — `crates/fava/Cargo.toml` lists `fava-ingest`, `fava-wire`, and `fava-event-cache` as direct dependencies of the facade. `crates/fava/src/relay.rs:106` decodes the frame (`decode_relay`), `:117-123` dispatches `RelayMessage` variants, `:265` performs subscription attribution against a facade-private `BTreeMap<SubscriptionId, Filter>`, and `:269-277` calls `fava_ingest::admit_subscription_event(cache, session.key(), &id, &id, filter, event, Timestamp::now())`. Steps 1, 2 and the cache handle of the ingest lifecycle are executed by the facade; `fava-ingest` is reduced to a free function. Note `&id, &id` — the wire subscription id is passed as both the wire and the logical identity, because no logical demand identity exists (item 1 #7).

**observable distinction** — A product replacing the ingest owner has no seam: there is no `Ingest` contract object on the builder (`crates/fava/src/lib.rs:265-388` has no `.ingest(...)`), so admission policy cannot be substituted the way every other provider role can. The wire-decode step likewise has no seam.

**proposed falsifier**

```rust
#[test]
fn the_facade_does_not_depend_on_ingest_or_wire() {
    let manifest = std::fs::read_to_string("Cargo.toml").unwrap();
    assert!(!manifest.contains("fava-ingest"));   // fails today
    assert!(!manifest.contains("fava-wire"));     // fails today
}
```

(Stronger behavioural form: assert a builder-selected alternative admission owner changes which events reach an open query.)

**confidence** — confirmed.

---

### router-query-service-permits-recursion — major — failure isolation

**authority** — WRITE-014: "A router MUST NOT open private sockets, bypass event admission, own generic subscription grouping, or recursively invoke automatic routing for its own acquisition." `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:874`; "Router-owned acquisition is explicitly routed. This prevents automatic-routing recursion", `docs/spec/ARCHITECTURE.md:1317`; the two named services are `local_queries.open(query)` (`:1303`) and `explicit_queries.open(query, exact_relays)` (`:1312`).

**implementation** — `impl QuerySource for Fava` (`crates/fava/src/query_source.rs:13-55`) accepts any `&Query`, including `QueryAcquisition::Automatic`, and dispatches through `Fava::observe` → `live::open` (`crates/fava/src/live.rs:12-15`) → `routes::open` → `fava_routing::open(&fava.routers, &request)` (`crates/fava/src/routes.rs:24`), re-entering the same router's `open`. `OutboxRouter::new` (`crates/fava-router-outbox/src/lib.rs:43-47`) takes a single untyped `Arc<dyn QuerySource>` that conflates both spec services; nothing type-level or runtime-level rejects an automatic query. The prohibition is enforced only by `OutboxRouter` happening to call `.from_relays(...)` at `crates/fava-router-outbox/src/lib.rs:158`.

**observable distinction** — A second router (or an edited `OutboxRouter`) that opens an automatic input query produces unbounded recursion — one new router session and one new observation per level — with no typed refusal and no bound. Today no public API refuses it.

**proposed falsifier**

```rust
#[tokio::test]
async fn a_router_input_query_cannot_request_automatic_acquisition() {
    let router = RecursingRouter::new(query_service.clone());   // opens Query::events() (Automatic)
    let fava = assembly_with_router(router);
    let error = fava.observe(automatic(author_a)).await.unwrap_err();
    assert!(matches!(error, ObserveError::Relay(_)));   // today: recursion, not refusal
}
```

**confidence** — confirmed (structural); the recursion itself is `suspected` only in that no shipping router triggers it.

---

### router-session-leaks-on-source-open-failure — major — failure isolation

**authority** — QUERY-003: "return a typed refusal and leave no ownerless demand, partial dependency, or relay work", `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:305`; router contract: "closing the session releases all router-owned acquisition work", `docs/spec/ARCHITECTURE.md:1224`.

**implementation** — `crates/fava/src/routes.rs:24` opens `routes: Box<dyn RouterSession>`; `crates/fava/src/routes.rs:33` then does `let mut observation = fava.observer.open(query.clone())?;`. On the `?` early return, `routes` is dropped without `RouterSession::close()` being called. The same applies to the `RoutePlan::from_contribution` failure at `:29-30`. The router's own acquisition observation (e.g. `OutboxSession.changes`, `crates/fava-router-outbox/src/lib.rs:173`) is only released in `OutboxSession::close` (`:233-240`), which `Drop` does not invoke — `OutboxSession` has no `Drop` impl.

**observable distinction** — Inject an event-cache open failure. `fava.observe(automatic_query)` returns `ObserveError::SourceOpen`, but the outbox router's indexer observation stays open: the indexer relay still holds a live `REQ` and the transport still holds a session. This is distinct from the baseline's partial-open *relay* leak — the leaked resource here is a router session, on the automatic path, before any relay is opened.

**proposed falsifier**

```rust
#[tokio::test]
async fn a_failed_local_source_open_closes_the_router_session() {
    let fava = assembly_with_failing_event_cache_and_outbox_router(transport.clone());
    let error = fava.observe(automatic(author_a)).await.unwrap_err();
    assert!(matches!(error, ObserveError::SourceOpen { .. }));
    wait_until(|| transport.open_sessions() == 0).await;   // fails today: indexer session remains
}
```

**confidence** — confirmed.

---

### no-facade-close-or-command-admission — critical — ownership

**authority** — "The facade owns Fava-instance identity, top-level command admission, startup and shutdown order", `docs/spec/ARCHITECTURE.md:2371-2372`; required ordering includes "stop of new commands before observations, publications, routers, transports, and stores close", `:2382`; public surface includes "deterministic close and destructive reset", `:2401`; the shutdown sequence `facade enters Closing → new application work is refused → ... → facade enters Closed`, `:2930-2953`; "| Public engine lifecycle | `fava` | application/SDK |", `:2991`.

**implementation** — `struct Fava` (`crates/fava/src/lib.rs:82-93`) has no lifecycle field; `impl Fava` (`:95-248`) has no `close`, no `reset`, no admission state. `grep -n 'fn close\|fn shutdown\|Closing\|Closed' crates/fava/src/lib.rs` returns only the `ObservationClosed` re-export at `:17`. `#[derive(Clone)]` at `:82` means the "Fava-instance identity" the facade is supposed to own is freely duplicable. Consequently `ObserveError` (`crates/fava-observe/src/lib.rs:250-267`) has no shutdown variant, so QUERY-003's required distinction between shutdown refusal and local-source failure (`docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:307`) is unrepresentable.

**observable distinction** — An application cannot deterministically stop a `Fava`. Dropping the last clone leaves every spawned task (`crates/fava/src/routes.rs:53,158`, `crates/fava/src/live.rs:59`, `crates/fava-publication/src/run.rs:40`) running until the Tokio runtime itself is dropped; relay `CLOSE` frames, write-store flush, and receipt settlement are all skipped. There is no way to ask whether the engine is accepting work.

**proposed falsifier**

```rust
#[tokio::test]
async fn close_refuses_new_work_and_joins_owned_resources() {
    let _observation = fava.observe(explicit(relay_a.clone(), author_a)).await.unwrap();
    fava.close().await.unwrap();                               // no such method today
    assert_eq!(script.open_sessions(), 0);
    assert!(matches!(fava.observe(query).await, Err(ObserveError::EngineClosed)));
}
```

**confidence** — confirmed.

---

### observe-has-no-evidence-at-the-owner — major — behavioral proof

**authority** — gate 6: "public promises have falsifiable evidence at the owning component, through the real public path" (`AGENTS.md`, restated in the brief); `docs/spec/ARCHITECTURE.md:2059-2115` assigns the live-query promises to `fava-observe`.

**implementation** — `ls crates/fava-observe/` → `BUILD.bazel`, `Cargo.toml`, `src`. There is no `tests/` directory. The crate's only evidence is three `#[cfg(test)]` unit tests inside `crates/fava-observe/src/lib.rs:358`, `:382`, `:403`, covering source-open rollback, initial-evaluation rollback, and post-open source closure. Every falsifier for QUERY-002, QUERY-004, QUERY-010, QUERY-011, QUERY-012, QUERY-013, QUERY-014, and QUERY-015 lives in `crates/fava/tests/` (`explicit_live.rs:189,217,239,273,342`, `multi_relay.rs:184,230,281`, `automatic_routes.rs:140,192,219`, `observation_bounds.rs:27,51`) — i.e. the promises of `fava-observe` are proved only through the facade that currently owns the behaviour. This is the structural reason the baseline deviation went undetected: moving the evidence to the owner would have made the crate boundary visible.

**observable distinction** — Not application-observable; this is a process/gate finding. Reported because it is the mechanism by which the whole class of deviations became invisible, and because remediation must relocate the evidence, not only the code.

**proposed falsifier** — the falsifiers proposed in `facade-decides-freshness-policy` and `post-open-evaluation-failure-is-silent`, placed in a new `crates/fava-observe/tests/` directory, are exactly this.

**confidence** — confirmed.

---

### coalescing-counter-conflates-source-and-query-revisions — minor — boundedness

**authority** — QUERY-011: "Current-state streams ... MAY coalesce intermediate states. The next delivered state MUST be correctly rebased onto what the application actually received", `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:432`; the field is documented as "Intermediate current-**query** revisions intentionally superseded by newer state" (`crates/fava-diagnostics/src/lib.rs:18-19`).

**implementation** — `report_skipped` (`crates/fava-observe/src/lib.rs:174-183`) is called from two places with two different units. At `:139-145` it is called with `current.revision.0` and `snapshot.revision.0`, which are `SourceRevision`s owned by the *provider*; at `:196-200` it is called with `delivered_revision.0` and `latest.revision.0`, which are `QueryRevision`s. Both increment the same `Diagnostics::query_updates_coalesced` counter (`crates/fava/src/lib.rs:403-406`).

**observable distinction** — A provider whose `SourceRevision` sequence advances by more than one per emitted change (permitted — `SourceRevision` is documented as "Monotonic provider-owned revision", `crates/fava-query/src/lib.rs:258`) inflates `fava.diagnostics().coalesced_query_updates` even when the application consumed every delivered query revision.

**proposed falsifier**

```rust
#[tokio::test]
async fn source_revision_gaps_are_not_counted_as_coalesced_query_updates() {
    let source = SkippingSource::new(/* emits revisions 1, 5, 9 */);
    let mut observation = fava.observe(cache_only_query()).await.unwrap();
    for _ in 0..2 { observation.changed().await.unwrap(); }
    assert_eq!(fava.diagnostics().coalesced_query_updates, 0);   // fails today: 6
}
```

**confidence** — confirmed.

---

## Conforming (verified, not merely unexamined)

Each of these was checked against a specific authority line and a specific code line, and found to agree.

- **QUERY-012 single pending pull.** `Observation::changed(&mut self)` (`crates/fava-observe/src/lib.rs:191`) makes a concurrent second pull a borrow-check error, satisfying "at most one `next` operation may be pending per handle; a second concurrent pull is refused without consuming data" (`:444-445`) structurally rather than at runtime.
- **QUERY-012 repeated close is harmless.** `close()` uses `send_replace` (`crates/fava-observe/src/lib.rs:208-211`); `FavaChanges::close` guards with `self.closed` (`crates/fava/src/query_source.rs:73-78`); `OutboxSession::close` likewise (`crates/fava-router-outbox/src/lib.rs:234`).
- **QUERY-011 bounded observation memory.** Delivery is a single-slot `watch` (`crates/fava-observe/src/lib.rs:104`), so a slow consumer retains exactly one snapshot; proved through the public path by `crates/fava/tests/observation_bounds.rs:51`.
- **QUERY-003 local all-or-nothing open.** `Observer::open` closes the first source when the second refuses (`crates/fava-observe/src/lib.rs:59-67`) and closes both when initial evaluation fails (`:97-102`); both are unit-proved at `:358` and `:382`. (The *relay* half of all-or-nothing is the baseline deviation; the local half is correct.)
- **QUERY-010 fresh request identity on reconnect.** `reconnect` re-enters `establish` (`crates/fava/src/relay.rs:137-144`), which allocates a new `SubscriptionId` from the counter (`:179` → `:214-222`); late frames bearing the old id are refused as unattributed (`:265-267`, `:284-287`, `:296-298`). Proved by `crates/fava/tests/multi_relay.rs:230`.
- **QUERY-010 EOSE/CLOSED/AUTH/failure remain distinct.** Five separate diagnostic categories, never collapsed (`crates/fava/src/relay.rs:281-302`; `crates/fava-diagnostics/src/lib.rs:30-38`). Proved by `crates/fava/tests/explicit_live.rs:342`.
- **QUERY-009 no global-completeness claim.** `QuerySnapshot` (`crates/fava-query/src/lib.rs:407-417`) and `QueryEvidence` (`:385-401`) expose only per-source revision and open/closed status; no `synced`, `complete`, or percentage field exists anywhere in `fava-query`, `fava-observe`, or `fava-diagnostics`.
- **Post-open source closure is scoped evidence, not query termination.** `mark_source_closed` (`crates/fava-observe/src/lib.rs:238-242`) sets `SourceStatus::Closed` and the observation continues on the surviving source; unit-proved at `crates/fava-observe/src/lib.rs:403`.
- **Bounded diagnostics.** `Diagnostics::bounded` with a 256-per-category default (`crates/fava-diagnostics/src/lib.rs:63-66`, `:70-76`) and `VecDeque` per category (`:49-60`) satisfy gate 5 for retained evidence — the defect found is attribution, not boundedness.
- **`crates/fava/src/publication.rs` is thin.** No mutable state; `PublishAs`/`PublishTo` are inert until `publish` (`:109`, `:144`); the neutral `WriteIntent` door is `pub(crate)` (`:227`) and the closure is enforced by `compile_fail` doctests at `crates/fava/src/lib.rs:46-81`.
- **Subscription-plan validation is defensive at the right strength.** `validate_plan` (`crates/fava/src/relay.rs:224-248`) rejects a planner that returns a mis-scoped relay, empty attribution, non-REQ messages, or attribution that disagrees with its own REQ — this is correct adversarial treatment of a replaceable provider. (Its *location* in the facade is the deviation, not its content.)
- **Route contribution bounds.** `RoutePlan::from_contribution` calls `chain::validate_combined` (`crates/fava-routing/src/lib.rs:241`) before building, and `RoutePlan::explicit` calls `validate_router_contribution` (`:306`), so an unbounded router contribution is refused at the routing boundary.
- **`SourceKind` set matches the architecture's two standard local sources** (`crates/fava-query/src/lib.rs:233-240` vs `docs/spec/ARCHITECTURE.md:2620-2623`).

Searches that ran and returned nothing (absence claims):

- `grep -rn 'ObservationId\|QueryBranchId\|RelayReadConstraints' crates/` → 0 hits.
- `grep -rn 'fn close\|fn shutdown\|Closing\|Closed' crates/fava/src/lib.rs` → only the `ObservationClosed` re-export.
- `grep -rn 'SubscriptionPlanner' crates/*/src/` → the only shipping caller of `plan` is `crates/fava/src/relay.rs:181`; all other hits are the contract, the two planner crates, and `apps/canary/src/grouping.rs`.
- `ls crates/fava-observe/tests` → does not exist.
- `grep -rn 'derive\|Derived\|nested\|projection\|from_query' crates/fava-query/src/selection.rs` → only the `#[derive(...)]` attribute; no derived-query support.

---

## Open questions

1. **Where does `RelayReadConstraints` come from?** `docs/spec/ARCHITECTURE.md:1488` makes it a required `plan` argument and `:1544` sources it from NIP-11, but no NIP-11 service is wired into the engine today (`fava-nip11` is not a `crates/fava` dependency). Remediation must decide whether `fava-observe` supplies a default-permissive constraint value or the planner signature takes `Option`.
2. **`QueryBounds` vs the existing whole-query `limit`.** `Query::limit` is a whole-query bound (`crates/fava-query/src/lib.rs:166`, QUERY-008 `:397`), while the spec's `RelayDemand.bounds` is per-branch. With no derived-query graph there is exactly one branch, so the two coincide today — but the contract change should not encode that coincidence.
3. **Does `WriteStore::cancel` reset destination outcomes?** I confirmed `Fava::cancel_write` bypasses `Publication::cancel`'s watch signal, and that `start_lanes` (`crates/fava-publication/src/delivery.rs:33-45`) is invoked once with the freshly-read receipt before the `is_terminal()` break at `crates/fava-publication/src/run.rs:79`. Whether that can actually launch a post-cancel handoff depends on what each `WriteStore` writes into `destinations()` on cancel — a write-store-area question I did not settle. The facade-ownership finding stands regardless.
4. **Should `fava-observe` depend on `fava-transport` directly, or should relay execution sit behind a narrower `fava-observe`-facing contract?** `docs/spec/ARCHITECTURE.md:1520` says "`fava-transport` performs the plan", which reads as a direct dependency, but `:2350` puts "transport sessions" under `fava-runtime`'s owned resources. The remediation design needs to pick one; I read the ledger (`:2980`, transport owns the connection generation; observe is a consumer) as favouring a direct `fava-observe → fava-transport` contract edge with `fava-runtime` supplying the task/join primitives.
5. **Is `Freshness::CacheOnly` allowed to reach a `QuerySource`-shaped local service at all?** The `local_queries` service (`:1300-1306`) and a `CacheOnly` query are nearly the same thing. Collapsing them would remove one of the two router services; I did not find authority resolving whether they must remain distinct nouns.
