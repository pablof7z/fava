# Phase 07.8 Research: MaxAge, Source Coverage, and Declarative Router Inputs

**Phase:** 07.8 — Independent correctness defects
**Researched against:** HEAD `44ea2d4fe2ce6acc7c828d378dd1e40454877ce1` on 2026-08-24
**Status:** Ready for replacement planning, with one mandatory Pablo vocabulary checkpoint before implementation

## User Constraints

The following is copied verbatim from the locked current phase context. It supersedes the previous `RESEARCH.md` and Plans 02–05.

DATA_R8T4V2NK_START
## Decision taken by Pablo — use the ordinary query model

The outbox router needs ordinary Fava query results for authors' NIP-65 relay
lists. That need does not justify a router-owned cache, a router-specific
observation, or an imperative query-opening capability.

The corrected behavior is:

1. The router declares a bounded ordinary `Query` for the complete requested
   author set and its configured indexers.
2. The query declares the maximum acceptable age of source-scoped completion
   evidence.
3. The engine evaluates that freshness policy when the query opens, owns the
   resulting `Observation`, and supplies current `QuerySnapshot` replacements.
4. A recent completion fact suppresses redundant acquisition only for the
   exact indexer/access and every requested filter semantically contained by
   the completed filter. For example, completed `{kind: 0, authors: [1, 2]}`
   covers requested `{kind: 0, authors: [1]}`. Missing, stale, non-covering, or
   unsafe limited facts cause acquisition for that source while the local
   result remains immediately usable.
5. App-relay, outbox, hints, fallback, and other routers retain independent
   contributions. The routing core unions them by destination and withdraws a
   destination only when every contributing router has withdrawn it.

`Freshness::MaxAge(Duration)` is already required by
`partial-spec-api-semantics.md:106-115`. `QUERY-013A` requires open-time
evaluation and forbids turning maximum-age queries into implicit polling loops.
`EVENT-007` permits cache-owned coverage facts keyed to relay, request shape,
access, and interval. Reuse follows semantic query containment, not filter
equality.

The current Rust implementation is incomplete: `Freshness` contains only
`CacheOnly` and `Live`, and the event-cache contract retains no reusable
query-completion coverage after an observation closes.

## Ownership and lifecycle

- `Query` owns the requested freshness policy.
- The event-cache contract owns reusable source-scoped coverage and keeps it
  coherent with event, tombstone, expiry, eviction, reset, and restart
  guarantees.
- `fava-observe` remains the sole owner of ordinary observation identity,
  source merge, relay demand, evidence, cancellation, and close.
- The routing engine hosts and bounds router input observations. Routers consume
  current `QuerySnapshot` values and replace only their own contribution.
- The outbox router owns neither a second `KnownLists` truth nor a `last_checked`
  map.

An event's `created_at` or `RelayEvidence.observed_at` is not query-freshness
proof. Positive and empty results require an actual source-complete fact whose
filter contains the requested filter. Containment means every event matching
the requested filter also matched the completed filter: author/kind/tag sets
are supersets, the completed interval encloses the requested interval, and
relay access is identical. A limited or otherwise non-exhaustive broader query
does not cover a narrower query. One fresh indexer cannot suppress work against
another indexer.

Maximum age is evaluated at open. An observation continues reacting to local
source changes but does not start a timer-driven refresh when the age expires.
A continuously live remote view uses live freshness instead; no hidden polling
policy is introduced here.

## Required public behavior

- Fresh covering coverage returns the immediate local result and opens no new
  work against the covered indexer, including narrower author subsets.
- Stale, absent, non-covering, cross-access, or unsafe limited coverage returns
  the same immediate local result and opens only the required relay work.
- Recent empty-with-EOSE is reusable; silence, timeout, refusal, authentication
  requirement, disconnect, or observation close is not converted to absence.
- A newer relay list replaces its predecessor. Cancellation, deletion, expiry,
  or cache removal reveals the next qualified current answer or unresolved
  absence through the ordinary `QuerySnapshot` path.
- Preview reads local state only, opens no relay acquisition, and retains no
  observation.
- When app-relay and outbox both contribute one destination, the combined route
  contains it once and retains it until both contributions withdraw it.
- Router-input count, query shape, result size, retained coverage, and evidence
  are bounded or produce typed refusal/shortfall.

## Subtractive consequences

- Delete `KnownLists`, `OutboxRouter::remember`, and cumulative snapshot
  ingestion when the outbox migration lands.
- Do not introduce `RouterQueries`, `RouterObservation`, `OpenedQuery`, an
  imperative opener, or another source of query truth merely to preserve the
  rejected plan.
- Do not restore `impl QuerySource for Fava`, the canary's second engine or
  transport, or any files removed by the merged canary teardown.
- Replace superseded router-query scaffolding completely in authoritative docs;
  do not preserve aliases or migration narration.

## Plan sequence

`07.8-01` is complete and remains unchanged. Replace Plans 02 through 05:

1. **07.8-02 — MaxAge and source-coverage architecture.** Architecture,
   vocabulary, and falsifier contract only; no production implementation.
2. **07.8-03 — MaxAge vertical implementation.** Public-`fava` RED evidence,
   then the contract/default implementation, containment-aware coverage
   invalidation, and deliberate-break proof.
3. **07.8-04 — engine-owned router inputs architecture.** Define the smallest
   bounded declarative contract using ordinary `Query` and `QuerySnapshot`
   values; do not assume a router-owned observation or imperative opener.
