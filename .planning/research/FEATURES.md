# Feature Landscape

**Project:** Fava  
**Domain:** Embeddable cross-platform Nostr client engine  
**Researched:** 2026-08-21  
**Overall confidence:** HIGH for normative Fava scope; MEDIUM for ecosystem characterization

## Scope and Interpretation

This document classifies already-decided Fava behavior; it does not redefine scope. The authoritative specification and M0-M11 implementation plan remain controlling.

- **Normative Fava requirement:** required regardless of whether the wider ecosystem treats it as optional.
- **Ecosystem expectation:** demonstrated by official Nostr protocol documents or current official SDK projects.
- **Fava differentiator:** required Fava behavior whose rigor goes beyond basic protocol or SDK availability.
- **Anti-feature:** behavior the Fava specification explicitly rejects.

M0 is complete and must remain the independent evidence foundation. The next product claim is completion of all M1 exit gates, not expansion or replacement of M0.

## Table Stakes

Features users of an embeddable Nostr engine reasonably expect. Every row below is also normative Fava scope; “expected” does not weaken its acceptance criteria.

| Feature | Why Expected | Fava Behavioral Bar | Complexity | Dependencies / Milestone |
|---------|--------------|---------------------|------------|--------------------------|
| Raw Nostr events and filters | NIP-01 defines signed events, authors, ids, kinds, tags, time windows, limits, subscriptions, and publication messages; SDKs expose these primitives | Arbitrary/future kinds and validated raw tags remain expressible through the small public surface | Med | Semantic values; M1-M2 |
| Cryptographic and contextual event admission | A client cannot trust relay-supplied ids, signatures, subscription attribution, or filter compliance | Parse under bounds; recompute id; verify Schnorr signature; bind to exact session/request; refuse stale and off-filter input before cache, routing, or application visibility | High | Wire identity, verification, query context; M2, hardened M8 |
| Reactive live subscriptions | Official SDKs expose reactive subscriptions; NIP-01 distinguishes stored events, EOSE, live events, CLOSE, and relay-side CLOSED | Open atomically; return the complete local view immediately; start live demand at open; update on every relevant state/evidence change; deterministic cancel/close | High | M1 semantic state, M2 transport, M3 observation |
| Canonical event-state semantics | Applications expect one current view despite duplicate, replaceable, addressable, deleted, or expired events | Deterministic event-id deduplication, replaceable winner/tie rules, author-valid deletion, expiry, source removal, and evidence merge across providers | High | Complete M1 before networking claims |
| Multi-relay connection and subscription management | Relay pools, deduplication, reconnect, and grouped subscriptions are standard SDK capabilities | One logical result across relays; only actual serving relays receive provenance; fresh generation identity on reconnect; safe planner grouping with logical attribution preserved | High | M2 then M3-M4 |
| Local caching and offline reuse | Current SDKs expose memory, SQLite, LMDB/nostrdb, IndexedDB, Redis, and other cache adapters | Baseline cache is coherent without implying persistence; each product profile declares eviction, provenance, tombstone, expiry, coverage, restart, and resource guarantees truthfully | High | M1 memory sources; persistent/ephemeral qualification M9 |
| Pluggable signing and crypto operations | Local, browser/extension, remote, and hardware-compatible signer shapes are established SDK expectations | Exact pubkey/body/generation binding; distinct unavailable/rejected/invalid/cancelled/stale outcomes; signer cannot own publication success; secrets never enter generic state | High | M5 publication; M8 isolation; M11 bridges |
| Event construction and protocol helpers | SDKs commonly provide builders and typed NIP helpers while retaining raw event access | One general builder; event-kind meaning stays in independent capability crates; unknown Nostr remains expressible | Med | M5 primitive; M7 capability composition |
| Publication with relay outcomes | NIP-01 defines event-specific `OK` acceptance/refusal and machine-readable reasons | Exact per-destination receipt facts, verbatim relay messages, mixed outcomes, bounded retry/give-up, ambiguity preservation, and one aggregate terminal result | High | M5 explicit publication; M6 automatic routing; M8 hostile cases |
| Explicit and automatic relay routing | NIP-65 defines author write relays and tagged-user read relays; current SDKs advertise outbox routing | Explicit non-empty relay sets bypass routers exactly; automatic routing composes ordered policies; known routes start immediately and expand asynchronously | High | M4 read routing; M6 write routing |
| Relay authentication | NIP-42 allows challenges at connection or request time and distinguishes `auth-required` from `restricted` | Access context is separate from author/account/filter identity; challenge and completion are generation-scoped; denial affects only the exact operation | High | M8 |
| Relay metadata and limit awareness | NIP-11 exposes supported NIPs and practical read/write/message/subscription limits | Service-owned freshness; honor known limits or expose exact relay-scoped shortfall; never silently clamp or omit requested work | High | M8 limits; M9 service/cache profile |
| NIP-05 identity resolution | NIP-05 is a common optional service and explicitly identifies rather than cryptographically verifies | Independent validation, positive/negative caching, freshness, last-good and last-error facts; service data never becomes event-cache truth | Med | M9 |
| Account and session support | Multi-account sessions and signer selection are common client SDK features | Current account is reactive only where declared; accepted writes retain resolved author; all-or-nothing restore; logout/account removal does not rewrite durable writes | High | M7-M9; parity M11 |
| Application-facing diagnostics | Embeddable engines need relay, subscription, cache, routing, and publication observability | Bounded queryable facts, exact identities/reasons/shortfalls, no synthesized health or completeness score, lazy/coalesced delivery | Med-High | Incremental in every milestone |
| Application and provider test facilities | Current SDKs provide mock relays, deterministic event factories, time control, and network failure simulation | Public falsifiers for protocol faults, races, restart, cancellation, provider substitution, lifecycle, and resource claims; real-relay capstones remain independent | High | M0 foundation, expanded M1-M11 |
| Native consumable artifacts | Official Nostr SDK projects publish Swift, Android, JVM, Kotlin Multiplatform, and other packages | Ordinary external Swift/Kotlin artifacts expose only selected profile/capabilities and do not require repository-relative sources or raw binding loading | High | M10-stable public contracts; M11 |
| Bounded resources and deterministic teardown | Mobile and embedded consumers cannot accept unbounded queues, tasks, retained receipts, or sessions | Every externally influenced collection/queue has a bound, backpressure, typed refusal, or exact shortfall; close wakes pending work and returns resources to baseline | High | Starts M1; stress/hostile qualification M3, M8, M10-M11 |

