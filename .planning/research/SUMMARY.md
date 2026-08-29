# Project Research Summary

**Project:** Fava
**Domain:** Embeddable cross-platform Nostr client engine
**Researched:** 2026-08-21
**Confidence:** HIGH for normative scope, architecture, and sequencing; MEDIUM for ecosystem versions and unbuilt platform integrations

## Executive Summary

Fava is a clean-room Rust implementation of an embeddable Nostr client engine with first-class Swift and Kotlin products. Its defining value is not basic protocol access: it is coherent live queries and durable write intents whose source evidence, cancellation, recovery, failure, and resource behavior remain exact across replaceable provider compositions. Experts should build it as a set of semantic owners and neutral public contracts, with runtime, storage, transport, protocol, and native mechanisms subordinate to those owners. The authoritative specifications under `docs/spec/` decide behavior and the M0-M11 sequence; research recommendations select mechanisms only where those documents intentionally leave room.

The current status must remain explicit. M0-M6 are complete: the independent evidence foundation, deterministic local state, exact single- and multi-relay queries, routing/planning, durable explicit publication, and automatic partial delivery are implemented and recorded by their focused milestone evidence. M7-M11 remain future specified work. The recommended delivery approach is therefore to build semantic edits and independent capability crates on the existing write spine, then continue with hostile-boundary qualification, truthful profiles, provider substitution, and native parity.

The largest risks are architectural shortcuts that initially look simpler: collapsing event-cache and write-store authority, turning request-scoped evidence into global truth, using logical IDs without generation identity, reporting write acceptance before durable commitment, waiting for routing to settle instead of exposing partial progress, and calling providers replaceable while defaults retain private access. Prevent these with one owner per mutable fact and lifecycle, facts-before-effects ordering, exact operation/generation correlation, explicit bounds and shortfalls from each introducing slice, contract-plus-implementation-plus-conformance delivery, independent witnesses, and deliberate-break evidence. M8 qualifies the mature system under hostile conditions; it must not be used to postpone boundedness or isolation in earlier public contracts.

## Status and Decision Boundary

| Category | Current conclusion | Planning consequence |
|----------|--------------------|----------------------|
| Normative | `docs/spec/` owns behavior, architecture, evidence discipline, and M0-M11 sequencing | Roadmap work may refine slices but must not reorder, weaken, or rename away milestone exit gates |
| Implemented | M0-M6 complete; M7-M11 absent | Requirements must distinguish completed milestone evidence from future specifications |
| Recommended | Stack, provider, transport, test, and FFI mechanisms described below | Adopt only with the first real vertical slice and prove them against the owning contract |
| Open | Windowing, partial-handoff cancellation, outage backfill, full delivery history, recommended persistent event-cache profile | Resolve in the owning milestones from forcing workloads; do not guess during initialization |

## Key Findings

### Recommended Stack

Keep the existing Rust 1.90/edition 2024/MSRV baseline, Tokio 1.53.1, `nostr` 0.45.3 primitives, and `thiserror` 2.0.20. Add current-stable CI rather than silently raising the MSRV. Tokio supplies execution, timers, I/O, and bounded channels, but Fava owns operation identity, generation checks, cancellation meaning, and shutdown truth. Use `nostr` for protocol mechanics only; do not import a high-level SDK whose relay pool, subscriptions, storage, or lifecycle would compete with Fava's owners.

The leading implementation recommendations are `tokio-websockets` for the engine while retaining the existing `tokio-tungstenite` canary as an independent witness, Redb for standard persistent providers, SQLite through `rusqlite` as a materially different M10 external falsifier, Reqwest plus platform Rustls verification for bounded services, and UniFFI behind hand-written Swift/Kotlin wrappers at M11. These are researched choices, not normative commitments. Redb remains a candidate until M5 crash/durability evidence qualifies its exact version and configuration; native versions remain lab baselines until package installation, cancellation, concurrency, and lifecycle work on real platform processes.

**Core technologies:**

