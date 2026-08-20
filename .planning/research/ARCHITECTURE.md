# Architecture Patterns

**Domain:** Embeddable cross-platform Nostr client engine
**Project:** Fava
**Researched:** 2026-08-21
**Confidence:** HIGH for repository-owned target and implemented-state claims; MEDIUM for external implementation guidance

## Research Posture

The specified architecture is the recommendation. Research found no reason to redesign validated M0 or weaken the required direction:

```text
semantic values and pure rules
            ↑
neutral public contracts
            ↑
replaceable providers

semantic values + contracts
            ↑
single lifecycle owners
            ↑
coordinator / thin facade / native projection
```

The open work is to introduce that architecture vertically from the incomplete M1 tracer through M11, challenge each new boundary with a competing implementation, and prove public behavior independently. Crate names and illustrative signatures remain provisional; ownership, dependency direction, lifecycle semantics, and milestone exit gates are authoritative.

| Dimension | Specified Target | Implemented State | Research Recommendation |
|-----------|------------------|-------------------|-------------------------|
| Evidence | Independent real processes, public-facade canary, conformance and mutation evidence | M0 real-relay lab is complete | Preserve M0 as an external witness; extend scenarios without importing Fava internals |
| Local query state | Deterministic merge of independent event-cache and write-store sources | Narrow M1 tracer exists | Finish M1 semantics and public-facade evidence before networking claims |
| Providers | Neutral contract crate, implementation crate, public conformance corpus | Cache/write-store splits and one external null cache exist | Add each later contract with its first vertical use and competing falsifier; never collapse the split |
| Async lifecycles | One owner per observation, route, session, publication, auth operation, and runtime resource | One task per local observation with latest-state delivery | Introduce hierarchical identity, cancellation, bounded queues, and joins as each owner appears |
| Durability | Write store owns accepted obligations and receipts; facts commit before effects | Volatile memory write store only | Select the standard durable provider only after crash-boundary qualification |
| Native SDKs | Rust owns state; Swift/Kotlin project idiomatic handles, errors, and async behavior | Not implemented | Make native wrappers projections, never second lifecycle owners; require real-process parity at M11 |

## Recommended Architecture

```text
                                  APPLICATION
                                      │
                           Rust / Swift / Kotlin facade
                                      │
                                      ▼
                           coordinator + command admission
                 ┌────────────────────┼────────────────────┐
                 ▼                    ▼                    ▼
             observe owner       publication owner     session/auth owners
                 │                    │                    │
      ┌──────────┼──────────┐         ├── durable WriteStore
      ▼          ▼          ▼         ├── live RoutePlan
 EventCache   WriteStore  Query       ├── signer operation
 QuerySource  QuerySource Evaluator   ├── delivery lanes/policy
      │          │          │         └── publisher attempt
      └──────────┴──────────┘                    │
                 │                              ▼
        complete current view             RelayTransport
                 ▲                              │
                 │                              ▼
 relay bytes → wire decode → attributed ingest → relay session generation
                             │
                             └→ verified state decision → EventCache commit

 runtime executes authorized work, applies bounds, and returns correlated completions;
 it does not own query, route, publication, protocol, or storage meaning.
```

Static application composition selects one provider per semantic role and an ordered list of policy providers. The selected set is fixed for an engine instance and compiled into native artifacts. No runtime plugin registry or hidden standard-provider bypass is needed.

### Component Boundaries