## Differentiators

These are not optional enhancements. They are normative Fava requirements that define why Fava is more than another protocol wrapper.

| Feature | Value Proposition | Complexity | Notes / Proof Point |
|---------|-------------------|------------|---------------------|
| One coherent view from independent event-cache and write-store authorities | Gives applications immediate optimistic local state without corrupting relay-observed cache truth | High | Same event merges once; pending local replacement shadows cached predecessor; cancellation naturally reveals predecessor; M1 |
| Exact source-scoped evidence without global completeness | Lets applications explain what each relay/request actually proved without false “synced” or authoritative-empty claims | High | EOSE, silence, timeout, CLOSED, auth, retry exhaustion, cancellation, and shortfall remain distinct; M2-M3, M8 |
| Durable write identity and reattachable receipts | Turns publication into a recoverable obligation rather than a transient send call | Very High | `Accepted` follows atomic durable commit and local visibility; same write/receipt survives kill/restart; generations, lanes, and outcomes remain correlated; M5-M6 |
| Partial progress as a first-class outcome | Avoids delaying useful work behind unresolved routing, signing, relay, or recipient knowledge | Very High | Known relays start immediately; later contributions expand the same query/receipt; unresolved never becomes absent through elapsed time; M4, M6 |
| Semantic write rematerialization | Preserves user intent when newer replaceable source state appears after an offline edit | Very High | Same receipt across corrected event generations; unrelated source changes preserved; stale signing/delivery completions rejected; M7 |
| Loss-honest bounded observation | Supports slow mobile/UI consumers without unbounded memory or false state | High | Current-state streams may coalesce but next state is exactly rebased; causal receipt/lifecycle facts cannot disappear silently; M1, M3, M8 |
| Exact operation and generation identity | Prevents late reconnect, signer, provider, cancellation, and delivery completions from mutating current work | High | Every late completion is attributable and stale results are inert; cross-cutting M2-M11 |
| Static provider composition with no privileged defaults | Lets applications select storage, routing, planning, transport, publisher, delivery, signer, and service implementations without core forks | Very High | Contracts and implementations remain separate from their first real slice; external implementations use the same public contracts and conformance kits; M4-M10 |
| Failure isolation across replaceable providers | One blocking, failing, panicking, or human-mediated provider cannot wedge unrelated relays, queries, writes, or shutdown | Very High | No foreign work under another owner’s lock/transaction; bounded execution and exact stale completion handling; M8-M10 |
| Truthful product profiles | Makes persistence, routing, retry, ambiguity, service, and platform guarantees auditable instead of implicit defaults | High | Persistent and ephemeral cache profiles share semantics while declaring different restart guarantees; M9-M10 |
| Executable Rust/Swift/Kotlin behavioral parity | Gives product teams one semantic engine rather than similarly named but divergent native SDKs | Very High | Same event records, evidence, receipts, errors, cancellation, routing, restart, and close outcomes in real platform processes; M11 |
| Independent evidence and deliberate-break proof | Prevents Fava from acting as the sole witness for its own network, persistence, or lifecycle claims | High | Public facade plus independent proxy/relay/process evidence; every protection must fail under its named falsifier; M0 foundation used through M11 |

