<!-- refreshed: 2026-08-21 -->
# Architecture

**Analysis Date:** 2026-08-21

## System Overview

Fava is a statically assembled Rust library. The implemented product covers the completed M0-M6
milestone set recorded in `docs/issues/0001-local-source-merge.md` through
`docs/issues/0008-automatic-write-routing.md`: coherent local queries, explicit and automatic live
relay queries, bounded multi-relay observation, ordered routing, durable explicit publication, and
asynchronously expanding automatic write routing. M7-M11 remain delivery specifications in
`docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`; their named owners and native artifacts are not
present under `crates/`, `apps/`, or `docs/issues/`.

The authority chain is:

1. `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md` for required behavior.
2. `docs/spec/ARCHITECTURE.md` for responsibilities and lifecycle ownership.
3. `docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md` for behavioral proof.
4. `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md` for milestone sequencing and exit gates.
5. `docs/spec/partial-spec-api-semantics.md` for compatible query refinements.

```text
┌──────────────────────────────────────────────────────────────────────────────┐
│ Downstream application / canary                                             │
│ `crates/fava/src/lib.rs` · `apps/canary/src/main.rs`                    │
└───────────────────────────────────┬──────────────────────────────────────────┘
                                    │ public `Fava` operations
                                    ▼
┌──────────────────────────────────────────────────────────────────────────────┐
│ Thin facade and static assembly · `crates/fava/src/`                      │
├───────────────────────────────┬──────────────────────────────────────────────┤
│ Query path                    │ Publication path                             │
│ `Fava::observe`             │ `Fava::publish` / receipt operations       │
└───────────────┬───────────────┴───────────────────┬──────────────────────────┘
                │                                   │
                ▼                                   ▼
┌───────────────────────────────┐     ┌────────────────────────────────────────┐
│ `fava-observe`              │     │ `fava-publication`                   │
│ merged latest-state owner     │     │ durable signing/routing/delivery owner │
└───────────┬───────────────────┘     └──────┬──────────┬──────────┬───────────┘
            │                                │          │          │
            ▼                                ▼          ▼          ▼
┌───────────────────────────────┐     ┌──────────┐ ┌──────────┐ ┌─────────────┐
│ `QuerySource` contracts     │     │ `Signer`│ │ `Router` │ │ `Publisher`│
│ event cache + write store     │     │ contract │ │ chain    │ │ + delivery  │
└──────────┬───────────┬────────┘     └──────────┘ └────┬─────┘ └──────┬──────┘
           │           │                                └──────┬───────┘
           ▼           ▼                                       ▼
┌──────────────┐ ┌────────────────┐              ┌────────────────────────────┐
│ memory event │ │ memory / Redb  │              │ subscription + transport   │
│ cache        │ │ write stores   │              │ + NIP-01 wire boundaries   │
└──────┬───────┘ └────────┬───────┘              └──────────────┬─────────────┘
       └──────────┬───────┘                                     │ real WebSocket
                  ▼                                             ▼
        one `QuerySnapshot`                            external Nostr relay
        with exact evidence

Independent evidence boundary:
`apps/canary/src/relay.rs` -> `apps/canary/src/proxy.rs`
    -> `apps/canary/src/wire.rs` -> `apps/canary/src/artifacts.rs`
```

The dependency direction is domain values -> neutral contracts -> implementations and lifecycle
owners -> facade. Concrete standard providers are selected by the application; they are not normal
dependencies of `crates/fava/Cargo.toml`. Cargo owns package metadata in `Cargo.toml` and
`Cargo.lock`; Bazel mirrors first-party edges in `crates/*/BUILD.bazel` and imports the locked
third-party graph through `MODULE.bazel`.

## Component Responsibilities

