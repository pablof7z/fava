# Query / state / event-cache audit

Area slug: `query-state-cache`
Scope crates: `fava-query`, `fava-query-standard`, `fava-state`, `fava-event-cache`, `fava-event-cache-memory`.

## Scope checked

Specs read in full or in the cited ranges:

- `docs/spec/partial-spec-api-semantics.md` (all 628 lines)
- `docs/spec/ARCHITECTURE.md` — 100–215 (types-live-with-owner, merged source state, cache profiles),
  357–453 (`fava-state`), 562–727 (`fava-query`), 763–861 (storage roles, `fava-event-cache`),
  1032–1106 (query-source composition), 2960–3012 (ownership ledger)
- `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md` — 270–520 (QUERY-001..017), 520–720 (EVENT-001..014)
- `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md` — 178–224 (milestone map), 295–420 (M1/M2), M5/M6 required behavior, 1400–1445 (canary/requirement map)
- `AGENTS.md`, `docs/internals/vocabulary.toml` (terms 265–430)

Code read in full:

- `crates/fava-query/src/lib.rs` (459), `crates/fava-query/src/selection.rs` (76)
- `crates/fava-query-standard/src/lib.rs` (199)
- `crates/fava-state/src/lib.rs` (384)
- `crates/fava-event-cache/src/lib.rs` (93)
- `crates/fava-event-cache-memory/src/lib.rs` (223)
- All tests in those crates (`query_identity.rs`, `source_merge.rs`, `relay_authority.rs`, `multi_kind.rs`, `event_state.rs`)

Adjacent code read for consumer evidence: `crates/fava-observe/src/lib.rs` (1–200),
`crates/fava/src/query_source.rs`, `crates/fava/src/lib.rs` (15–40, 255–300, 395–440),
`crates/fava/src/relay.rs` (260–285), `crates/fava-ingest/src/lib.rs`,
`crates/fava-subscriptions/src/lib.rs:99–117`.

Searches actually run (all workspace `*.rs`): `ValueSet`, `CurrentAccount`, `tag_pubkeys`,
`newest_first`, `fn union|intersection|difference`, `Selection::`, `MaxAge`, `since|until`,
`state_slice`, `CacheMaintenance`, `CommittedCacheChange`, `Tombstone`, `RetractionCause`,
`AffectedEventState`, `EventStateDecision`, `StateSlice`, `SourceChangeSet`, `expire(`,
`\.admit(`, `\.events()`, `QueryEvidence|SourceEvidence|SourceKind::`.

---

## Part 1 — Enumeration of the `partial-spec-api-semantics.md` surface

Every constructor / combinator / source-selector the doc specifies, with verdict.

| # | Spec surface | Spec line | Verdict | Implementation |
|---|---|---|---|---|
| 1 | `events()` query root | :29 | PRESENT (as `Query::events()`) | `fava-query/src/selection.rs:25` |
| 2 | `.kind(k)` | :30 | PRESENT | `fava-query/src/selection.rs:35` |
| 3 | `.authors(literal)` | :80 | PRESENT | `fava-query/src/selection.rs:42` |
| 4 | `.authors([a, b])` | :81 | PRESENT | same |
| 5 | `.ids(...)` (implied by QUERY-001) | — | PRESENT | `fava-query/src/selection.rs:49` |
| 6 | exact tag-value axis | :10 §1 / QUERY-001 | PRESENT | `fava-query/src/selection.rs:61` |
| 7 | `ValueSet<T>` type | :18, :66 | **ABSENT** | no symbol in workspace |
| 8 | `.authors(reactive_pubkeys)` | :82 | **ABSENT** | `.authors` takes `IntoIterator<Item = PublicKey>` only |
| 9 | `tag_pubkeys("p")` projection | :50, :57 | **ABSENT** | no symbol |
| 10 | `CurrentAccount::pubkey()` reactive root | :47, :449 | **ABSENT** | no symbol |
| 11 | `a.union(b)` | :99 | **ABSENT** | `Query::union` specified at ARCHITECTURE.md:599, no impl |
| 12 | `a.intersection(b)` | :100 | **ABSENT** | — |
| 13 | `a.difference(b)` | :101 | **ABSENT** | — |
| 14 | `.freshness(Freshness::MaxAge(Duration))` | :112 | **ABSENT** | `Freshness` = `{CacheOnly, Live}` only, `fava-query/src/lib.rs:71`; no `.freshness()` setter, only `.cache_only()` at :157 |
| 15 | `Auto` acquisition is the default, no syntax | :26, :126 | PRESENT | `QueryAcquisition::Automatic` default, `fava-query/src/lib.rs:47` |
| 16 | `.from_relays([...])` | :141 | PRESENT | `fava-query/src/lib.rs:120` |
| 17 | `.only_from_relays([...])` | :168 | **DIVERGENT** — see `only-from-relays-local-shadow` | `fava-query/src/lib.rs:139`, `fava-query-standard/src/lib.rs:69–103` |
| 18 | source mode is part of query identity | :214 | PRESENT (structurally) but leaks — see finding | `QuerySourcePolicy` in `Query` Eq/Hash, `fava-query/src/lib.rs:88–105` |
| 19 | `EventRecord { event, relay_evidence, publication }` | :225 | PRESENT | `fava-query/src/lib.rs:339` |
| 20 | `RelayEvidence` naming actual serving relays | :234 | PRESENT (richer: session-keyed) | `fava-state/src/lib.rs:63` |
| 21 | local source: `EventCache` | :257 | PRESENT | `SourceKind::EventCache`, `fava-query/src/lib.rs:235` |
| 22 | local source: `WriteStore` | :258 | PRESENT | `SourceKind::WriteStore`, `fava-query/src/lib.rs:237` |
| 23 | local source: **live admitted relay events** | :259 | **ABSENT** — see `no-live-relay-query-source` | `SourceKind` has exactly two variants |
| 24 | `fava.observe(query).await?` | :277 | PRESENT | `fava/src/lib.rs` / `fava-observe` |
| 25 | `feed.current()` | :299 | PRESENT | `fava-observe/src/lib.rs:171` |
| 26 | `feed.changed().await` | :301 | PRESENT | `fava-observe/src/lib.rs:183` |
| 27 | `feed.close()` | :306 | PRESENT | `fava-observe` |
| 28 | `QuerySnapshot { revision, events: Arc<[EventRecord]>, evidence }` | :293 | PRESENT (shape) / **DIVERGENT** (evidence content) | `fava-query/src/lib.rs:407` |
| 29 | `.newest_first()` | :362, :380 | **ABSENT** (default only; inverse `oldest_first()` exists) | `fava-query/src/lib.rs:170` |
| 30 | `.limit(100)` | :363 | PRESENT | `fava-query/src/lib.rs:161` |
| 31 | no `watch::Receiver` in the public surface | :313 | PRESENT (conforming) | `Observation` wraps `watch` privately |
| 32 | `Row` is not the event-domain name | :226, §11.8 | PRESENT (conforming) | no `Row` type in workspace |
| 33 | `contact_list / followers_of / follows_of` | :471 | PRESENT (`fava-nip02`, out of area) | `fava-nip02/src/query.rs` |