4. **07.8-05 — outbox migration and subtraction.** Prove simultaneous routers,
   warm/cold/stale/negative behavior, replacement/cancellation, failure,
   preview, and close; then remove the superseded outbox state and paths.

Each item is one focused local issue, branch, validation set, and commit series.
Architecture/vocabulary plans must be approved by Pablo before their dependent
feature plan executes.

## Vocabulary constraints

`Freshness`, `Query`, `QuerySnapshot`, `Observation`, `Router`, and
`RouterSession` are existing concepts. Do not approve adjective-qualified
router query synonyms merely because the old plan expected them.

Any unavoidable new public or cross-crate coverage type, persisted coverage
entity, router configuration concept, or provider-contract change must use its
own focused architecture change. Its proposal must include the closest existing
concept, observable distinction, counterexample, owner/lifecycle, forcing
requirement, reason existing state is insufficient, and executable falsifier.

The next Pablo checkpoint is that concrete vocabulary proposal. Naming the
router boundary waits until working `MaxAge` evidence exists.

## Constraints

- Preserve `07.8-01` and its settled-absence behavior.
- Re-research current `44ea2d4`; the previous `RESEARCH.md` and Plans 02-05 are
  based on the rejected imperative-opener premise.
- No shims, adapters, feature flags, compatibility paths, or alternate query
  execution path.
- Observable behavior first, RED evidence before implementation, and one named
  deliberate break per protection.
- Public promises must be proved through the public `fava` path and at their
  owning component.
- Run `python3 tools/check_vocabulary.py` and
  `python3 -m unittest tools.tests.test_vocabulary_check` for every
  architecture or public-API slice. The repository-wide gate remains
  intentionally red for unrelated unapproved vocabulary; each slice must prove
  its own delta without laundering existing diagnostics.
- Preserve the untracked vocabulary-approval tool files in the main worktree.
DATA_R8T4V2NK_END

[VERIFIED: .planning/phases/07.8-independent-correctness-defects/07.8-CONTEXT.md:6-149]

### The agent's discretion

None. The four replacement slices, order, gates, and subtractive constraints are locked. Research discretion is limited to the smallest sound architecture and executable proof within them. [VERIFIED: .planning/phases/07.8-independent-correctness-defects/07.8-CONTEXT.md:98-149]

### Deferred ideas

Naming any router-input boundary is deliberately deferred until `MaxAge` has working evidence. Timer-driven refresh and shared router-specific observations remain outside Phase 07.8; semantic containment for reusable source coverage is locked inside 07.8-02/03. [VERIFIED: .planning/phases/07.8-independent-correctness-defects/07.8-CONTEXT.md:30-35,54-66,98-131]

## Phase Requirements

| Requirement | Phase relevance |
|---|---|
| `QUERY-009` / `QUERY-010` | Completion evidence remains exact per relay/request/generation; only actual EOSE proves the completed predicate, including an empty result. Reuse may then prove a contained request without changing the original attribution. [VERIFIED: docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:403-428] |
| `QUERY-013A` | `MaxAge` is evaluated once when opening against source-scoped covering completion; it neither sweeps unrelated state nor polls. [VERIFIED: docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:465-471] |
| `EVENT-005` / `EVENT-007` | Coverage must fall when bounded cache retention invalidates its proof; records remain keyed exactly by relay/access and completed predicate, while lookup may reuse one for a semantically contained requested predicate. [VERIFIED: docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:580-606] |
| `WRITE-012` / `WRITE-013` | Routers contribute independently and asynchronously; replacements retract only the owning router's contribution. [VERIFIED: docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:852-874] |
| `WRITE-014` / `WRITE-015` / `WRITE-016` | Router inputs use ordinary explicit/local Fava queries, absence remains distinct from unresolved, and preview performs no acquisition. [VERIFIED: docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:876-900] |
| `ROUTER-001` | Outbox discovery uses configured indexers, source-complete evidence, coalesced needs, and no invented absence when no authoritative source answers. [VERIFIED: docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1150-1163] |

Planning requirements `LOCAL-08`, `READ-06`, `READ-07`, `READ-10`, `ROUTE-02`, `ROUTE-05`, `ROUTE-07`, `ROUTE-08`, `WRITE-13`, and `WRITE-15` provide corresponding public-path acceptance vocabulary. [VERIFIED: .planning/REQUIREMENTS.md:65-68,89-97,132-145,153-158]

The roadmap entry and old Plans 02–05 still describe the rejected imperative opener. They are stale planning artifacts, not implementation authority; replace them rather than reconciling their type names into the new design. [VERIFIED: .planning/ROADMAP.md:368-394] [VERIFIED: .planning/phases/07.8-independent-correctness-defects/07.8-02-PLAN.md:102-175]

## Summary

The stale premise is rejected: a router must not receive a capability that can imperatively open queries. `RouterQueries`, `RouterObservation`, `OpenedQuery`, `CachedQueries`, or a renamed equivalent would create a second query-opening surface, put `Observation` ownership outside `fava-observe`, and preserve the architecture Pablo removed. [VERIFIED: .planning/phases/07.8-independent-correctness-defects/07.8-CONTEXT.md:78-88]

Current implementation has two concrete gaps:

1. `Query` can request only local-only or continuously live behavior; it cannot express a bounded acceptable completion age. [VERIFIED: crates/fava-query/src/lib.rs:81-89]
2. EOSE completeness exists only in a running relay slot and per-observation evidence. The event-cache contract has no retained completion operation, and the memory cache stores only events, revision, and retractions. [VERIFIED: crates/fava-event-cache/src/lib.rs:13-105] [VERIFIED: crates/fava-event-cache-memory/src/lib.rs:24-31]

