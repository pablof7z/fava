# Phase 7: Semantic Writes and Capability Composition - Research

**Researched:** 2026-08-21
**Domain:** Durable semantic replaceable-event edits, generation-correlated publication, and protocol-crate composition
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

DATA_A71C9E4B_START

### Locked Decisions

#### Phase Boundary

Deliver M7's replaceable-event-edit lifecycle and protocol-crate composition
through the public Rust `fava` facade. Protocol crates own event-kind meaning and
edit application; the write store owns durable custody and current
materialization; publication owns generations, signing, routing, delivery, and
receipts. Native projections, profiles, and M8 hardening remain later phases.

#### Authoritative behavior
- `WriteIntent` gains the third authoritative accepted form: a bounded,
  persistable replaceable-event edit carrying its actor before materialization.
- The edit's protocol crate owns its coordinate, empty-state behavior, durable
  change format, inverse, and application to qualified source state.
- First-value edits materialize without a predecessor. Newer qualified source
  state rematerializes every still-live edit while preserving unrelated source
  changes.
- One accepted operation, `WriteId`, and `ReceiptId` survive every
  materialization generation. Exact generation, event, signer, route, relay
  session, and attempt identity make retired completions attributable and inert.
- The event cache never receives unpublished local materializations. Atomic
  replacement and retraction remain write-store query-source mutations.
- Protocol crates cannot sign, route, publish, deliver, own receipts, or depend
  on runtime, transport, store implementations, or standard routers.

#### Capability proof
- Implement NIP-02 follow/unfollow and a separate bookmarks
  bookmark/unbookmark capability to challenge the shared contract.
- Both capabilities must pass one public conformance corpus, including first
  value, inverse, source change, deterministic composition, and bounds.
- Prove N+1 outside the workspace or selected product assembly: universal core
  behavior does not change, and raw arbitrary/future kinds remain constructible
  and publishable.

#### Behavioral evidence
- Write observable public behavior first, confirm it fails before production
  implementation, and preserve a named deliberate-break failure for the
  generation or rematerialization invariant.
- Include memory-store state-machine coverage, redb crash/reopen coverage,
  public-facade integration, dependency-negative compilation, and the external
  N+1 falsifier.
- Compilation is structural evidence only; completion requires behavioral
  materialization, rematerialization, stale-completion, query, receipt, and
  restart proof.

### the agent's Discretion
- Exact neutral value and materializer trait names, internal module boundaries,
  observation wiring, generation token representation, and plan decomposition,
  provided all six architecture gates and the authoritative ownership split hold.

### Deferred Ideas (OUT OF SCOPE)

Swift/Kotlin projection, selected persistent/ephemeral profiles, authentication,
hostile boundary expansion, and release packaging remain Phases 8-11.

DATA_A71C9E4B_END

[VERIFIED: .planning/phases/07-semantic-writes-and-capability-composition/07-CONTEXT.md:7-60,106-110]
</user_constraints>

<phase_requirements>
## Phase Requirements

DATA_09D4C6F2_START

| ID | Description | Research Support |
|----|-------------|------------------|
| CAP-01 | Protocol capability crates expose ordinary event values or semantic replaceable-event edits and their inverses without signing, routing, publishing, or owning receipts. | Put the opaque edit value and replaceable materializer contract in a neutral contract crate; keep protocol crates pure and dependency-negative. |
| CAP-02 | Actor identity exists on a semantic edit before materialization and becomes the author of every resulting event generation. | Persist the actor in the edit and validate actor plus coordinate on every materializer output before atomic install. |
| CAP-03 | A first-value semantic operation can materialize when no prior replaceable event exists. | Make `None` a normal materializer input with a protocol-owned empty state, not an error or fixture shortcut. |
| CAP-04 | A newer qualified source event rematerializes still-live operations while preserving unrelated source changes. | Observe committed source changes by coordinate, exclude self-produced local state, reapply outside the store lock, then compare-and-install atomically. |
| CAP-05 | One write and receipt identity remains stable across materialization generations. | Allocate identities once at acceptance; increment only the durable current generation and event identity. |
| CAP-06 | Signer, route, publisher, and delivery completions for retired materialization generations are attributable and inert. | Thread generation, event, provider-operation, route, lane/session, and attempt identity through every store mutation; make the store the final currentness guard. |
| CAP-07 | At least two unrelated protocol capability crates prove the semantic-edit contract is not shaped around one NIP. | Run NIP-02 and public NIP-51 bookmarks through the same public conformance corpus. |
| CAP-08 | Adding capability N+1 changes only its crate and selected assembly/artifact metadata, with zero universal-core behavior changes. | Build an external, non-workspace capability against public contracts and enforce a universal-core change/dependency allowlist. |
| CAP-09 | Raw arbitrary and future Nostr event kinds remain usable without adding universal-core switches over event-kind meaning. | Retain the generic event builder and add a facade proof that constructs and publishes a raw custom/future kind without registering a semantic capability. |

DATA_09D4C6F2_END

[VERIFIED: .planning/REQUIREMENTS.md:98-108]
</phase_requirements>

## Summary

M7 is one extension to the existing durable publication lifecycle, not a protocol-helper layer. `WriteIntent` must gain an opaque, persistable edit form; the selected protocol materializer must turn that edit plus qualified source state into an unsigned event; and the write store must atomically own the edit, stable identities, current materialization generation, current event, receipt, and query-source contribution before `Accepted`. Publication then uses the ordinary signer, router, publisher, delivery, and receipt path. A newer qualified source repeats the same transformation and atomically replaces only the write-store contribution. [VERIFIED: docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:706-770] [VERIFIED: docs/spec/ARCHITECTURE.md:1971-2017]

The current M6 seam cannot satisfy that contract incrementally: the accepted payload has only two forms, receipts contain no materialization generation or retained semantic edit, store mutations correlate signing/routing/delivery primarily by receipt and relay session, and publisher attempts omit materialization generation. The publication owner also observes receipt changes but no coordinate-qualified source changes. [VERIFIED: crates/fava-write/src/lib.rs:44-51,351-363,414-439] [VERIFIED: crates/fava-write-store/src/lib.rs:26-137] [VERIFIED: crates/fava-publisher/src/lib.rs:11-26] [VERIFIED: crates/fava-publication/src/run.rs:32-165,167-249]

DATA_36B8F901_START

```text
Event(UnsignedEvent),
Presigned(Event),
```

DATA_36B8F901_END

The quote above is the complete current `WritePayload` variant set; the third edit variant is genuinely absent rather than merely hidden behind another constructor. [VERIFIED: crates/fava-write/src/lib.rs:44-51]

**Primary recommendation:** plan a behavior-first vertical tracer that accepts `follow(Alice, Bob)` with no predecessor through public `Fava`, immediately exposes generation 1 through the write-store query source, ingests a newer relay-observed contact list, atomically installs generation 2 under the same `WriteId`/`ReceiptId`, and then releases delayed generation-1 signer, route, publisher, and delivery completions to prove all four are attributable and inert. Add bookmarks only after that shared lifecycle is correct, then prove N+1 from a separate external workspace. [VERIFIED: docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md:724-759]

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|--------------|----------------|-----------|
| Edit coordinate, durable change format, empty state, inverse, semantic apply | Protocol crate (`fava-nip02`, `fava-bookmarks`) | Neutral edit/materializer contract | Event-kind meaning must remain outside universal owners. [VERIFIED: docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1210-1239] |
| Durable edit custody, stable IDs, current generation/materialization, receipt and query-source mutation | Write-store contract and provider | Publication orchestration | The store is the durable truth/CAS boundary; publication drives work only from committed facts. [VERIFIED: docs/spec/ARCHITECTURE.md:145-161,896-938] |
| Qualified-source observation and rematerialization scheduling | Publication owner | Query evaluator and event-cache source | Publication owns the live lifecycle; qualified relay-observed state comes from the event-cache side of the canonical source model. [VERIFIED: docs/spec/ARCHITECTURE.md:180-187,1971-2013,2580-2612] |
| Signing, route acquisition, publishing, delivery | Existing replaceable providers coordinated by publication | Write store for exact completion commit | Protocol crates must not own or bypass any of these mechanisms. [VERIFIED: docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1216-1228] |
| Optimistic query visibility and atomic replacement | Write-store `QuerySource` | Observer/query evaluator | Unpublished values are local source contributions and never event-cache entries. [VERIFIED: docs/spec/ARCHITECTURE.md:911-938] |
| Capability selection and recovery registry | `FavaBuilder` selected application assembly | Publication owner | The application supplies the materializers needed to recover its accepted edits. [VERIFIED: docs/spec/ARCHITECTURE.md:712-736,2255-2277] |
| Raw arbitrary/future kinds | General event/query primitives | Facade | Raw construction and publication cannot require a semantic materializer or a core kind switch. [VERIFIED: docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:692-704,1241-1249] |