| Component | Responsibility / Authority | Communicates With | First Product Gate |
|-----------|----------------------------|-------------------|--------------------|
| `fava-wire` | Canonical Nostr frame values, encoding, decoding, and encoded bounds | transport, publisher, ingest | M2 |
| `fava-state` | Pure event identity, replacement, deletion, expiry, tombstone, and relay-evidence decisions | ingest, evaluator, event-cache corpus | M1 |
| `fava-query` + evaluator contract | Query identity, source protocol, merge, ordering, limits, and evidence vocabulary | observe, query sources, planners/routers through services | M1 |
| `fava-write` / capability values | Event construction, write identity, receipt and publication facts; protocol-owned edits carry actor and format | publication, write store, capability crates | M1 values; M5 lifecycle; M7 edits |
| `EventCache` contract/provider | Retained admitted signed relay state and declared cache guarantees | ingest, observe as `QuerySource` | M1 baseline; M9 persistence |
| `WriteStore` contract/provider | Accepted obligations, current materializations, route revisions, delivery facts, receipts, recovery | publication, observe as `QuerySource` | M1 baseline; M5 durability |
| `FetchCache` contract/provider | Opaque namespaced bytes only | NIP-05/NIP-11 service owners | M9 |
| `fava-observe` | One observation lifecycle, source coherence, current projection, route demand, bounded app delivery, teardown | query sources, routing, planner, transport, facade | M1 local; M2-M4 relay work |
| `fava-routing` session | Ordered reactive composition and one current attributed route plan | routers, observe, publication | M4 |
| Router implementation | Its own input acquisition, derived state, contribution, and cancellation | routing through public `RouterServices` | M4/M6 |
| Subscription planner | Pure logical-demand-to-wire-plan calculation and exact shortfall | observe, transport | M2 no-grouping; M4 standard |
| Relay transport | Physical socket/session generation, bounded byte queues, handoff truth, reconnect, close/join | wire, ingest, publisher, auth | M2-M3 |
| `fava-ingest` | Attribution, bounds, event id/signature/filter verification, serialized admission order | transport, wire, state, event cache | M2 |
| `fava-publication` | One accepted write from commit through materialization, signing, route revisions, attempts, cancellation, settlement, and recovery | write store and all publication contracts | M5-M7 |
| Publisher | One protocol attempt for one exact event/session/attempt | transport, publication owner | M5 |
| Delivery policy | Pure attempt/wait/park/give-up decision over committed lane facts | publication owner | M5-M6 |
| Session/signer/auth owners | Account attachment, exact signing operation, access-context and challenge lifecycles | publication, transport, facade | M5/M8 |
| Runtime/coordinator | Bounded execution, timers, cancellation propagation, failure/panic isolation, startup/shutdown barriers, joins | every lifecycle owner | Incremental from M2; hardened M8 |
| Diagnostics | Bounded typed projection of owner facts, never a policy or truth backdoor | owners, facade, canary | Added with each milestone |
| `fava` / `fava-standard` | Thin public commands and explicit assembly; standard profile documents selected guarantees | all universal owners/contracts | Every public capstone |
| FFI/native packages | Value/error/handle projection and platform packaging | public Rust facade only | M11 |
| Canary/falsifiers | Independent process, wire, restart, public API, and external-provider evidence | released/public artifacts only | M0 and every later gate |

### Ownership and Correlation Keys

Exact identity should be opaque, allocated by the owner, persisted when it guards durable facts, and carried on every completion. It must not be reconstructed from relay URL, event body, task handle, or current map membership.

| Lifecycle / Fact | Minimum Correlation Identity | Stale Rule |
|------------------|------------------------------|------------|
| Source contribution | provider/source instance + `SourceRevision` | Reject or ignore revisions older than the installed source revision |
| Observation | engine + `ObservationId` + installed query identity | A closed observation receives no later delivery; shared dependencies keep independent owner refs |
| Route session | owner operation + `RouteSessionId` + `RouteRevision` | A replacement contribution supersedes only that router instance's previous snapshot |
| Relay connection | normalized relay + access context + `SessionGeneration` | Frames/handoffs from a retired generation remain evidence but cannot mutate current work |
| Wire subscription | session generation + wire subscription id + planner generation | Attribute only to installed logical demand for that exact plan |
| Materialization | `WriteId` + `ReceiptId` + materialization generation + event id | Signer/router/publisher results for a retired generation cannot advance the current write |
| Delivery attempt | materialization identity + relay session + `AttemptId` | Commit the result only if the lane and attempt remain current; retain historical facts separately |
| Authentication | access context + session generation + challenge identity | A challenge or authenticated state dies with its connection or a replacement challenge |
| Native handle | engine + object handle + Rust lifecycle state | Native deallocation/cancellation requests close the Rust owner idempotently; they do not create new truth |

### Data Flow

#### 1. Coherent Query Open and Update