The smallest sound path is: approve one source-coverage value, one shared semantic-containment rule, and the `Freshness::MaxAge` extension in 07.8-02; implement and prove them through public `fava` in 07.8-03; approve the declarative engine-owned router-input shape in 07.8-04; migrate outbox and delete its parallel truth in 07.8-05. No compatibility layer is needed. [VERIFIED: .planning/phases/07.8-independent-correctness-defects/07.8-CONTEXT.md:98-131]

## Current-HEAD Findings

### Query freshness is incomplete

The complete current enum is:

DATA_H4V8N1RC_START

```rust
pub enum Freshness {
    /// Use configured local sources only.
    CacheOnly,
    /// Keep relay demand live. This is the ordinary default.
    #[default]
    Live,
}
```

DATA_H4V8N1RC_END

[VERIFIED: crates/fava-query/src/lib.rs:81-89]

`Observer::open` reduces this enum to `query.freshness() != Freshness::CacheOnly`; every non-cache-only query starts the engine and binds every route destination. There is no per-source freshness partition before demand retention. [VERIFIED: crates/fava-observe/src/observer.rs:162-221]

The specification already shows `Freshness::MaxAge(Duration)` as an ordinary query policy and says nested queries own freshness independently. [VERIFIED: docs/spec/partial-spec-api-semantics.md:92-123]

### Completion is live evidence, not retained coverage

EOSE is accepted only after the planner classifies the wire request. The complete current classification is:

DATA_B3K6T2WF_START

```rust
pub enum EoseCompleteness {
    Proven,
    LimitedRequest,
    RelayDefaultLimit,
}
```

DATA_B3K6T2WF_END

[VERIFIED: crates/fava-subscriptions/src/plan.rs:83-103]

`fava-observe` publishes `StoredEventsComplete` only for `Proven`; bounded/default-limited requests become shortfalls instead. That is the correct producer gate for retained coverage. [VERIFIED: crates/fava-observe/src/completions.rs:262-292]

Today a relay slot remembers only a `bool` per installed subscription. A late joiner to a still-running request is credited with `Timestamp::now()` rather than the original EOSE time, so that replay path cannot establish historical completion age. When the slot closes, the fact disappears. [VERIFIED: crates/fava-observe/src/slot.rs:17-31] [VERIFIED: crates/fava-observe/src/engine.rs:224-283]

`RelayQueryEvidence` is observation-scoped and distinguishes EOSE from refusal, authentication, timeout, disconnect, unreachable, and withdrawal. Reusing `observed_at`, event `created_at`, or a generic source-open state would collapse distinctions the current type preserves. [VERIFIED: crates/fava-query/src/evidence.rs:141-243]

### The event cache lacks the required owner contract

The complete `EventCache` surface covers event transactions, admission, expiry, commits, lookup, and count only; it has no completion-coverage read or write. [VERIFIED: crates/fava-event-cache/src/lib.rs:13-105]

`MemoryEventCache` has one bounded event capacity and atomically publishes event/retraction state. Capacity-driven event eviction is explicitly reported as `RetractionCause::Evicted`, supplying the invalidation hook for coverage whose filter could have matched the evicted event. [VERIFIED: crates/fava-event-cache-memory/src/lib.rs:33-49,66-155]

Architecture already assigns optional historical acquisition records and cache-profile retention to `EventCache`; it does not authorize an outbox-owned historical cache. [VERIFIED: docs/spec/ARCHITECTURE.md:761-847]

### Existing `filter_covers` is a pattern, not the retained-coverage contract

`fava-subscriptions::filter_covers(wide, narrow)` already implements conservative predicate containment for ids, authors, kinds, inclusive `since`/`until`, and conjunctive tag names with disjunctive values. Its three production consumers answer a different question: whether a currently running wire subscription can carry a newly arriving live demand. [VERIFIED: crates/fava-subscriptions/src/coverage.rs:13-48,80-130] [VERIFIED: crates/fava-subscriptions-standard/src/attach.rs:54-77] [VERIFIED: crates/fava-observe/src/engine.rs:224-283] [VERIFIED: crates/fava-observe/src/slot.rs:74-101]

It is not sufficient unchanged for reusable completion:

- It returns `true` for byte-identical limited filters before its limit guard. That is correct for two live owners sharing the same already-running bounded REQ, but an EOSE for that REQ is `LimitedRequest`, never source-complete. [VERIFIED: crates/fava-subscriptions/src/coverage.rs:20-38] [VERIFIED: crates/fava-subscriptions-standard/src/lib.rs:292-305]
- It refuses an unlimited completed filter covering a requested filter with a local result limit. Live attachment cannot reproduce the relay's limited row choice from a wider stream, but retained exhaustive data can: `fava-query-standard` performs deterministic ordering and truncation after local matching. Therefore the completed side must be unlimited; the requested presentation limit need not defeat coverage. [VERIFIED: crates/fava-subscriptions/src/coverage.rs:23-38] [VERIFIED: crates/fava-query-standard/src/lib.rs:30-50]
- It deliberately treats `None` and `Some(empty)` as the same unconstrained wire axis because current `nostr::Filter::match_event` does. Fava's query contract says a present-empty ids/authors/kinds/tag set matches nothing, so a match-nothing `Query` must be resolved before wire compilation rather than widened and then judged by this helper. [VERIFIED: crates/fava-subscriptions/src/coverage.rs:80-97,114-130] [VERIFIED: crates/fava-query/src/selection.rs:7-19] [VERIFIED: crates/fava-query-standard/src/lib.rs:171-189]
- It knows no relay/access identity, completion classification, cache coherence, or freshness. Those are mandatory independent gates around predicate containment. [VERIFIED: crates/fava-state/src/lib.rs:11-49] [VERIFIED: crates/fava-subscriptions/src/plan.rs:83-103]