### Ownership reconciliation

The context shorthand says publication owns generations/receipts, while the complete architecture says durable state is in `WriteStore` and publication owns live orchestration/current operation generations. These statements are compatible only with an explicit split: **the write store owns durable current-generation, receipt, and completion truth; publication owns the in-memory tasks and decides which correlated effects to request.** The planner should use that split everywhere and never let an in-memory task token outrank the store's currentness check. [VERIFIED: .planning/phases/07-semantic-writes-and-capability-composition/07-CONTEXT.md:10-14] [VERIFIED: docs/spec/ARCHITECTURE.md:1971-1987]

## Standard Stack

### Core

| Component | Version | Purpose | Prescription |
|-----------|---------|---------|--------------|
| Rust | 1.90.0, edition 2024 | Domain values, provider contracts, owners, and tests | Keep the pinned repository toolchain and workspace lint policy. [VERIFIED: Cargo.toml:40-43] [VERIFIED: environment probe `rustc --version`] |
| `fava-write` | workspace 0.1.0 | Own `ReplaceableEventEdit`, stable write values, generation-bearing receipt values, and the neutral materializer contract or its closest approved contract location | Extend the semantic owner; do not add a generic common crate. [VERIFIED: Cargo.toml:40-43,75-78] [VERIFIED: docs/internals/vocabulary.toml:395-420] |
| `fava-write-store` + memory/redb providers | workspace 0.1.0 | Atomic acceptance/rematerialization, currentness checks, receipt/query-source truth, recovery | Add generation-aware mutations to the neutral contract first, then both providers. [VERIFIED: crates/fava-write-store/src/lib.rs:26-176] |
| `fava-publication` | workspace 0.1.0 | Materializer selection, qualified-source observation, generation task cancellation/restart, and ordinary signing/routing/delivery orchestration | Keep semantic branches out; dispatch only through registered neutral contracts. [VERIFIED: docs/spec/ARCHITECTURE.md:1971-2042] |
| `fava-query`, `fava-state`, `fava-event-cache` | workspace 0.1.0 | Exact coordinate selection, source/provenance qualification, and committed source changes | Reuse canonical source records and winner rules; add only the smallest exact-coordinate selection seam required for bounded observation. [VERIFIED: crates/fava-query/src/lib.rs:16-25,272-305] [VERIFIED: crates/fava-state/src/lib.rs:151-212] |
| `nostr` | 0.45.3 | Nostr event, key, kind, tag, coordinate, and signature primitives | Reuse existing pinned primitives; do not hand-roll Nostr wire/domain types. [VERIFIED: Cargo.toml:79-79] |
| `serde` / `serde_json` | 1.0.229 / 1.0.151 | Existing durable record and opaque edit codec building blocks | Use an explicit protocol-owned format version and golden decode tests; do not rely on the Rust enum layout as the durable schema. [VERIFIED: Cargo.toml:81-82] [VERIFIED: crates/fava-write-store-redb/src/lib.rs:92-132,193-214] |
| `redb` | 4.2.0 | Durable write-store transactions and crash/reopen proof | Commit edit, generation, materialization, receipt, and source-visible state in one immediate transaction. [VERIFIED: Cargo.toml:83-83] [VERIFIED: crates/fava-write-store-redb/src/lib.rs:92-132] |
| `tokio` | 1.53.1 | Bounded observation and cancellable publication tasks | Continue using controlled channels/barriers in tests; cancellation is advisory and store-side identity validation is authoritative. [VERIFIED: Cargo.toml:85-85] [VERIFIED: docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md:313-325] |

### Supporting

| Component | Version | Purpose | When to Use |
|-----------|---------|---------|-------------|
| `fava-nip02` | workspace 0.1.0 (new in M7) | NIP-02 contact-list decode, follow/unfollow edits, inverse, empty state, and materializer | First capability and first vertical tracer. Its crate name is already specified in the vocabulary registry. [VERIFIED: docs/internals/vocabulary.toml:108-116] |
| `fava-bookmarks` | workspace 0.1.0 (new in M7) | NIP-51 public bookmark-list decode, bookmark/unbookmark edits, inverse, empty state, and materializer | Second unrelated capability after the lifecycle tracer. Its crate name is already specified in the vocabulary registry. [VERIFIED: docs/internals/vocabulary.toml:118-126] |
| `fava` facade and builder | workspace 0.1.0 | Selected materializer assembly and public end-to-end API | Add materializer registration and keep protocol crates absent from facade production dependencies. [VERIFIED: crates/fava/src/lib.rs:230-402] |
| Rust canary | separate workspace | Ordinary application proof and shared public capability corpus | Own the four M7 scenarios and selected NIP-02/bookmark assembly. [VERIFIED: apps/canary/Cargo.toml:1-10] [VERIFIED: docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md:736-759] |
| Bazel `rules_rust` | 0.73.0 | Authoritative build graph | Add BUILD metadata for every new crate/edge and run `bazel test //...`. [VERIFIED: MODULE.bazel:3-20] |

### Alternatives Considered

| Instead of | Rejected alternative | Why rejected |
|------------|----------------------|--------------|
| Opaque protocol-owned edit bytes + selected materializer | Add follow/bookmark variants to `WritePayload` or branch on event kind in publication/store | Violates PROTO-001 and makes N+1 a core behavior change. [VERIFIED: docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1210-1215] |
| Durable generation CAS in store mutations | Cancel old tasks and trust cancellation | Providers may return late or ignore cancellation; exact identity, not cancellation timing, protects current state. [VERIFIED: docs/spec/ARCHITECTURE.md:159-161,3155-3168] |
| Canonical source observation with explicit self-exclusion | Rebase on the merged `AnyLocal` query winner | The merged evaluator includes write-store local contributions; without exclusion the edit can consume its own prior materialization and amplify itself. [VERIFIED: crates/fava-query-standard/src/lib.rs:23-49,69-112] |
| Explicit versioned opaque codec | Persist the protocol edit as an unversioned Rust/serde enum in universal core | Couples durable data to one protocol and makes schema evolution or unknown-format refusal ambiguous. Protocol crates own durable edit formats. [VERIFIED: docs/spec/ARCHITECTURE.md:712-736,1995-1998] |
| Latest-state coordinate observation | Unbounded source-change journal in publication | Rematerialization needs the current qualified state and must remain bounded to the affected coordinate. [VERIFIED: docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:755-768] |

**Installation:** no new external package is required. Add only in-repository workspace crates and reuse the versions pinned in `Cargo.toml`. [VERIFIED: Cargo.toml:45-86]

## Package Legitimacy Audit

Not applicable. M7 adds two repository-owned crates and should introduce no new third-party package. Therefore no registry package-legitimacy gate is required. [VERIFIED: docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md:716-723] [VERIFIED: Cargo.toml:45-86]

## Architecture Patterns

### System Architecture Diagram