```text
validate/canonicalize query before work
        ↓
allocate provisional ObservationId
        ↓
open continuous EventCache and WriteStore sources
        ↓
buffer source changes while initial snapshots are assembled
        ↓
evaluate one complete merged snapshot
        ↓
install observation owner, then expose handle/current value
        ↓
route/plan/transport work contributes later admitted facts
        ↓
owner reevaluates affected branches and replaces current state
```

If a provisional open fails, the observation owner closes every resource it already opened. Source failure remains scoped evidence or a typed terminal result; it does not erase another source's retained truth.

#### 2. Relay Ingest

```text
bytes + exact SessionGeneration
        ↓
decode bounded relay frame
        ↓
verify installed subscription/plan attribution
        ↓
verify event id, signature, and logical-filter match
        ↓
pure fava-state decision
        ↓
atomic EventCache mutation commit
        ↓
committed source revision wakes affected observations/routers/publications
```

NIP-01 subscription identifiers are scoped to one WebSocket connection. `EOSE` marks the transition from stored results to new live events for that subscription; `CLOSED` ends/refuses that subscription. These facts justify exact request evidence, never global relay or network completeness. [MEDIUM]

#### 3. Durable Publication

```text
validate write intent
        ↓
WriteStore transaction commits identity, receipt, payload/materialization,
and query-source contribution
        ↓
committed local fact becomes observable
        ↓
return Accepted
        ↓
signing and route acquisition proceed independently
        ↓
commit current route revision and eligible lane before attempting
        ↓
publisher performs one attempt through exact relay session
        ↓
commit correlated handoff/protocol outcome
        ↓
project receipt and query evidence
```

The write store owns durable facts; the publication owner owns live orchestration. Cancellation is a durable owner decision based on current signature and handoff facts, not merely dropping a future. A relay echo is admitted into the event cache and merges with the existing write-source record; unpublished materialization is never copied into the event cache.

#### 4. Restart and Shutdown

Startup opens and validates provider-owned formats, recovers write obligations, reconstructs exact materialization generations, reconciles query-source truth, then enters `Running`. Open writes reopen current signer/router/delivery work once; applications reopen desired queries.

Shutdown changes coordinator state before resource teardown:

```text
refuse new commands
  → close observations/withdraw demand
  → stop new publication effects
  → close router-owned acquisition
  → cancel/detach signer, publisher, auth work by contract
  → close and join transports
  → flush/close stores
  → join runtime resources or record typed deadline failure
```

Tokio's documented pattern separates cancellation notification from waiting for tasks to exit. A task tracker or equivalent owner registry is therefore needed; cancellation tokens alone do not prove resource termination. [MEDIUM]

## Patterns to Follow

### Pattern 1: Facts Before Effects, Local to Each Owner

**What:** The lifecycle owner calculates a decision, commits any required durable fact, then authorizes the effect. A completion carries exact operation/generation identity and becomes a fact only if still applicable.

**When:** Write acceptance, route-lane creation, cancellation, delivery attempts, cache admission, and recovery.

**Example:**

```rust
let accepted = write_store.accept(validated)?; // durable authority
query_source.publish(accepted.source_revision());
commands.reply_accepted(accepted.receipt_id());
runtime.start_signing(accepted.operation_key());
```

Do not create one global effect enum. Each owner should expose the smallest semantic command/fact/completion vocabulary that proves its ordering.

### Pattern 2: Complete Replacement Signals for Current Knowledge

**What:** Query sources, router contributors, route plans, diagnostics, and application current-state observations expose an immediate complete current value followed by complete replacements or changes defined against the last delivered revision.

**When:** Intermediate states may safely coalesce because only current truth matters.

Tokio `watch` retains only the latest value; `changed()` is cancellation-safe, and `borrow_and_update()` avoids a race/duplicate-read pattern. This validates the current M1 primitive for latest-state delivery. Never hold a watch borrow across `.await`, and never use latest-only delivery for receipts or other causal facts that must remain inspectable. [MEDIUM]

### Pattern 3: Contract + Implementation + Conformance Corpus

**What:** The neutral contract owns semantic boundary values and lifecycle promises; the implementation owns resources and private format; the conformance corpus is public and versioned with the contract.

**When:** Every replaceable cache, router, planner, transport, publisher, delivery policy, signer, and service-cache seam.