Place one pure axis-containment implementation in `fava-query` beside `SourceCoverage`, the lowest neutral crate already depended on by both `fava-subscriptions` and `fava-event-cache`. Keep the existing `fava-subscriptions::filter_covers` surface as the live-attachment wrapper with its current limit/equality rule; make it delegate the axis work. Event-cache covering lookup delegates the same axis work but requires an exhaustive completed side, ignores only the requested local presentation limit, and returns no cross-session candidate. This gives the predicate relation one owner without copying authors/kinds/tags/window logic into a provider. [VERIFIED: crates/fava-subscriptions/Cargo.toml:7-13] [VERIFIED: crates/fava-event-cache/Cargo.toml:7-13]

### The current outbox is the parallel truth to delete

`OutboxRouter` owns an `Arc<dyn QuerySource>` and `KnownLists`. Its public `remember` path mutates that second map, source snapshots are cumulatively ingested into it, and a route session queries only authors missing from the map. [VERIFIED: crates/fava-router-outbox/src/lib.rs:51-62,94-155,171-212]

That shape cannot express removal: once copied into `KnownLists`, an event disappearing from a current source snapshot does not remove the derived relay list. It also changes query scope from the complete requested author set to a history-dependent missing subset. [VERIFIED: crates/fava-router-outbox/src/lib.rs:118-155,176-207]

The `impl QuerySource for Fava` adapter opens a nested public observation, converts its merged result back into a source snapshot, and labels it `EventCache`. This is source-identity impersonation and an alternate query execution path; Phase 07.8 must remove, not restore or rename, it. [VERIFIED: crates/fava/src/query_source.rs:14-72,120-150]

### 07.8-01 is complete and orthogonal

07.8-01 made settled absence depend on an actual answer from every router. A failed, panicked, cancelled, or silent router leaves the target unresolved; zero-router and all-answered-absent cases still settle. The completed slice did not change publication, the write store, or this phase's query/coverage architecture. Preserve it unchanged. [VERIFIED: .planning/phases/07.8-independent-correctness-defects/07.8-01-SUMMARY.md:48-307] [VERIFIED: crates/fava-routing/src/chain.rs:156-253,433-460]

## Architecture Responsibility Map

| Fact or lifecycle | Sole owner after Phase 07.8 | Consumers |
|---|---|---|
| Requested maximum acceptable age | `Query` / `Freshness` in `fava-query` | `fava-observe` at open [VERIFIED: docs/spec/partial-spec-api-semantics.md:92-123] |
| Meaning of a source-complete predicate and semantic containment | neutral query domain value in `fava-query` | subscriptions, event-cache contract, observation evidence |
| Retention, bounds, invalidation, reset, restart behavior | selected `EventCache` provider | `fava-observe` [VERIFIED: docs/spec/ARCHITECTURE.md:761-847] |
| Actual EOSE attribution and exact request/generation check | `fava-observe` with subscription planner evidence | event cache, `QuerySnapshot.evidence` [VERIFIED: docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:403-428] |
| Covering-record lookup under exact relay/access | `EventCache` contract using the shared query-domain predicate | `fava-observe` |
| Open-time freshness decision | `fava-observe`, using one captured open timestamp | relay-demand compiler |
| Observation identity, source merge, current snapshot, cancellation, close | `fava-observe` | routing engine, application [VERIFIED: docs/spec/ARCHITECTURE.md:2059-2092] |
| Router-input observation hosting and bounds | routing engine, using `fava-observe` | routers receive current snapshots |
| One router's replacement contribution | that router session | routing chain [VERIFIED: docs/spec/ARCHITECTURE.md:1197-1225] |
| Merged/deduplicated destination | routing chain | observe/publication [VERIFIED: docs/spec/ARCHITECTURE.md:1247-1280] |

## Mandatory Pablo Checkpoint: Concrete MaxAge / Source-Coverage Vocabulary Proposal

07.8-02 must present this as a focused architecture/vocabulary change and stop for Pablo's approve/rename/reject decision. It contains no production implementation. [VERIFIED: .planning/phases/07.8-independent-correctness-defects/07.8-CONTEXT.md:98-131]

### Existing-vocabulary extension

- Add `Freshness::MaxAge(Duration)` to existing `Freshness`.
- Add ordinary `Query::max_age(Duration) -> Query`; retain `Query::freshness()` as accessor.
- Compute age once at `Observation` open; coverage is fresh when `open_time - completed_at <= max_age`; never schedule an age-expiry timer.

This adds a policy value, not a lifecycle owner. The required semantics already exist in the authoritative partial API and `QUERY-013A`. [VERIFIED: docs/spec/partial-spec-api-semantics.md:106-115] [VERIFIED: docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:465-471]

### New domain value: `SourceCoverage`