## Anti-Features

Features and shortcuts to explicitly not build.

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| Reopening or replacing validated M0 | M0 is already the independent evidence foundation; revisiting it obscures the next incomplete claim | Reuse its relay lab, supervisor, wire evidence, and falsifier machinery for M1-M11 gates |
| Application framework ownership | UI state, navigation, ranking, moderation, recommendation, account UX, and product workflows vary by application | Keep Fava an embeddable library; expose values, lifecycles, diagnostics, and platform-native wrappers |
| Global `synced`, `complete`, percentage, authoritative-empty, or end-of-history | No queried relay set proves the whole Nostr network | Report exact relay/request EOSE, errors, auth, shortfall, and records currently known |
| Inferring gap-free history from reconnect | Reconnect restores current demand but cannot prove events missed during outage were recovered | Require explicit backfill/windowing behavior before making a history guarantee |
| Automatic negentropy or a parallel history workload | It silently changes the ordinary query lifecycle and protocol surface | Keep ordinary reads as NIP-01 requests; any reconciliation remains explicit opt-in protocol work |
| Runtime plugin discovery, hot swapping, or dynamic provider registries | Adds lifecycle/migration complexity without serving the static product assembly model | Select providers explicitly at build/application composition; fix the implementation set for an engine instance |
| Collapsing contract and implementation crates because only one provider exists | Shapes the public seam around the first mechanism and creates a privileged default | Split the neutral contract from the implementation when its first real vertical slice lands; challenge it with conformance and substitution |
| Generic “common” bucket or duplicate semantic values | Blurs ownership and permits the same fact to acquire different meanings | Put shared values in their semantic-owner crate and use them across neutral contracts |
| Copying unsigned or unpublished local events into the event cache | Couples independent authorities and requires compensating cache writes on cancel/rematerialization | Let the durable write store remain a local query source; cache only admitted signed relay observations per cache contract |
| Deletion as local cancellation | A kind:5 event is a new public protocol write and cannot unsend local work | Keep deletion publication, event-state application, write cancellation, and receipt removal separate |
| Invented “unsend” after possible handoff | Bytes may already have left Fava and cannot be recalled | Permit proven pre-handoff cancellation; retain exact ambiguous/completed history after possible handoff; resolve partial-handoff policy explicitly |
| Silent truncation, clamping, drops, or unbounded queues | Hides missing work and makes resource exhaustion indistinguishable from success | Use explicit bounds with typed refusal, backpressure, loss facts, or source-scoped shortfall |
| Hardcoded public relay fallback | Contacts unjustified infrastructure and overrides application routing policy | Compose selected outbox, hint, app-relay, and fallback routers; report no-destination/unresolved honestly |
| Provider-specific private facade bypass | Makes external substitution nominal rather than real | Require standard and external implementations to use identical public contracts and conformance corpora |
| Universal core branching on event-kind meaning | Expands the engine into a protocol catalog and makes capability N+1 change core owners | Put typed decoding, builders, validation, references, and semantic edits in independent capability crates |
| Service payloads in the event cache | NIP-05/NIP-11 HTTP data has different validation, freshness, and failure meaning from Nostr events | Store opaque payloads in a service cache while each service owns semantics |
| App-owned reconnect, retry, route, or receipt reducers | Creates duplicate lifecycle owners and inconsistent recovery/cancellation behavior | Let Fava/provider owners manage these lifecycles; applications consume exact public state |
| Cross-provider persisted-format compatibility by default | Private formats and migrations belong to each provider and cannot be inferred safely | Each provider validates, migrates, or explicitly refuses its own bytes; application owns deliberate provider migration |
| Hidden feature flags, legacy NMP compatibility, or copied old implementation paths | Violates the clean-room rewrite and introduces behavior outside the authoritative documents | Implement only the Fava specifications through explicit profiles and public contracts |
| Expensive eager diagnostics or global health scoring | Wastes resources and invents causality/completeness not supported by facts | Produce bounded, lazy, coalesced diagnostic facts with exact identities and loss counts |
| Compile-only or simulator-only native parity claims | Artifact shape does not prove cancellation, restart, suspension, or resource behavior | Run shared scenarios through ordinary external artifacts in real native processes; use physical devices for claims that require them |
| Public-relay availability as a deterministic release gate | Public infrastructure is uncontrolled and outages/policy changes make results non-repeatable | Use controlled real third-party relay processes for gates; keep public relays as explicit reconnaissance |
| Silently resolving the five open product decisions | Premature defaults would become accidental promises | Decide windowing, partial-handoff cancellation, outage backfill, full delivery history, and recommended persistent cache profile in their owning milestones |