```text
public capability call
fava_nip02::follow / fava_bookmarks::bookmark
        |
        v
opaque ReplaceableEventEdit(actor, coordinate, format, bounded bytes)
        |
        v
Fava::publish -> publication selects registered materializer
        |                         |
        |                         +--> qualified coordinate source observation
        |                              (relay-observed cache state; self local state excluded)
        v
protocol materializer(source or empty, edit, exact timestamp/context)
        |
        +-- typed refusal/panic/output-bound failure --> scoped receipt/build refusal
        v
WriteStore atomic commit
edit + stable WriteId/ReceiptId + generation + event + current receipt/source row
        |
        +--> QuerySource revision --> Observer --> immediate public EventRecord
        |
        v
publication opens signer and route work for exact generation/event
        |
        v
signed current event + route lanes --> publisher/transport --> correlated outcomes

new committed source event at same coordinate
        |
        v
publication reloads qualified base -> materializer applies same durable edit
        |
        v
WriteStore compare-and-installs successor generation atomically
        |
        +--> direct old-local -> new-local query update
        +--> current route + predecessor correction destinations
        +--> old signer/route/lane/attempt completions fail currentness CAS
```

This is the authority-prescribed commit-before-effect flow and M7 source-change sequence. [VERIFIED: docs/spec/ARCHITECTURE.md:145-159,2580-2612]

### Recommended Project Structure

```text
crates/
├── fava-write/                    # neutral edit/generation/materializer values and validation
├── fava-write-store/              # generation-aware durable mutation contract
├── fava-write-store-memory/       # model/state-machine implementation
├── fava-write-store-redb/         # versioned durable records, atomic recovery
├── fava-publication/              # qualified-source reconciliation and live orchestration
├── fava-publisher/                # exact generation-bearing attempt value
├── fava-routing/                  # generation-bearing write route request
├── fava-nip02/                    # NIP-02 contact-list semantics only
├── fava-bookmarks/                # NIP-51 public bookmark semantics only
└── fava/                          # selected assembly and public vertical tests
apps/canary/                       # shared capability corpus and four M7 scenarios
falsifiers/
└── external-protocol-capability/  # proposed separate workspace for N+1 proof
```

The listed existing crate paths are verified; `fava-nip02`, `fava-bookmarks`, and the external falsifier are planned additions already required or implied by M7. [VERIFIED: Cargo.toml:3-38] [VERIFIED: docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md:716-723] [ASSUMED: exact external falsifier directory name]

### Pattern 1: Opaque, versioned semantic edit

**Adopted contract:** the accepted edit carries actor, coordinate, format version, and protocol-owned bytes; the protocol crate owns how those bytes decode and apply. [VERIFIED: docs/spec/ARCHITECTURE.md:163-178,712-736]

**Recommended shape:** keep the durable payload neutral and bounded. Select a materializer from an assembly registry by its advertised coordinate/domain claim; refuse duplicate claims at build, unknown materializers at acceptance, and unsupported format versions at decode. The registry must not contain a switch whose branches name NIP-02 or bookmarks. This shape follows the adopted contract; exact names and registry representation are discretionary. [VERIFIED: .planning/phases/07-semantic-writes-and-capability-composition/07-CONTEXT.md:57-60]

**Validation sequence:**

```text
validate edit byte/coordinate bounds before custody
  -> resolve exactly one selected materializer
  -> load exact qualified source or None
  -> call materializer outside store lock/transaction
  -> validate output author == edit.actor
  -> validate output coordinate == edit.coordinate
  -> validate event id/body, event-size/tag limits, and expiration
  -> atomically accept/compare-and-install in WriteStore
```

Output revalidation is required because the protocol crate is replaceable and cannot be allowed to change another author's coordinate or bypass ordinary event validation. [VERIFIED: docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:716-724,994-1000] [VERIFIED: AGENTS.md:40-49,68-75]

### Pattern 2: Exact generation identity at every mutation

Generation is not merely a receipt field. Every asynchronous completion must return the complete identity of the work it was authorized to perform: stable write/receipt, materialization generation, exact event id/body, and its operation-specific signer, route revision/session, lane/relay session, and attempt identity. Store mutation methods compare that identity to durable current state before changing anything. [VERIFIED: docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:772-786,921-928] [VERIFIED: docs/spec/ARCHITECTURE.md:1629-1649]

```text
old completion(expected_generation, expected_event, operation_id, result)
  -> WriteStore loads current receipt
  -> mismatch: return typed stale result; current state unchanged
  -> match: validate exact result and commit one current fact
```

Cancellation should still stop unnecessary work, but it is not the safety mechanism. A completion that races or ignores cancellation must remain inert through the store compare-and-set. [VERIFIED: docs/spec/ARCHITECTURE.md:3155-3168]

### Pattern 3: Qualified source without self-feedback

The canonical merged query result contains both relay-cache and write-store sources, and the evaluator may choose the local replacement as the current coordinate winner. Feeding that merged winner directly back into the same edit would apply the edit to its own output. Publication therefore needs an explicit qualification rule: the base for rematerialization must exclude materializations produced by the operation being recomputed; for the M7 tracer, use the newest signed relay-observed/cache contribution at the exact actor/kind/identifier coordinate, or defined empty state. Preserve source provenance separately from acquisition scope. [VERIFIED: docs/spec/ARCHITECTURE.md:180-187,704-721] [VERIFIED: crates/fava-query/src/lib.rs:36-74,272-305] [VERIFIED: crates/fava-query-standard/src/lib.rs:23-49,69-112]

Use one coordinate-scoped latest-state observation per active coordinate, shared by edits at that coordinate. Coalescing intermediate cache revisions is safe only if the reconciliation loop always reloads and compares the exact current source event id before committing. A source change for another coordinate must create no materializer/store work. [VERIFIED: docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:755-768] [ASSUMED: exact sharing/coalescing implementation]

### Pattern 4: Durable codec and recovery gate

The redb provider currently stores each whole `Receipt` as unversioned JSON bytes in the `receipts` table and deserializes it directly during open. Adding required edit/generation fields without an explicit durable schema decision will either make existing rows unreadable or encourage silent defaults that invent missing generation facts. [VERIFIED: crates/fava-write-store-redb/src/lib.rs:19-22,92-132,193-214]

Recommended hard-cut for this clean-room repository:

1. Define a versioned durable write record/envelope for M7 and version the protocol-owned edit bytes independently.
2. Preserve opaque bytes exactly; protocol materializers decode only versions they own.
3. Refuse unsupported durable record/edit versions explicitly. Do not add legacy aliases, heuristic decoding, or defaulted generation identity.
4. Construct the selected materializer registry before `Publication::recover`; recovery of an open semantic edit with no selected decoder must fail the build before new commands are admitted.
5. Reconstruct only bounded current obligations and bounded retained predecessor evidence; repeated rematerialization must not recover one active lane per historical generation.
6. Prove the new record through actual SIGKILL/reopen, not a second in-process store.

This follows the no-compatibility rule and the authority's recovery boundary. [VERIFIED: AGENTS.md:1-17,72-74] [VERIFIED: docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:984-990] [VERIFIED: docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md:327-338]

### Pattern 5: Protocol semantics remain pure