| Proposal field | Decision |
|---|---|
| Closest existing concepts | `RelayQueryEvidence`, `RelaySourceState::StoredEventsComplete`, and `EoseCompleteness::Proven`. They describe current observation/wire evidence; none survives observation lifetime. [VERIFIED: crates/fava-query/src/evidence.rs:141-203] [VERIFIED: crates/fava-subscriptions/src/plan.rs:83-103] |
| Observable distinction | `SourceCoverage` means one exact relay session actually sent EOSE for one exhaustive admitted filter at `completed_at`, and the selected cache still retains coherent reusable proof for every requested predicate contained by that completed predicate. It is not routing `CoverageState`, event provenance, or a generic “source checked” timestamp. |
| Fields | Keep fields private: `session: RelaySessionKey`, `completed_filter: nostr::Filter`, `completed_at: Timestamp`. `session` carries exact relay plus access; `completed_filter` is the actual attributed wire predicate and must have no limit by construction. The more explicit field name prevents callers from mistaking it for the requested filter. Do not add `exhaustive: bool` or a second scope nominal: `SourceCoverage` itself means the producer already proved exhaustiveness. [VERIFIED: crates/fava-state/src/lib.rs:11-49] [VERIFIED: crates/fava-subscriptions/src/demand.rs:22-61] [VERIFIED: crates/fava-subscriptions/src/plan.rs:83-103] |
| Domain location | Define immutable `SourceCoverage` in `fava-query`, beside query evidence. The `EventCache` contract exclusively owns retention/lifecycle. This keeps the value neutral while preserving the cache as mutable-fact owner. [VERIFIED: docs/spec/ARCHITECTURE.md:3096-3115] |
| Contract change | Add covering lookup and retention operations to `EventCache`, conceptually `source_coverage(session, requested_filter) -> Option<SourceCoverage>` and `retain_source_coverage(coverage)`, both returning existing `EventCacheError`. Lookup matches `session` exactly, applies the single shared containment rule, and returns the newest valid covering record rather than an unbounded collection. `fava-observe` evaluates its age against the captured open time. Expose the applied fact in bounded `QueryEvidence`, using the same value rather than a second evidence type. |
| Match rule | A record covers a request iff session/access are equal, the completed filter is exhaustive, and every event matching the requested predicate also matched the completed predicate. Filter equality is one case, not the rule. One record must cover the whole requested filter; do not subtract residuals or synthesize a union across records in this slice. |
| Producer rule | Retain only after attributed EOSE whose current wire classification is `EoseCompleteness::Proven`; store actual EOSE time. Refusal, timeout, authentication, silence, disconnect, unreachable, close, and limited EOSE never produce `SourceCoverage`. [VERIFIED: crates/fava-observe/src/completions.rs:262-305] |
| Bounds | Default memory provider uses existing non-zero capacity as both maximum event count and maximum coverage-record count, deduplicates by exact `(session, completed_filter)`, and evicts oldest coverage deterministically. Bound filter axis counts/encoded size and lookup work; refuse an over-bound record before retention. Retention failure becomes a scoped query shortfall and causes future acquisition, never false completeness. [VERIFIED: crates/fava-event-cache-memory/src/lib.rs:33-49] |
| Invalidation | Capacity eviction of an event invalidates every record for an exact session present in that event's relay evidence whose `completed_filter` could match the event. If exact dependency cannot be proved cheaply, removing more records is safe; retaining one that may have lost a matching event is not. Evicting a tombstone or other state needed to prevent resurrection likewise removes affected records. Reset clears all coverage; the volatile provider retains neither events nor coverage across restart. Deletion, supersession, and expiry may preserve coverage only while their coherent tombstone/current-state facts remain retained. Do not attempt filter subtraction: remove the whole affected record. [VERIFIED: crates/fava-state/src/lib.rs:52-118,129-172] [VERIFIED: docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:580-606] |
| Counterexample | Proven EOSE from indexer A for `{authors: [alice,bob], kind: 10002}` covers A/public `{authors: [alice], kind: 10002}` while fresh, but not indexer B, A under another access, `{authors: [alice,carol]}`, an enclosing interval, a request with a different constrained tag, or the same request after maximum age. A limited EOSE from the broader filter covers nothing after close. Empty-result EOSE is reusable under the same containment rule; it is not restricted to filter equality. |
| Forcing requirements | `QUERY-013A`, `EVENT-005`, `EVENT-007`, and locked fresh/stale/mismatch/negative behavior. [VERIFIED: docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:465-471,580-606] |
| Existing state insufficient | `RelayQueryEvidence` ends with observation; `Slot::settled` stores only boolean; late attachment fabricates a new timestamp; `EventCache` has no retained-coverage API. [VERIFIED: crates/fava-query/src/evidence.rs:141-183] [VERIFIED: crates/fava-observe/src/slot.rs:17-31] [VERIFIED: crates/fava-observe/src/engine.rs:224-283] [VERIFIED: crates/fava-event-cache/src/lib.rs:13-105] |
| Executable falsifier | After relay sends proven EOSE for `{kind:0, authors:[alice,bob]}` and the first observation closes, opening `{kind:0, authors:[alice]}` through public `fava` returns the immediate local snapshot and emits zero new REQ work for that exact session. Replace containment with equality: the narrower-reuse test fails. Reverse any set/window polarity, ignore access, retain limited EOSE, or retain coverage after matching eviction: the independently causal matrix case for that defect fails. |

`SourceCoverage` is the only recommended new nominal term for 07.8-02. A provider-specific persisted entity uses the same concept; do not introduce `QueryCoverage`, `FreshCoverage`, `CoverageRecord`, or synonyms. Shortfall, error, and evidence additions extend existing nominal types unless Pablo approves another term.

