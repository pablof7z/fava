# Roadmap: Fava

## Overview

Fava advances from the completed M0 evidence baseline through authoritative milestones M1 through M11 plus focused inserted phases that repair or add required public capabilities before the next major milestone. M1-M7 and the tag-value query slice are complete; Phase 07.1 is active and the approved `fava-simple-groups` Phase 07.1.1 follows before M8. Each phase delivers one public-facade capability, retains its complete exit gates, and is complete only when every mapped requirement satisfies the project Definition of Done.

## Completed Prerequisite Baseline

M0 is complete and remains outside the active phase list. Its independent real-relay lab, process-kill/restart proof, bounded evidence bundle, fail-not-skip behavior, and separation from Fava internals remain prerequisites and witnesses for all later phases.

## Binding Completion Contract

The success criteria below summarize observable application and public-facade outcomes. A phase is not complete until every mapped requirement, every complete exit gate in `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`, and every item in `.planning/REQUIREMENTS.md`'s Definition of Done passes, including causal pre-implementation failure, the named deliberate break, public capstones, independent evidence where required, exact lifecycle/resource behavior, current documentation, the scoped validation set, and committed implementation.

The five open product decisions remain unpromised unless their owning phase qualifies a choice: public windowing/resume tokens and outage-interval backfill in Phase 3; partial-handoff cancellation and full historical attempt-detail retention in Phase 6; and the recommended persistent event-cache guarantee profile in Phase 9. No roadmap wording establishes a default.

## Phases

- [x] **Phase 1: Deterministic Local Semantic State** - Complete M1's coherent local view from independent event-cache and write-store sources.
- [x] **Phase 2: Exact Single-Relay Live Query** - Complete M2's explicit real-relay read path with exact admission, evidence, cancellation, and close.
- [x] **Phase 3: Multi-Relay Reactivity and Bounded Observation** - Complete M3's deduplication, provenance, reconnect, removal, and bounded observation behavior.
- [x] **Phase 4: Ordered Routing and Subscription Planning** - Complete M4's asynchronous route contributions and meaning-preserving wire planning.
- [x] **Phase 5: Durable Explicit-Route Publication** - Complete M5's durable write, local visibility, exact explicit delivery, receipt, cancellation, and recovery spine.
- [x] **Phase 6: Automatic Routing and Partial Delivery** - Complete M6's live write routing and route expansion under one durable receipt.
- [x] **Phase 7: Semantic Writes and Capability Composition** - Complete M7's replaceable-event edits and protocol-crate extensibility without core kind switches.
- [ ] **Phase 07.1: Universal Publication Vocabulary and Typed NIP-02 Reads** - Complete the active follow-up slice selected before group capability work.
- [ ] **Phase 07.1.1: Multi-Relay Simple Groups** - Deliver `fava-simple-groups` from its README North Star with exact per-host truth and ordinary Fava lifecycles.
- [ ] **Phase 8: Authentication, Hostile Boundaries, and Boundedness** - Complete M8's exact auth, hostile-input, limit, retry, ambiguity, isolation, and resource behavior.
- [ ] **Phase 9: Truthful Profiles and Protocol Services** - Complete M9's persistent/ephemeral profiles, restart/reset guarantees, and service-owned cache semantics.
- [ ] **Phase 10: Provider Substitution Qualification** - Complete M10's public-contract substitution matrix and architecture falsifiers.
- [ ] **Phase 11: Native Products and Release Qualification** - Complete M11's packaged Rust, Swift, Kotlin, Android, and iOS parity through real processes.

## Phase Details

### Phase 1: Deterministic Local Semantic State

**Goal:** Applications receive one deterministic, coherent local query view merged from independent event-cache and write-store authorities.
**Mode:** mvp
**Depends on:** Completed M0 prerequisite baseline
**Requirements:** LOCAL-01, LOCAL-02, LOCAL-03, LOCAL-04, LOCAL-05, LOCAL-06, LOCAL-07, LOCAL-08, LOCAL-09, LOCAL-10, LOCAL-11, LOCAL-12
**Success Criteria** (what must be TRUE):

  1. An application opens a local query and immediately receives one complete deterministic snapshot whose identity, replacement/addressability, deletion, expiry, ordering, and same-event evidence merge semantics are stable.
  2. A pending local replacement shadows a cached predecessor without cache pollution; cancellation or source removal retracts only the owning contribution and every affected open query naturally reveals the still-qualified state.
  3. Equivalent query descriptions have stable semantic identity, and slow consumers remain bounded while each delivered value rebases them onto the exact latest result with truthful coalescing.
  4. An ordinary downstream application uses only the public Fava facade to inspect cache/write-store evidence and passes the same semantic corpus against the independent memory providers without relay, transport, or runtime networking.