The first standard provider and one meaningfully different provider must exercise the same ordinary facade path before the contract stabilizes. External providers receive no internal constructors or default-only bypasses.

### Pattern 4: Hierarchical Cancellation With Exact Ownership

**What:** Engine shutdown cancels child owner scopes; closing one observation, router session, route preview, write, or native handle cancels only its owned work. Shared acquisition persists until its final owner releases it.

**When:** Queries, router-owned explicit queries, reconnects, signing, publication attempts, native streams, and shutdown.

Cancellation has two separately proved outcomes:

1. **Semantic:** no stale completion can change current state.
2. **Resource:** owned tasks, sockets, provider calls, and threads terminate or are quarantined within declared deadlines.

### Pattern 5: Bound by Signal Category

**What:** Use the bound that preserves each signal's meaning.

| Signal | Bound / Representation |
|--------|------------------------|
| Current query/route/diagnostic state | Single latest value or bounded replacement stream with revision |
| Owner commands/completions | Bounded ordered queue with backpressure or typed refusal |
| Receipt/delivery history | Durable bounded retention plus paging; never silent coalescing |
| Relay frames | Byte/frame/message limits before allocation and parsing |
| Router contributions/unresolved needs | Per-router and whole-plan cardinality/fan-out limits |
| Provider execution | Bounded concurrency, deadline, cancellation contract, and late-result rejection |
| Evidence artifacts | Bounded run windows with preserved manifest/hashes and explicit truncation facts |

Tokio bounded `mpsc` provides backpressure, whereas an unbounded channel may buffer until the process runs out of memory. Use unbounded queues only when an independent, proven semantic bound makes growth impossible. [MEDIUM]

### Pattern 6: Rust-Owned Native Handles

**What:** FFI exports owned values, typed errors, and idempotent handle operations. Rust owns query/publication/runtime state; Swift/Kotlin map native cancellation and close into those explicit operations.

**When:** M11 SDK projection.

UniFFI can map Rust async calls to Swift async/await and Kotlin suspend functions, but its cancellation behavior and Swift 6 concurrency support remain version-sensitive. Kotlin object wrappers require explicit close for reliable resource release. Treat generated bindings as transport scaffolding: expose explicit `cancel`/`close`, avoid reference cycles, and prove native cancellation races and process restart through public SDKs. [MEDIUM]

## Independent Evidence Architecture

| Evidence Layer | Proves | Must Not Be Used As |
|----------------|--------|---------------------|
| Pure/property/model tests | Universal state, query, planner, and policy semantics | Proof of real I/O or public usability |
| Provider conformance | Public contract behavior, bounds, close, and replacement parity | Proof that a standard provider has no private facade bypass by itself |
| Owner integration/crash tests | Commit/effect ordering, exact generations, recovery | Substitute for real relay/platform behavior |
| Scripted adversarial process | Exact malformed frames, races, ambiguous handoff, silence, late results | Sole interoperability relay |
| Real third-party relay lab | Socket/protocol interoperability, relay persistence, auth where supported | Source of deterministic internal state assertions |
| Rust canary | Ordinary public facade, diagnostics, resource behavior, restart | Internal test harness or database inspector |
| External provider workspace | Dependency direction and public substitutability | Standard provider implementation test only |
| Swift/Kotlin capstones | Native artifact, lifecycle, errors, cancellation, restart parity in real processes | Generated-code compilation proof alone |

Every major claim should combine the application-visible result with an independent witness appropriate to the boundary: wire proxy for bytes, process supervisor for kill/restart, conformance fixture for provider behavior, and platform process for native lifecycle. Deliberately disabling the owned mechanism must make the narrow evidence and its public capstone fail causally.

## Anti-Patterns to Avoid

### Anti-Pattern 1: Stabilizing Future Traits Before Their Slice

**What:** Creating every named provider interface now because the target architecture lists it.

**Why bad:** The signature encodes guesses before ordinary use and an alternative implementation challenge it.

**Instead:** Keep the contract/implementation split, but introduce each pair with its first full public behavior, conformance corpus, and falsifier.

### Anti-Pattern 2: Runtime or Facade as a Global Semantic Owner

**What:** One actor/global state machine mutates queries, routes, writes, sockets, auth, retries, and diagnostics.