Also absent against the higher authority that this doc refines:

| Spec surface | Authority | Verdict |
|---|---|---|
| `Selection::{Union, Intersection, Difference, Derived}` | ARCHITECTURE.md:578–587 | **ABSENT** — only `FilterSelection` exists (`fava-query/src/selection.rs:9`) |
| `Query::union(queries)` | ARCHITECTURE.md:599 | **ABSENT** |
| `since` / `until` window axes | QUERY-016 (:491) | **ABSENT** — no field in `FilterSelection`, no builder |
| settable relay access | QUERY-001 (:275), QUERY-007 (:372) | **ABSENT** — `Query.access` is private with a getter (`fava-query/src/lib.rs:97, :186`) and no setter; every query is permanently `RelayAccess::public()` |
| `QueryEvaluator::update(query, previous, sources, changed: &SourceChangeSet)` | ARCHITECTURE.md:646–658 | **ABSENT** — trait has `evaluate` only (`fava-query/src/lib.rs:438`) |
| `EventCache::state_slice(StateLookup)` | ARCHITECTURE.md:791 | **ABSENT** — replaced by `events() -> Vec<CachedEvent>` (`fava-event-cache/src/lib.rs:65`) |
| `EventCache::maintain(CacheMaintenance) -> CacheMaintenanceResult` | ARCHITECTURE.md:806 | **ABSENT** |
| `commit(...) -> CommittedCacheChange` | ARCHITECTURE.md:800 | **DIVERGENT** — returns `()` (`fava-event-cache/src/lib.rs:51`) |
| `CacheMutation::{Insert, Replace, MergeEvidence, Retract{cause}, RecordTombstone}` | ARCHITECTURE.md:391–402 | **DIVERGENT** — impl has `{Upsert, Retract(EventId)}` only (`fava-state/src/lib.rs:126`) |
| `EventStateDecision` / `AffectedEventState` | ARCHITECTURE.md:404 | **ABSENT** |
| `Tombstone` / `RetractionCause` | ARCHITECTURE.md:399, :401 | **ABSENT** (grep returned empty) |

---

## Findings

### only-from-relays-local-shadow — critical — ownership (source-policy leakage)

**authority**
`docs/spec/partial-spec-api-semantics.md:194` — "For an event already in the event cache to match this query, its provenance MUST include at least one relay in the specified set."
`docs/spec/partial-spec-api-semantics.md:200` — "An unpublished local event with no qualifying relay provenance MUST NOT appear."
`docs/spec/partial-spec-api-semantics.md:214` — "Two otherwise identical queries using different source modes MUST NOT accidentally share evidence or local-result visibility in a way that changes either query's results."
`docs/spec/partial-spec-api-semantics.md:264` — "`only_from_relays(...)` excludes a write-store-only event until it has qualifying relay provenance."

**implementation**
`crates/fava-query-standard/src/lib.rs:79-97`. Under `ResultAuthority::OnlyRelays`, records with `publication.is_some()` are collected into `local_by_coordinate` **regardless of relay evidence** (`:79-81`), and then at `:92-97` a qualifying per-relay winner is *overwritten* by the newer local record:

```rust
for ((_, coordinate), record) in &mut by_relay_coordinate {
    if let Some(local) = local_by_coordinate.get(coordinate)
        && record_is_newer(local, record)
    {
        *record = local.clone();
    }
}
```

`matches_authority` (`crates/fava-query-standard/src/lib.rs:189-194`) then drops that substituted record because it has no qualifying relay evidence. Net effect: the coordinate contributes **nothing**. The unpublished local event does not merely fail to appear — it **erases** the relay-qualified cached event that the query is explicitly asking for.

The existing test corpus encodes the wrong behavior rather than the spec:
`crates/fava-query-standard/tests/source_merge.rs:215` is literally named
`local_replacement_without_relay_evidence_shadows_qualified_cached_predecessor`
and asserts `overlaid.events.is_empty()` at `:236`.

**observable distinction**
An application opens `Query::events().kind(0).authors([me]).only_from_relays(["wss://a"])`.
Relay `a` served profile v1, so the query shows v1. The application then calls
`fava.publish(profile_v2)`. With no other change and no relay involvement, the
open query goes **empty**. The relay-only view of the author's profile
disappears because of a purely local, unpublished write — exactly the
local-result-visibility leakage `:214` forbids. Correct behavior is that the
query keeps showing v1 until relay `a` actually serves v2.

**proposed falsifier**
```rust
// crates/fava-query-standard/tests/relay_authority.rs
#[test]
fn unpublished_local_event_cannot_hide_a_relay_qualified_predecessor() {
    let cache  = cache(vec![(predecessor_v1.clone(), evidence(&["wss://a.example"]))]);
    let writes = snapshot(SourceKind::WriteStore, vec![local_unsigned(successor_v2)]);
    let query  = Query::events().kind(Kind::Metadata)
        .only_from_relays([relay("wss://a.example")]).unwrap();
    let out = StandardQueryEvaluator.evaluate(&query, &[cache, writes]).unwrap();
    assert_eq!(result_ids(&out), BTreeSet::from([predecessor_v1.id]));
}
```
Fails today (`out.events` is empty). Passes once `OnlyRelays` excludes
non-qualifying local records from candidate selection instead of from the
final filter.