- **Rust 1.90, edition 2024:** semantic engine and neutral contracts — existing reproducible baseline with current-stable CI for forward drift.
- **Cargo workspace:** dependency and profile assembly — represents values → contracts → implementations → selected profiles.
- **`nostr` 0.45.3:** Nostr values, parsing, signatures, and message primitives — reuse protocol mechanics without importing competing lifecycle owners.
- **Tokio 1.53.1 plus `tokio-util` 0.7.19:** bounded async execution and cancellation mechanics — subordinate to Fava-issued identity and lifecycle decisions.
- **`bytes` 1.12.1:** outer byte budgeting — bounds wire and service payloads before JSON/Nostr allocation.
- **`tokio-websockets` 0.13.3:** recommended product WebSocket transport — independent from the canary's Tungstenite stack.
- **Redb 4.2.0:** recommended standard persistence — keep `EventCache`, `WriteStore`, and `FetchCache` separate and qualify durability by process-kill evidence.
- **`rusqlite` 0.40.2 with bundled SQLite:** M10 external provider candidate — different mechanics provide a stronger contract falsifier than another Redb adapter.
- **Reqwest 0.13.4 plus `rustls-platform-verifier` 0.7.0:** bounded NIP-05/NIP-11 HTTPS and platform trust.
- **UniFFI 0.32.0:** low-level Swift/Kotlin projection — generated bindings carry ABI data while hand-written wrappers own idiomatic lifecycle mapping.
- **`proptest`, Loom, Nextest, `cargo-deny`, and `cargo-hack`:** focused semantic, concurrency, isolation, supply-chain, and profile evidence.

**Critical version policy:**

- Preserve Rust 1.90 MSRV and exact M0 relay/canary versions as historical evidence.
- Pin product and evidence dependencies; add them only with their owning vertical slice.
- Pin one Rustls crypto provider explicitly at profile assembly.
- Pin database engine/settings and distinguish process-crash from power-loss durability claims.
- Pin UniFFI runtime and bindgen together, plus native toolchain inputs used for evidence.

Detailed findings: [STACK.md](./STACK.md).

### Expected Features

The feature landscape is largely normative rather than market-selected. Fava's launch definition is all M0-M11 exit gates, so a conventional “v2” cut must not silently drop required behavior. Table stakes establish usable Nostr read/write functionality; differentiators establish the exactness, boundedness, recovery, replaceability, and cross-platform parity that justify Fava.

**Must have (table stakes):**

- Raw Nostr events, arbitrary/future kinds, validated tags, and declarative filters.
- Cryptographic and contextual admission bound to exact session, request, generation, filter, and access context.
- Reactive local, single-relay, and multi-relay queries with coherent opening, deterministic state, bounded observation, cancellation, reconnect, and exact terminal evidence.
- Canonical deduplication, replacement, addressability, deletion, expiration, removal, and merged cache/write-store semantics.
- Cache reuse with truthful profile-specific persistence, retention, provenance, tombstone, coverage, and restart guarantees.
- Event construction, pluggable signing, explicit and automatic routing, publication, per-destination outcomes, and one durable reattachable receipt identity.
- Relay authentication, NIP-11 limits, NIP-05 resolution, accounts/sessions, and scoped application diagnostics.
- Static provider composition, public conformance kits, bounded resources, deterministic teardown, and ordinary Swift/Kotlin artifacts.

**Should have (Fava differentiators, still required for release):**

- One coherent view from independent event-cache and write-store authorities without cache pollution.
- Exact source-scoped evidence without global completeness claims.
- Durable write identity and receipts across kill/restart, route expansion, retry, cancellation, and reapplication.
- Useful partial progress before routing or recipient discovery settles.
- Loss-honest separation of coalescible current state from causal receipt/lifecycle facts.
- Exact operation and generation identity across reconnect, signer, provider, publication, recovery, and native handles.
- Replaceable providers without privileged defaults; failure isolation for blocking, panicking, malformed, cancellation-ignoring, and late providers.
- Executable Rust/Swift/Kotlin parity through packaged artifacts and real processes.
- Independent wire/process/platform evidence plus named deliberate-break proof.

**Defer, but not to an unspecified v2:**

- Resolve the five intentionally open product decisions only in their owning milestones: windowing, outage backfill, partial-handoff cancellation, full delivery history, and the recommended persistent event-cache profile.
- Defer native packaging/API stabilization until M11, while maintaining a parity inventory earlier.
- Add each dependency or provider only with its first real vertical slice; do not prebuild empty future frameworks.
- Keep application state/UX, ranking, moderation, runtime plugins, compatibility layers, global completeness, and silent fallback permanently out of scope.