**Why bad:** Failure scope, durable authority, replacement, and teardown become ambiguous.

**Instead:** The runtime executes; lifecycle owners decide; providers own their facts/resources; the coordinator owns only cross-owner barriers.

### Anti-Pattern 3: Connection-Scoped IDs Without Generation

**What:** Correlating frames or `OK`/`EOSE`/`CLOSED` only by relay URL, subscription string, or event id.

**Why bad:** NIP-01 subscription IDs are per connection and reusable; NIP-42 challenge/auth state is connection-scoped. Reconnect makes old completions dangerous. [MEDIUM]

**Instead:** Include exact session/access/generation plus logical owner identity in every correlation.

### Anti-Pattern 4: `spawn_blocking` as Complete Provider Isolation

**What:** Moving an arbitrary blocking provider call to `spawn_blocking` and assuming abort/shutdown can stop it.

**Why bad:** Tokio documents that started `spawn_blocking` work cannot be aborted and runtime shutdown may wait indefinitely. [MEDIUM]

**Instead:** Require bounded calls that terminate cooperatively; bound concurrency; use dedicated owned threads or separate processes for long-lived/untrusted blocking work; enforce shutdown deadlines; reject all late generations. Quarantine is resource isolation, not semantic cancellation.

### Anti-Pattern 5: “Committed” Without a Qualified Durability Profile

**What:** Returning `Accepted` after an API-level transaction while leaving journal/sync/version guarantees implicit.

**Why bad:** Provider configuration determines whether power-loss durability exists. SQLite WAL with `synchronous=NORMAL` may lose the latest committed transaction after OS crash/power loss, and SQLite documents a WAL-reset corruption bug fixed in 3.51.3 and selected backports. [MEDIUM]

**Instead:** Provider/profile documentation pins engine version and settings; M5 crash tests cover every commit/effect boundary. If SQLite WAL is selected, require a fixed version (3.51.3+ or documented fixed backport), preserve the WAL as database state, use `synchronous=FULL` for a power-loss durability claim, serialize write authority, and bound/checkpoint readers. Re-run this investigation if another backend is selected.

### Anti-Pattern 6: Native Future Lifetime as Engine Lifetime

**What:** Treating Swift task cancellation, Kotlin coroutine cancellation, garbage collection, or generated wrapper destruction as the authoritative close operation.

**Why bad:** Native/runtime cleanup differs by language and binding version; Kotlin requires explicit close for reliable UniFFI object release, and Swift async concurrency projection still has open limitations. [MEDIUM]

**Instead:** Rust-owned idempotent handle lifecycle, native close/cancel adapters, typed terminal results, and real-process leak/race tests.

### Anti-Pattern 7: Panic Crossing the Native Boundary

**What:** Letting provider, callback, or owner panics unwind through FFI.

**Why bad:** Rust documents undefined/abort-prone behavior when unwinding crosses an incompatible FFI boundary. [MEDIUM]

**Instead:** Use typed `Result` failures for expected errors, contain Rust panics at owned execution/FFI boundaries where possible, convert them to attributable terminal/provider facts, and deliberately panic a provider in real Swift/Kotlin process tests.

### Anti-Pattern 8: Evidence That Shares the Authority It Claims to Prove

**What:** Using internal commands, direct database inspection, Fava wire parsing, or diagnostics alone to prove public or external effects.

**Why bad:** The system becomes its own witness.

**Instead:** Keep the independent proxy, real processes, public-facade canary, external provider workspace, and native process capstones.

## Scalability Considerations

These are architecture-pressure bands, not product promises. Exact budgets must be measured against each selected profile.