**Plans:** Pre-GSD execution; completion provenance and requirement evidence are recorded in `01-VERIFICATION.md` without inventing retrospective plans.

### Phase 2: Exact Single-Relay Live Query

**Goal:** Applications can run one exact explicit live query against a real relay with verified admission, source-scoped evidence, and deterministic cancellation.
**Mode:** mvp
**Depends on:** Phase 1
**Requirements:** READ-01, READ-02, READ-03, READ-04, READ-05, READ-06, READ-07, READ-08, READ-09, READ-10
**Success Criteria** (what must be TRUE):

  1. An application opens a query against an exact non-empty explicit relay list, starts live work immediately without automatic routers, receives bounded NIP-01 stored events and actual EOSE, and continues receiving later matching events on the same query.
  2. Only events verified and attributed to the accepted relay session, request generation, access context, and subscription can affect cache/query state; malformed, forged, off-filter, stale, and post-terminal input remains inert.
  3. The application can distinguish empty-plus-EOSE, silence, failure, authentication required, NOTICE, CLOSED, timeout, cancellation, and shortfall without any global completeness claim.
  4. Cancellation performs exact withdrawal, wakes pending pulls, prevents later delivery for the cancelled generation, and idempotent close releases owned resources; public diagnostics and the independent wire witness agree on identities and effects.

**Plans:** Pre-GSD execution; completion provenance and requirement evidence are recorded in `02-VERIFICATION.md` without inventing retrospective plans.

### Phase 3: Multi-Relay Reactivity and Bounded Observation

**Goal:** Applications retain exact current state and lifecycle truth as multiple relays, reconnect generations, removals, and slow consumers interact.
**Mode:** mvp
**Depends on:** Phase 2
**Requirements:** READ-11, READ-12, READ-13, READ-14, READ-15, READ-16, READ-17, READ-18, READ-19, READ-20
**Success Criteria** (what must be TRUE):

  1. The same event served by several relays appears once with evidence only for relays that actually served it, and provenance-only revisions update that record without duplication.
  2. Reconnect restores active demand under fresh session/request generation identity without application resubscription; stale completions are inert and no outage backfill or history-completeness claim is implied.
  3. Slow current-state consumers receive a bounded exact latest result with truthful coalescing/loss diagnostics, while causal lifecycle facts remain loss-honest and repeated pending-pull cancellation cannot retain stale waiters or backlog.
  4. Public diagnostics expose exact query, relay session, access context, request generation, logical demand, wire subscription, terminal reason, and source counts, while at least 1,000 idle observations stay within the declared standard resource envelope.

**Plans:** Pre-GSD execution; completion provenance and requirement evidence are recorded in `03-VERIFICATION.md` without inventing retrospective plans.
**Open product decisions:** A public growable-window/resume-token model and any outage-interval backfill guarantee remain explicitly unpromised unless Phase 3 qualifies them from forcing workloads and measured bounds.

### Phase 4: Ordered Routing and Subscription Planning

**Goal:** Applications gain immediate, reactive automatic read routing while routing policy remains separate from per-relay subscription wire shape.
**Mode:** mvp
**Depends on:** Phase 3
**Requirements:** ROUTE-01, ROUTE-02, ROUTE-03, ROUTE-04, ROUTE-05, ROUTE-06, ROUTE-07, ROUTE-08, ROUTE-09, ROUTE-10, ROUTE-11
**Success Criteria** (what must be TRUE):

  1. The selected router chain contributes in configured order, starts known destinations immediately, lets later complete replacements expand/retract work, and deduplicates destinations while preserving every reason, target, and unresolved need.
  2. Explicit routing creates no automatic router session or router-owned acquisition, router acquisition uses explicit non-recursive sources, and route preview matches real derivation without creating write or delivery effects.
  3. Standard and no-grouping planners may produce different bounded wire shapes for relay-assigned logical demand while applications observe identical query meaning, evidence, access isolation, and cancellation.
  4. Relay limits and router contribution/fan-out budgets produce exact typed shortfall rather than silent omission, and public diagnostics show immediate versus delayed contributions without private inspection.