## Feature Dependencies

The implementation plan’s milestone graph is authoritative:

```text
M0 evidence foundation [COMPLETE]
  -> M1 deterministic local semantic state and merged sources
       -> M2 explicit one-relay live query
            -> M3 multi-relay reactivity and bounded observation
                 -> M8 hostile/auth/limits qualification
                 -> M9 truthful cache/service restart profiles
       -> M5 durable explicit-route publication
            -> M6 automatic routing and partial delivery
       -> M4 ordered async routing and exact subscription planning
            -> M6 automatic routing and partial delivery

M7 semantic writes requires M1 + M5 + enough M6 source/routing behavior
M8 requires the mature read/routing/publication paths from M3/M4/M6
M9 requires M3 and M8
M10 provider substitution requires qualified profiles through M9
M11 native parity requires stabilized public contracts and profiles through M10
```

Critical behavioral dependencies:

```text
Canonical semantic identity -> equivalent query sharing -> safe subscription planning
Exact request/session generation -> admission -> EOSE/provenance/reconnect truth
Independent cache + write-store sources -> optimistic visibility -> relay-echo merge
Durable acceptance -> stable receipt -> restart recovery -> asynchronous route expansion
Static public contracts -> alternative providers -> profile matrix qualification
Rust semantic oracle -> FFI value/lifecycle projection -> Swift/Kotlin parity corpus
Bounded owner delivery -> deterministic close -> mobile lifecycle/resource qualification
```

## Milestone Capability Map

| Milestone | Capability Claim Earned Only When All Gates Pass | Primary Feature Risk |
|-----------|--------------------------------------------------|----------------------|
| M1 | Coherent local state across independent memory cache/write-store sources | False merge, unstable query identity, incomplete deletion/expiry/removal, unbounded observation |
| M2 | Exact explicit live query against one real relay | Unverified/off-filter ingress, false EOSE, cancellation leakage |
| M3 | Multi-relay dedup/provenance, reconnect generations, bounded latest-state delivery | Stale completion, bystander provenance, slow-consumer backlog |
| M4 | Ordered asynchronous read routing and semantics-preserving wire planning | Blocking on route settlement, explicit-route contamination, grouping that changes meaning |
| M5 | Durable explicit-route write lifecycle and process-death recovery | Reporting `Accepted` before commit, cache pollution, ambiguous outcomes collapsed |
| M6 | Independently composed automatic routing with partial delivery under one receipt | Duplicate sends, frozen routes, unresolved targets blocking known work |
| M7 | Semantic edits and capability N+1 without core changes | Lost source changes, stale generation completion, NIP-specific core branching |
| M8 | Authentication, hostile relay/provider isolation, limits, ambiguity, give-up, resource bounds | Cross-account leakage, silent shortfall, runaway retries/queues, wedged shutdown |
| M9 | Truthful persistent/ephemeral event-cache, durable write, NIP-05/NIP-11, and reset profiles | Implied persistence, service semantic leakage, incomplete reset, corrupt-state reinterpretation |
| M10 | Real substitution across every major provider seam | Privileged defaults, interface shaped around one backend, non-public test kits |
| M11 | Equivalent Rust/Swift/Kotlin products through ordinary native artifacts | API/outcome drift, binding-only proof, untested process/suspension semantics |

## MVP Recommendation

For the next validated increment, prioritize:

1. Complete M1’s entire semantic corpus: stable equivalent-query identity, canonical replacement/deletion/expiry, source merge/removal, bounded current-state delivery, shared provider corpora, and all public-canary exit gates.
2. Deliver M2’s explicit single-relay read path with exact admission, stored/live behavior, EOSE, cancellation, diagnostics, and independent wire proof.
3. Deliver M3’s multi-relay dedup/provenance, reconnect-generation fencing, source removals, and slow-consumer boundedness.

Then continue M4-M11 under the authoritative dependency graph. No normative later milestone is dropped; it is deferred only until its prerequisite product claims pass. Do not call the existing M1 tracer an M1 completion and do not reopen M0.