**confidence** confirmed

---

### query-evidence-cannot-name-relays — critical — behavioral proof

**authority**
`docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:422` (QUERY-010) — "Timeout, disconnect, retry exhaustion, silence, local cancellation, and relay refusal MUST remain distinct and MUST NOT be reinterpreted as EOSE or emptiness."
`…:401` (QUERY-008) — "Per-branch and per-relay evidence MUST remain associated with the branch and source that produced it." Acceptance at `:403`: "overlapping branches deliver one event record while preserving each branch's separate EOSE/error/auth state."
`…:414` (QUERY-009) — "An empty result from one relay/request means only that the source returned no matching events for that request."
`docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md:390` (M2 exit) — "Empty + EOSE differs from silence, failure, auth-required, and CLOSED."

**implementation**
`crates/fava-query/src/lib.rs:383-397`:

```rust
pub struct QueryEvidence { pub sources: Vec<SourceEvidence> }
pub struct SourceEvidence { pub kind: SourceKind, pub revision: SourceRevision, pub status: SourceStatus }
```

`SourceKind` (`:233-238`) has exactly two variants, `EventCache` and `WriteStore`.
`SourceStatus` (`:281-287`) has exactly two variants, `Open` and `Closed`.
`QuerySnapshot.evidence` (`:413`) is the only evidence surface an application receives.

The complete set of required evidence facts that are **unrepresentable** in
`QueryEvidence` / `SourceEvidence` today:

1. Which relays the query is actually asking (route state / desired plan).
2. Per-relay EOSE for the exact current request identity (QUERY-010).
3. Per-relay failure, timeout, silence, retry exhaustion (QUERY-010).
4. Per-relay `CLOSED` / refusal text (QUERY-010, M2 exit gate).
5. Per-relay auth-required / NIP-42 challenge state (QUERY-008 acceptance).
6. Subscription shortfall (a planner that could not carry all demand).
7. Desired-plan revision, so a late completion can be recognized as stale (QUERY-010: "Reopening dropped demand MUST use fresh request identity").
8. Shared-work ownership — which observations share a relay session/subscription (QUERY-002 acceptance).
9. Provider-operation / relay-connection generation identity (ownership ledger, ARCHITECTURE.md:2984 "Relay connection generation").
10. Route-contribution arrival/withdrawal (QUERY-014).
11. Bounded-loss/coalescing shortfall (QUERY-011: "Any bounded loss MUST be explicit and typed") — reported today only through the out-of-band `Observer::with_coalescing` callback (`crates/fava-observe/src/lib.rs:38`), never in the snapshot.
12. Cause of a source's termination — `QuerySourceClosed` (`crates/fava-query/src/lib.rs:335`) is a unit struct with no cause, so ARCHITECTURE.md:724 merge rule 5 ("one source's failure becomes scoped evidence") degrades to an unattributed `SourceStatus::Closed`.

`fava-diagnostics` records EOSE frames (`crates/fava-diagnostics/src/lib.rs:30`),
but that is an engine-wide diagnostic surface, not evidence scoped to the
observation, and QUERY-008/009/010 place these facts on the query result.

**observable distinction**
Two runs of the same live query return `snapshot.events.is_empty() == true` and
byte-identical `snapshot.evidence`. In run A the relay sent `EOSE` with no
matching events; in run B the relay was unreachable the whole time. The
application cannot tell "the relay says it has nothing" from "we never got an
answer" through any public query surface — which is the exact confusion
QUERY-009/010 exist to prevent.

**proposed falsifier**
```rust
// crates/fava/tests/query_evidence.rs
#[tokio::test]
async fn empty_with_eose_is_distinguishable_from_unreachable_relay() {
    let served = observe_against(relay_that_eoses_empty()).await.current();
    let dark   = observe_against(unreachable_relay()).await.current();
    assert_ne!(served.evidence, dark.evidence);
    assert!(served.evidence.relay(&url).unwrap().stored_events_complete());
}
```
Does not compile today (no relay-scoped accessor on `QueryEvidence`); once it
compiles the two evidences must differ.

**confidence** confirmed

---

### no-live-relay-query-source — critical — ownership

**authority**
`docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:344-347` (QUERY-005) — the merge MUST "accept admitted live relay occurrences as current query input even when the selected event cache does not retain them"; `:348` — "Caching is not a prerequisite for delivering a verified live event to an already-open query." Acceptance at `:350`: "With a null event cache, a verified live event still reaches the open query but is absent from a later newly opened query."
`docs/spec/partial-spec-api-semantics.md:255-259` — "At minimum: `EventCache` / `WriteStore` / **live admitted relay events**."

**implementation**
`crates/fava-query/src/lib.rs:233-238` — `SourceKind` is `{EventCache, WriteStore}`. There is no third contribution role, so the merge model has no way to express a live relay occurrence that no store retains.
`crates/fava-observe/src/lib.rs:24-28` — `Observer::new` takes exactly `event_cache` and `write_store`.
`crates/fava-ingest/src/lib.rs:52-57` — the only path from a verified relay event to a query is `cache.admit(...)`.
`crates/fava/src/relay.rs:269-279` — if `admit` returns `Err`, the event is written to diagnostics and dropped; it never reaches any open observation.

**observable distinction**
Assemble Fava with `MemoryEventCache::bounded(NonZeroUsize::new(1))`, seed one
event so the cache is full, open a live query, then have the relay serve a
second matching event. The event is verified, on-filter, and attributed — and
the application never sees it. Under QUERY-005 the open query must receive it
regardless of retention. The same test with a hypothetical null cache shows
zero events for the whole session.

**proposed falsifier**
```rust
// crates/fava/tests/live_without_retention.rs
#[tokio::test]
async fn verified_live_event_reaches_open_query_when_cache_cannot_retain_it() {
    let fava = fava_with(MemoryEventCache::bounded(one())); // capacity 1, already full
    let mut feed = fava.observe(q.clone()).await.unwrap();
    relay.serve(second_matching_event.clone()).await;
    let snap = feed.changed().await.unwrap();
    assert!(snap.events.iter().any(|r| r.id() == second_matching_event.id));
}
```

