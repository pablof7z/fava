<!-- refreshed: 2026-08-20 -->
# Architecture

**Analysis Date:** 2026-08-20

## System Overview

The checkout implements the M0 evidence lab plus an intentionally narrow M1 local-source tracer. The full target is specified but is not yet present. Behavioral authority, architectural ownership, evidence discipline, and sequencing live respectively in `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md`, `docs/spec/ARCHITECTURE.md`, `docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md`, and `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`; `docs/spec/partial-spec-api-semantics.md` refines query-source semantics without outranking those four files. Current milestone status is recorded outside the normative specs in `docs/issues/0001-local-source-merge.md` and `docs/issues/0002-m0-evidence-foundation.md`.

```text
┌──────────────────────────────────────────────────────────────────────┐
│                         Downstream Rust app                          │
│                   `crates/fava/src/lib.rs`                           │
└──────────────────────────────┬───────────────────────────────────────┘
                               │ Fava::observe(EventQuery)
                               ▼
┌──────────────────────────────────────────────────────────────────────┐
│                     Local observation owner                         │
│                 `crates/fava-observe/src/lib.rs`                     │
└───────────────┬──────────────────────────────┬───────────────────────┘
                │                              │
                ▼                              ▼
┌────────────────────────────┐   ┌─────────────────────────────────────┐
│ Neutral source contracts   │   │ Replaceable evaluation contract    │
│ `crates/fava-query/`       │   │ `crates/fava-query/src/lib.rs`     │
└──────────────┬─────────────┘   └──────────────────┬──────────────────┘
               │                                    │
      ┌────────┴────────┐                           ▼
      ▼                 ▼               ┌──────────────────────────────┐
┌──────────────┐  ┌──────────────┐       │ Standard full reevaluator    │
│ Event cache  │  │ Write store  │       │ `crates/fava-query-standard/`│
│ contracts +  │  │ contracts +  │       └──────────────┬───────────────┘
│ memory impl  │  │ memory impl  │                      │
└──────┬───────┘  └──────┬───────┘                      │
       └────────────┬─────┴──────────────────────────────┘
                    ▼
          one merged QuerySnapshot / EventRecord
             `crates/fava-query/src/lib.rs`

Separate evidence boundary:

`apps/canary/src/main.rs` -> relay supervisor `apps/canary/src/relay.rs`
    -> transparent proxy `apps/canary/src/proxy.rs`
    -> independent NIP-01 witness `apps/canary/src/wire.rs`
    -> preserved run bundle `apps/canary/src/artifacts.rs`
```

The implemented dependency direction is acyclic: semantic crates `crates/fava-state/` and `crates/fava-write/` feed the query vocabulary in `crates/fava-query/`; neutral contracts in `crates/fava-event-cache/` and `crates/fava-write-store/` depend on that vocabulary; implementations in `crates/fava-event-cache-memory/`, `crates/fava-write-store-memory/`, and `crates/fava-query-standard/` depend on the contracts or semantic owners; `crates/fava-observe/` owns orchestration; and `crates/fava/` is the facade. Cargo remains the dependency-metadata authority in `Cargo.toml` and `Cargo.lock`; Bazel mirrors first-party edges explicitly in targets such as `crates/fava-query/BUILD.bazel`, `crates/fava-observe/BUILD.bazel`, and `crates/fava/BUILD.bazel`, and imports the locked third-party graph through `MODULE.bazel`.

## Component Responsibilities