**Plans:** Pre-GSD execution; completion provenance and requirement evidence are recorded in `04-VERIFICATION.md` without inventing retrospective plans.

### Phase 5: Durable Explicit-Route Publication

**Goal:** Applications can durably accept, observe, publish, cancel, recover, and reattach explicit-route writes under one exact write and receipt identity.
**Mode:** mvp
**Depends on:** Phase 4
**Requirements:** WRITE-01, WRITE-02, WRITE-03, WRITE-04, WRITE-05, WRITE-06, WRITE-07, WRITE-08, WRITE-09, WRITE-10, WRITE-11
**Success Criteria** (what must be TRUE):

  1. An application accepts an unsigned or verified pre-signed event only after its obligation, current materialization, receipt identity, and recovery cursor are durable; unsigned authorship selects the signer without becoming relay-auth identity.
  2. Matching queries expose the accepted write-store materialization before relay acknowledgement, while the event cache contains no unsigned/unpublished local event and may later admit only the verified relay echo.
  3. Exact explicit destinations bypass routers; publisher handoff and delivery retry/give-up policy remain separate, and each destination exposes exact attempt generation, relay text, acknowledgement, rejection, ambiguity, cancellation, and terminal reason.
  4. Proven pre-handoff cancellation emits zero EVENT frames, retracts local visibility, and records an idempotent terminal receipt independently of receipt removal; process kill after acceptance recovers the same obligation, write, receipt, and materialization without resubmission.

**Plans:** Pre-GSD execution; completion provenance and requirement evidence are recorded in `05-VERIFICATION.md` without inventing retrospective plans.

### Phase 6: Automatic Routing and Partial Delivery

**Goal:** Applications can publish immediately to known automatic destinations and add later destinations under the same signed event and receipt without duplicate delivery.
**Mode:** mvp
**Depends on:** Phase 5
**Requirements:** WRITE-12, WRITE-13, WRITE-14, WRITE-15, WRITE-16, WRITE-17, WRITE-18, WRITE-19, WRITE-20, WRITE-21, WRITE-22, WRITE-23
**Success Criteria** (what must be TRUE):

  1. The application-selected outbox, hint, app-relay, and fallback router chain is the only automatic write policy, with each crate owning its documented facts and route preview matching initial real derivation without side effects.
  2. Known destinations begin delivery while needs remain unresolved; later route contributions add lanes under the same receipt and signed event, and duplicate contributions never duplicate handoffs.
  3. Route retraction retires only work proven not to have crossed handoff, preserves exact historical delivery facts, and continued route evaluation uses exact route-revision and lane-generation identity.
  4. Route contributions, fan-out, destinations, attempts, retries, receipt facts, and retained history remain explicitly bounded or return typed refusal/shortfall while independent wire evidence proves partial progress.

**Plans:** Pre-GSD execution; completion provenance and requirement evidence are recorded in `06-VERIFICATION.md` without inventing retrospective plans.
**Open product decisions:** Cancellation after partial handoff and retention of full historical attempt detail remain explicitly unpromised unless Phase 6 qualifies exact semantics and bounds.

### Phase 06.1: Literal Tag-Value Query Semantics Remediation (INSERTED)

**Goal:** As a Fava application developer, I want to select events by any exact case-sensitive ASCII one-letter Nostr tag key and safely group compatible relay demand, so that wire optimization never changes logical results or evidence.
**Mode:** mvp
**Requirements:** LOCAL-09, ROUTE-10
**Depends on:** Phase 6
**Plans:** 3/3 plans complete

**Success Criteria** (what must be TRUE):

  1. The public query surface accepts all 52 ASCII one-letter Nostr tag keys, preserves `#e` and `#E` as distinct axes, and has stable identity independent of construction order.
  2. Exact values are ORed within one key and distinct keys are ANDed with each other and with ids, authors, and kinds; a present empty value set matches nothing.
  3. Local evaluation and NIP-01 relay filters preserve exact key case and value bytes, with executable opposite-case and UTF-8 evidence through the public facade.
  4. Three hundred compatible tag-value logical queries may share one wire request while exact per-query matching and evidence remain unchanged.