**confidence** confirmed

---

### deletion-refused-at-capacity — critical — failure isolation

**authority**
`docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:594` (EVENT-006) — "When Fava ingests or locally publishes a valid kind:5 deletion, it MUST apply it only to targets the author is permitted to delete and retract affected event records from current queries."
`…:580` (EVENT-005) — a bounded cache "MUST NOT selectively retain mutually inconsistent positive and negative facts. For example, it cannot retain a stale event while forgetting a retained tombstone that invalidates it."
`docs/spec/ARCHITECTURE.md:830` — "explicit eviction or capacity shortfall rather than silent semantic corruption."

**implementation**
`crates/fava-state/src/lib.rs:225-233` emits the deletion batch **upsert-first**:

```rust
let mut mutations = vec![CacheMutation::Upsert(incoming.clone())];
mutations.extend(current.iter().filter(...).map(|known| CacheMutation::Retract(known.event.id)));
```

`crates/fava-event-cache-memory/src/lib.rs:84-89` refuses the whole batch when the map is at capacity:

```rust
if next.events.len() == self.capacity.get() {
    return Err(EventCacheError::Refused(format!("bounded event cache capacity {} reached", self.capacity)));
}
```

Because `commit` is all-or-nothing (`crates/fava-event-cache-memory/src/lib.rs:68`, `:101`),
a full bounded cache **cannot apply any deletion**: the retraction of the
deleted event is refused together with the insertion of the kind-5 event. The
deleted event remains visible in every open query indefinitely, and no
tombstone is recorded (`Tombstone`/`RetractionCause` do not exist — grep
returned empty across the workspace).

There is also no eviction at all in `MemoryEventCache`; once full it refuses
every new event forever, which makes this state reachable and permanent rather
than transient.

**observable distinction**
Fill a bounded cache to capacity, then have the author publish a valid kind-5
deletion of one of the retained events. The application's open query keeps
showing the deleted event. With capacity+1 the same deletion works. Deletion
enforcement becomes a function of cache fullness.

**proposed falsifier**
```rust
// crates/fava-event-cache-memory/tests/deletion_at_capacity.rs
#[test]
fn valid_deletion_applies_even_when_the_bounded_cache_is_full() {
    let cache = MemoryEventCache::bounded(NonZeroUsize::new(2).unwrap());
    cache.admit(cached(note_a.clone()), now).unwrap();
    cache.admit(cached(note_b.clone()), now).unwrap();      // now full
    cache.admit(cached(deletion_of(&note_a)), now).unwrap(); // refused today
    assert!(cache.event(note_a.id).unwrap().is_none());
}
```

**confidence** confirmed

---

### expiry-is-never-swept — critical — ownership (no lifecycle owner)

**authority**
`docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:598` (EVENT-006) — "Expiration MUST retract events when their expiry becomes due."
`…:358` (QUERY-006) — a live query MUST update for "expiry".
`…:471` (QUERY-013A) — "Opening one query MUST NOT trigger unrelated expiry sweeps" (i.e. the sweep exists and belongs to someone other than query open).

**implementation**
`crates/fava-event-cache/src/lib.rs:37` defines `EventCache::expire(now)`. A
workspace-wide grep for `expire(` returns only its own definition plus
`crates/fava/tests/local_source_merge.rs:312` and `apps/canary/src/local.rs:188`.
No production component ever calls it. `admission_mutations`
(`crates/fava-state/src/lib.rs:220`) refuses an already-expired *incoming*
event, but a retained event whose `expiration` tag falls due later is never
retracted.

**observable distinction**
Admit an event with `["expiration", now+2]`. Open a live query matching it.
Two seconds later the event is due; the query still shows it, forever, with no
further change delivered. Under EVENT-006/QUERY-006 the query must receive an
update retracting it.

**proposed falsifier**
```rust
// crates/fava/tests/expiry.rs
#[tokio::test]
async fn expiring_event_is_retracted_from_an_open_query_without_app_action() {
    let mut feed = fava.observe(q).await.unwrap();
    assert_eq!(feed.current().events.len(), 1);
    tokio::time::advance(Duration::from_secs(3)).await;
    let snap = feed.changed().await.unwrap();
    assert!(snap.events.is_empty());
}
```

**confidence** confirmed

---

### event-cache-contract-forces-full-revision — major — replaceability

**authority**
`docs/spec/ARCHITECTURE.md:791-795` — the baseline `EventCache` contract is
`fn state_slice(&self, key: StateLookup) -> Result<StateSlice, EventCacheError>`.
`docs/spec/ARCHITECTURE.md:452` — "This allows a pure read/decide/commit boundary without requiring cache implementations to duplicate Nostr rules."
`docs/spec/ARCHITECTURE.md:658` — "A source implementation owns efficient access to its own retained facts."
`AGENTS.md` gate 5 (boundedness), gate 3 (replaceability).

**implementation**
`crates/fava-event-cache/src/lib.rs:65` —
`fn events(&self) -> Result<Vec<CachedEvent>, EventCacheError>;` — every
implementation must be able to apply its **entire** retained corpus into
one `Vec`. The default `admit` (`:23`) and `expire` (`:38`) both call it on
every single admitted relay event:

```rust
let mutations = admission_mutations(&self.events()?, event, now);
```

`state_slice`, `StateLookup`, `StateSlice`, `CacheMaintenance`,
`CacheMaintenanceResult`, and `CommittedCacheChange` do not exist anywhere in
the workspace (grep empty). `commit` returns `()` rather than
`CommittedCacheChange` (`crates/fava-event-cache/src/lib.rs:51`), so the atomic
change is not a value any consumer can observe.