| Component | Responsibility | File |
|-----------|----------------|------|
| Behavioral authority | Defines required behavior without reporting completion status. | `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md` |
| Architecture authority | Assigns facts, lifecycle owners, dependency rules, boundaries, and falsifiers. | `docs/spec/ARCHITECTURE.md` |
| Delivery authority | Defines M0-M11 vertical milestones and exit gates. | `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md` |
| Vocabulary authority | Closes concepts, crate names, and public nominal Rust symbols by default. | `docs/internals/vocabulary.toml` |
| `fava-state` | Owns relay/access/session evidence, event coordinates, replaceable ordering, deletion, and expiration mutations. | `crates/fava-state/src/lib.rs` |
| `fava-write` | Owns checked event building, write intents, routing mode, identities, materialized events, delivery outcomes, and receipts. | `crates/fava-write/src/lib.rs`, `crates/fava-write/src/builder.rs` |
| `fava-query` | Owns the sole public query, acquisition versus result authority, source contracts, event records, snapshots, and evaluator contract. | `crates/fava-query/src/lib.rs` |
| `fava-query-standard` | Supplies same-ID evidence merge, coordinate winner selection, filters, ordering, and limits. | `crates/fava-query-standard/src/lib.rs` |
| `fava-event-cache` | Defines signed relay-event retention as a specialized `QuerySource`; applies universal admission and expiration rules. | `crates/fava-event-cache/src/lib.rs` |
| `fava-event-cache-memory` | Owns bounded in-process cached events, atomic mutation batches, revisions, and snapshots. | `crates/fava-event-cache-memory/src/lib.rs` |
| `fava-write-store` | Defines custody, receipt transitions, routing application, attempts, cancellation, recovery, removal, and source observation. | `crates/fava-write-store/src/lib.rs` |
| Write-store providers | Implement bounded volatile custody and immediate-durability Redb custody/recovery. | `crates/fava-write-store-memory/src/lib.rs`, `crates/fava-write-store-redb/src/lib.rs`, `crates/fava-write-store-redb/src/ops.rs` |
| `fava-observe` | Atomically opens local sources, reevaluates current state, coalesces bounded snapshots, and owns close. | `crates/fava-observe/src/lib.rs` |
| `fava-wire` | Encodes and decodes exact NIP-01 messages without owning transport or subscription policy. | `crates/fava-wire/src/lib.rs` |
| `fava-subscriptions` | Defines logical demand, exact wire plans, inbound attribution, planner contract, and query-to-filter conversion. | `crates/fava-subscriptions/src/lib.rs` |
| Subscription planners | Provide one-REQ-per-demand and bounded compatible-author grouping policies. | `crates/fava-subscriptions-no-grouping/src/lib.rs`, `crates/fava-subscriptions-standard/src/lib.rs` |
| `fava-transport` | Defines exact session generation, frame handoff, receive, and idempotent close contracts. | `crates/fava-transport/src/lib.rs` |
| WebSocket transport | Owns WebSocket connections, generation allocation, socket resources, frame bounds, and handoff ambiguity. | `crates/fava-transport-websocket/src/lib.rs` |
| `fava-ingest` | Attributes EVENT frames, verifies ID/signature/filter, and admits only valid signed events. | `crates/fava-ingest/src/lib.rs` |
| `fava-diagnostics` | Owns bounded public facts for coalescing, routes, sessions, subscriptions, EOSE, CLOSED, AUTH needs, failures, and withdrawal. | `crates/fava-diagnostics/src/lib.rs` |
| `fava-routing` | Owns read/write requests, targets, coverage, contributions, ordered live composition, preview, deduplication, and bounds. | `crates/fava-routing/src/lib.rs`, `crates/fava-routing/src/chain.rs` |
| Router providers | Supply app-relay, fallback, NIP-65 outbox, Nostr hint/evidence, and delayed-test policies. | `crates/fava-router-app-relays/src/lib.rs`, `crates/fava-router-fallback-relays/src/lib.rs`, `crates/fava-router-outbox/src/lib.rs`, `crates/fava-router-hints/src/lib.rs`, `crates/fava-router-testkit/src/lib.rs` |
| `fava-nip65` | Owns pure kind:10002 relay-list parsing and replacement ordering, not routing. | `crates/fava-nip65/src/lib.rs` |
| Signer boundary | Separates exact author-bound signing from its local-key implementation. | `crates/fava-signer/src/lib.rs`, `crates/fava-signer-local/src/lib.rs` |
| Publisher boundary | Separates one exact publication attempt from the NIP-01 implementation. | `crates/fava-publisher/src/lib.rs`, `crates/fava-publisher-nip01/src/lib.rs` |
| Delivery boundary | Separates retry/give-up decisions from the bounded standard policy. | `crates/fava-delivery/src/lib.rs`, `crates/fava-delivery-standard/src/lib.rs` |
| `fava-publication` | Orders durable acceptance, signing, route revisions, destination lanes, policy decisions, cancellation, terminal waiting, and recovery. | `crates/fava-publication/src/lib.rs`, `crates/fava-publication/src/run.rs` |
| `fava` | Exposes the facade, validates assembly, owns live-query relay tasks, and delegates publication lifecycle. | `crates/fava/src/lib.rs`, `crates/fava/src/live.rs`, `crates/fava/src/relay.rs`, `crates/fava/src/routes.rs` |
| Canary/evidence app | Runs enabled M0-M6 scenarios, relay processes, proxies, hostile witnesses, crash children, and artifacts. | `apps/canary/src/main.rs`, `apps/canary/src/lib.rs`, `apps/canary/scenarios.json` |
| External falsifier | Proves an outside-workspace event-cache provider can use only public contracts. | `falsifiers/external-null-cache/src/lib.rs` |