| Concern | Small Embedded Profile (≤100 active handles) | Large Local Profile (~10K) | Stress / Research Scale (~1M) |
|---------|---------------------------------------------|----------------------------|-------------------------------|
| Observations | Per-handle state is acceptable; latest-value delivery | Share equivalent dependencies and route work; affected-branch reevaluation | Requires indexed dependency graph, aggressive sharing, and explicit admission/refusal; do not assume one task per observation |
| Query source updates | Full reevaluation is the semantic oracle | Incremental evaluator must match oracle corpus | Partitioned indexes and bounded fan-out; benchmark mutation must detect missed invalidation |
| Events/evidence | Bounded memory cache can replace complete snapshot | Persistent indexed provider and paged evidence | Retention/coverage partitioning and typed shortfall; never fabricate completeness |
| Writes/receipts | One serialized durable authority is simplest | Fair per-lane scheduling and bounded open-write recovery | Sharded execution may be possible, but receipt/write authority remains singular per identity |
| Relay demand | One session per relay/access context | Subscription planner coalesces equivalent demand within limits | Strict session/subscription admission and plan cardinality bounds |
| Provider calls | Direct short calls outside owner locks | Bounded executor/semaphore and deadlines | Isolation pool/process boundary; overload refusal before queues grow |
| Diagnostics/evidence | Full bounded snapshot | Aggregate plus paged detail | Sampling/aggregation with exact lost/coalesced counters |
| Native handles | Explicit close is easy to audit | Handle registry, leak counters, lifecycle stress tests | Admission limits and bulk close; no assumption that GC will keep pace |

## Build Order and Roadmap Implications

The numbered milestone gates remain the safest roadmap. Preparatory pure-value or test-harness work may start earlier, but no milestone claim exists until its complete facade/canary gate passes.

| Phase | Architecture Result | Dependency / Ordering Rationale | Research Flag |
|------:|---------------------|---------------------------------|---------------|
| M1 | Finish deterministic local state, stable equivalent-query identity, deletion/expiry/source removal, shared corpora, and public-facade local write/query evidence | This is the semantic oracle and source model every later relay/write path consumes; do not add networking to earn M1 | Standard repository work; HIGH confidence |
| M2 | Add wire, no-grouping planner, transport, ingest, explicit one-relay query, minimal diagnostics | Explicit acquisition establishes exact session/subscription/admission semantics before automatic routing | Deeper research only for chosen WebSocket/TLS packaging |
| M3 | Add multi-relay provenance, reconnect generations, bounded observation, cancellation/race tests | Generation identity and bounded delivery must be correct before router and publication fan-out increase concurrency | Research outage-backfill/windowing only when owning decision is reached |
| M4 | Add ordered async routing primitive, app/fallback policies, `RouterServices`, and standard planner | Routing decides relays; planner decides wire shape. Prove immediate partial progress and explicit bypass before write routing | Phase-specific research for planner equivalence and relay-limit behavior |
| M5 | Add durable explicit publication, signer, publisher, delivery, recovery, receipts | Explicit routes isolate the durability/signing/handoff lifecycle from automatic routing complexity | Mandatory backend/version/durability research and crash harness before provider selection |
| M6 | Integrate outbox/hints/app/fallback routing with publication route revisions and partial delivery | Reuses M4 routing and M5 durable lanes; later destinations remain under one receipt/generation | Phase-specific NIP-65/hint and route-retirement research |
| M7 | Add capability contract, NIP-02, second unrelated capability, rematerialization | Challenges semantic edit format and stale-generation rules only after source state and durable publication exist | Research each chosen capability's current NIP semantics |
| M8 | Add auth, hostile input, NIP-11 limits, provider isolation, ambiguity, ceilings, resource bounds | Hardens every existing boundary under failure; requires the real concurrency graph to exist | High research need: NIP-42 interoperability, runtime blocking/panic isolation, resource budgets |
| M9 | Qualify persistent/ephemeral cache profiles, fetch cache, NIP-05/NIP-11, reset/restart guarantees | Separates durable write custody from cache reuse and service-owned data after failure model is hardened | Mandatory provider persistence/migration and HTTP cache-policy research |
| M10 | External-provider matrix, negative dependency tests, change-amplification audit | Stabilize contracts only after materially different providers pass the same public corpus | High research need per alternative provider and toolchain |
| M11 | FFI inventory, selected native artifacts, Swift/Kotlin parity, real process/device lifecycle evidence | Projection comes last so SDKs encode proven Rust behavior rather than freeze incomplete semantics | Mandatory current UniFFI/Swift 6/Kotlin lifecycle research and platform-specific testing |

### Parallel Work That Does Not Violate Ordering