Consequence in the shipped implementation:
`crates/fava-event-cache-memory/src/lib.rs:68` deep-clones the whole
`BTreeMap<EventId, CachedEvent>` on every commit, and
`crates/fava-event-cache-memory/src/lib.rs:47-59` builds a full
`Vec<SourceEvent>` of the entire cache for every snapshot. Each observation
then deep-clones that whole snapshot per change
(`crates/fava-event-cache-memory/src/lib.rs:152`). Per admitted relay event the
work is O(retained events) × O(open observations), all driven by
peer-controlled event volume.

**observable distinction**
A competing `EventCache` backed by redb/SQLite with 5M retained events cannot
implement the contract without loading 5M events into memory per admitted
event. It is not replaceable at the specified scale. Observably: admit rate
against `MemoryEventCache` degrades quadratically with retained count, and a
persistent implementation cannot pass the same conformance suite within any
memory bound.

**proposed falsifier**
```rust
// crates/fava-event-cache/tests/contract_shape.rs
#[test]
fn admission_reads_only_the_state_slice_it_needs() {
    let cache = CountingCache::default(); // counts events() / state_slice() calls
    for e in ten_thousand_unrelated_events() { cache.admit(e, now).unwrap(); }
    assert_eq!(cache.full_scans(), 0);
}
```
Fails today: 10 000 full scans.

**confidence** confirmed

---

### state-mutation-vocabulary-loses-cause — major — behavioral proof

**authority**
`docs/spec/ARCHITECTURE.md:391-405` specifies
`CacheMutation::{Insert, Replace{coordinate, previous, current}, MergeEvidence{event_id, evidence}, Retract{event_id, cause: RetractionCause}, RecordTombstone(Tombstone)}`
and `EventStateDecision { mutations, affected: AffectedEventState }`.
`docs/spec/ARCHITECTURE.md:429` — owned semantics include "NIP-09 deletion authorization and **tombstones**", "prevention of resurrection", and "**exact affected-state descriptions used to invalidate queries**".
`docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:580` (EVENT-005) — "it cannot retain a stale event while forgetting a retained tombstone that invalidates it."

**implementation**
`crates/fava-state/src/lib.rs:126-132`:
```rust
pub enum CacheMutation { Upsert(CachedEvent), Retract(EventId) }
```
No cause, no tombstone, no `EventStateDecision`, no `AffectedEventState`.
Deletion suppression is emergent rather than recorded: `deletion_applies`
(`crates/fava-state/src/lib.rs:344`) can only suppress a re-arriving event while
the kind-5 event **itself is still retained in `current`**
(`crates/fava-state/src/lib.rs:221-223`). Nothing pins the kind-5 event:
it competes for the same bounded capacity as any other event, and
`CacheMutation::Retract(id)` in `MemoryEventCache::commit`
(`crates/fava-event-cache-memory/src/lib.rs:96-98`) will remove it if any
future decision names it. Once the kind-5 event is gone, the deleted event can
be re-admitted — resurrection, which ARCHITECTURE.md:430 names as owned
semantics.

**observable distinction**
Admit event `E`, admit a valid kind-5 deleting `E` (query correctly loses `E`).
Cause the kind-5 event to leave the cache. Re-serve `E` from the relay. `E`
reappears in the query. Under EVENT-005/EVENT-006 the deletion must remain
authoritative for as long as the implementation's declared tombstone profile
says it does.

Second distinction: because `Retract` has no cause, an application receiving a
new `QuerySnapshot` where a record vanished cannot tell deletion from expiry
from eviction from a replaceable-winner change — which is the whole point of
QUERY-006's enumerated update reasons.

**proposed falsifier**
```rust
// crates/fava-state/tests/event_state.rs
#[test]
fn deletion_suppression_survives_loss_of_the_deletion_event() {
    let mut current = vec![cached(note.clone())];
    apply_admission(&mut current, cached(deletion_of(&note)));
    current.retain(|c| c.event.kind != Kind::EventDeletion); // tombstone must survive this
    assert!(!apply_admission(&mut current, cached(note.clone())));
}
```
Fails today (`note` is re-admitted).

**confidence** confirmed

---

### derived-and-window-query-surface-absent — major — behavioral proof

**authority**
`docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:277-283` (QUERY-001) — the query language MUST support "exact event and address coordinates; a reactive current-account input; values projected from another query; union, intersection, and difference over derived values; and independently configured nested queries."
`…:285` — "An empty derived set MUST mean 'match nothing.' It MUST NOT erase a filter axis and widen the query."
`…:360` (QUERY-006) — "When a derived dependency shrinks, records that matched only through the removed values MUST be retracted from the same open query."
`…:493` (QUERY-016) — "Fava MUST preserve application-supplied `since`, `until`, and limit semantics."
`docs/spec/ARCHITECTURE.md:578-587` — `Selection::{Filter, Union, Intersection, Difference, Derived}`; `:599` — `Query::union`.
`docs/spec/partial-spec-api-semantics.md:45-52, :99-101` — `ValueSet<T>`, `tag_pubkeys`, `union/intersection/difference`.

**implementation**
`crates/fava-query/src/selection.rs:9-21` — `FilterSelection { ids, authors, kinds, tag_values }`. No `Selection` enum, no derived node, no `since`, no `until`.
`crates/fava-query/src/lib.rs:88-105` — `Query { selection: FilterSelection, ... }` is a single flat filter; no branch vector.
Workspace greps for `ValueSet`, `CurrentAccount`, `tag_pubkeys`, `Selection::`, `fn union|intersection|difference` all returned empty.

`docs/spec/partial-spec-api-semantics.md:566` marks generic `ValueSet<T>`
composition "a separate unpromised boundary" — but QUERY-001 and
ARCHITECTURE.md:578 are higher authorities and do promise the derived algebra.
The shipped substitute
(`fava_nip02::follows_of(snapshot) -> Vec<PublicKey>`, `crates/fava-nip02/src/query.rs`)
forces exactly what `partial-spec-api-semantics.md:87-92` forbids — the
application holds the expanded set, diffs it, and reopens the outer query.

`since`/`until` absence is the sharpest instance: QUERY-016 is a MUST with no
partial-spec escape hatch, and `demand_for_query`
(`crates/fava-subscriptions/src/lib.rs:99-117`) has no branch that could emit
them onto the wire filter.