## Explicitly Unresolved Product Decisions

These are not feature gaps to fill opportunistically:

1. Public growable-window/query API and resume token model.
2. Cancellation semantics after partial relay handoff.
3. Whether any profile promises outage-interval backfill.
4. Retention of full historical attempt detail beyond exact current receipt evidence.
5. Which persistent event-cache guarantee profile is recommended for the primary shipped client artifact.

## Sources

### Normative project authorities — HIGH confidence

- [Fava project definition](../PROJECT.md)
- [Full Fava Rewrite Specification](../../docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md)
- [Fava Rewrite Implementation Plan](../../docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md)
- [Specification index and authority order](../../docs/spec/README.md)

### External protocol and ecosystem evidence — MEDIUM confidence

Confidence is MEDIUM because the configured research-confidence seam caps verified web/official-source findings at MEDIUM. External sources clarify expectations only; they do not alter Fava scope.

- [NIP-01 basic protocol, exact inspected revision](https://github.com/nostr-protocol/nips/blob/656cecc7c0a815b6a2b218d3b5d6f078b3f4dbab/01.md) — event/filter/wire baseline, per-connection subscription identity, `OK`, `EOSE`, `CLOSED`.
- [NIP-05, exact inspected revision](https://github.com/nostr-protocol/nips/blob/656cecc7c0a815b6a2b218d3b5d6f078b3f4dbab/05.md) — HTTPS identity mapping, non-cryptographic meaning, redirect restriction.
- [NIP-09, exact inspected revision](https://github.com/nostr-protocol/nips/blob/656cecc7c0a815b6a2b218d3b5d6f078b3f4dbab/09.md) — author-scoped deletion request semantics.
- [NIP-11, exact inspected revision](https://github.com/nostr-protocol/nips/blob/656cecc7c0a815b6a2b218d3b5d6f078b3f4dbab/11.md) — relay metadata, supported NIPs, and relay-specific practical limits.
- [NIP-40, exact inspected revision](https://github.com/nostr-protocol/nips/blob/656cecc7c0a815b6a2b218d3b5d6f078b3f4dbab/40.md) — expiration behavior and its non-security limitation.
- [NIP-42, exact inspected revision](https://github.com/nostr-protocol/nips/blob/656cecc7c0a815b6a2b218d3b5d6f078b3f4dbab/42.md) — connection-scoped challenges and exact auth outcomes.
- [NIP-65, exact inspected revision](https://github.com/nostr-protocol/nips/blob/656cecc7c0a815b6a2b218d3b5d6f078b3f4dbab/65.md) — author write relays, tagged-user read relays, and outbox routing inputs.
- [Nostr Dev Kit Rust workspace, exact inspected revision](https://github.com/nostrdevkit/nostr/blob/4dcf307d6102e598f3a4172b8ba5a7a779fa1630/README.md) — high-level SDK, signer integrations, database contracts/backends, broad NIP surface.
- [NDK TypeScript ecosystem, exact inspected revision](https://github.com/nostr-dev-kit/ndk/blob/4b86acd13fe3c1284fddcb81a7f0d63e491db64a/README.md) — reactive/grouped subscriptions, caching, signers, outbox routing, modules, testing utilities, mobile bindings.
- [NDK Kotlin/Android, exact inspected revision](https://github.com/nostr-dev-kit/kotlin/blob/1039b3bd4e25f4f601c8f5580bd4c5c5d93df916/README.md) — Flow-based subscriptions, pluggable cache/signer/relay policy, dynamic outbox routing, testing and diagnostics.
- [Nostr SDK native bindings, exact inspected revision](https://github.com/nostrdevkit/nostr-sdk-ffi/blob/0aa3edde1438d95048a1306f039b98e97642da35/README.md) — Swift and Kotlin/Android/JVM/KMP packaging; project explicitly reports alpha status.

## Confidence Notes

| Area | Confidence | Reason |
|------|------------|--------|
| Normative Fava features and anti-features | HIGH | Directly derived from authoritative local specification and milestone gates |
| M1-M11 dependencies and ordering | HIGH | Directly derived from the authoritative implementation plan |
| Nostr protocol baseline | MEDIUM | Current official NIP repository inspected at a pinned revision; confidence tier assigned by seam |
| Current SDK ecosystem expectations | MEDIUM | Current official repositories cloned and inspected at pinned revisions, cross-checked across Rust, TypeScript, Kotlin, and native binding projects |
| Claims of comparative uniqueness | Not asserted | Research supports that Fava sets a stronger explicit contract, but does not claim no other project can provide similar guarantees |