## Pattern Overview

**Overall:** Statically assembled ports-and-adapters library with semantic-owner crates, neutral
contracts, separately selectable implementations, and focused asynchronous lifecycle owners.

**Key Characteristics:**
- Use `FavaBuilder` in `crates/fava/src/lib.rs` for composition; missing mandatory roles return
  `BuildError`, and publication roles become jointly mandatory when publication is selected.
- Keep each contract independent of its implementation, as in `crates/fava-transport/` versus
  `crates/fava-transport-websocket/` and `crates/fava-write-store/` versus provider crates.
- Use complete replacement snapshots for current source and router state. Providers own
  `SourceRevision`; the observer owns `QueryRevision`; route owners apply monotonic revisions.
- Use `tokio::sync::watch` for bounded latest state and `tokio::sync::broadcast` for causal receipt
  changes that must report lag rather than silently coalesce.
- Keep acquisition scope separate from result provenance in `QuerySourcePolicy`; asking a relay
  never fabricates evidence that it served an event.
- Keep optimistic local events solely in `WriteStore`; only verified relay observations enter
  `EventCache` through `fava-ingest` and `EventCache::admit`.
- Route reads and writes through the same `RouteRequest`, `Router`, and ordered chain. Preview uses
  the same derivation but opens no router sessions or relay work.
- Start independent work as facts become available: automatic reads open immediate relays and add
  later ones; automatic writes deliver while other targets remain unresolved.
- Retain exact relay/access/session, subscription, receipt, attempt, and route revision identity at
  asynchronous boundaries.
- Keep protocol meaning in protocol crates such as `crates/fava-nip65/`; universal routing,
  building, publication, and facade crates do not switch on NIP-specific product behavior.

## Implementation Coverage