| Component | Responsibility | File |
|-----------|----------------|------|
| Behavioral authority | Defines required live-query/write-intent behavior and universal invariants; it does not report implementation status. | `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md` |
| Target architecture | Assigns target owners, crate families, dependency direction, flows, and falsifiers; many named crates remain specification-only. | `docs/spec/ARCHITECTURE.md` |
| Delivery authority | Sequences vertical slices and milestone exit gates; M0 precedes any M1 completion claim. | `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md` |
| Evidence authority | Requires behavior-first TDD/BDD, narrow owner tests, deliberate breaks, and public capstones. | `docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md` |
| `fava-state` | Owns implemented relay-evidence values, event coordinates, and deterministic same-coordinate winner comparison. | `crates/fava-state/src/lib.rs` |
| `fava-write` | Owns implemented write/receipt identifiers, unsigned-or-signed event values, and local publication evidence. | `crates/fava-write/src/lib.rs` |
| `fava-query` | Owns query descriptions, source policy, source observation contracts, `EventRecord`, `QuerySnapshot`, and evaluator contract. | `crates/fava-query/src/lib.rs` |
| `fava-query-standard` | Supplies the current merge oracle: same-id evidence merge, coordinate winner selection, authority filtering, ordering, and limits. | `crates/fava-query-standard/src/lib.rs` |
| `fava-event-cache` | Defines the neutral event-cache provider contract as a specialized `QuerySource`. | `crates/fava-event-cache/src/lib.rs` |
| `fava-event-cache-memory` | Owns bounded current-process relay-event retention, atomic batch application, source revisions, and latest snapshots. | `crates/fava-event-cache-memory/src/lib.rs` |
| `fava-write-store` | Defines the current neutral contract for accepting/cancelling already-materialized local events and exposing them as a `QuerySource`. | `crates/fava-write-store/src/lib.rs` |
| `fava-write-store-memory` | Owns bounded volatile accepted-local-event state, write/receipt allocation, cancellation, source revisions, and latest snapshots. | `crates/fava-write-store-memory/src/lib.rs` |
| `fava-observe` | Owns all-or-nothing opening of both local sources, current source state, reevaluation, query revisions, coalesced delivery, and close. | `crates/fava-observe/src/lib.rs` |
| `fava` | Provides explicit provider assembly and the current public `observe` facade; it silently selects no provider. | `crates/fava/src/lib.rs` |
| M0 canary | Runs a separate-process relay, independent proxy/wire witness, kill/restart scenario, reconnaissance, and evidence persistence without depending on Fava crates. | `apps/canary/src/lib.rs` |
| External-provider falsifier | Proves an outside-workspace null event cache can implement public contracts and assemble through the facade. | `falsifiers/external-null-cache/src/lib.rs` |

## Pattern Overview

**Overall:** Statically assembled, layered library with semantic-owner crates, neutral provider contracts, replaceable implementations, and lifecycle-owner orchestration (`Cargo.toml`, `docs/spec/ARCHITECTURE.md`).

**Key Characteristics:**
- Keep one semantic owner for every type and mutable lifecycle; implemented owners are visible in `crates/fava-state/src/lib.rs`, `crates/fava-write/src/lib.rs`, `crates/fava-query/src/lib.rs`, `crates/fava-event-cache-memory/src/lib.rs`, `crates/fava-write-store-memory/src/lib.rs`, and `crates/fava-observe/src/lib.rs`.
- Keep contract crates separate from implementation crates; the current pairs are `crates/fava-event-cache/` with `crates/fava-event-cache-memory/`, and `crates/fava-write-store/` with `crates/fava-write-store-memory/`.
- Assemble providers statically through `FavaBuilder`; do not add runtime plugin registries or hidden defaults to `crates/fava/src/lib.rs`.
- Merge event-cache and write-store contributions only through the evaluator contract in `crates/fava-query/src/lib.rs` and the current oracle in `crates/fava-query-standard/src/lib.rs`.
- Treat complete current snapshots as the public observation model; `tokio::sync::watch` remains private to providers and the observation owner in `crates/fava-event-cache-memory/src/lib.rs`, `crates/fava-write-store-memory/src/lib.rs`, and `crates/fava-observe/src/lib.rs`.
- Stabilize boundaries through vertical slices and competing implementations, not empty frameworks; the required sequence is in `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`, and the current external challenge is `falsifiers/external-null-cache/src/lib.rs`.
- Keep build topology aligned with architecture: declare dependency metadata once in `Cargo.toml` and `Cargo.lock`, mirror first-party crate edges in per-crate targets such as `crates/fava-state/BUILD.bazel` and `crates/fava/BUILD.bazel`, and make `bazel test //...` the authoritative main-workspace build through `.bazelrc` and `MODULE.bazel`.