### Precise safe containment

Let `C` be the completed filter and `R` the requested filter. Reuse requires `matches(R) ⊆ matches(C)` after resolving Fava match-nothing queries locally. Every axis is conjunctive with every other axis; one failed axis makes the whole relation false.

| Axis | Safe condition for `C` to cover `R` |
|---|---|
| Authors, kinds, ids | An absent/unconstrained `C` axis covers any `R`; a constrained `C` never covers an absent/unconstrained `R`; otherwise `R.values ⊆ C.values`. Thus `{authors:[1,2]}` covers `{authors:[1]}`, not the reverse. [VERIFIED: crates/fava-subscriptions/src/coverage.rs:80-97] |
| Tag values | Tag names are ANDed; values under one name are ORed. For every tag name constrained by `C`, `R` must constrain the same name and `R.values ⊆ C.values`. A name absent from `C` is unconstrained and covers a constraint added only by `R`; a name present only in `C` cannot cover an unconstrained `R`. [VERIFIED: crates/fava-subscriptions/src/coverage.rs:114-130] |
| Time interval | Bounds are inclusive. `C.since` is absent or `C.since <= R.since`; `C.until` is absent or `C.until >= R.until`. If `R` omits a side, `C` must omit that side. Equal endpoints cover. A reversed `since > until` interval is match-nothing: resolve/refuse it before acquisition and never use it to cover a nonempty request. [VERIFIED: crates/fava-subscriptions/src/coverage.rs:99-112] |
| Completed limit | `C.limit` must be absent, and the relay must not have imposed a default limit. Any explicit or relay-default limit makes EOSE non-exhaustive and forbids retaining `SourceCoverage`, even when filters are byte-identical. [VERIFIED: crates/fava-subscriptions-standard/src/lib.rs:292-305] |
| Requested limit | A requested local result limit does not narrow the predicate proof and may consume an unlimited covering record because the evaluator applies deterministic ordering/truncation locally. If acquisition is still required, the limited REQ remains non-exhaustive and cannot produce future coverage. [VERIFIED: crates/fava-query-standard/src/lib.rs:30-50] |
| Present-empty query axis | Fava defines it as match-nothing. Resolve it before relay demand and return the local empty result; do not serialize it into `nostr::Filter` and inherit that crate's empty-means-unconstrained wire behavior. [VERIFIED: docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:282-288] [VERIFIED: crates/fava-query/src/selection.rs:7-19] |
| Search or a future filter field | Require equality or typed refusal until a sound containment rule is approved. Keep exhaustive `Filter` destructuring so an upstream field addition fails compilation instead of silently widening reuse. The current helper already treats search conservatively. [VERIFIED: crates/fava-subscriptions/src/coverage.rs:50-78] |

Relay/access is not a filter axis: `SourceCoverage.session == requested_session` is mandatory before the table is evaluated. Freshness is not containment: after a covering record is found, `fava-observe` separately requires `open_time - completed_at <= max_age`. [VERIFIED: crates/fava-state/src/lib.rs:11-49] [VERIFIED: .planning/phases/07.8-independent-correctness-defects/07.8-CONTEXT.md:54-66]

## Implementation Architecture by Locked Slice

### 07.8-02 — MaxAge and source-coverage architecture only

Deliver one architecture/vocabulary change containing the proposal above, containment-aware open-time flow, coherence table, bounds, and executable falsifiers. Replace superseded architecture text completely. Run both vocabulary commands, report known baseline diagnostics separately, and stop at Pablo's checkpoint. No Rust production code belongs here. [VERIFIED: .planning/phases/07.8-independent-correctness-defects/07.8-CONTEXT.md:98-149]

### 07.8-03 — MaxAge vertical implementation

```text
validate ordinary Query
  -> resolve present-empty match-nothing without relay demand
  -> open EventCache and WriteStore for immediate local state
  -> compile route to exact per-session filters
  -> lookup newest covering SourceCoverage for each exact session/filter
  -> evaluate all covering records at one captured open timestamp
  -> retain whole demand only for missing, stale, non-covering, or cross-access sessions
  -> publish immediate QuerySnapshot with applied coverage/shortfalls
  -> on actual Proven EOSE, atomically retain actual completed filter/time
  -> react to local revisions; never create an age timer
```

Work belongs in `fava-query`, `fava-event-cache`, default memory provider, and `fava-observe`; it requires no router API or outbox change. `Observer::open` currently sequences sources, routes, demand, initial evaluation, installation, and handle release. [VERIFIED: crates/fava-observe/src/observer.rs:147-254]

Preserve partial acquisition: fresh covering coverage for indexer A suppresses only A while missing/stale/non-covering B opens B. A non-covering predicate executes whole; never subtract a residual filter. `Live` remains continuous and `CacheOnly` local-only. [VERIFIED: .planning/phases/07.8-independent-correctness-defects/07.8-CONTEXT.md:14-25,54-73]

### 07.8-04 — engine-owned router inputs architecture only

Do not name a router-specific observation or opener. Define these semantics using existing nouns until Pablo approves an unavoidable cross-crate shape:

1. Router declaratively returns a bounded set of ordinary `Query` values required for current route request.
2. Routing engine validates query count and shape before work opens.
3. Engine asks `fava-observe` to own each ordinary `Observation`; router never receives opener or handle.
4. Engine supplies complete current `QuerySnapshot` replacements to router contribution calculation.
5. Router returns complete replacement `RouteContribution`; routing unions independent contributors.
6. Preview evaluates inputs as one-shot local-only snapshots, closes immediately, and retains no observation/acquisition.
7. Outbox inputs are explicitly routed to configured indexers, so engine ownership cannot recurse through automatic routing.