| Milestone | Current implementation boundary | Evidence |
|-----------|---------------------------------|----------|
| M0 | Independent real-relay process/wire/persistence lab and reconstructable artifacts. | `docs/issues/0002-m0-evidence-foundation.md`, `apps/canary/src/lib.rs`, `features/relay-lab.feature` |
| M1 | Local event state, memory cache/write sources, source merge, deletion/expiry, cancellation retraction, and bounded observation. | `docs/issues/0001-local-source-merge.md`, `crates/fava-query-standard/tests/source_merge.rs`, `crates/fava/tests/local_source_merge.rs` |
| M2 | Explicit live query, NIP-01 planning/wire/transport, verified admission, EOSE/CLOSED diagnostics, and CLOSE. | `docs/issues/0004-explicit-live-query.md`, `crates/fava/tests/explicit_live.rs`, `crates/fava-ingest/tests/admission.rs` |
| M3 | Multi-relay provenance, reconnect generations, stale-subscription refusal, and bounded coalescing. | `docs/issues/0005-multi-relay-observation.md`, `crates/fava/tests/multi_relay.rs`, `crates/fava/tests/observation_bounds.rs` |
| M4 | Ordered live routers, relay reconciliation, preview, explicit bypass, and interchangeable subscription grouping. | `docs/issues/0006-ordered-automatic-routing.md`, `crates/fava/tests/automatic_routes.rs`, `crates/fava-subscriptions-standard/tests/grouping.rs` |
| M5 | Durable explicit publication, signer/publisher/delivery seams, Redb recovery, receipts, and cancellation. | `docs/issues/0007-durable-explicit-publication.md`, `crates/fava/tests/explicit_publication.rs`, `crates/fava-write-store-redb/tests/process_kill.rs` |
| M6 | Automatic write routing, partial delivery, live expansion under one receipt, router providers, and preview parity. | `docs/issues/0008-automatic-write-routing.md`, `crates/fava/tests/automatic_publication.rs`, `crates/fava-router-outbox/tests/outbox.rs` |
| M7-M11 | Specification and vocabulary entries only. No edit protocol, auth, fetch-cache/service, persistent event-cache, provider matrix, FFI, Swift, or Kotlin implementation exists. | `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`, `docs/internals/vocabulary.toml`, `Cargo.toml`, `apps/canary/scenarios.json` |

## Layers

**Authoritative Design Layer:**
- Purpose: Own behavior, architecture, proof discipline, sequencing, and closed vocabulary.
- Location: `docs/spec/`, `docs/internals/`
- Contains: The five authoritative specifications and `docs/internals/vocabulary.toml`.
- Depends on: No implementation crate.
- Used by: `docs/issues/`, `features/`, `crates/`, `apps/canary/`, and `tools/check_vocabulary.py`.

**Domain and Protocol Value Layer:**
- Purpose: Define stable Nostr/Fava values and deterministic rules without provider resources.
- Location: `crates/fava-state/`, `crates/fava-write/`, `crates/fava-query/`, `crates/fava-wire/`, `crates/fava-nip65/`
- Contains: Event state/evidence, write/receipt values, query/result values, NIP-01 messages, and NIP-65 lists.
- Depends on: `nostr`, serialization/error libraries, and lower semantic owners.
- Used by: Contract, provider, lifecycle, facade, and acceptance crates.

**Neutral Contract Layer:**
- Purpose: Define replaceable roles without selecting mechanisms.
- Location: `crates/fava-event-cache/`, `crates/fava-write-store/`, `crates/fava-subscriptions/`, `crates/fava-transport/`, `crates/fava-routing/`, `crates/fava-signer/`, `crates/fava-publisher/`, `crates/fava-delivery/`
- Contains: Object-safe provider traits, exact request/result values, and scoped typed errors.
- Depends on: Domain values and neutral lower contracts only.
- Used by: Providers, lifecycle owners, facade, testkits, and external falsifiers.

**Provider and Policy Layer:**
- Purpose: Supply concrete algorithms, storage, transport, signing, routing, and planning choices.
- Location: `crates/fava-*-memory/`, `crates/fava-write-store-redb/`, `crates/fava-*-standard/`, `crates/fava-transport-websocket/`, `crates/fava-signer-local/`, `crates/fava-publisher-nip01/`, `crates/fava-router-*/`
- Contains: Memory/Redb stores, evaluator, planners, WebSocket sessions, signing, NIP-01 publication,
  delivery policy, and router policies.
- Depends on: Corresponding neutral contracts and domain crates; concrete routers do not flow into
  `crates/fava-routing/Cargo.toml`.
- Used by: `apps/canary/Cargo.toml` and public integration tests under `crates/fava/tests/`.

**Universal Lifecycle Owner Layer:**
- Purpose: Order work across contracts while leaving policy/mechanism with their owners.
- Location: `crates/fava-observe/`, `crates/fava-ingest/`, `crates/fava-publication/`
- Contains: Query observation, relay-event admission, and accepted-write execution/recovery.
- Depends on: Neutral contracts, not standard implementations.
- Used by: `crates/fava/`.