**observable distinction**
An application cannot express "kind-1 notes from the last hour"
(`.since(now - 3600)`) at all, nor "articles by people my follows muted", nor
"authors A minus authors B". `.limit(100)` is the only bound, and it is a
result bound, not a window — so an all-time query cannot be narrowed.

**proposed falsifier**
```rust
// crates/fava-query/tests/query_windows.rs
#[test]
fn app_authored_time_window_is_exact() {
    let q = Query::events().kind(Kind::TextNote)
        .since(Timestamp::from(100)).until(Timestamp::from(200));
    let out = StandardQueryEvaluator.evaluate(&q, &[cache_with(t_50, t_150, t_250)]).unwrap();
    assert_eq!(result_ids(&out), BTreeSet::from([t_150.id]));
}
```
Does not compile today.

**confidence** confirmed

---

### query-source-open-window-is-untyped — major — boundedness

**authority**
`docs/spec/ARCHITECTURE.md:1038` — "The observation owner buffers source changes while all initial source snapshots are being acquired, then calculates one merged initial `QuerySnapshot`."
`docs/spec/ARCHITECTURE.md:636` — "the initial snapshot and all later changes form one **gapless** local sequence for that source."
`docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:436` (QUERY-011) — "Any bounded loss MUST be explicit and typed. Observation memory MUST remain bounded even when an application is slow."

**implementation**
`crates/fava-query/src/lib.rs:290-292` documents `OpenedQuerySource` as "Initial source snapshot and its **gapless** later sequence", but neither the contract nor the shipped provider can deliver a gapless *revision* sequence:
`crates/fava-event-cache-memory/src/lib.rs:139-149` is a `tokio::sync::watch` receiver — every intermediate `SourceRevision` between two `next_change()` awaits is silently dropped and `SourceSnapshot.revision` jumps.

Because the snapshots are complete latest state, the *state* is not lost. But
`SourceSnapshot` (`crates/fava-query/src/lib.rs:255-267`) carries no field for
"revisions superseded" or "contributions elided", and neither does
`SourceEvidence` (`:392-399`). So the coalescing is real and undeclared. The
only report path is the out-of-band `Observer::with_coalescing` callback
(`crates/fava-observe/src/lib.rs:36-41`), which is a `dyn Fn(u64)` closure
configured at assembly time and is not reachable from `QuerySnapshot`.

On the open window itself: `MemoryEventCache::open`
(`crates/fava-event-cache-memory/src/lib.rs:126-129`) calls `subscribe()` then
`borrow()`. A commit landing between the two produces an initial snapshot at
revision *N+1* while the receiver is still marked at *N*, so the very next
`next_change()` returns the same revision again. No change is lost, but the
contract has no rule saying whether a redelivered revision is legal, so a
different provider is free to instead lose it. **Suspected** on that half.

**observable distinction**
An application receives `snapshot.evidence.sources[0].revision` jumping from 3
to 17 with no typed statement that 4..16 were superseded. QUERY-011 requires
bounded loss to be explicit and typed, on the delivered value.

**proposed falsifier**
```rust
// crates/fava-query/tests/coalescing_is_typed.rs
#[tokio::test]
async fn superseded_source_revisions_are_declared_in_the_snapshot() {
    for e in burst_of_20() { cache.admit(e, now).unwrap(); }
    let snap = feed.changed().await.unwrap();
    assert!(snap.evidence.sources[0].superseded_revisions() > 0);
}
```
Does not compile today (no such accessor).

**confidence** confirmed (undeclared coalescing) / suspected (open-window redelivery rule)

---

### admit-is-a-nonatomic-read-decide-commit — major — failure isolation

**authority**
`docs/spec/ARCHITECTURE.md:452` — "The Fava instance has one serialized event-state writer. This allows a pure read/decide/commit boundary."
`docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:706` (EVENT-014) — "For one admitted relay event or one cache-owned removal, the event cache MUST expose the event value, relay evidence, replacement/address consequences, deletion/expiry consequences, indexes, and emitted query-source change as **one coherent mutation**."

**implementation**
`crates/fava-event-cache/src/lib.rs:19-29` — `admit` is a **default method on the public trait**:

```rust
let mutations = admission_mutations(&self.events()?, event, now);
if mutations.is_empty() { return Ok(false); }
self.commit(mutations)?;
```

`events()` and `commit()` each take the mutex independently
(`crates/fava-event-cache-memory/src/lib.rs:66`, `:118`). Nothing in the
contract, the trait, or `MemoryEventCache` serializes the read/decide/commit
triple. The spec's "one serialized event-state writer" does not exist anywhere:
`crates/fava-ingest/src/lib.rs:52` calls `cache.admit(...)` directly from the
per-relay-session task in `crates/fava/src/relay.rs:269`, so N concurrent relay
sessions run N concurrent read-decide-commit cycles against one cache.

**observable distinction**
Two relays serve two different replaceable events at the same coordinate
concurrently. Both `admit` calls read the same pre-state, both decide "I am the
winner, retract the other", and the second commit retracts the event the first
just installed. The cache ends with **zero** events at that coordinate; the
open query loses the author's profile entirely. Serially the same input yields
exactly one winner. EVENT-002's "two provider implementations produce the same
event view for the same admitted event/source sequence" fails against itself
under concurrency.

**proposed falsifier**
```rust
// crates/fava-event-cache-memory/tests/concurrent_admission.rs
#[tokio::test(flavor = "multi_thread")]
async fn concurrent_replaceable_admissions_leave_exactly_one_winner() {
    let (a, b) = (profile_v2.clone(), profile_v3.clone());
    tokio::join!(spawn_admit(cache.clone(), a), spawn_admit(cache.clone(), b));
    assert_eq!(cache.events().unwrap().len(), 1);
}
```

**confidence** confirmed (the race is unguarded by construction); the exact
interleaving above is **suspected** pending a stress run.

---

### query-relay-access-is-unsettable — minor — behavioral proof

**authority**
`docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:275` (QUERY-001) — "A live query MUST state its selection, routing mode, source authority, **relay access**, freshness policy, cache-use policy, and result/acquisition bounds."
`…:374` (QUERY-007) — an inner and an outer query MUST each retain their own "relay access".