Detailed findings: [FEATURES.md](./FEATURES.md).

### Architecture Approach

Use semantic-owner values and pure rules at the base, neutral public contracts above them, replaceable implementations outside universal owners, and explicit profile assembly at the edge. Each observation, route session, relay session, publication, signer/auth operation, durable record, and native handle has one owner and exact correlation identity. Runtime executes authorized bounded work; it does not own query, routing, protocol, storage, publication, or receipt meaning. Rust retains mutable truth; Swift and Kotlin project values, errors, streams, cancel/close, and terminal state without reconstructing lifecycle.

**Major components:**

1. **Semantic state and query algebra** — canonical query identity; replacement, deletion, expiry, tombstone, merge, ordering, limits, and evidence semantics.
2. **EventCache and WriteStore** — independent admitted-relay state versus accepted write obligations/revisions/receipts, each exposed as a query source.
3. **Observation owner** — coherent opening, merged current view, bounded delivery, route demand, cancellation, and teardown.
4. **Wire, transport, ingest, and relay sessions** — bounded bytes, exact attribution, validation, cache admission, reconnect generations, and close/join.
5. **Routing and subscription planning** — ordered reactive contributions, explicit bypass, logical-demand grouping, and exact shortfall without semantic change.
6. **Publication owner** — durable acceptance, revision, signing, route revisions, attempts, receipts, cancellation, settlement, and recovery under one write identity.
7. **Capability and service owners** — protocol-specific edits and NIP-05/NIP-11 validation/freshness outside universal core; FetchCache stores opaque bytes.
8. **Runtime/coordinator and diagnostics** — bounded execution, failure isolation, barriers, joins, and typed facts; never a global semantic owner or second authority.
9. **Thin facade, profiles, and native projection** — explicit assembly and ordinary Rust/Swift/Kotlin products over identical semantics.
10. **Independent evidence systems** — owner/property/conformance tests, adversarial processes, real relays, public canary, external providers, and native capstones.

**Patterns to enforce:** facts before effects; complete replacement signals for current knowledge; separate causal delivery; contract + implementation + public conformance corpus; hierarchical cancellation; bounds by signal category; exact owner-issued generation identity; Rust-owned native handles.

Detailed findings: [ARCHITECTURE.md](./ARCHITECTURE.md).

### Critical Pitfalls

1. **Turning source-scoped evidence into global truth** — bind facts to exact relay session, access context, request, and generation; never credit planned or merely contacted relays.
2. **Collapsing event-cache and write-store authority** — keep independent contracts/providers/lifecycles, merge only in evaluation, and never cache unpublished local events.
3. **Reactive loops before coherent identity/removal** — canonicalize queries, use an all-or-nothing opening barrier, model retraction as source revision, and separate current from causal delivery.
4. **Logical IDs without exact generations** — require owner-issued tokens for every asynchronous completion; stale work is attributable but inert.
5. **`Accepted` before durable ownership** — atomically commit obligation, receipt, revision, and recovery cursor before response or external effect; kill at each boundary.
6. **Serializing partial routing or merging routing with planning** — publish immediate/replacement contributions and allow planning to change wire shape only.
7. **Nominal replaceability with privileged defaults** — universal owners use neutral contracts; standard and external providers use the same public path and corpus.
8. **Providers blocking/panicking on owner tasks** — bounded execution, deadlines, panic containment, exact call identity, scoped failure, late-result rejection, bounded shutdown.
9. **Record count as resource envelope** — bound bytes, tags, evidence, query structure, tasks, descriptors, fan-out, retries, histories, diagnostics, and artifacts.
10. **Flattened cancellation/refusal/failure/ambiguity** — preserve never-handed-off, ambiguous, acknowledged, rejected, gave-up, cancelled, and stale distinctions.
11. **Deletion/expiry as erasure** — retain qualified tombstone/expiry semantics and deterministic restart behavior; never claim network erasure.
12. **Self-proving green capstones** — require causal red, owner proof, mutation failure, public capstone, and boundary-appropriate independent witness.

Detailed findings: [PITFALLS.md](./PITFALLS.md).