Plans:
**Wave 1**

- [x] 06.1-01-PLAN.md — Restore canonical literal tag identity and exact signed/unsigned local observation through public Fava.

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 06.1-02-PLAN.md — Preserve exact tag axes on NIP-01 wire demand and safely group one compatible axis with full attribution.

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 06.1-03-PLAN.md — Prove 300-query controlled-relay equivalence, causal case-fold mutation detection, and full validation.

### Phase 7: Semantic Writes and Capability Composition

**Goal:** As a Fava application developer, I want to express replaceable-event edits through independent protocol crates, so that they reuse one durable publication lifecycle and survive source-state changes.
**Mode:** mvp
**Depends on:** Phase 06.1
**Requirements:** CAP-01, CAP-02, CAP-03, CAP-04, CAP-05, CAP-06, CAP-07, CAP-08, CAP-09
**Success Criteria** (what must be TRUE):

  1. A protocol capability crate exposes ordinary event values or authorless semantic edits and opposing operations; acceptance freezes the author once, and a first-value edit materializes and publishes through the ordinary write receipt without the crate signing, routing, delivering, or owning receipts.
  2. When newer qualified source state arrives, still-live edits rematerialize while preserving unrelated changes and the same write/receipt identity across generations.
  3. Signer, route, publisher, and delivery completions for retired materialization generations are attributable but inert.
  4. Two unrelated capability crates pass the shared public corpus; adding capability N+1 changes only its crate and selected assembly metadata, while arbitrary/future event kinds remain usable without universal-core switches.

**Plans:** 9/9 complete

- [x] 07-01-PLAN.md
- [x] 07-02-PLAN.md
- [x] 07-03-PLAN.md
- [x] 07-04-PLAN.md
- [x] 07-05-PLAN.md
- [x] 07-06-PLAN.md
- [x] 07-07-PLAN.md
- [x] 07-08-PLAN.md
- [x] 07-09-PLAN.md

### Phase 07.1: Universal publication vocabulary and typed NIP-02 reads (INSERTED)

**Goal:** An application can read, discover, edit, publish, and await NIP-02 contact-list changes through the README-level typed Rust API, while the same universal `publish`/`by`/`to` vocabulary accepts every write payload without exposing `WriteIntent` ceremony.
**Requirements**: R1, R2, R3, R4, R5, R6, R7, R8, R9
**Depends on:** Phase 7
**Plans:** 11/12 plans executed

Plans:
**Wave 1**

- [x] 07.1-01-PLAN.md — Trace universal synchronous publication from public payload to durable, query-visible `Write`

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 07.1-02-PLAN.md — Add inert signer/relay scopes and ordered explicit-route custody

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 07.1-03-PLAN.md — Add exact receipt summaries and bounded caller-selected settlement

**Wave 4** *(blocked on Wave 3 completion)*

- [x] 07.1-04-PLAN.md — Decode kind-3 events into conserving typed `ContactList` values

**Wave 5** *(blocked on Wave 4 completion)*

- [x] 07.1-05-PLAN.md — Add one/many contact reads, follower discovery, and pure projection

**Wave 6** *(blocked on Wave 5 completion)*

- [x] 07.1-06-PLAN.md — Complete lossless README-shaped NIP-02 edit helpers

**Wave 7** *(blocked on Wave 6 completion)*

- [x] 07.1-07-PLAN.md — Migrate primary facade publication regressions
- [x] 07.1-08-PLAN.md — Migrate store, lifecycle, signer, and fault harnesses
- [x] 07.1-09-PLAN.md — Remove the old facade compatibility surface and prove independent consumption
- [x] 07.1-10-PLAN.md — Migrate the existing downstream canary corpus

**Wave 8** *(blocked on Wave 7 completion)*

- [x] 07.1-11-PLAN.md — Replace authoritative specs, vocabulary, issues, validation, and README

**Wave 9** *(blocked on Wave 8 completion)*

- [ ] 07.1-12-PLAN.md — Run the controlled Croissant canary twice and close all phase gates

### Phase 07.1.1: Multi-Relay Simple Groups (INSERTED)