## Implemented State Versus Specified Target

| Status | Scope | Evidence |
|--------|-------|----------|
| Implemented M0 | Independent real-relay evidence lab, enabled `lab-real-relay-smoke`, bounded public reconnaissance, and reconstructable artifacts. | `apps/canary/src/lib.rs`, `apps/canary/scenarios.json`, `docs/issues/0002-m0-evidence-foundation.md`, `features/relay-lab.feature` |
| Implemented tracer | Local query values, independent cache/write sources, full reevaluation, latest-state observation, memory providers, and thin observe facade. | `crates/fava-query/src/lib.rs`, `crates/fava-query-standard/src/lib.rs`, `crates/fava-observe/src/lib.rs`, `crates/fava/src/lib.rs` |
| M1 incomplete | Stable equivalent-query sharing, full deletion/expiry semantics, the named local-source-removal canary, and the complete shared provider corpus are not claimed complete. | `docs/issues/0001-local-source-merge.md`, `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md` |
| Specified only | Wire/ingest, routing, subscription planning, transport, publication, delivery, signing, sessions, auth, diagnostics, persistent providers, services, capabilities, runtime/coordinator, Swift, and Kotlin remain target architecture. | `docs/spec/ARCHITECTURE.md`, `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md` |

## Layers

**Authoritative Design Layer:**
- Purpose: Own required behavior, responsibility allocation, proof discipline, and sequencing without mixing status into normative text.
- Location: `docs/spec/`
- Contains: `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md`, `docs/spec/ARCHITECTURE.md`, `docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md`, `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`, and `docs/spec/partial-spec-api-semantics.md`.
- Depends on: No implementation crate; authority order is indexed by `docs/spec/README.md`.
- Used by: Local slice records in `docs/issues/`, behavior features in `features/`, and implementation under `crates/` and `apps/canary/`.

**Semantic Value Layer:**
- Purpose: Define stable protocol/query/write values and deterministic rules without provider resources.
- Location: `crates/fava-state/`, `crates/fava-write/`, `crates/fava-query/`
- Contains: Relay evidence and coordinates in `crates/fava-state/src/lib.rs`; write identity and local materializations in `crates/fava-write/src/lib.rs`; query/source/result contracts in `crates/fava-query/src/lib.rs`.
- Depends on: `nostr` plus lower semantic owners as declared in `crates/fava-state/Cargo.toml`, `crates/fava-write/Cargo.toml`, and `crates/fava-query/Cargo.toml`.
- Used by: Contracts, providers, observation, facade, and tests under `crates/`.

**Neutral Provider Contract Layer:**
- Purpose: Name one replaceable storage responsibility per trait while reusing `QuerySource` for continuous local state.
- Location: `crates/fava-event-cache/`, `crates/fava-write-store/`
- Contains: `EventCache` in `crates/fava-event-cache/src/lib.rs` and `WriteStore` in `crates/fava-write-store/src/lib.rs`.
- Depends on: Semantic/query crates declared in `crates/fava-event-cache/Cargo.toml` and `crates/fava-write-store/Cargo.toml`.
- Used by: Memory providers, facade builder, acceptance tests, and the external falsifier in `falsifiers/external-null-cache/`.

**Provider and Policy Layer:**
- Purpose: Supply replaceable algorithms/resources without changing universal meanings.
- Location: `crates/fava-event-cache-memory/`, `crates/fava-write-store-memory/`, `crates/fava-query-standard/`
- Contains: Two bounded in-memory authorities and one deliberately simple full-reevaluation oracle in their respective `src/lib.rs` files.
- Depends on: Neutral contracts and semantic values via each crate's `Cargo.toml`.
- Used by: Public acceptance tests in `crates/fava/tests/local_source_merge.rs`; implementations are dev-dependencies rather than facade dependencies in `crates/fava/Cargo.toml`.