Bound router-input count, authors/filter shape, result size, retained observations, and evidence, or return typed refusal/shortfall before opening. Do not add a provider framework, second evaluator, or router cache. Current `ARCHITECTURE.md:1294-1369` still describes imperative services and router-owned discovery state; replace it completely. [VERIFIED: docs/spec/ARCHITECTURE.md:1294-1379]

### 07.8-05 — outbox migration and subtraction

Outbox declares one bounded ordinary query for kind `10002`, complete requested authors, all configured indexers, and approved `MaxAge`. It consumes current `QuerySnapshot` as a replacement, parses current newest qualified NIP-65 event per author, and derives its whole contribution without cumulative state. [VERIFIED: .planning/phases/07.8-independent-correctness-defects/07.8-CONTEXT.md:8-25,90-108]

After public behavior is green, delete `KnownLists`, `OutboxRouter::remember`, cumulative `SourceSnapshot` ingestion, `Arc<dyn QuerySource>`, missing-author narrowing, `impl QuerySource for Fava`, and tests/docs existing only for those paths. Do not replace them with adapters or aliases. [VERIFIED: crates/fava-router-outbox/src/lib.rs:51-62,94-155,171-223] [VERIFIED: crates/fava/src/query_source.rs:14-150]

App-relay and outbox remain simultaneous independent routers. Routing already deduplicates identical sessions while retaining attributed reasons; prove withdrawing one contribution does not remove destination until the other withdraws. [VERIFIED: docs/spec/ARCHITECTURE.md:1247-1280] [VERIFIED: crates/fava-routing/src/chain.rs:156-253]

## Validation Architecture

Repository guidance requires behavior first, RED evidence, smallest implementation, named deliberate break, and public-path capstones. Test doubles cause observable protocol/cache facts rather than set internal coverage flags. [VERIFIED: docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md:32-116,207-227]

### 07.8-02 gates

- Architecture diff contains no production implementation.
- Proposal includes closest concept, distinction, counterexample, owner/lifecycle, forcing requirement, insufficiency, bound, and falsifier.
- `python3 tools/check_vocabulary.py`: capture pre-existing red baseline and prove only expected delta.
- `python3 -m unittest tools.tests.test_vocabulary_check`: green.
- Pablo approves or renames proposal before 07.8-03.

### 07.8-03 owner and public RED cases

- Event-cache conformance: exact `(session, completed_filter)` retention/deduplication; newest covering lookup; positive/empty retention; own bound; session-attributed invalidation on matching eviction/tombstone loss; reset/restart profile; no retention on refusal.
- Containment matrix: authors, kinds, and ids supersets cover subsets but not the reverse; unconstrained completed axes cover constrained requests; constrained completed axes do not cover unconstrained requests; tag-name/value polarity; enclosing inclusive time windows; one mismatching axis defeats the whole relation; unsupported/search fields require equality or refusal.
- Limit matrix: explicit-limit and relay-default-limit EOSE retain nothing, including byte-identical reopen; an unlimited completed predicate may cover a requested local result limit; present-empty match-nothing queries open no relay work.
- Observation owner: exact relay/access fresh covering record; stale, cross-relay, cross-access, and non-covering cases; mixed fresh/stale/covering indexers; no expiry timer; local replacements continue.
- Protocol cause: scripted relay sends event(s)+EOSE or empty EOSE. Silence, CLOSED, authentication, timeout, disconnect, and close never create coverage.
- Public `fava`: close a completed `{kind:0, authors:[alice,bob]}` observation, open `{kind:0, authors:[alice]}` with `MaxAge`, and assert immediate local result plus zero additional REQ frames for only that covered session.
- Independently causal deliberate breaks: replace containment with equality -> narrower-author public case fails; reverse set/tag/window polarity -> its focused matrix case fails; ignore access -> cross-access case fails; treat either limited EOSE as complete -> limited case fails; skip matching eviction invalidation -> eviction case fails; stamp late attachment with current time -> controlled-clock age case fails.

### 07.8-04 gates

- Architecture maps every mutable fact/lifecycle to one owner.
- Dependency graph remains domain values -> neutral contracts -> providers.
- Falsifier rejects automatic router input before observation opens.
- Falsifiers cover query count/shape, result/evidence, and retained-observation bounds.
- No `RouterQueries`, `RouterObservation`, `OpenedQuery`, or qualified synonym.
- Pablo approves architecture before 07.8-05.

### 07.8-05 owner and public RED cases

- Warm: fresh covering coverage, including a broader completed author set, uses current relay lists and emits no indexer REQ.
- Cold: immediate local snapshot while only configured indexers acquire complete author set.
- Stale/non-covering/cross-access: local result remains immediate; only affected indexer acquires.
- Negative: empty proven EOSE settles absence; failure/close remains unresolved.
- Replacement: newer list replaces predecessor; deletion, expiry, cancellation, or cache removal reveals next current answer or unresolved.
- Preview: local contribution only; zero retained observations/acquisition.
- Simultaneous app-relay/outbox: one destination; withdrawing either alone retains it; withdrawing both removes it.
- Close: input observations and relay demand drain exactly once.
- Subtraction search: no `KnownLists`, `OutboxRouter::remember`, `impl QuerySource for Fava`, `RouterQueries`, `RouterObservation`, or `OpenedQuery` in production/authoritative docs.
- Deliberate breaks: restore missing-author narrowing; replacement/removal fails. Restore cumulative ingestion; deletion/cache-removal fails. Withdraw on first contributor; simultaneous-router fails.