**Facade and Relay Coordination Layer:**
- Purpose: Validate assembly and bind public handles to relay/route tasks.
- Location: `crates/fava/`
- Contains: `Fava`, `FavaBuilder`, explicit/automatic query opening, reconnect loops, route
  reconciliation, `QuerySource for Fava`, and publication delegation.
- Depends on: Contract and lifecycle-owner crates; concrete providers are dev dependencies only.
- Used by: Rust applications, the canary, and the external falsifier.

**Evidence and Falsification Layer:**
- Purpose: Prove owner behavior, public composition, real sockets/processes, persistence, and replaceability.
- Location: `crates/*/tests/`, `crates/fava/tests/`, `features/`, `apps/canary/`, `falsifiers/`, `docs/issues/`
- Contains: Component corpora, facade tests, BDD features, 22 enabled M0-M6 canary scenarios,
  crash/restart evidence, and an outside-workspace null cache.
- Depends on: Public contracts and selected providers; M0 wire witnessing in
  `apps/canary/src/wire.rs` remains independent from Fava diagnostics.
- Used by: Milestone exit decisions.

## Data Flow

### Primary Request Path

1. The application constructs `Query` and calls `Fava::observe` in `crates/fava/src/lib.rs`.
2. `Freshness::CacheOnly` opens `Observer` directly; live queries dispatch through
   `crates/fava/src/live.rs` to explicit or automatic relay acquisition.
3. `Observer::open` in `crates/fava-observe/src/lib.rs` opens event-cache then write-store sources.
   A second-source refusal closes the provisional first source.
4. `StandardQueryEvaluator` in `crates/fava-query-standard/src/lib.rs` merges same-ID evidence,
   selects one event per coordinate, applies authority, ordering, and limits, and produces revision 1.
5. Sources publish complete replacement snapshots; the observer reevaluates and replaces one
   `watch` value. `Observation::current` is immediate; `changed` yields newer complete state.
6. Dropping or closing the handle cancels attached relay and router work.

### Explicit Live Relay Flow

1. `crates/fava/src/live.rs` creates one `RelaySessionKey` per exact relay and asks
   `OpenedRelay::open` in `crates/fava/src/relay.rs` to establish it.
2. `SubscriptionPlanner` creates exact REQ messages plus subscription/filter attribution.
3. `Transport` opens a fresh generation; `fava-wire` encodes frames and the session hands them off.
4. Inbound frames match current attribution. `fava-ingest` verifies signature, ID, and filter before
   `EventCache::admit`; cache changes then use the ordinary observation path.
5. EOSE, CLOSED, AUTH need, failure, and withdrawal remain bounded diagnostic facts.
6. Disconnect records the generation and establishes a fresh session/subscription identity; close
   sends exact CLOSE frames.

### Automatic Live Query Flow

1. `crates/fava/src/routes.rs` opens configured routers through `fava_routing::open` with
   `RouteRequest::Read`.
2. `crates/fava-routing/src/chain.rs` obtains immediate contributions in application-selected order
   and sends later routers the current upstream `RoutePlan`.
3. Immediate destinations start before unresolved routers settle. Later complete contributions
   replace prior contributions and produce newer plans.
4. Reconciliation leaves unchanged relay tasks live, cancels retracted relays, and opens added relays.
5. `Fava::preview_routes` runs the same derivation without sessions or connections.

### Durable Explicit Publication Flow

1. The application creates `WriteIntent` in `crates/fava-write/src/lib.rs` and calls `Fava::publish`.
2. `Publication::accept` in `crates/fava-publication/src/lib.rs` requires Tokio, then
   `WriteStore::accept` atomically commits identities, materialization, destinations, and revision.
3. The write-source snapshot makes the local event immediately visible without cache pollution.
4. `Publication` selects signer by author; `install_signed` accepts only a matching body.
5. Each lane durably calls `begin_attempt`; `Publisher` performs one attempt through `Transport`;
   `DeliveryPolicy` alone decides retry or give-up.
6. The store commits exact outcomes. Bounded receipt broadcasts preserve causal changes and report lag.

### Automatic Publication and Route Expansion