**Lifecycle Owner Layer:**
- Purpose: Order source opening, evaluate coherent current state, own a query's live task, and close provisional or installed resources.
- Location: `crates/fava-observe/`
- Contains: `Observer`, `Observation`, typed open errors, source-state replacement, and bounded watch delivery in `crates/fava-observe/src/lib.rs`.
- Depends on: Only the neutral query/evaluator vocabulary declared in `crates/fava-observe/Cargo.toml`.
- Used by: The facade in `crates/fava/src/lib.rs`.

**Facade and Product Assembly Layer:**
- Purpose: Validate explicit provider selection and expose the current public workload entry point.
- Location: `crates/fava/`
- Contains: `Fava`, `FavaBuilder`, and `BuildError` in `crates/fava/src/lib.rs`.
- Depends on: Contract and owner crates only in normal dependencies declared by `crates/fava/Cargo.toml`; concrete providers are test-only dev-dependencies there.
- Used by: Downstream applications and public acceptance evidence in `crates/fava/tests/local_source_merge.rs` and `falsifiers/external-null-cache/src/lib.rs`.

**Evidence and Falsification Layer:**
- Purpose: Prove public behavior with BDD examples, an independent real-relay application, and outside-workspace provider substitution.
- Location: `features/`, `apps/canary/`, `falsifiers/`, `docs/issues/`
- Contains: Behavior declarations in `features/local-source-merge.feature` and `features/relay-lab.feature`; canary orchestration in `apps/canary/src/`; external provider proof in `falsifiers/external-null-cache/src/lib.rs`; slice status in `docs/issues/`.
- Depends on: The canary has its own workspace and no Fava dependency in `apps/canary/Cargo.toml`; the falsifier has its own workspace and consumes only public crates in `falsifiers/external-null-cache/Cargo.toml`.
- Used by: Milestone exit decisions defined in `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`.

## Data Flow

### Primary Request Path: Open a Local Query

1. The application calls `Fava::observe` through the public facade (`crates/fava/src/lib.rs:33`).
2. `Observer::open` canonicalizes the query, opens the event-cache source, then opens the write-store source; a second-source refusal explicitly closes the first provisional source (`crates/fava-observe/src/lib.rs:41`).
3. `Observation::start` evaluates both initial snapshots before installing the observation and assigns revision 1 (`crates/fava-observe/src/lib.rs:73`).
4. The selected evaluator merges current source contributions into one `QuerySnapshot`; the standard implementation deduplicates by event id and then selects one winner per coordinate (`crates/fava-query-standard/src/lib.rs:17`, `crates/fava-query-standard/src/lib.rs:64`).
5. The caller reads the complete current value immediately with `Observation::current` and awaits coalesced newer current values with `Observation::changed` (`crates/fava-observe/src/lib.rs:149`, `crates/fava-observe/src/lib.rs:159`).

### Local Write Visibility and Cancellation

1. In the current tracer, the caller/test retains the selected provider and calls `MemoryWriteStore::accept_materialized` directly; the facade does not yet expose the target write-intent lifecycle (`crates/fava-write-store-memory/src/lib.rs:80`, `crates/fava/tests/local_source_merge.rs`).
2. The store validates a stable event id, allocates matching write/receipt identities, commits the local contribution and new source revision under one mutex, then replaces the bounded latest snapshot (`crates/fava-write-store-memory/src/lib.rs:80`, `crates/fava-write/src/lib.rs`).
3. The observation task receives the complete write-source snapshot, reevaluates both sources, and publishes a new complete query revision (`crates/fava-observe/src/lib.rs:73`).
4. Cancellation removes only the write-store contribution and emits another source revision; the evaluator can therefore reveal a still-retained cached predecessor naturally (`crates/fava-write-store-memory/src/lib.rs:124`, `crates/fava-query-standard/tests/source_merge.rs`).

### Relay-Evidence Enrichment in the M1 Tracer