Current focused baseline is green:

```text
cargo test -p fava-query -p fava-event-cache-memory -p fava-observe -p fava-routing -p fava-router-outbox --no-fail-fast
```

Vocabulary unit suite passes 33 tests. Repository-wide vocabulary checker is intentionally red with unrelated diagnostics; compare it as a delta. [VERIFIED: .planning/phases/07.8-independent-correctness-defects/07.8-CONTEXT.md:133-149]

## Standard Stack

No external dependency is required. Use current Rust 1.90 workspace package set (`0.1.0`): `fava-query`, `fava-event-cache`, `fava-event-cache-memory`, `fava-subscriptions`, `fava-observe`, `fava-routing`, `fava-router-outbox`, and public `fava`. [VERIFIED: Cargo.toml:46-50]

The testing guide places cache coverage in provider conformance, composition in routing tests, protocol causes in scripted-relay tests, and public promises through `fava`. [VERIFIED: docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md:233-310]

## Common Pitfalls

- Treating event `created_at`, relay observation time, source-open time, or cache revision as completion time.
- Recording limited EOSE, silence, refusal, or close as coverage.
- Stamping historical completion with reopen time.
- Applying one indexer's completion to another relay/access.
- Requiring filter equality and therefore reacquiring semantically contained author/kind/tag/time subsets.
- Calling live-attachment `filter_covers` as if it also proved exhaustive completion, freshness, access identity, and cache coherence.
- Letting a requested local result limit defeat an unlimited completed predicate, or letting a limited completed query cover anything after close.
- Widening a Fava present-empty match-nothing axis through `nostr::Filter`'s empty-as-unconstrained wire behavior.
- Reversing tag polarity: an absent completed tag name is broad; a tag name present only on the completed side is narrow.
- Polling when `MaxAge` expires after open.
- Retaining coverage after backing eviction or volatile restart.
- Querying only authors absent from derived state.
- Keeping `KnownLists` “temporarily”.
- Giving routers an opener/observation under another name.
- Letting preview retain an observation or acquire.
- Restoring Fava-as-`QuerySource` or canary second engine.
- Editing 07.8-01.

[VERIFIED: .planning/phases/07.8-independent-correctness-defects/07.8-CONTEXT.md:30-96,118-149]

## Security Domain

Applicable domain is input/resource validation. Authentication, credential storage, session management, cryptography, and authorization policy are unchanged. Exact `RelaySessionKey` matching preserves access identity; bounded query/filter/evidence/coverage protects memory. [VERIFIED: crates/fava-query/src/lib.rs:101-127]

| Threat | Protection | Proof home |
|---|---|---|
| Spoofed freshness | Only exact attributed `Proven` EOSE creates a record; reuse separately checks containment and age | `fava-observe` protocol-cause tests |
| Cross-access/relay reuse | Exact `RelaySessionKey` | cache conformance + public mismatch |
| Silent under-fetch | Shared semantic containment with correct set/tag/window polarity; whole-filter acquisition on miss | containment matrix + public narrower-reuse case |
| Truncated history claimed complete | Completed side must be unlimited and classified `Proven`; relay-default limits also refuse retention | protocol + cache conformance |
| False negative | Failures never become completion/absence | evidence + outbox negative tests |
| Resource exhaustion | Bound queries, shape, results, observations, coverage, evidence | contract/engine tests |
| Stale proof after loss | Invalidate on eviction/reset; declare restart profile | provider conformance |

## Open Questions and Assumptions

### Blocking checkpoint

Pablo must approve, rename, or reject the proposed `SourceCoverage` packet and `Freshness::MaxAge` extension in 07.8-02. 07.8-03 must not implement before that decision. This is the only implementation blocker.

### Assumptions

None. Recommendations are proposals, not claims of approved vocabulary. Router-boundary naming remains intentionally deferred.

## Sources

- Context: `.planning/phases/07.8-independent-correctness-defects/07.8-CONTEXT.md` [VERIFIED: .planning/phases/07.8-independent-correctness-defects/07.8-CONTEXT.md:1-149]
- Goals: `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md` [VERIFIED: docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:403-481,580-616,852-900,1150-1163]
- Architecture: `docs/spec/ARCHITECTURE.md` [VERIFIED: docs/spec/ARCHITECTURE.md:761-847,1032-1059,1119-1379,2059-2092,2968-3002,3096-3118]
- API semantics: `docs/spec/partial-spec-api-semantics.md` [VERIFIED: docs/spec/partial-spec-api-semantics.md:8-24,92-172,282-330,518-534]
- Proof method: `docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md` [VERIFIED: docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md:32-116,207-310]
- Current code: `fava-query`, `fava-event-cache`, `fava-event-cache-memory`, `fava-subscriptions`, `fava-observe`, `fava-routing`, `fava-router-outbox`, and `fava/src/query_source.rs`.
- Preserved slice: `.planning/phases/07.8-independent-correctness-defects/07.8-01-SUMMARY.md`.

## Research Verdict

Ready to replace Plans 02–05 exactly as locked. 07.8-02 is next and must end at Pablo's concrete `Freshness::MaxAge` / `SourceCoverage` checkpoint. Stale imperative-opener research is not reusable.
> Historical phase record. Superseded by STATE-ARCH-1; not current implementation guidance.