1. Automatic intent acceptance commits durable custody before router acquisition in
   `crates/fava-publication/src/run.rs`.
2. `Publication` opens `RouteRequest::Write` and atomically applies its immediate contribution.
3. Known destinations begin independent work while other targets remain unresolved. Later
   contributions increment route revision under the same receipt.
4. `WriteStore::apply_route` adds lanes, retires only definite pre-handoff withdrawals, and retains
   attempting or terminal history.
5. Duplicate destination reasons collapse by `RelaySessionKey`. `preview_write_routes` derives the
   initial plan without custody, signing, router sessions, store changes, or transport effects.

### Restart Recovery

1. `RedbWriteStore::open` loads identities/receipts and converts persisted `Attempting` to exact
   `Unknown` ambiguity in `crates/fava-write-store-redb/src/lib.rs`.
2. `FavaBuilder::build` constructs `Publication` and calls `Publication::recover`.
3. Every nonterminal receipt restarts without application resubmission; stable receipt identity and
   committed facts remain authoritative.

### Independent M0 Evidence Flow

1. `apps/canary/src/main.rs` dispatches `lab-real-relay-smoke`.
2. `apps/canary/src/relay.rs` starts a pinned relay and `apps/canary/src/proxy.rs` records frames.
3. `apps/canary/src/wire.rs` publishes, observes OK, queries EVENT+EOSE, hard-kills, restarts the
   same data directory, and repeats the query.
4. `apps/canary/src/artifacts.rs` writes manifest, JSONL, frames, process facts, logs, and hashes
   below ignored `apps/canary/runs/`.

**State Management:**
- Instance-local `Mutex` state owns memory cache/write facts in
  `crates/fava-event-cache-memory/src/lib.rs` and `crates/fava-write-store-memory/src/lib.rs`.
- Immediate Redb transactions own durable receipt and identity facts in
  `crates/fava-write-store-redb/src/lib.rs` and `crates/fava-write-store-redb/src/ops.rs`.
- `watch` retains one latest source/query/route value; `broadcast` retains bounded causal receipt
  changes; route lane completion is bounded by the 256-destination limit.
- Diagnostics retain at most 256 facts per category by default in
  `crates/fava-diagnostics/src/lib.rs`.
- No mutable process-global Fava state exists; facts belong to assembled instances and handles.

## Key Abstractions

**`Query`:**
- Purpose: One valid request covering selection, acquisition, result authority, access, freshness,
  ordering, and result bound.
- Examples: `crates/fava-query/src/lib.rs`, `crates/fava-query/tests/query_identity.rs`.
- Pattern: Immutable builder; invalid relay sets and zero limits fail before work.

**`QuerySource` / `QueryEvaluator`:**
- Purpose: Separate independent complete contributions from merge semantics.
- Examples: `crates/fava-query/src/lib.rs`, `crates/fava-query-standard/src/lib.rs`.
- Pattern: Replaceable source stream plus pure evaluator; cache, write store, and `Fava` reuse it.

**`EventCache` / `WriteStore`:**
- Purpose: Separate learned signed relay state from accepted local obligations.
- Examples: `crates/fava-event-cache/src/lib.rs`, `crates/fava-write-store/src/lib.rs`.
- Pattern: Contract crates own operations; provider crates own representation, capacity, and schema.

**`Router` / `RouterSession` / `RoutePlan`:**
- Purpose: Produce ordered, attributable live relay knowledge for reads and writes.
- Examples: `crates/fava-routing/src/lib.rs`, `crates/fava-routing/src/chain.rs`.
- Pattern: Immediate current plus replacement contributions; core validates bounds and deduplicates.

**`SubscriptionPlanner`:**
- Purpose: Map relay-assigned logical demand into exact messages and inbound attribution.
- Examples: `crates/fava-subscriptions/src/lib.rs`, `crates/fava-subscriptions-standard/src/lib.rs`.
- Pattern: Replaceable exact planner independent from routing and transport.

**`Transport` / `RelaySession`:**
- Purpose: Own connection generations and frame handoff/receive/close behavior.
- Examples: `crates/fava-transport/src/lib.rs`, `crates/fava-transport-websocket/src/lib.rs`.
- Pattern: Object-safe async contract with handed-off, not-handed-off, and ambiguous results.