1. Tests seed a canonical `CacheMutation` directly because production wire admission is a later slice (`crates/fava-event-cache-memory/src/lib.rs:64`, `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`).
2. `MemoryEventCache` applies the entire batch to a cloned next state, refuses capacity/conflicting-body errors before replacing current state, increments its source revision, and publishes a complete snapshot (`crates/fava-event-cache-memory/src/lib.rs:64`).
3. `StandardQueryEvaluator` merges the cache's relay evidence with the write store's publication evidence for the same event id, producing one `EventRecord` (`crates/fava-query-standard/src/lib.rs:64`, `crates/fava-query/src/lib.rs:376`).

### Independent M0 Relay-Lab Flow

1. The CLI selects `run lab-real-relay-smoke` in `apps/canary/src/main.rs` and dispatches to `run_real_relay_smoke` (`apps/canary/src/lib.rs:162`).
2. The canary creates an isolated run bundle, launches a pinned third-party relay, and starts a transparent proxy (`apps/canary/src/artifacts.rs`, `apps/canary/src/relay.rs`, `apps/canary/src/proxy.rs`).
3. Independent wire code signs/publishes an event, observes matching `OK`, queries to exact `EVENT` plus `EOSE`, hard-kills the relay, restarts the same data directory, and repeats the query (`apps/canary/src/lib.rs:229`, `apps/canary/src/wire.rs`).
4. The canary records JSONL evidence, process facts, wire frames, logs, hashes, and a manifest below ignored `apps/canary/runs/` (`apps/canary/src/artifacts.rs`, `.gitignore`).

**State Management:**
- Provider state is instance-local, not global: `MemoryEventCache` uses `Mutex<CacheState>` in `crates/fava-event-cache-memory/src/lib.rs`, and `MemoryWriteStore` uses `Mutex<WriteState>` in `crates/fava-write-store-memory/src/lib.rs`.
- Each source owns a monotonic `SourceRevision`; each observation owns delivered `QueryRevision` and a private vector of current source snapshots in `crates/fava-query/src/lib.rs` and `crates/fava-observe/src/lib.rs`.
- Provider-to-observer delivery uses single-value Tokio watch channels, so slow consumers coalesce intermediates while retaining exact current state in `crates/fava-event-cache-memory/src/lib.rs`, `crates/fava-write-store-memory/src/lib.rs`, and `crates/fava-observe/src/lib.rs`.
- The facade stores only the configured `Observer`; selected providers remain ordinary `Arc` instances owned by the assembly/caller in `crates/fava/src/lib.rs`.

## Key Abstractions

**`EventQuery` / `CanonicalQuery`:**
- Purpose: Carry inert selection, acquisition scope, result authority, access context, freshness, ordering, and result bounds.
- Examples: `crates/fava-query/src/lib.rs`, `docs/spec/partial-spec-api-semantics.md`.
- Pattern: Builder-like immutable description followed by typed validation/canonicalization before any source work (`crates/fava-query/src/lib.rs:205`).

**`QuerySource`:**
- Purpose: Establish one coherent initial `SourceSnapshot` plus one continuous stream of complete later source revisions.
- Examples: Contract in `crates/fava-query/src/lib.rs:348`; implementations in `crates/fava-event-cache-memory/src/lib.rs:125`, `crates/fava-write-store-memory/src/lib.rs:161`, and `falsifiers/external-null-cache/src/lib.rs`.
- Pattern: Neutral, closeable provider contract returning semantic values rather than backend handles.

**`QueryEvaluator`:**
- Purpose: Own exact matching, source merge, coordinate winner selection, ordering, and whole-query limits.
- Examples: Contract in `crates/fava-query/src/lib.rs:475`; reference oracle in `crates/fava-query-standard/src/lib.rs`.
- Pattern: Stateless replaceable policy over complete immutable source snapshots.

**`EventCache` and `WriteStore`:**
- Purpose: Specialize `QuerySource` into independent storage authorities for relay-observed signed events and accepted local materializations respectively.
- Examples: `crates/fava-event-cache/src/lib.rs`, `crates/fava-write-store/src/lib.rs`.
- Pattern: Contract crate plus provider crate; never merge the two roles because they may share physical storage later.