For NIP-02, the official protocol defines contact lists as kind 3 replacement events with `p` tags; replacement republishes the complete list, and a contact entry may retain a relay URL and petname. The materializer should treat follow/unfollow as idempotent changes over a decoded complete list, preserve unrelated contacts/tag fields, and use the generic event builder. [CITED: https://github.com/nostr-protocol/nips/blob/master/02.md]

For the M7 bookmarks tracer, use the public NIP-51 bookmark list: standard replaceable kind 10003 with public event (`e`) and address (`a`) bookmark tags. Bookmark/unbookmark should preserve unrelated tags/content and normalize duplicate targets. Private encrypted bookmark content introduces a separate cryptographic/key-management surface and is not needed to prove the shared public edit contract unless Pablo explicitly includes it. [CITED: https://github.com/nostr-protocol/nips/blob/master/51.md] [ASSUMED: M7 is limited to public bookmarks]

Both protocol materializers must be deterministic for the same edit, source, and injected materialization context; inverses must be ordinary edits through the same write lifecycle, not direct store mutations. [VERIFIED: .planning/phases/07-semantic-writes-and-capability-composition/07-CONTEXT.md:37-44] [VERIFIED: docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md:724-755]

### Pattern 6: Smallest vertical tracer and wave order

1. **Vocabulary/behavior gate:** register the exact new public symbols/provider contract in a separate focused architecture change; then add failing public scenarios and provider/model tests. The crate nouns already exist in the registry, but the current `ReplaceableEventEdit` registry entry does not yet list the actual edit symbol, and no materializer/generation provider symbol is registered. [VERIFIED: docs/internals/vocabulary.toml:108-126,411-420] [VERIFIED: AGENTS.md:51-60]
2. **Neutral durable seam:** edit value, materializer contract, generation-bearing receipt/evidence/route/publish identities, and store CAS mutations. [VERIFIED: docs/spec/ARCHITECTURE.md:896-938,1111-1127,1629-1649]
3. **Memory owner model:** first-value acceptance, atomic rematerialization, correction destinations, stale completion matrix, bounds, cancellation/supersession, and deterministic coordinate composition. [VERIFIED: docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md:267-269]
4. **Publication/facade tracer:** NIP-02 only, public `Fava` path, canonical cache source v2, delayed generation-1 completions, ordinary query/receipt proof. [VERIFIED: docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md:736-750]
5. **Redb durability:** new record format, crash boundaries, reopen before command admission, same identities/generation, and continued delivery once. [VERIFIED: docs/spec/ARCHITECTURE.md:3040-3049]
6. **Composition:** bookmarks through the same corpus, dependency-negative checks, raw future kind, then external N+1. [VERIFIED: docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md:752-759]
7. **Full gates:** Cargo, clippy, canary, external falsifier, Bazel, vocabulary, deliberate break, and focused issue evidence. [VERIFIED: AGENTS.md:30-38,51-60]

### Anti-Patterns to Avoid

- **Core kind switch:** never match kind 3 or kind 10003 in `fava`, publication, routing, stores, query, transport, signer, publisher, or delivery. Register neutral materializers at assembly. [VERIFIED: docs/issues/0010-m7-semantic-writes-and-capability-composition.md:30-37]
- **Self-rebase:** never treat the operation's current unpublished local materialization as its newer qualified source. It causes duplicate/amplified edits and violates source preservation. [VERIFIED: crates/fava-query-standard/src/lib.rs:23-49,91-112]
- **Receipt-only completion:** never accept a signer, route, publisher, or delivery result using only `ReceiptId` and relay session. Current code does this and must be replaced at the store boundary. [VERIFIED: crates/fava-write-store/src/lib.rs:58-116]
- **Cancellation as correlation:** cancellation can lose a race. Always reject retired work by durable exact identity. [VERIFIED: docs/spec/ARCHITECTURE.md:3155-3168]
- **Materializer under lock/transaction:** call replaceable provider code before entering the atomic compare-and-install transaction. Provider block/panic/failure must not hold another owner's authority. [VERIFIED: docs/spec/ARCHITECTURE.md:3155-3168]
- **Unversioned serde evolution:** do not add default generation/edit fields to old receipt JSON and pretend missing facts existed. Use a hard version boundary and typed refusal. [VERIFIED: crates/fava-write-store-redb/src/lib.rs:193-214] [VERIFIED: AGENTS.md:1-17,72-74]
- **Protocol-owned receipt/cache row:** protocol crates return values; they do not insert cache events, allocate receipts, or mutate stores. [VERIFIED: docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1216-1239]
- **One test repeated at every layer:** put algebra in protocol/model tests, ownership/durability in stores/publication, and only complete cross-boundary promises in facade/canary. [VERIFIED: docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md:233-251]
- **Sleep-based race evidence:** use delayed fakes, barriers, channels, and process markers. [VERIFIED: docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md:313-325]
- **File growth by accretion:** `fava-write/src/lib.rs` and memory store are already near the 500-line soft limit; split cohesive modules before adding the M7 state machine. [VERIFIED: session `wc -l` measured 467 and 482 lines] [VERIFIED: AGENTS.md:62-66]

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Nostr event/id/signature/tag primitives | Parallel custom event structs or crypto | Existing pinned `nostr` types plus `fava-write::EventBuilder` | The one general builder and existing verification path are authoritative. [VERIFIED: docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:692-704,802-806] |
| Durable database engine | Custom file log/journal | Existing redb provider transaction boundary | Redb already commits receipt and identity metadata atomically with immediate durability. [VERIFIED: crates/fava-write-store-redb/src/lib.rs:92-132] |
| Per-protocol publication path | Signer/router/publisher/receipt implementation in NIP crates | One `WriteIntent`/publication/write-store lifecycle | Separate lifecycles violate WRITE-002 and PROTO-002. [VERIFIED: docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:706-714,1216-1228] |
| Late-result safety | Best-effort task cancellation or timestamps | Exact generation/event/operation CAS in `WriteStore` | Providers may return late; completion authority is durable current identity. [VERIFIED: docs/spec/ARCHITECTURE.md:159-161,1647-1649] |
| Source polling | Timer loop over cache/store internals | Canonical bounded `QuerySource`/observation changes | Source owners already expose current revisions; tests require canonical committed facts. [VERIFIED: crates/fava-query/src/lib.rs:272-305] [VERIFIED: .planning/phases/07-semantic-writes-and-capability-composition/07-CONTEXT.md:87-93] |
| Protocol codec framework | New general serialization package or reflection system | Existing serde/JSON plus explicit protocol format number and golden bytes | The edit format is small, opaque, and protocol-owned; another package adds no required authority. [VERIFIED: Cargo.toml:81-82] |
| Capability test path | Hand-written write-store fixtures | Public Fava/canary calls plus pure protocol/store model tests | Fixtures must be causes through supported operations, not the result under proof. [VERIFIED: docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md:219-227] |

**Key insight:** the difficult part is not follow/bookmark tag manipulation. It is preserving one durable semantic intention while every materialized event is immutable and every signer/route/delivery task can finish after that event has retired. The store's exact-generation state machine is the reusable capability. [VERIFIED: docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:755-786]

## Current Seam Audit

| Seam | Current state | Required M7 delta |
|------|---------------|-------------------|
| Accepted payload | Exact variants are `Event(UnsignedEvent)` and `Presigned(Event)`. [VERIFIED: crates/fava-write/src/lib.rs:44-51] | Add bounded `ReplaceableEventEdit` as the third form; retain edit even after materialization. |
| Receipt/evidence | Stable IDs and current event exist; no materialization generation, source-basis id, retained edit, or generation-scoped destination history exists. [VERIFIED: crates/fava-write/src/lib.rs:351-363,414-439] | Persist current generation/source basis/edit and bounded predecessor/correction evidence. |
| Store mutation identity | Sign/refusal/route/attempt/outcome accept receipt plus event/session/plan, not a complete generation token. [VERIFIED: crates/fava-write-store/src/lib.rs:58-116] | Make every mutation generation/event/operation aware and return typed stale without mutation. |
| Memory state | One `BTreeMap<ReceiptId, Receipt>` with receipt-derived query snapshots. [VERIFIED: crates/fava-write-store-memory/src/lib.rs:37-50,74-90] | Add coordinate/source/edit indexes or equivalent bounded owner state and atomic rematerialization. |
| Redb state | Tables are `"receipts"`, `"meta"`; metadata key is `"next_id"`; whole receipts are JSON. [VERIFIED: crates/fava-write-store-redb/src/lib.rs:19-22,92-132] | Add explicit durable schema/version and recovery of open semantic edits/current generations; do not silently default old rows. |
| Redb recovery | Current recovery converts `Attempting` destinations to `Unknown`; it does not restore materializers or source reconciliation. [VERIFIED: crates/fava-write-store-redb/src/lib.rs:216-235] | Reconstruct current semantic obligation before accepting new commands, then reopen only current generation work. |
| Publication | One task per receipt; route and lane active sets are receipt/session scoped; signing completes through receipt id. [VERIFIED: crates/fava-publication/src/run.rs:16-92,134-249] | Supervise per-receipt materialization generations, observe coordinate source changes, retire/reopen tasks, and pass exact tokens. |
| Publisher | `PublishAttempt` carries write, receipt, number, session, event, timeout. [VERIFIED: crates/fava-publisher/src/lib.rs:11-26] | Add materialization generation and exact durable attempt identity required by the architecture. |
| Facade builder | Selects cache/store/evaluator/transport/routers/signers/publisher/delivery; recovers publication before returning `Fava`; no materializer registry exists. [VERIFIED: crates/fava/src/lib.rs:230-402] | Select materializers before publication recovery and fail build on missing/duplicate recovery capability. |
| Query qualification | `AnyLocal` admits local and cached sources; selection filters only ids/authors/kinds. [VERIFIED: crates/fava-query/src/lib.rs:16-25,36-43] | Add or construct a bounded exact-coordinate qualified-source view that explicitly excludes self materialization. |
| Build metadata | Workspace contains neither `fava-nip02` nor `fava-bookmarks`; Bazel imports Cargo metadata and pins Rust 1.90.0. [VERIFIED: Cargo.toml:3-38] [VERIFIED: MODULE.bazel:3-31] | Add crate manifests, root membership/dependencies, BUILD targets, canary dependencies, and no facade production dependency. |
| Canary | Scenarios currently end at M6. [VERIFIED: apps/canary/scenarios.json:112-142] | Add the four named M7 scenarios and independently witnessed outputs. |

## Common Pitfalls

### Pitfall 1: The edit applies to its own current local event

**What goes wrong:** follow/edit application can duplicate tags or compound state on every write-store revision. **Why:** the ordinary merged query prefers a current local replacement and carries publication evidence. **Avoid:** qualify the base by exact coordinate/source authority and exclude the operation's own write-store contribution; rematerialize only from a changed qualified source id. **Warning signs:** materialization runs after its own store commit, event IDs churn without a new relay source, or repeated follow creates duplicates. [VERIFIED: crates/fava-query-standard/src/lib.rs:23-49,91-112]

### Pitfall 2: Generation is added to the receipt but not to mutation commands

**What goes wrong:** a delayed generation-1 route or outcome mutates generation 2 because both share the receipt/session. **Why:** current methods authorize by receipt and session. **Avoid:** carry and validate generation plus operation-specific identity at every async completion boundary. **Warning signs:** `install_signed`, `apply_route`, `begin_attempt`, or `record_outcome` can still be called with only `ReceiptId` and event/session/plan. [VERIFIED: crates/fava-write-store/src/lib.rs:58-116]

### Pitfall 3: New generation forgets correction destinations

**What goes wrong:** a relay that may hold the predecessor never receives the successor, or an old acknowledgment settles the successor. **Avoid:** successor destinations are the union of current route and destinations that may require correction; keep outcomes generation-scoped. **Warning signs:** destination maps are overwritten globally or acknowledged lanes are simply copied to the new generation. [VERIFIED: docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:921-928]

### Pitfall 4: Materializer invocation violates the durable boundary

**What goes wrong:** a blocking/panicking provider holds the store lock or creates a half-applied edit. **Avoid:** resolve source and compute a candidate outside authority, then enter one short compare-and-install transaction that rechecks source/current generation. **Warning signs:** trait calls inside a mutex/redb transaction or query observer notification before commit. [VERIFIED: docs/spec/ARCHITECTURE.md:145-159,3155-3168]

### Pitfall 5: Redb schema change invents missing state

**What goes wrong:** old JSON rows deserialize with default generation/edit values that never existed, or open fails without an actionable version error. **Avoid:** version the durable record and protocol codec, hard-refuse unsupported versions, and test exact reopen. **Warning signs:** broad `#[serde(default)]` on authoritative identity fields or no golden record/unknown-version tests. [VERIFIED: crates/fava-write-store-redb/src/lib.rs:193-214] [VERIFIED: AGENTS.md:1-17,72-74]

### Pitfall 6: Recovered work starts before materializers exist

**What goes wrong:** a semantic edit is durable but cannot be decoded/rematerialized, while new conflicting commands are accepted. **Avoid:** builder assembles materializers first, then publication recovery reconciles every open obligation, then the engine admits commands. **Warning signs:** `Publication::recover()` runs without the registry or recovery silently parks an unknown format. [VERIFIED: docs/spec/ARCHITECTURE.md:2040-2042] [VERIFIED: crates/fava/src/lib.rs:355-402]

### Pitfall 7: Protocol codec loses unrelated fields

**What goes wrong:** follow/bookmark reconstructs only known tags and erases relay hints, petnames, unrelated tags, content, or another bookmark. **Avoid:** decode enough to identify the target semantic element while retaining untouched raw data and stable order. **Warning signs:** materialization starts from an empty builder even when source exists, or sorts/deduplicates every source tag globally. [VERIFIED: docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:755-770] [CITED: https://github.com/nostr-protocol/nips/blob/master/02.md] [CITED: https://github.com/nostr-protocol/nips/blob/master/51.md]

### Pitfall 8: Timestamp selection fails replacement

**What goes wrong:** the new local materialization does not win against its source because its `created_at` is older or ties with an event-id ordering that loses. The local authorities require a successor/current materialization but do not lock a materialization timestamp rule. **Avoid:** make time an injected exact input, test equal/newer/future-skewed source timestamps, and lock the winner rule before implementation. **Warning signs:** protocol crates call wall-clock time directly or tests pass only because they sleep between generations. [VERIFIED: crates/fava-state/src/lib.rs:207-212] [ASSUMED: exact timestamp policy remains undecided]

### Pitfall 9: “Inverse” creates a second lifecycle

**What goes wrong:** unfollow/unbookmark directly edits store state or cancels a prior receipt rather than producing another semantic edit through publication. **Avoid:** inverses return ordinary `ReplaceableEventEdit` values and pass the same acceptance/materialization/generation corpus. **Warning signs:** protocol crate dependencies on store/publication or special facade methods per inverse. [VERIFIED: docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1216-1239]

### Pitfall 10: N+1 proof is inside the universal workspace

**What goes wrong:** a capability appears replaceable only because it can reach crate-private APIs or because core metadata already knows it. **Avoid:** implement one outside-workspace crate against public contracts and use only selected assembly changes. **Warning signs:** the external crate is a root workspace member, or its name appears in `fava`, publication, routing, stores, query, state, transport, or provider implementations. [VERIFIED: docs/spec/ARCHITECTURE.md:3053-3079]

## Code Examples

These are behavioral pseudocode, not locked Rust signatures. Exact names require the vocabulary gate.

### Atomic semantic acceptance

```text
edit = capability.follow(actor, target)
source = qualified_source.current(edit.coordinate)  // None is valid
candidate = materializer.apply(edit, source, exact_context)
validate(candidate.author == edit.actor)
validate(candidate.coordinate == edit.coordinate)
accepted = store.accept_edit_if_source_current(edit, source.id_or_empty, candidate)
// Accepted contains stable write/receipt and generation 1 only after the source row is visible.
```

This is the required acceptance ordering; failed commit leaves no receipt, local record, signer, route, or delivery work. [VERIFIED: docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:728-743]

### Compare-and-install rematerialization

```text
on committed source change for coordinate:
    current = store.current_semantic_write(receipt)
    source = qualified_source.current(coordinate)
    if source.id == current.source_basis: stop
    successor = materializer.apply(current.edit, source, exact_context)
    next = store.install_successor_if_current(
        expected_generation = current.generation,
        expected_source_basis = current.source_basis,
        successor
    )
    publication.start_current_tasks(next.identity)
```

The provider call occurs outside the durable transaction; the final mutation rechecks both old generation and source basis so concurrent source changes cannot install a stale successor. [VERIFIED: docs/spec/ARCHITECTURE.md:2003-2013,3155-3168]

### Deliberate-break schedule

```text
accept edit -> generation 1
hold generation-1 signer result and publisher outcome behind barriers
ingest qualified source v2
observe atomic generation 2 under same write/receipt
release generation-1 signer, route, publisher, and delivery completions
assert current event/generation/receipt/query are unchanged
assert old completions remain attributable to generation 1
```

Named deliberate break: remove the generation/current-event equality guard from one store completion mutation; `retired_generation_completions_are_inert` must fail observably through public receipt/query state. [VERIFIED: .planning/phases/07-semantic-writes-and-capability-composition/07-CONTEXT.md:46-55] [VERIFIED: docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md:313-325]

## State of the Art in This Repository

| M6 approach | M7 required approach | Impact |
|-------------|----------------------|--------|
| Two finalized accepted payload forms | Third durable semantic edit form plus current materialization | Acceptance can precede final immutable body while retaining intent. [VERIFIED: docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:706-714] |
| One receipt-level current event and route/destination state | Stable receipt with current materialization generation and bounded predecessor/correction evidence | Rematerialization does not allocate a new receipt or let old outcomes settle new work. [VERIFIED: docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:755-768,921-928] |
| Receipt/session-scoped async mutations | Exact generation/event/provider-operation/attempt CAS | Retired completions are attributable and inert. [VERIFIED: docs/spec/ARCHITECTURE.md:1647-1649] |
| Publication reacts to receipt/router changes | Publication also supervises coordinate-qualified source state | Newer source events trigger deterministic rematerialization. [VERIFIED: docs/spec/ARCHITECTURE.md:2003-2013] |
| Whole `Receipt` JSON with no schema marker | Explicit durable record and protocol edit format versions | Crash/reopen can distinguish supported facts from incompatible data without invented defaults. [VERIFIED: crates/fava-write-store-redb/src/lib.rs:19-22,193-214] |
| Protocol support exemplified by NIP-65 routing helper | Two semantic edit crates behind one materializer contract | The contract is challenged by unrelated kind semantics and inverses. [VERIFIED: docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md:716-734] |

## Contradictions and Decisions That Must Be Locked

### No blocking authority contradiction found

The apparent “generation ownership” difference is a durable/live split, not a contradiction: the store owns durable generation/receipt facts and publication owns live orchestration. Likewise, architecture's optional content-pending profile does not require M7 to accept an unmaterializable edit; the locked first-value tracer should materialize before `Accepted`, while missing/unknown materializers are refused before custody. [VERIFIED: docs/spec/ARCHITECTURE.md:896-938,1971-1998] [VERIFIED: .planning/phases/07-semantic-writes-and-capability-composition/07-CONTEXT.md:21-35]

### Decision 1: Qualified-source exclusion rule

The authorities require “best qualified source” and the canonical query path but do not state verbatim whether an operation's own current local materialization qualifies. The existing merged evaluator can return that local value. Lock this rule in the plan: **a semantic operation never uses its own materialization as source; the M7 qualified base is the newest signed relay-observed event at the exact coordinate, or empty state.** If later phases allow other local semantic operations as a base, they need explicit deterministic composition semantics rather than accidental query-winner feedback. [VERIFIED: docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:755-770] [VERIFIED: crates/fava-query-standard/src/lib.rs:23-49,91-112]

### Decision 2: Multiple live edits at one coordinate

The context requires deterministic composition, and WRITE-021 requires a newer accepted replaceable event to retire obsolete active delivery, but the authorities do not fully specify whether older semantic edits remain composition inputs, how their receipts become superseded, or which generation owns the one query-visible coordinate candidate. The planner must record one model before writing store state. Recommended invariant: serialize semantic edits per coordinate in stable acceptance order, retain the durable intentions needed to recompute the current desired state, expose/publish only the newest current coordinate materialization, and retire obsolete predecessor delivery without deleting bounded historical receipt facts. This recommendation needs a focused authority checkpoint because receipt settlement for earlier composed edits is observable and not merely an internal module choice. [VERIFIED: .planning/phases/07-semantic-writes-and-capability-composition/07-CONTEXT.md:37-41] [VERIFIED: docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:913-928] [ASSUMED: recommended coordinate-local composition model]

### Decision 3: Materialization timestamp/winner rule

The current universal winner rule compares `created_at`, then event id, but local authorities do not define how publication chooses `created_at` for a successor over an equal/future-skewed source. The planner must add a checkpoint that locks an injected, deterministic timestamp policy and proves the successor becomes the current query candidate without sleep. Do not leave this to protocol-specific wall-clock calls. [VERIFIED: crates/fava-state/src/lib.rs:207-212] [ASSUMED: exact timestamp policy]

### Decision 4: Public versus private bookmarks

NIP-51 includes both public tags and encrypted private bookmarks. The M7 contract needs an unrelated edit shape, not a new crypto/key-management owner. Recommend explicitly scoping M7 to public bookmark/unbookmark over standard replaceable kind 10003 and deferring private encrypted content unless Pablo locks broader scope. [CITED: https://github.com/nostr-protocol/nips/blob/master/51.md] [ASSUMED: public-only bookmark scope]

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | M7 bookmarks cover public kind-10003 `e`/`a` tags; private encrypted bookmarks are out of scope. | Protocol pattern / Decision 4 | Private support would add encryption, key selection, content codec, and security tests. |
| A2 | Coordinate observations may be shared/coalesced if reconciliation always reloads exact current source state. | Qualified source pattern | A per-receipt observer may be required instead, increasing resource cost but not changing contract. |
| A3 | Coordinate-local stable acceptance order is the intended deterministic multi-edit composition model. | Decision 2 | A different receipt/supersession model changes durable state, receipt outcomes, and recovery. |
| A4 | The materialization timestamp policy is not specified and requires an explicit authority decision. | Pitfall 8 / Decision 3 | A naive clock policy can fail winner selection or create future-dated successors. |
| A5 | The external N+1 falsifier directory will be named `falsifiers/external-protocol-capability`. | Project structure / Validation | Path is plan-level only; behavior is unaffected. |

## Open Questions

1. **What is the observable receipt state of earlier edits composed at the same coordinate?**
   - What we know: deterministic composition is required; obsolete active delivery must retire; receipt identity remains stable across generations. [VERIFIED: .planning/phases/07-semantic-writes-and-capability-composition/07-CONTEXT.md:26-31,37-41] [VERIFIED: docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:913-928]
   - What's unclear: whether an earlier receipt is terminal-superseded, remains open as an input to a later desired state, or receives successor generations.
   - Recommendation: resolve in a focused local authority issue before finalizing the store schema.

2. **What exact timestamp policy makes each materialization the intended current replacement?**
   - What we know: the repository winner rule is timestamp then lowest event id. [VERIFIED: crates/fava-state/src/lib.rs:207-212]
   - What's unclear: source events equal to or ahead of local wall clock.
   - Recommendation: inject time, define allowed skew/refusal, and prove without sleep.

3. **Are private NIP-51 bookmarks part of M7?**
   - What we know: M7 requires bookmark/unbookmark as the unrelated capability; NIP-51 defines public and private forms. [VERIFIED: .planning/phases/07-semantic-writes-and-capability-composition/07-CONTEXT.md:37-41] [CITED: https://github.com/nostr-protocol/nips/blob/master/51.md]
   - What's unclear: whether encrypted private content is required now.
   - Recommendation: lock public-only for M7 to avoid introducing an unrequested cryptographic owner.

4. **Does exact coordinate selection require a query contract extension now?**
   - What we know: current selection filters ids/authors/kinds, while `EventCoordinate` also supports an addressable identifier. [VERIFIED: crates/fava-query/src/lib.rs:16-25] [VERIFIED: crates/fava-state/src/lib.rs:151-165]
   - What's unclear: the two selected M7 capabilities are non-addressable standard replacements, but the neutral contract and external N+1 falsifier may challenge addressable coordinates.
   - Recommendation: either add exact coordinate filtering behind vocabulary approval or constrain the M7 external N+1 edit to a non-addressable replaceable kind and record addressable observation as later work. Do not scan all events unboundedly.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|-------------|-----------|---------|----------|
| Rust compiler | All crates/tests | ✓ | 1.90.0 | — |
| Cargo | Workspace/canary/falsifier tests | ✓ | 1.90.0 | — |
| Bazel | Authoritative build graph | ✓ | 9.2.0 | — |
| Python | Vocabulary checker/tests | ✓ | 3.14.6 | — |
| redb | Durable provider | ✓ via Cargo lock/workspace | 4.2.0 | Memory store for non-durability tests only |
| `nostr-rs-relay` binary | Only live relay canaries | Not probed; not needed for deterministic M7 tracer | — | Scripted/test transport and existing canary support |

Tool versions were probed in this session; repository versions are pinned in Cargo/Bazel metadata. [VERIFIED: environment probes] [VERIFIED: Cargo.toml:40-85] [VERIFIED: MODULE.bazel:7-20]

**Missing dependencies with no fallback:** none for planning or deterministic M7 implementation/testing. [VERIFIED: environment probes]

**Missing dependencies with fallback:** live third-party relay availability is not required for the headless semantic-generation invariant; the canary README requires it only for the real-relay smoke scenario. [VERIFIED: apps/canary/README.md:1-24]

## Validation Architecture

Nyquist validation is enabled, so the plan must create executable requirement evidence before implementation and keep per-task feedback under roughly 30 seconds. [VERIFIED: .planning/config.json:20-25]

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in test harness via Cargo 1.90.0; Bazel `rules_rust` 0.73.0 is the authoritative build graph. [VERIFIED: Cargo.toml:40-43] [VERIFIED: MODULE.bazel:3-20] |
| Config files | `Cargo.toml`, `Cargo.lock`, `MODULE.bazel`, per-crate `BUILD.bazel`, separate `apps/canary/Cargo.toml`. [VERIFIED: Cargo.toml:1-3] [VERIFIED: MODULE.bazel:24-39] [VERIFIED: apps/canary/Cargo.toml:1-10] |
| Current baseline | `cargo test --workspace --all-targets` passed on 2026-08-21; 11.54 seconds elapsed including compilation in this session. [VERIFIED: session command output] |
| Quick run command | `cargo test -p fava-write -p fava-write-store-memory -p fava-write-store-redb -p fava-publication -p fava --all-targets` (add `-p fava-nip02 -p fava-bookmarks` once created). |
| Full suite command | `cargo test --workspace --all-targets && cargo clippy --workspace --all-targets -- -D warnings && cargo test --manifest-path apps/canary/Cargo.toml && cargo clippy --manifest-path apps/canary/Cargo.toml --all-targets -- -D warnings && cargo test --manifest-path falsifiers/external-protocol-capability/Cargo.toml && bazel test //... && python3 tools/check_vocabulary.py && python3 -m unittest tools.tests.test_vocabulary_check` [ASSUMED: external falsifier path/name] |

### Proof Layers and Placement

| Layer | Required M7 proof | Correct owner |
|-------|-------------------|---------------|
| Pure protocol | Exact codec golden bytes/version rejection, empty-state apply, follow/unfollow, bookmark/unbookmark, idempotence, inverse, unrelated-field preservation, deterministic operation orders, bounds | `fava-nip02`, `fava-bookmarks` [VERIFIED: docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md:291-293] |
| Neutral value/contract | Edit validation, actor/coordinate/output invariants, arbitrary kind remains generic, dependency-negative source/manifest checks | `fava-write` and protocol crate tests |
| Store model/state machine | Acceptance atomicity, generation increment, same IDs, source-basis CAS, correction destinations, all stale-completion transitions, cancellation/supersession, retained-history bounds | memory provider/contract corpus [VERIFIED: docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md:267-269] |
| Durable owner | Acceptance/rematerialization SIGKILL boundaries, opaque edit/version reopen, missing decoder recovery refusal, same IDs/current generation, ambiguous prior attempt evidence, bounded current recovery | redb process-kill tests [VERIFIED: docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md:327-338] |
| Headless cross-owner | Delayed signer, route, publisher, and delivery completion after source v2; materializer failure/panic scoped; current correction routing | publication/facade controlled schedules [VERIFIED: docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md:287-289,313-325] |
| Public Rust facade | First value visible before acceptance return/effect; source v2 atomic query change; same receipt/generation evidence; inverse; raw future kind | `crates/fava/tests/semantic_writes.rs` (proposed) |
| Ordinary app canary | Four named M7 scenarios using selected protocol crates and public APIs, no private store fixture | `apps/canary` [VERIFIED: docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md:736-759] |
| External falsifier | Outside-workspace N+1 capability implements public contract/current+empty/inverse and assembles with zero universal-core edits | proposed separate falsifier workspace [VERIFIED: docs/spec/ARCHITECTURE.md:3053-3079] |
| Structural | Protocol crates have no forbidden deps; universal core contains no capability names/kind switches; Bazel/Cargo graphs agree | compile/source/manifest checks; compilation is not behavioral completion [VERIFIED: .planning/phases/07-semantic-writes-and-capability-composition/07-CONTEXT.md:46-55] |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|--------------|
| CAP-01 | Pure values/edits/inverses; forbidden dependency absence | unit + dependency-negative compile | `cargo test -p fava-nip02 -p fava-bookmarks` | ❌ Wave 0 |
| CAP-02 | Actor persists and authors every generation; wrong-author output refused | unit + store model + facade | `cargo test -p fava --test semantic_writes actor` | ❌ Wave 0 |
| CAP-03 | `None` source produces first event, visible before effects | pure + public facade/canary | `cargo test -p fava --test semantic_writes first_value` | ❌ Wave 0 |
| CAP-04 | Source v2 rematerializes atomically, preserves unrelated state, self-feedback absent | model + headless + facade | `cargo test -p fava --test semantic_writes rematerialization` | ❌ Wave 0 |
| CAP-05 | Same write/receipt, new generation across rematerialization/restart | store model + redb process kill | `cargo test -p fava-write-store-redb --test process_kill semantic` | ⚠️ existing file, M7 cases missing |
| CAP-06 | Delayed signer/route/publisher/delivery results are attributable/inert | controlled headless schedule + deliberate break | `cargo test -p fava --test semantic_writes retired_generation` | ❌ Wave 0 |
| CAP-07 | NIP-02 and bookmarks pass the identical public corpus | parameterized canary/facade corpus | `cargo test --manifest-path apps/canary/Cargo.toml semantic_capability_corpus` | ❌ Wave 0 |
| CAP-08 | External N+1 current/empty/inverse; zero universal-core behavior/dependency changes | external compile + public behavior + allowlist | `cargo test --manifest-path falsifiers/external-protocol-capability/Cargo.toml` | ❌ Wave 0 |
| CAP-09 | Raw arbitrary/future kind constructs and publishes with no semantic registration/switch | facade + dependency-negative source check | `cargo test -p fava --test semantic_writes raw_future_kind` | ❌ Wave 0 |

### Critical State-Machine Matrix

The store/publication tests must cross each completion type with `current generation`, `retired generation`, `same generation wrong event/body`, `same event wrong provider operation`, and `terminal/cancelled receipt`. Only the exact current cell may mutate. [VERIFIED: docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:772-786] [VERIFIED: docs/spec/ARCHITECTURE.md:1647-1649]

| Completion | Required exact identity beyond receipt | Retired-result assertion |
|------------|----------------------------------------|--------------------------|
| Materializer | old generation + source-basis event/revision + coordinate | successor install refuses; no source/query revision |
| Signer | generation + unsigned body/id + actor + signer operation | no signature promotion/refusal on current generation |
| Route | generation + event + route session/revision | no current destinations/shortfalls/lane mutation |
| Publisher/delivery | generation + event id + relay session + lane + durable attempt | no current outcome/settlement; bounded old evidence remains attributable |

### Redb Crash/Reopen Boundaries

Extend the existing process-marker/SIGKILL harness; it already proves hard-kill boundaries for M5 acceptance, signature, attempt, outcome, and cancel. [VERIFIED: crates/fava-write-store-redb/tests/process_kill.rs:1-103]

Required M7 boundaries:

1. before semantic acceptance commit → zero edit/receipt/materialization/provider work;
2. after edit + generation-1 materialization commit → same edit/IDs/current event visible after reopen;
3. after source v2 observed but before successor commit → generation 1 remains exact; recovery reconciles once;
4. after generation-2 atomic commit but before new signer/route effects → generation 2 visible, old contribution absent, same IDs;
5. after predecessor attempt authorization but before successor correction → ambiguity/correction destination survives;
6. after successor commit with delayed predecessor completion → reopen/current generation remains unchanged;
7. unknown edit format or missing selected materializer → build/recovery refuses before admitting a conflicting command;
8. many superseding generations at one coordinate → reopen scales with bounded current work, not historical active tasks.

These are direct applications of WRITE-004/006/022/029 and the durability proof protocol. [VERIFIED: docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:728-770,921-928,984-990] [VERIFIED: docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md:327-338]

### Public Canary Spine

Add and enable these exact scenario identifiers:

DATA_710B5E2C_START

```text
replaceable-edit-first-value
replaceable-edit-rematerialization
replaceable-edit-inverse
protocol-crate-n-plus-one
```

DATA_710B5E2C_END

[VERIFIED: docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md:736-759,1298-1300]

The first three must use supported public capability/Fava/query/receipt calls. The rematerialization scenario must inject source v2 through the canonical verified cache/ingest boundary rather than editing write-store state. The N+1 scenario must compile and run outside the main workspace. [VERIFIED: docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md:219-227] [VERIFIED: docs/spec/ARCHITECTURE.md:3053-3079]

### Sampling Rate

- **Per task commit:** run the smallest owning crate tests plus `python3 tools/check_vocabulary.py` for every architectural/public API edit. [VERIFIED: AGENTS.md:51-60]
- **Per wave merge:** run the quick M7 package command, canary tests for public/assembly waves, and the external falsifier when its contract changes.
- **Phase gate:** full suite above; four canary scenarios green; exact deliberate-break failure recorded in `docs/issues/0010-m7-semantic-writes-and-capability-composition.md`; every CAP row has direct behavioral evidence. [VERIFIED: docs/issues/0010-m7-semantic-writes-and-capability-composition.md:39-51]

### Wave 0 Gaps

- [ ] Public scenario/feature evidence for all four M7 canaries.
- [ ] `crates/fava/tests/semantic_writes.rs` — first value, source v2, stable receipt, stale completion matrix, raw future kind.
- [ ] Pure codec/edit tests in both new protocol crates.
- [ ] Shared public capability corpus in the selected canary/test assembly.
- [ ] Memory store semantic state-machine/model tests.
- [ ] M7 cases in `crates/fava-write-store-redb/tests/process_kill.rs`.
- [ ] External N+1 separate workspace and universal-core allowlist/dependency checks.
- [ ] Bazel targets for new crates/tests and root Cargo metadata.
- [ ] Vocabulary checker registry update and separate architecture approval for every new public/cross-crate nominal/provider symbol.
- [ ] Named deliberate-break procedure and evidence location in issue 0010.

## Security Domain

Security enforcement and ASVS level 1 are enabled. M7 adds no authentication/session/access-control surface, but it does add untrusted relay-derived source input, durable opaque bytes, replaceable provider output, and cross-generation mutation authority. [VERIFIED: .planning/config.json:47-49] [VERIFIED: docs/spec/ARCHITECTURE.md:3155-3168]

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | No in M7 | Authentication is explicitly deferred to M8. [VERIFIED: .planning/phases/07-semantic-writes-and-capability-composition/07-CONTEXT.md:106-110] |
| V3 Session Management | No application session surface | Relay session identity already remains part of exact delivery correlation. [VERIFIED: docs/spec/ARCHITECTURE.md:1647-1649] |
| V4 Access Control | No new user authorization boundary | Enforce actor/coordinate equality and selected materializer ownership before custody/install. [VERIFIED: docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:716-724] |
| V5 Input Validation | Yes | Bound edit bytes/coordinate/materializer output; reject unknown format, malformed source, wrong author/coordinate, invalid id/body, oversized/expired event before durable/current mutation. [VERIFIED: docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:692-704,994-1000] |
| V6 Cryptography | Existing only | Reuse `nostr` verification/signing paths; do not hand-roll crypto. Private bookmark encryption is out unless separately approved. [VERIFIED: docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:772-806] [ASSUMED: private bookmarks deferred] |

### Known Threat Patterns for M7

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Malformed/oversized opaque edit or unknown version | Tampering / DoS | Pre-custody byte/coordinate bounds, exact version dispatch, typed refusal, no residue. |
| Materializer returns another actor/coordinate or oversized/expired event | Spoofing / Tampering / DoS | Revalidate author, coordinate, id/body, signature state, size, tags, expiry before atomic install. |
| Old signer/route/publisher/delivery result mutates current generation | Tampering | Durable exact generation/event/operation CAS at every store mutation. |
| Self-rebase repeatedly amplifies semantic changes | Tampering / DoS | Exclude own local materialization; compare source-basis identity; coordinate-scoped latest-state reconciliation. |
| Provider blocks, panics, ignores cancellation, or exceeds output bound | DoS | Invoke outside authoritative lock/transaction, isolate task failure, validate output, keep unrelated work progressing. [VERIFIED: docs/spec/ARCHITECTURE.md:3155-3168] |
| Durable decoder missing after restart | Availability / Integrity | Assemble registry before recovery; fail closed before new commands; never silently discard or reinterpret accepted edit. |
| Unbounded generation history/correction destinations | DoS | Bounded retained evidence/current work and typed refusal/shortfall; recovery proportional to current obligations. [VERIFIED: docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:945-954,984-990] |

## Project Constraints (from AGENTS.md)

- Treat `docs/spec` as authoritative in the declared order; preserve behavior/ownership when illustrative names differ; record a real contradiction in a focused local issue before choosing. Do not copy previous implementation code or add compatibility paths. [VERIFIED: AGENTS.md:1-17]
- Use one focused local issue, branch, validation set, and commit series; write observable behavior, then failing evidence/deliberate break, then production code; prove vertical behavior through public `fava`; never push without explicit authorization. [VERIFIED: AGENTS.md:30-38]
- Pass ownership, dependency direction, replaceability, failure isolation, boundedness, and behavioral-proof gates in proportion to every change. [VERIFIED: AGENTS.md:40-49]
- Treat vocabulary as closed. New crates/public or cross-crate nominal types/provider contracts/persisted entities/configuration/lifecycle owners require a separate focused architecture change approved by Pablo; update `docs/internals/vocabulary.toml`; run the checker and its tests. [VERIFIED: AGENTS.md:51-60]
- Keep code files below 800 lines, justify files over 500; keep shared values with their meaning owner; preserve declarative queries/write intents; never copy unpublished local events to event cache; refuse invalid use before work; use exact operation/generation identity; no feature flags/compatibility; separate neutral contracts from implementations early. [VERIFIED: AGENTS.md:62-75]

## Sources

### Primary (HIGH confidence)

- `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md` — WRITE-001 through WRITE-010, WRITE-021/022/029/030, PROTO-001 through PROTO-004.
- `docs/spec/ARCHITECTURE.md` — durable owner flow, edit value, query-source separation, write-store state, publication lifecycle/recovery, materializer assembly, M7 sequence, N+1 and provider-failure falsifiers.
- `docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md` — test placement, controlled schedules, durability proof, deliberate breaks.
- `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md` — M7 scope, required behavior, four canaries, exit gates.
- `.planning/phases/07-semantic-writes-and-capability-composition/07-CONTEXT.md` — locked implementation/capability/evidence decisions and deferred scope.
- Current M5/M6 sources cited inline — exact implementation seam audit.

### Secondary (MEDIUM confidence)

- [Official NIP-02 specification](https://github.com/nostr-protocol/nips/blob/master/02.md) — kind 3 contact-list tag/replacement semantics.
- [Official NIP-51 specification](https://github.com/nostr-protocol/nips/blob/master/51.md) — public bookmark kind/tags and private bookmark distinction.

### Tertiary (LOW confidence)

- None. Assumptions are isolated in the Assumptions Log and require planning confirmation.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — repository versions and crate boundaries were read directly; no new third-party dependency is recommended.
- Architecture: HIGH — authority documents and current M5/M6 seams agree on the required durable/live ownership split.
- Protocol wire semantics: MEDIUM — checked against official NIP specifications; the public-only bookmarks scope remains a user decision.
- Pitfalls: HIGH for generation/self-feedback/redb risks, MEDIUM for timestamp and multi-edit receipt policy because authorities leave those rules open.
- Validation: HIGH — placement and durability rules are authoritative; exact new test filenames are proposed.

**Research date:** 2026-08-21
**Valid until:** 2026-09-20 for stable local authorities; re-audit after any M7 contract/schema change.