## Implications for Requirements

- Preserve normative, implemented, and recommended status in every requirement.
- Mark M0 evidence as validated; do not reopen M0 when later scenarios extend the lab.
- Treat M1-M6 as completed regression authorities backed by their focused issue records and phase verification reports.
- Keep M7-M11 active and unvalidated until their complete owner, public-facade, independent-witness, resource, and mutation gates pass.
- Express source evidence, access context, identity, generation, bounds, shortfall, cancellation, ambiguity, and teardown as observable requirements.
- Preserve declarative live queries and durable write intents as the two primary long-lived workloads.
- Require a separate neutral contract and implementation crate with the first real provider slice.
- Attach every promise to its owner, smallest falsifier, public capstone when additive, independent witness where required, and named deliberate break.
- Keep stack choices out of requirements; milestone planning selects and qualifies mechanisms.

## Implications for Roadmap

The authoritative M0-M11 order remains the roadmap. Research supports finer vertical slicing within each milestone, not a replacement sequence. M0-M6 are preserved completed phases; the active phases are M7-M11.

### Phase M1: Deterministic Local Semantic State

**Rationale:** Every relay, routing, publication, persistence, and native path depends on one correct semantic oracle and one coherent local-source lifecycle.

**Delivers:** Stable equivalent-query identity; replacement/deletion/expiry/source-removal corpus; coherent open barrier; independent memory cache/write-store merge; local shadow/cancel/reveal; bounded current-state observation; shared provider corpora; public-facade capstones and deliberate breaks.

**Addresses:** Canonical event state, reactive local query, coherent cache/write visibility, bounded teardown, diagnostics, provider-test facilities.

**Avoids:** Calling the tracer complete; source concatenation; access-context omission; duplicate acceptance poisoning; stale opening; one oversized lifecycle owner.

**Research flag:** Skip broad external research. Repository authorities and known-code evidence define the work; any vocabulary change still needs separate approval.

### Phase M2: Exact Single-Relay Live Query

**Rationale:** Establish bounded wire admission and exact source attribution with explicit acquisition before multi-relay sharing or automatic policy.

**Delivers:** Wire values/bounds, no-grouping planner, transport, relay-session/request identity, decode/verification/filter admission, stored/live/EOSE/CLOSED semantics, explicit one-relay observation, cancellation, diagnostics, and independent wire/relay evidence.

**Addresses:** Cryptographic/contextual admission, explicit relay query, exact evidence, raw Nostr wire behavior.

**Avoids:** Caller-forged cache evidence, off-filter/stale frames, false EOSE/global completeness, and canary/engine transport sharing.

**Research flag:** Targeted research for selected WebSocket/TLS behavior, payload/frame/close bounds, and platform trust.

### Phase M3: Multi-Relay Reactivity and Bounded Observation

**Rationale:** Generation fencing, dedup/provenance merge, sharing, and slow-consumer behavior must precede routing and publication fan-out.

**Delivers:** Multi-relay results, serving-relay evidence, reconnect generations, source removals, equivalent-demand sharing, bounded latest-state delivery, separate causal facts, and deterministic cancellation/race/resource envelopes.

**Addresses:** Multi-relay subscriptions, dedup/provenance, reconnect, loss-honest observation, deterministic teardown.

**Avoids:** Bystander provenance, stale reconnect completion, causal-fact coalescing, and one task per handle at scale.

**Research flag:** Core patterns are documented. Research only forcing decisions around windowing/backfill and measured observation envelopes.

### Phase M4: Ordered Async Routing and Subscription Planning

**Rationale:** Routing selects logical destinations while planning selects wire shape; both must be replaceable before automatic publication depends on them.

**Delivers:** Live ordered route contributions, immediate partial progress, explicit bypass, router acquisition services, app/fallback policies, standard/no-grouping planners, exact limit shortfall, and differential evidence.

**Addresses:** Read routing, subscription grouping, partial progress, provider composition.

**Avoids:** One-shot settled routes, recursive acquisition, policy leakage, silent planner truncation, and grouping that changes meaning.

**Research flag:** Research planner equivalence, relay-advertised limits, and current routing-policy NIPs.

### Phase M5: Durable Explicit-Route Publication