**`Publication`:**
- Purpose: Order durable receipt facts, signer work, routing, attempts, and recovery.
- Examples: `crates/fava-publication/src/lib.rs`, `crates/fava-publication/src/run.rs`.
- Pattern: Universal owner over neutral contracts; it owns no concrete mechanism.

**`FavaBuilder`:**
- Purpose: Make provider selection explicit and assemble the facade.
- Examples: `crates/fava/src/lib.rs`, `falsifiers/external-null-cache/src/lib.rs`.
- Pattern: Static dependency injection through `Arc<dyn Contract>` with typed refusal.

## Entry Points

**Rust Library Facade:**
- Location: `crates/fava/src/lib.rs`
- Triggers: `Fava::builder`, `observe`, `publish`, route preview, diagnostics, receipt, cancellation,
  recovery inspection, or terminal waiting.
- Responsibilities: Validate assembly, expose public values/handles, and delegate to owners.

**Canary CLI:**
- Location: `apps/canary/src/main.rs`
- Triggers: `list`, `run <scenario>`, `recon`, and internal `crash-child`.
- Responsibilities: Dispatch M0-M6 scenarios, enforce prerequisites, and fail on missing evidence.

**External Provider Proof:**
- Location: `falsifiers/external-null-cache/src/lib.rs`
- Triggers: Its separate workspace test.
- Responsibilities: Implement public contracts outside the main workspace and assemble normally.

**Vocabulary Gate:**
- Location: `tools/check_vocabulary.py`
- Triggers: Direct invocation and `tools/tests/test_vocabulary_check.py`.
- Responsibilities: Reject unregistered crates and public nominal symbols.

## Architectural Constraints

- **Threading:** Tokio tasks own observations, routes, relay sessions, signing, and delivery; no OS
  thread is allocated per query. Live publication/query-source entry points require Tokio
  (`crates/fava-publication/src/lib.rs`, `crates/fava/src/query_source.rs`).
- **Global state:** No product singleton exists. Mutable facts live in provider/lifecycle instances.
- **Circular imports:** Dependencies are acyclic. Value/contract crates do not depend on concrete
  providers or facade; `crates/fava/Cargo.toml` uses contracts and owners only.
- **Static assembly:** Do not add hidden flags, silent defaults, or compatibility aliases to
  `crates/fava/src/lib.rs`.
- **Exact identity:** Use `RelaySessionKey`, generation, `SubscriptionId`, `WriteId`, `ReceiptId`,
  route revision, and attempt count at asynchronous boundaries.
- **Boundedness:** Explicit routes and contributions cap at 256, routers at 32, events at 131,072
  bytes, diagnostics/receipt changes at 256 by default, and WebSocket frames at 1,048,576 bytes.
  Extend with typed refusal, never silent truncation.
- **Storage separation:** Never place unpublished local events in `EventCache`; only verified relay
  echoes enter it.
- **Vocabulary:** New crates, nominal types, contracts, persisted entities, configuration concepts,
  synonyms, and owners require `docs/internals/vocabulary.toml` plus `AGENTS.md` approval.
- **File size:** Rust code uses 500-line soft and 800-line hard limits; current files are at or below 500.
- **Build:** Cargo owns dependency metadata; every main-workspace crate has matching `BUILD.bazel`.
- **Implementation boundary:** M7-M11 names are not implemented contracts. Do not depend on absent
  edit, auth, service-cache, profile, or native crates before their vertical slices.

## Anti-Patterns

### Concrete Defaults in the Facade

**What happens:** `crates/fava/src/lib.rs` constructs a standard provider internally.
**Why it's wrong:** The default gains a private bypass unavailable to external providers.
**Do this instead:** Preserve neutral contracts and require explicit `FavaBuilder` selection.

### Contract/Implementation Collapse

**What happens:** A trait moves into its first implementation crate or its split is deferred.
**Why it's wrong:** Universal owners couple to one mechanism and private state.
**Do this instead:** Follow `crates/fava-transport/` plus `crates/fava-transport-websocket/` and
`crates/fava-write-store/` plus its provider crates.