- M5 durable-store spike and crash harness may proceed after M1 semantics stabilize, but cannot claim publication before the ordinary public transport/facade path exists.
- M11 can maintain an operation/parity inventory early, but generated bindings must not stabilize incomplete operations.
- M8 adversarial relay fixtures and resource measurement can grow alongside M2-M7, while hardening claims remain at M8.
- M9 pure service values may be researched earlier; persistent profile guarantees remain provider- and restart-qualified work.

## Open Decisions and Gaps

- Standard durable write-store backend remains open. Decide at M5 from crash semantics, supported platforms, fixed versions, migrations, and measured write/recovery behavior—not familiarity.
- Recommended persistent event-cache profile remains an M9 product decision. Do not infer persistence from the `EventCache` contract.
- Outage backfill, query windowing, partial-handoff cancellation, and full delivery-history retention remain owner-specific decisions at their documented milestones.
- Whether long-lived application-supplied providers require process isolation cannot be settled before concrete provider contracts and platform constraints exist; `spawn_blocking` alone is insufficient.
- UniFFI cancellation documentation differs between the public futures guide and low-level scaffolding details. Pin the M11 version and verify generated Swift/Kotlin behavior rather than relying on generic documentation.
- Native iOS suspension/resume guarantees require physical-device evidence for any profile that claims transparency.

## Sources

### Repository Authorities — HIGH

- `.planning/PROJECT.md` — project scope, validated/current milestone status, constraints, and open decisions.
- `docs/spec/ARCHITECTURE.md` — authoritative target responsibilities, ownership ledger, flows, dependency rules, falsifiers, and vertical slices.
- `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md` — M0-M11 product gates, ordering, canary scenarios, and release definition.
- `.planning/codebase/ARCHITECTURE.md` — implemented M0 plus narrow M1 tracer, concrete dependency graph, current lifecycle and evidence limitations.

### Current Primary Documentation — MEDIUM (official sources discovered by web search and cross-checked)

- [NIP-01: Basic protocol flow](https://github.com/nostr-protocol/nips/blob/master/01.md) — per-connection subscription identity, `REQ` replacement, `EOSE`, `CLOSED`, and event attribution.
- [NIP-42: Authentication of clients to relays](https://github.com/nostr-protocol/nips/blob/master/42.md) — challenge and authenticated-session connection lifetime.
- [NIP-11: Relay information document](https://github.com/nostr-protocol/nips/blob/master/11.md) — advertised message, subscription, filter, and authentication limits.
- [Tokio 1.53.1 `watch`](https://docs.rs/tokio/1.53.1/tokio/sync/watch/index.html) and [`Receiver`](https://docs.rs/tokio/1.53.1/tokio/sync/watch/struct.Receiver.html) — latest-value semantics, cancel safety, and borrow hazards.
- [Tokio graceful shutdown](https://tokio.rs/tokio/topics/shutdown) — cancellation signaling followed by task tracking/joining.
- [Tokio 1.53.1 `spawn_blocking`](https://docs.rs/tokio/1.53.1/tokio/task/fn.spawn_blocking.html) — non-abortable started blocking work and shutdown consequences.
- [Tokio 1.53.1 bounded `mpsc`](https://docs.rs/tokio/1.53.1/tokio/sync/mpsc/fn.channel.html) — bounded capacity, ordering, and backpressure.
- [SQLite atomic commit](https://www.sqlite.org/atomiccommit.html), [`PRAGMA synchronous`](https://www.sqlite.org/pragma.html#pragma_synchronous), and [WAL](https://www.sqlite.org/wal.html) — transaction recovery, durability settings, persistent WAL state, checkpoint behavior, and current fixed-version caveat.
- [UniFFI async/future support](https://mozilla.github.io/uniffi-rs/next/futures.html), [async FFI details](https://mozilla.github.io/uniffi-rs/latest/internals/async-ffi.html), [Kotlin lifetimes](https://mozilla.github.io/uniffi-rs/latest/kotlin/lifetimes.html), and [Swift bindings](https://mozilla.github.io/uniffi-rs/next/swift/overview.html) — async projection, cancellation/version caveats, explicit cleanup, and Swift concurrency status.
- [Rust Reference: panic and FFI unwinding](https://doc.rust-lang.org/stable/reference/panic.html#unwinding-across-ffi-boundaries) — native-boundary unwind constraints.