**Goal:** As a Fava application developer, I can use the README-shaped `fava-simple-groups` capability to read, project, discover, and publish one NIP-29 group across one or several host relays without losing relay-local authority or creating a second query/publication lifecycle.
**Mode:** mvp
**Requirements:** GROUP-01, GROUP-02, GROUP-03, GROUP-04, GROUP-05, GROUP-06, GROUP-07, GROUP-08, GROUP-09, GROUP-10, GROUP-11, GROUP-12
**Depends on:** Phase 07.1
**Success Criteria** (what must be TRUE):

  1. An application constructs one `Group` from a non-empty host set and opaque id; the same public helpers produce ordinary exact-`h` content queries, exact-`d` record queries, and kind-blind exact-host write intents for one or several hosts.
  2. A controlled two-relay fork appears as one event-id-deduplicated feed with exact serving-relay evidence while typed projections retain each host's independent records, expose disagreement, and never choose or field-merge a winner.
  3. Typed record/saved-row parsing and ordinary `Query`/`ValueSet` discovery cover metadata, admins, members, roles, participants, pins, saved groups, and saved relays without raw-tag work or global completeness claims.
  4. The crate README's public flow passes pure, facade, cancellation/close, bounds, deliberate-break, and two-relay wire evidence while the crate owns no engine lifecycle and universal owners contain no NIP-29 switch.

**Plans:** TBD

- [ ] Run GSD phase planning after Phase 07.1 inputs settle.

### Phase 8: Authentication, Hostile Boundaries, and Boundedness

**Goal:** Applications receive exact, isolated outcomes under relay authentication, malformed or hostile input, overload, provider failure, retry, ambiguity, and shutdown pressure.
**Mode:** mvp
**Depends on:** Phase 07.1.1
**Requirements:** HARD-01, HARD-02, HARD-03, HARD-04, HARD-05, HARD-06, HARD-07, HARD-08, HARD-09, HARD-10
**Success Criteria** (what must be TRUE):

  1. NIP-42 authentication is explicit and generation-scoped, remains separate from authorship/filter identity, and denial for one account terminates only its exact operation while another account continues.
  2. Malformed, oversized, off-filter, stale, post-CLOSED, never-EOSE, truncated, silent-limit, and disconnected relay behavior remains scoped and attributable; NIP-11 limits yield a valid plan or exact shortfall before knowingly invalid work.
  3. Offline time consumes no attempt budget, real retryable attempts reach configured give-up ceilings, and a completed handoff without outcome remains explicitly ambiguous rather than being rewritten.
  4. Every external input, queue, set, fan-out, history, diagnostic stream, and artifact is bounded, and a panicking, blocking, late, malformed, or cancellation-ignoring provider cannot block unrelated work or bounded shutdown; separate-process socket evidence publishes the resource/failure envelope.

**Plans:** TBD

### Phase 9: Truthful Profiles and Protocol Services

**Goal:** Applications can select persistent or ephemeral provider profiles and service caches whose restart, freshness, reset, and failure guarantees are explicit and truthful.
**Mode:** mvp
**Depends on:** Phase 8
**Requirements:** PROF-01, PROF-02, PROF-03, PROF-04, PROF-05, PROF-06, PROF-07, PROF-08, PROF-09
**Success Criteria** (what must be TRUE):

  1. The baseline cache contract implies no persistence, while selected persistent and ephemeral profiles exhibit exactly their declared cold-cache reuse, provenance, deletion/expiry/eviction, coverage, and restart behavior without global completeness claims.
  2. After restart, an ephemeral event cache contains no prior relay events while a selected durable write store recovers accepted writes; cache eviction revises open queries and cache-owned coverage coherently.
  3. NIP-05 and NIP-11 independently expose bounded validation, freshness, negative-cache, stale-result, and failure semantics even when an opaque FetchCache provider is physically shared.
  4. Each persistent provider validates and owns its schema/version/migration/corruption/refusal behavior, explicit reset affects exactly the selected profile state, and the same application source proves profiles by changing provider selection only.

**Plans:** TBD
**Open product decision:** The recommended persistent event-cache guarantee profile remains explicitly unpromised unless Phase 9 qualifies it through restart, corruption, coverage, resource, and application evidence.