**`EventRecord`:**
- Purpose: Present one logical event plus independently merged relay and publication evidence.
- Examples: Type in `crates/fava-query/src/lib.rs:376`; merge proof in `crates/fava-query-standard/tests/source_merge.rs`.
- Pattern: Application-domain value; do not expose cache rows, store rows, or source precedence to callers.

**`Observer` / `Observation`:**
- Purpose: Own all-or-nothing query opening, current merged state, revision delivery, and exact source closure.
- Examples: `crates/fava-observe/src/lib.rs:14`, `crates/fava-observe/src/lib.rs:67`.
- Pattern: One lifecycle owner above neutral sources and evaluator; runtime primitives remain private.

**`FavaBuilder`:**
- Purpose: Require one event cache, one write store, and one evaluator for the current slice.
- Examples: `crates/fava/src/lib.rs:40`, external composition in `falsifiers/external-null-cache/src/lib.rs`.
- Pattern: Static construction-time composition with typed missing-role refusal.

## Entry Points

**Rust Library Facade:**
- Location: `crates/fava/src/lib.rs`
- Triggers: A downstream Rust application calls `Fava::builder`, supplies providers, then calls `Fava::observe` (`crates/fava/src/lib.rs:22`, `crates/fava/src/lib.rs:33`).
- Responsibilities: Validate the current assembly and delegate local observation without importing concrete providers (`crates/fava/Cargo.toml`).

**Canary CLI:**
- Location: `apps/canary/src/main.rs`
- Triggers: `fava-e2e-canary list`, `run lab-real-relay-smoke`, or `recon --relay ...` as documented in `apps/canary/README.md`.
- Responsibilities: Dispatch enabled/evidence-only scenarios and exit nonzero on orchestration or evidence failure (`apps/canary/src/main.rs`).

**Canary Library:**
- Location: `apps/canary/src/lib.rs`
- Triggers: The CLI or canary tests call `run_real_relay_smoke`, `run_public_recon`, or `scenario_registry` (`apps/canary/src/lib.rs:116`, `apps/canary/src/lib.rs:132`, `apps/canary/src/lib.rs:162`).
- Responsibilities: Own scenario orchestration while delegating process, proxy, wire, reconnaissance, and artifacts to sibling modules under `apps/canary/src/`.

**External Provider Proof:**
- Location: `falsifiers/external-null-cache/src/lib.rs`
- Triggers: The separate falsifier workspace runs its test through `falsifiers/external-null-cache/Cargo.toml`.
- Responsibilities: Compile an outside-workspace provider against public contracts and open it through `FavaBuilder` without private access.

## Architectural Constraints