**Rationale:** Explicit destinations isolate the durable acceptance/signing/handoff/recovery spine from automatic routing complexity.

**Delivers:** Durable write identity, atomic acceptance/local visibility, signer/publisher/delivery contracts, explicit lanes, exact receipts, cancellation, settlement, process-death recovery, and reattachment.

**Addresses:** Event construction, pluggable signing, explicit publication, durable receipts, optimistic local visibility, restart recovery.

**Avoids:** Effects before facts, cache pollution, same-process restart claims, collapsed ambiguity/rejection, and duplicate recovery.

**Research flag:** Mandatory database/version/settings, platform filesystem, process-kill harness, and power-loss-boundary research. Redb is the leading recommendation, not a pre-approved requirement.

### Phase M6: Automatic Routing and Partial Delivery

**Rationale:** Reuse M4's live route contributions and M5's durable lanes so new destinations extend the same write rather than creating a second lifecycle.

**Delivers:** Outbox/hint/app/fallback policies, route revisions, immediate known-destination attempts, later lane expansion, partial delivery, retirement, bounded fan-out, and one receipt across changes.

**Addresses:** Automatic write routing, useful partial progress, mixed outcomes, dynamic destination expansion.

**Avoids:** Waiting for settlement, duplicate sends/receipts, URL-only lane identity, and unresolved destinations blocking known work.

**Research flag:** Research pinned NIP-65/hint semantics, route retirement, partial delivery, and partial-handoff cancellation when forced.

### Phase M7: Semantic Writes and Capability Composition

**Rationale:** Rerevision requires current source state, durable publication, and automatic routing to share exact generation identity.

**Delivers:** Capability contract, NIP-02 edit flow, unrelated capability N+1, reapplication under one receipt, preservation of unrelated changes, stale completion rejection, and negative-dependency proof.

**Addresses:** Protocol helpers without core kind branching, semantic edits, reapplication, capability extensibility.

**Avoids:** Protocol-owned publication, universal NIP enums, receipt replacement, and stale generation completion.

**Research flag:** Recheck the pinned revision of each chosen NIP during phase planning; isolate findings to the capability crate.

### Phase M8: Authentication, Hostile Boundaries, Limits, and Isolation

**Rationale:** Qualify the mature read/routing/publication graph under block, panic, malformed input, auth, overload, ambiguity, retry, cancellation, and shutdown pressure.

**Delivers:** Generation-scoped NIP-42 auth, adversarial processes, outer limits, provider executor, panic/failure isolation, typed ambiguity/give-up/shortfall, resource ceilings, bounded shutdown, and relay diversity.

**Addresses:** Authentication, hostile behavior, exact failures, overload control, resource boundedness, teardown.

**Avoids:** Catch-all wrappers, URL-scoped auth, `spawn_blocking` as isolation, silent shortfall, runaway retries/queues, and wedged shutdown.

**Research flag:** High need: NIP-42 interoperability, relay fixtures, blocking/panic containment, resource budgets, and ambiguous handoff evidence.

### Phase M9: Truthful Persistent/Ephemeral Profiles and Services

**Rationale:** Persistence and freshness guarantees can be qualified only after semantic correctness and hostile failure behavior are established.

**Delivers:** Persistent/ephemeral event-cache profiles, durable recovery qualification, FetchCache, bounded NIP-05/NIP-11 services, schema/migration/refusal, restart/reset/corruption evidence, and generated profile guarantees.

**Addresses:** Offline reuse, truthful persistence, service freshness, relay metadata, identity resolution, reset/restart.

**Avoids:** Baseline traits implying persistence, merged stores, service bytes becoming event truth, freshness leakage, and accidental ephemeral reuse.

**Research flag:** Mandatory provider schema/migration/corruption, bounded HTTP/cache policy, service-NIP revision, and persistent-cache-profile research.

### Phase M10: Provider Substitution Qualification

**Rationale:** Replaceability is designed earlier, then falsified with materially different external implementations before native APIs freeze it.

**Delivers:** Outside-workspace implementations, public conformance matrix, dependency-negative gates, feature/profile matrix, change-amplification audit, isolation repetition, and zero private facade doors.

**Addresses:** Static composition, external substitution, profile truth, consumer builds.