### Local Write Cache Pollution

**What happens:** An unsigned or locally signed event enters an event-cache provider for visibility.
**Why it's wrong:** It fabricates relay provenance and destroys source ownership.
**Do this instead:** Commit through `WriteStore`; merge real relay echoes only in the evaluator.

### Routing, Planning, and Transport Conflation

**What happens:** A router emits REQ, a planner chooses relays, or transport owns retry policy.
**Why it's wrong:** Destination policy, wire planning, resources, and attempts cannot be substituted.
**Do this instead:** Use `fava-routing`, `fava-subscriptions`, `fava-transport`, and `fava-delivery`
for their separate responsibilities.

### Protocol Meaning in Universal Owners

**What happens:** `EventBuilder`, publication, routing, or facade switches on NIP-specific meaning.
**Why it's wrong:** Adding protocol N+1 changes universal code.
**Do this instead:** Keep protocol behavior in a protocol crate, as `crates/fava-nip65/src/lib.rs`
does, then pass ordinary values through universal owners.

### Acquisition-Provenance Conflation

**What happens:** A planned relay is credited as a source without serving the event.
**Why it's wrong:** Acquisition intent is not result evidence.
**Do this instead:** Record `RelayEvidence` only during exact admitted EVENT handling.

### Coalescing Causal Receipt Facts

**What happens:** Receipt transitions use a latest-value channel and erase committed facts.
**Why it's wrong:** Causal delivery cannot report lag or removal truthfully.
**Do this instead:** Use bounded broadcast in `crates/fava-write-store/src/lib.rs`; reserve `watch`
for current state.

## Error Handling

**Strategy:** Refuse invalid work before effects, preserve typed/scoped owner errors, and turn
post-open degradation into attributable evidence or exact terminal facts.

**Patterns:**
- Constructors reject invalid queries, event bodies, routes, limits, and assembly before work
  (`crates/fava-query/src/lib.rs`, `crates/fava-write/src/lib.rs`, `crates/fava/src/lib.rs`).
- Contracts use role errors such as `EventCacheError`, `WriteStoreError`, `RouterError`,
  `SubscriptionPlanError`, `TransportError`, and `SignerError`, not a common bucket.
- Query open is all-or-nothing; post-open source closure updates `SourceStatus`
  (`crates/fava-observe/src/lib.rs`).
- Router failure becomes bounded shortfall evidence without blocking immediate contributions
  (`crates/fava-routing/src/chain.rs`).
- Transport distinguishes definite refusal, handoff, and ambiguity; publication maps ambiguity to
  `RelayDeliveryOutcome::Unknown`.
- Store transitions validate receipt, route revision, destination, signature, and attempt state
  before commit; stale transitions are refused.
- Relay parse, attribution, verification, filter, CLOSED, NOTICE, and reconnect failures remain
  scoped to exact session/subscription diagnostics in `crates/fava/src/relay.rs`.

## Cross-Cutting Concerns

**Logging:** Product evidence uses bounded `DiagnosticsSnapshot` in
`crates/fava-diagnostics/src/lib.rs`; external effects use independent JSONL, process logs, wire
transcripts, reports, and manifests in `apps/canary/src/artifacts.rs`.

**Validation:** Constructors, contracts, provider commits, ingest, routing composition, and durable
receipt transitions validate exact shape and bounds. Architectural/public API changes run
`tools/check_vocabulary.py` and `tools/tests/test_vocabulary_check.py`.

**Authentication:** `RelayAccess` and AUTH-required diagnostics exist in `crates/fava-state/src/lib.rs`
and `crates/fava-diagnostics/src/lib.rs`; NIP-42 challenge policy/execution are unimplemented M8 scope.

**Durability:** `EventCache` does not promise persistence. `MemoryWriteStore` is volatile;
`RedbWriteStore` owns durable receipt schema, transactions, and recovery. Persistent event and
fetch/service cache profiles are M9 specification only.

---

*Architecture analysis: 2026-08-21*