- **Threading:** The library uses async Tokio tasks and bounded watch channels; `Observation::start` spawns one task per current observation, while provider mutation is synchronous under short `std::sync::Mutex` critical sections (`crates/fava-observe/src/lib.rs:73`, `crates/fava-event-cache-memory/src/lib.rs`, `crates/fava-write-store-memory/src/lib.rs`).
- **Global state:** No product-global mutable singleton is implemented; state belongs to `Fava`, `Observer`, each provider instance, each `Observation`, or each canary run object in `crates/fava/src/lib.rs`, `crates/fava-observe/src/lib.rs`, `crates/fava-event-cache-memory/src/lib.rs`, `crates/fava-write-store-memory/src/lib.rs`, and `apps/canary/src/artifacts.rs`.
- **Circular imports:** The current workspace dependency graph in `Cargo.toml`, `crates/fava-query/Cargo.toml`, `crates/fava-observe/Cargo.toml`, and `crates/fava/Cargo.toml` is one-way from semantic values to contracts/providers to owner/facade; no crate cycle is present.
- **Build graph:** Bazel is the authoritative build/test surface for the main workspace in `.bazelrc`; `MODULE.bazel` imports the dependency graph from `Cargo.toml` and `Cargo.lock`, while first-party edges remain explicit in targets such as `crates/fava-query-standard/BUILD.bazel` and `crates/fava/BUILD.bazel`. Keep each crate's `Cargo.toml` and `BUILD.bazel` synchronized when crate edges or test targets change.
- **Build platform:** `MODULE.bazel` configures only `aarch64-apple-darwin` and pins Rust 1.90.0; the separate workspaces at `apps/canary/Cargo.toml` and `falsifiers/external-null-cache/Cargo.toml` are not represented by the root `BUILD.bazel` or any child Bazel package.
- **Authority:** Behavior and ownership must follow `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md` before illustrative names in `docs/spec/ARCHITECTURE.md`; proof and sequencing follow `docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md` and `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`.
- **Clean-room boundary:** Add no compatibility path or copied prior implementation; the repository rule is explicit in `AGENTS.md`, and active sources are under `crates/`, `apps/`, `falsifiers/`, and `features/`.
- **File size:** Keep Rust code below the 800-line hard limit and require a cohesion reason above 500 lines as mandated by `AGENTS.md`; current product Rust files are below 500 lines.
- **Replaceability:** Universal owners and the facade depend on contracts rather than memory providers in `crates/fava-observe/Cargo.toml` and the normal dependency section of `crates/fava/Cargo.toml`.
- **Failure isolation:** Do not hold provider locks/transactions across external async work; current mutex guards are confined to synchronous memory-provider mutations in `crates/fava-event-cache-memory/src/lib.rs` and `crates/fava-write-store-memory/src/lib.rs`.
- **Boundedness:** Preserve explicit query limits, provider capacities, coalesced observation delivery, and canary deadlines/frame limits in `crates/fava-query/src/lib.rs`, both memory-provider `src/lib.rs` files, `crates/fava-observe/src/lib.rs`, and `apps/canary/src/wire.rs`.
- **Milestone claims:** Do not claim M1 or later from individual passing tests; `docs/issues/0001-local-source-merge.md` names the tracer's remaining M1 gates, and `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md` owns exit criteria.

## Anti-Patterns

### Facade-Bypassing Product Evidence

**What happens:** Current M1 acceptance tests retain `Arc<MemoryWriteStore>` and call `accept_materialized`/`cancel` directly because the facade exposes only observation (`crates/fava/tests/local_source_merge.rs`, `crates/fava/src/lib.rs`).
**Why it's wrong:** This proves source merge but not the specified ordinary application write-intent path or the M1 canary's public-facade gate (`docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`, `docs/issues/0001-local-source-merge.md`).
**Do this instead:** Keep direct provider mutation as narrow provider/owner evidence; when the write slice is implemented, route public capstones through the owning facade operation in `crates/fava/src/lib.rs` and leave provider-specific setup in component tests under the relevant crate.

### Treating Direct Cache Commit as Relay Admission

**What happens:** Current local-source tests insert `CacheMutation` directly through `EventCache::commit` because wire verification and ingest do not exist yet (`crates/fava/tests/local_source_merge.rs`, `crates/fava-event-cache/src/lib.rs`).
**Why it's wrong:** A direct cache mutation does not prove session attribution, signature verification, off-filter rejection, or the production relay path required by `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md`.
**Do this instead:** Keep direct commits as M1 state fixtures; introduce relay-originated state only through the future `fava-wire`/`fava-ingest` slice prescribed by `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md` and owned in `docs/spec/ARCHITECTURE.md`.

### Collapsing Post-Open Failures to Closure

**What happens:** A post-open evaluator error breaks the observation task, and callers receive only `ObservationClosed`; source-stream closure is retained as scoped `SourceStatus`, but evaluator cause is not (`crates/fava-observe/src/lib.rs`).
**Why it's wrong:** The target requires typed, attributable failure rather than losing which owner failed (`docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md`, `docs/spec/ARCHITECTURE.md`).
**Do this instead:** When post-open provider/evaluator failure semantics are expanded, preserve the owning failure as bounded query evidence or a typed terminal outcome in `crates/fava-query/src/lib.rs` and project it through `crates/fava-observe/src/lib.rs`.