### Phase 10: Provider Substitution Qualification

**Goal:** Applications can replace every major provider seam through public contracts and static assembly without core edits, privileged defaults, or unrelated behavior changes.
**Mode:** mvp
**Depends on:** Phase 9
**Requirements:** SUB-01, SUB-02, SUB-03, SUB-04, SUB-05, SUB-06, SUB-07, SUB-08
**Success Criteria** (what must be TRUE):

  1. Standard and materially different outside-workspace router, cache, durable store, planner, transport, publisher, delivery, signer, and fetch-cache implementations use the same public contracts, constructors, facade path, and unchanged conformance corpora.
  2. An application selects provider profiles by assembly/dependency changes only, and replacing one provider changes only its owned behavior without edits to universal core or unrelated providers.
  3. Provider panic/failure remains isolated and each provider owns its private persisted-format incompatibility rather than relying on a global assembly identity.
  4. The public provider matrix, dependency-negative tests, architecture falsifiers, profile matrix, and change-amplification audit pass before contracts are considered stable.

**Plans:** TBD

### Phase 11: Native Products and Release Qualification

**Goal:** Applications consume ordinary selected-profile Rust, Swift, Kotlin/JVM, Android, and iOS artifacts with equivalent lifecycle and evidence semantics in real platform processes.
**Mode:** mvp
**Depends on:** Phase 10
**Requirements:** NATIVE-01, NATIVE-02, NATIVE-03, NATIVE-04, NATIVE-05, NATIVE-06, NATIVE-07, NATIVE-08
**Success Criteria** (what must be TRUE):

  1. Rust, Swift, Kotlin/JVM, Android, and iOS applications consume declared release artifacts without repository-relative sources or raw generated bindings and see only providers/protocol capabilities selected by assembly.
  2. Native live-query open/current/next/cancel/close/terminal behavior and event evidence, route shortfall, receipts, errors, ambiguity, and restart outcomes match the shared Rust semantic corpus without flattening.
  3. Android fresh-process tests prove persistent-profile recovery, and any iOS suspension-transparency claim passes suspend/resume evidence on a physical device.
  4. Repeated native lifecycle cycles return tasks, handles, descriptors, Rust memory, and native heaps to declared envelopes, and the release candidate passes parity mutations, real-process evidence, two-relay interoperability, hostile/provider matrices, and release-build budgets.

**Plans:** TBD

## Progress

**Execution order:** Completed M0 baseline → Phase 1 (M1) → Phase 2 (M2) → Phase 3 (M3) → Phase 4 (M4) → Phase 5 (M5) → Phase 6 (M6) → Phase 06.1 (tag-filter remediation) → Phase 7 (M7) → Phase 07.1 → Phase 07.1.1 (`fava-simple-groups`) → Phase 8 (M8) → Phase 9 (M9) → Phase 10 (M10) → Phase 11 (M11)

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Deterministic Local Semantic State | pre-GSD | Complete | 2026-08-21 |
| 2. Exact Single-Relay Live Query | pre-GSD | Complete | 2026-08-21 |
| 3. Multi-Relay Reactivity and Bounded Observation | pre-GSD | Complete | 2026-08-21 |
| 4. Ordered Routing and Subscription Planning | pre-GSD | Complete | 2026-08-21 |
| 5. Durable Explicit-Route Publication | pre-GSD | Complete | 2026-08-21 |
| 6. Automatic Routing and Partial Delivery | pre-GSD | Complete | 2026-08-21 |
| 06.1. Literal Tag-Value Query Semantics Remediation | 3/3 | Complete    | 2026-08-21 |
| 7. Semantic Writes and Capability Composition | 9/9 | Complete | 2026-08-21 |
| 07.1. Universal Publication Vocabulary and Typed NIP-02 Reads | 11/12 | In Progress|  |
| 07.1.1. Multi-Relay Simple Groups | 0/TBD | Ready to plan after 07.1 | - |
| 8. Authentication, Hostile Boundaries, and Boundedness | 0/TBD | Not started | - |
| 9. Truthful Profiles and Protocol Services | 0/TBD | Not started | - |
| 10. Provider Substitution Qualification | 0/TBD | Not started | - |
| 11. Native Products and Release Qualification | 0/TBD | Not started | - |