**Avoids:** Adapters around defaults, internal constructors, default-shaped corpora, persisted-format coupling, and compile-only substitution.

**Research flag:** High need per alternative provider, public package boundary, compile matrix, and toolchain. SQLite is a candidate, not inherited proof.

### Phase M11: Swift and Kotlin Product Parity

**Rationale:** Native projection comes last so packaged SDKs encode stabilized, substitutable Rust semantics instead of freezing incomplete lifecycle behavior.

**Delivers:** Minimal FFI inventory, selected-profile artifacts, XCFramework/SwiftPM and AAR/Maven consumption, idiomatic async wrappers, explicit close/cancel/reattach, shared parity corpus, and real process/device lifecycle evidence.

**Addresses:** Native artifacts, cross-language parity, cancellation, lifecycle, restart, and resource return.

**Avoids:** Binding-only claims, wrapper-owned state, FFI panic, GC/task-drop as close, repository-relative artifacts, and same-process proof.

**Research flag:** Mandatory current UniFFI, Swift 6 concurrency/`Sendable`, Kotlin Flow, JNA, Android verification, ABI/toolchain, packaging, and device research.

### Phase Ordering Rationale

- M1 supplies the semantic oracle and independent local authorities consumed everywhere else.
- M2 establishes exact one-session ingress before M3 composes sessions and reconnect generations.
- M4 separates route policy from wire planning before M6 reuses it for publication.
- M5 establishes the one durable write/receipt spine; M6 and M7 extend it rather than creating new lifecycles.
- M8 hardens the assembled boundaries, while bounds, identity, and isolation start with each earlier slice.
- M9 qualifies persistence and service profiles only after the failure model is explicit.
- M10 proves public substitution before M11 freezes contracts into native packages.
- M11 projects Rust-owned semantics; native wrappers never become independent lifecycle authorities.

### Safe Preparatory Parallelism

- Grow adversarial fixtures and measurements alongside M2-M7 without claiming M8 early.
- Spike M5 store/crash mechanics after M1 stabilizes without claiming publication before its public path exists.
- Maintain an M11 operation/parity inventory early without stabilizing bindings for incomplete operations.
- Research M9 service values early while deferring persistence/freshness guarantees to qualified profiles.

### Research Flags

**Needs deeper phase research:** M2 WebSocket/TLS; M3 open decisions and budgets; M4 planner/limits; M5 durability; M6 routing/handoff; M7 selected NIPs; M8 hostile/auth/resource behavior; M9 persistence/services; M10 alternatives/toolchain; M11 FFI/native packaging and lifecycle.

**Established patterns:** M1 should skip broad external research; M3 core generation, observation, causal-delivery, and teardown patterns are documented, with research limited to open product choices and measured budgets.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH for specification fit; MEDIUM for ecosystem currency | Existing Rust/Tokio/`nostr` baseline is verified. Future transport, database, and native mechanisms need owning-phase executable qualification. |
| Features | HIGH for Fava scope; MEDIUM for ecosystem characterization | Normative features and anti-features come from authoritative specifications. External SDK comparisons establish expectations but do not define scope. |
| Architecture | HIGH for target/current-state distinctions; MEDIUM for external mechanisms | Ownership, dependency direction, lifecycle, and M0-M11 gates are repository-owned. Tokio/database/UniFFI guidance is official but unproved in Fava products. |
| Pitfalls | HIGH for Fava-specific risks; MEDIUM for time-sensitive external details | Risks follow from required invariants and implemented M1-M6 regression surfaces. NIP, database, runtime, and native details must be rechecked in their milestones. |

**Overall confidence:** HIGH that the authoritative sequence and architecture constraints are correct; MEDIUM that recommended future mechanisms and exact versions will survive milestone-specific validation unchanged.

### Gaps to Address