**implementation**
`crates/fava-query/src/lib.rs:97` declares the private field `access: RelayAccess`, `:111` initializes it to `RelayAccess::public()`, and `:186` exposes a getter. There is **no** builder method anywhere in `fava-query` that sets it (grep for `access` in `crates/fava-query/src` returns only those three sites). `crates/fava/src/live.rs:33` consumes it to build the `RelaySessionKey`, so the field is load-bearing — every query in the system is permanently pinned to public access.

**observable distinction**
An application cannot open a query under an authenticated relay access; a
NIP-42-gated relay is unreachable by any query, and QUERY-007's "an inner and
outer query each retain their own relay access" is untestable because both are
always `public`.

**proposed falsifier**
```rust
// crates/fava-query/tests/query_identity.rs
#[test]
fn relay_access_is_part_of_query_identity() {
    let a = Query::events().kind(Kind::TextNote).relay_access(RelayAccess::named("alice"));
    let b = Query::events().kind(Kind::TextNote);
    assert_ne!(a, b);
}
```
Does not compile today.

**confidence** confirmed

---

### fava-state-is-a-shared-primitive-hub — minor — dependency direction

**authority**
`AGENTS.md` Rust conventions — "Keep shared values in the crate that owns their meaning; do not create a generic common bucket."
`docs/spec/ARCHITECTURE.md:357` — `fava-state` responsibility is "**deterministic semantics for signed event state learned from relays**."
`docs/spec/ARCHITECTURE.md:104` — "cached event state and relay evidence live in `fava-state`."

**implementation**
`crates/fava-state/src/lib.rs:8-10` re-exports `nostr::{Event, EventId, Kind, Tag, PublicKey, RelayUrl, Timestamp}`. 28 of the 36 workspace crates declare a `fava-state` dependency. Several depend on it for nothing to do with signed event state:

- `crates/fava-transport/src/lib.rs:7` — `use fava_state::RelaySessionKey;`
- `crates/fava-subscriptions/src/lib.rs:6` — `use fava_state::RelaySessionKey;`
- `crates/fava-publisher/src/lib.rs:7` — `use fava_state::RelaySessionKey;`
- `crates/fava-diagnostics/src/lib.rs:7` — `use fava_state::RelaySessionKey;`
- `crates/fava-routing/src/lib.rs:9` — `use fava_state::{PublicKey, RelayAccess, RelaySessionKey, RelayUrl};` (two of those four are bare `nostr` primitives re-exported through `fava-state`)

`docs/internals/vocabulary.toml:370-376` does approve `RelayAccess`/`RelaySessionKey` under `owner = "fava-state"`, so this is not an unapproved vocabulary change. It is a cohesion drift: the relay-session authorization identity is transport/session vocabulary, not signed-event-state vocabulary, and `fava-state` is additionally being used as a `nostr` re-export hub.