### Speculative Empty Frameworks

**What happens:** The target architecture names many future owners, but only Slice 1 contracts have implementation evidence (`docs/spec/ARCHITECTURE.md`, `docs/issues/0001-local-source-merge.md`).
**Why it's wrong:** Stabilizing all future traits before a full vertical slice and competing implementation would encode guesses and violate the delivery sequence (`docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md`, `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`).
**Do this instead:** Add the next contract and implementation together with its narrow failing proof, public capstone where required, and external/alternative falsifier in the locations prescribed by `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`.

## Error Handling

**Strategy:** Return typed, owner-scoped library refusals before effects; preserve atomic provider state on refusal; represent expected post-open source termination in query evidence; make the canary fail closed with a nonzero process result and preserved failure artifacts (`crates/fava-query/src/lib.rs`, `crates/fava-event-cache/src/lib.rs`, `crates/fava-write-store/src/lib.rs`, `crates/fava-observe/src/lib.rs`, `apps/canary/src/main.rs`, `apps/canary/src/lib.rs`).

**Patterns:**
- Use per-owner `thiserror` enums such as `QueryError`, `QuerySourceError`, `QueryEvaluationError`, `EventCacheError`, `WriteStoreError`, `ObserveError`, and `BuildError` in the corresponding `src/lib.rs` files under `crates/fava-*`.
- Refuse invalid explicit relay sets, mismatched authority, and zero limits during query construction/canonicalization before opening sources in `crates/fava-query/src/lib.rs`.
- Close already-opened provisional sources if the second source or initial evaluation fails in `crates/fava-observe/src/lib.rs:41` and `crates/fava-observe/src/lib.rs:73`.
- Clone memory-provider state, apply the complete mutation, and replace current state only after all checks pass in `crates/fava-event-cache-memory/src/lib.rs`; validate before modifying write-store identity/revision in `crates/fava-write-store-memory/src/lib.rs`.
- Preserve post-open source closure as `SourceStatus::Closed` while retaining its last coherent snapshot in `crates/fava-query/src/lib.rs` and `crates/fava-observe/src/lib.rs`.
- Convert canary subsystem failures into one `CanaryError`, record failure evidence/report where a run bundle exists, and exit nonzero in `apps/canary/src/lib.rs` and `apps/canary/src/main.rs`.

## Cross-Cutting Concerns

**Logging:** The library crates under `crates/` do not implement a logging subsystem. The evidence application writes ordered JSONL, stdout/stderr, reports, process facts, resource samples, and wire frames through `apps/canary/src/artifacts.rs`, `apps/canary/src/proxy.rs`, and `apps/canary/src/lib.rs`.
**Validation:** Query/source-policy validation lives in `crates/fava-query/src/lib.rs`; event-value identity validation lives in `crates/fava-write/src/lib.rs`; provider capacity/atomicity checks live in the memory provider crates; independent canary wire events are cryptographically verified in `apps/canary/src/wire.rs`. Production relay admission remains unimplemented according to `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`.
**Authentication:** No Fava authentication owner or NIP-42 product path is implemented; the target boundary is specified in `docs/spec/ARCHITECTURE.md`, while the current M0 relay lab explicitly disables NIP-42 in generated configuration from `apps/canary/src/relay.rs`.
**Diagnostics:** No product diagnostics facade is implemented in `crates/`; current external evidence comes from `apps/canary/`, and future bounded diagnostics ownership is specified in `docs/spec/ARCHITECTURE.md` and sequenced by `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`.
**Testing and proof:** Put durable app-visible behavior in `features/`, narrow executable evidence beside the owning crate, public Rust composition evidence in `crates/fava/tests/`, independent relay proof in `apps/canary/`, and public-contract substitution proof in `falsifiers/`, following `docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md`.

---

*Architecture analysis: 2026-08-20*