- **M7 capability contract:** Qualify semantic edits, first-value revision, reapplication, stable receipt identity, stale-generation refusal, and two unrelated capability crates before M8.
- **Five open product decisions:** Decide windowing, partial-handoff cancellation, outage backfill, full delivery history, and the recommended persistent event-cache profile only with forcing milestone evidence.
- **Durable write regression:** Preserve the Redb M5 process-kill and recovery corpus while M7 adds revision generations.
- **Resource budgets:** Measure representative profiles and encode typed admission/refusal/shortfall per owner.
- **NIP drift:** Recheck and record pinned revisions for NIP-01/05/09/11/40/42/65 in their owning phases.
- **Provider isolation:** Determine where cooperative in-process isolation is sufficient and where dedicated thread/process quarantine is required.
- **Native support matrix:** Define OS/API floors and simulator/device claims at M11; current host versions are lab observations, not product promises.
- **UniFFI cancellation/concurrency:** Verify generated behavior in Swift/Kotlin; never infer Fava cancellation from foreign task/future drop.
- **Validation registry:** Mechanically align Cargo, Bazel, canary dispatch, external workspaces, and native matrices before naming one authoritative command.
- **External-provider proof:** Use materially different outside-workspace implementations and public corpora that do not encode standard internals.

## Sources

### Primary — repository authorities (HIGH confidence)

- [Project definition](../PROJECT.md) — scope, milestone status, constraints, and open decisions.
- [Full rewrite goals](../../docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md) — required behavior.
- [Architecture](../../docs/spec/ARCHITECTURE.md) — ownership, state, lifecycles, dependency direction, and replaceable boundaries.
- [TDD/BDD testing guide](../../docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md) — red/green/mutation/capstone evidence discipline.
- [Implementation plan](../../docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md) — authoritative M0-M11 sequence and exit gates.
- [Partial Rust API semantics](../../docs/spec/partial-spec-api-semantics.md) — query/source refinements where non-conflicting.
- [Technology research](./STACK.md), [feature research](./FEATURES.md), [architecture research](./ARCHITECTURE.md), and [pitfall research](./PITFALLS.md) — detailed inputs.

### External primary documentation — MEDIUM confidence until Fava qualification

- [Nostr protocol NIPs](https://github.com/nostr-protocol/nips) — NIP-01, NIP-05, NIP-09, NIP-11, NIP-40, NIP-42, and NIP-65; re-pin during owning phases.
- [Tokio documentation](https://docs.rs/tokio/1.53.1/tokio/) — bounded channels, watch, selection/cancellation, blocking tasks, and shutdown.
- [Redb](https://github.com/cberner/redb) — recommended embedded provider mechanism and durability configuration.
- [SQLite atomic commit](https://www.sqlite.org/atomiccommit.html), [WAL](https://www.sqlite.org/wal.html), and [`synchronous`](https://www.sqlite.org/pragma.html#pragma_synchronous) — alternative-provider durability caveats.
- [Reqwest](https://docs.rs/reqwest/latest/reqwest/) and [Rustls platform verifier](https://github.com/rustls/rustls-platform-verifier) — bounded HTTP and native trust.
- [UniFFI](https://mozilla.github.io/uniffi-rs/latest/) — Swift/Kotlin generation, async projection, cancellation, and lifecycle caveats.
- [Apple XCFramework](https://developer.apple.com/documentation/xcode/creating-a-multi-platform-binary-framework-bundle) and [Android build](https://developer.android.com/build) guidance — native artifact construction/consumption.
- [Proptest](https://proptest-rs.github.io/proptest/), [Loom](https://docs.rs/loom/latest/loom/), [Nextest](https://nexte.st/), [`cargo-deny`](https://embarkstudios.github.io/cargo-deny/), and [`cargo-hack`](https://github.com/taiki-e/cargo-hack) — focused evidence tooling.
- [nostr-rs-relay](https://github.com/scsibug/nostr-rs-relay) and [strfry](https://github.com/hoytech/strfry) — controlled real-relay fixtures with pinned configurations.

### Ecosystem comparisons — MEDIUM confidence

- [Rust Nostr SDK](https://github.com/rust-nostr/nostr) — feature baseline; not suitable as Fava's lifecycle owner.
- [NDK TypeScript](https://github.com/nostr-dev-kit/ndk) and [NDK Kotlin](https://github.com/nostr-dev-kit/ndk-kotlin) — reactive subscription, routing, signer, cache, test, and mobile expectations.
- [Nostr SDK FFI](https://github.com/rust-nostr/nostr-sdk-ffi) — native packaging precedent, not evidence of Fava parity.

---
*Research completed: 2026-08-21*
*Ready for roadmap: yes — preserve authoritative M0-M11 sequencing*