**observable distinction**
Replacing the event-state rule engine (the crate's stated responsibility)
requires transport, subscriptions, publisher, diagnostics, and routing to be
recompiled and, if the type moved, edited — even though none of them use any
event-state rule.

**proposed falsifier**
Not a runtime falsifier. A structural gate:
```
tools/check_vocabulary.py --owner-cohesion
  asserts that no crate depends on fava-state without using
  CachedEvent | CacheMutation | EventCoordinate | RelayEvidence | admission_mutations
```
Fails today for `fava-transport`, `fava-subscriptions`, `fava-publisher`, `fava-diagnostics`.

**confidence** confirmed (as a cohesion finding, not a vocabulary violation)

---

### evidence-types-not-nameable-from-the-facade — minor — behavioral proof

**authority**
`AGENTS.md` gate 6 — "public promises have falsifiable evidence at the owning component, through the real public path."
`docs/spec/partial-spec-api-semantics.md:293-297` — `QuerySnapshot.evidence: QueryEvidence` is part of the application-facing surface.

**implementation**
`crates/fava/src/lib.rs:21-23` re-exports only
`EventRecord, Freshness, Query, QueryRevision, QuerySnapshot, ResultAuthority, SingleLetterTag`.
`QueryEvidence`, `SourceEvidence`, `SourceKind`, `SourceStatus`, `SourceRevision`,
`QueryError`, `QueryAcquisition`, `QuerySourcePolicy`, `QueryOrdering`, and
`FilterSelection` are not re-exported. `Query::from_relays` returns
`Result<Query, QueryError>` (`crates/fava-query/src/lib.rs:120`), so a facade-only
application cannot name the error type it must match on, and cannot write a
function signature over `snapshot.evidence`.

**observable distinction**
`fn render(e: &fava::QueryEvidence)` does not compile; the application must add
a direct `fava-query` dependency to name a type reachable through a `fava`
type's public field.

**proposed falsifier**
```rust
// crates/fava/tests/public_surface.rs
#[test]
fn every_public_snapshot_field_type_is_nameable_from_the_facade() {
    fn _sig(_: &fava::QueryEvidence, _: fava::SourceKind, _: fava::QueryError) {}
}
```
Does not compile today.

**confidence** confirmed

---

## Conforming (verified, not merely unexamined)

- **No unpublished local event can reach the event cache.** `CachedEvent`
  (`crates/fava-state/src/lib.rs:117-122`) holds a `nostr::Event` — an unsigned
  `EventValue::Unsigned` is not representable in a `CacheMutation`.
  `MemoryEventCache::commit` additionally re-verifies every upserted signature
  (`crates/fava-event-cache-memory/src/lib.rs:73-76`) after `EventCache::admit`
  already verified it (`crates/fava-event-cache/src/lib.rs:20-22`). The only
  workspace caller of `admit` is `crates/fava-ingest/src/lib.rs:52`, which is
  fed exclusively from attributed relay frames. The AGENTS.md rule holds.

- **Query evaluation is pure and total.** `StandardQueryEvaluator::evaluate`
  (`crates/fava-query-standard/src/lib.rs:17-52`) contains no `unwrap`,
  `expect`, `panic!`, indexing, slicing, or arithmetic. `truncate` is
  saturating; all fallible conversions go through
  `QueryEvaluationError::MissingEventId`. `fava-state`'s decision functions
  (`admission_mutations`, `expiration_mutations`, `event_coordinate`,
  `candidate_is_newer`) are likewise panic-free —
  `values.get(1).cloned().unwrap_or_default()`
  (`crates/fava-state/src/lib.rs:174`) is the only near-miss and is total.
  The single `expect` in scope is
  `NonZeroUsize::new(10_000).expect("constant is non-zero")`
  (`crates/fava-event-cache-memory/src/lib.rs:31`), which is infallible.
  Mutex poisoning is converted to `EventCacheError::Refused` at all four lock
  sites rather than unwrapped.

- **Acquisition and result authority are structurally separate.**
  `QuerySourcePolicy { acquisition, authority }`
  (`crates/fava-query/src/lib.rs:38-43`) keeps them as independent fields, both
  inside `Query`'s derived `Eq`/`Hash`, so `.from_relays(R)` and
  `.only_from_relays(R)` are distinct query identities. Proved by
  `crates/fava-query/tests/query_identity.rs:38` and
  `crates/fava-query-standard/tests/source_merge.rs:245`. This satisfies
  `partial-spec-api-semantics.md:207-212` at the *identity* level; the
  *evaluation* level is the `only-from-relays-local-shadow` finding above.

- **Construction-time refusal is real.** `from_relays([])` →
  `QueryError::EmptyExplicitRelays`, `limit(0)` → `QueryError::ZeroLimit`
  (`crates/fava-query/src/lib.rs:120, :161`), proved at
  `crates/fava-query/tests/query_identity.rs:52`. Matches QUERY-001's
  "refused before opening relay work" for the axes that exist.

- **Empty literal sets mean "match nothing", not "widen".** Documented at
  `crates/fava-query/src/selection.rs:12-20` and proved by
  `crates/fava-query-standard/tests/source_merge.rs:542`
  (`present_empty_literal_tag_axis_matches_nothing`). Satisfies
  QUERY-001:285 for the literal case (the derived case does not exist).

- **Query identity is canonical under construction order.** Repeated
  `.kind()` unions into a `BTreeSet` and relay sets are `BTreeSet`s, so
  ordering and duplication do not change `Eq`/`Hash`
  (`crates/fava-query/tests/query_identity.rs:19, :37`). Satisfies QUERY-002's
  identity half. (The work-sharing half is the known-good baseline's finding
  in `fava-observe`, not re-reported here.)

- **Same-event-id merge across sources is correct.**
  `merge_contribution` (`crates/fava-query-standard/src/lib.rs:118-165`) keys on
  event id, merges `RelayEvidence` additively, upgrades unsigned→signed, and
  refuses conflicting publication evidence with a typed
  `QueryEvaluationError::Refused`. Satisfies EVENT-009 and ARCHITECTURE.md:719
  merge rules 1 and 4. Proved at
  `crates/fava-query-standard/tests/source_merge.rs:120`.

- **`RelayEvidence` credits only actual service.** `RelayEvidence::one`
  (`crates/fava-state/src/lib.rs:68`) requires a concrete `RelaySessionKey`;
  `merge` (`:79`) never fabricates a key and keeps the earliest `observed_at`
  per session. No code path adds a queried-but-silent relay. Satisfies
  EVENT-003.

- **No `Row` type anywhere.** Grep confirms `partial-spec-api-semantics.md`
  design rule 8 holds.

- **No unapproved private lifecycle owner in scope.** The only private nominal
  types in the five crates are `CacheState`
  (`crates/fava-event-cache-memory/src/lib.rs:25`, a plain data record) and
  `WatchChanges` (`:134`, the provider's implementation of the *approved*
  public `SourceChanges` contract). Neither is an unapproved vocabulary noun in
  the way `fava::OpenedRelay` is — `WatchChanges` owns nothing beyond the
  lifecycle the contract already names.

- **Per-relay replaceable winners under `only_from_relays` are intentional,
  not a bug.** `coordinate_winners`
  (`crates/fava-query-standard/src/lib.rs:69-103`) unions one winner per relay,
  which is what `partial-spec-api-semantics.md:611-616` requires for
  multi-host simple groups ("Several hosts do not become one protocol
  authority… explicit `metadata_differ`"). Conforming.

---

## Open questions

1. **Whole-query limit lowered to a per-relay wire limit.**
   `crates/fava-subscriptions/src/lib.rs:113-115` copies `Query::result_limit`
   into every relay's `Filter::limit`. QUERY-008:399 says the bound "applies to
   the combined result", and QUERY-016:495 forbids reinterpreting app-authored
   bounds. Reinterpreting a *result* bound as an *acquisition* bound per relay
   is arguably narrowing. `fava-subscriptions` is outside my scope — flagging
   for whoever owns it.

2. **Is `MemoryEventCache`'s permanent capacity refusal a conforming
   "Bounded memory" profile?** ARCHITECTURE.md:846 defines the profile as
   "current-process retention remains within a declared limit" and :830 allows
   "explicit eviction **or** capacity shortfall". A cache that refuses every
   admission forever once full satisfies the letter. EVENT-004:566 requires
   the profile to document "eviction behavior" — nothing in the workspace
   declares this one. Needs a product-profile decision, not just a code fix.

3. **Who should own the expiry sweep?** QUERY-013A:471 says query open must not
   trigger it, and the ownership ledger (ARCHITECTURE.md:2960-3010) names no
   owner for periodic cache maintenance. `fava-runtime` does not exist. This
   blocks a clean fix for `expiry-is-never-swept`.

4. **Does the `SourceChanges` contract permit a redelivered revision?**
   `MemoryEventCache::open` can hand back initial revision *N+1* while the
   receiver is still at *N*, causing the first `next_change()` to redeliver it.
   Harmless for full-state snapshots, but the contract is silent, so a
   different provider could legally *lose* it instead. Needs a stated rule in
   `crates/fava-query/src/lib.rs:290`'s doc comment.
> Historical audit record. Superseded by STATE-ARCH-1; not current implementation guidance.
