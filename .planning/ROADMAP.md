# Roadmap: Fava

## Overview

Fava advances from the completed M0 evidence baseline through authoritative milestones M1 through M11 plus focused inserted phases that repair or add required public capabilities before the next major milestone. M1-M7, the tag-value query slice, Phase 07.1, and the approved `fava-simple-groups` Phase 07.1.1 are complete; urgent Phase 07.2 restores the specified runtime signer lifecycle before M8. Each phase delivers one public-facade capability, retains its complete exit gates, and is complete only when every mapped requirement satisfies the project Definition of Done.

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
- [x] **Phase 07.1: Universal Publication Vocabulary and Typed NIP-02 Reads** - Complete the active follow-up slice selected before group capability work. (completed 2026-08-22)
- [x] **Phase 07.1.1: Multi-Relay Simple Groups** - Deliver `fava-simple-groups` from its README North Star with exact per-host truth and ordinary Fava lifecycles.
- [x] **Phase 07.2: Runtime Signer Lifecycle and Parked-Write Wakeup** - Attach, replace, and remove signers at runtime and wake only exact matching accepted writes without rebuilding Fava. (completed 2026-08-23)
- [ ] **Phase 07.3: Architecture Gate Integrity and Requirement Traceability** - Make the vocabulary, requirement, and verification gates truthful before measuring any remediation against them.
- [ ] **Phase 07.4: Neutral Contract Correction** - Reshape transport, subscription-planning, evidence, and diagnostics contracts so their specified owners can express the facts they own.
- [ ] **Phase 07.5: Create the fava-runtime Execution Owner** - Build the named-but-absent execution owner: tasks, deadlines, isolation, cancellation, and shutdown joins.
- [ ] **Phase 07.6: Restore fava-observe Live-Query Ownership** - Move the live-query lifecycle to its specified owner and delete the facade relay layer outright.
- [ ] **Phase 07.7: Facade Lifecycle and fava-session Signer Ownership** - Give the facade a real lifecycle and the account/signer set its specified owner.
- [ ] **Phase 07.8: Independent Correctness Defects** - Fix the confirmed defects that are not consequences of the ownership inversion.
- [ ] **Phase 07.9: Evidence Reconstruction and Milestone Verdict Revocation** - Prove public promises through the real path and withdraw the verdicts that rest on evidence which cannot.
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
**Plans:** 12/12 plans complete

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

- [x] 07.1-12-PLAN.md — Run the controlled Croissant canary twice and close all phase gates

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

**Plans:** 12/12 plans complete

- [x] 07.1.1-01-PLAN.md
- [x] 07.1.1-02-PLAN.md
- [x] 07.1.1-03-PLAN.md
- [x] 07.1.1-04-PLAN.md
- [x] 07.1.1-05-PLAN.md
- [x] 07.1.1-06-PLAN.md
- [x] 07.1.1-07-PLAN.md
- [x] 07.1.1-08-PLAN.md
- [x] 07.1.1-09-PLAN.md
- [x] 07.1.1-10-PLAN.md
- [x] 07.1.1-11-PLAN.md
- [x] 07.1.1-12-PLAN.md

### Phase 07.2: Runtime Signer Lifecycle and Parked-Write Wakeup (INSERTED)

**Goal:** As a Fava application developer, I want to add, explicitly replace, and remove bounded signers on a running Fava instance, so that exact matching accepted writes resume without rebuilding the engine or losing durable write identity.
**Mode:** mvp
**Requirements:** SESSION-01, SESSION-02, SESSION-03, SESSION-04, SESSION-05, SESSION-06, SESSION-07
**Depends on:** Phase 07.1.1
**Success Criteria** (what must be TRUE):

  1. An application adds a signer after Fava is built and an already accepted write awaiting that exact pubkey signs and continues under the same write and receipt identity.
  2. Duplicate add refuses without mutation, replacement is explicit, removal preserves accepted writes, and re-add wakes only work for the exact restored pubkey.
  3. A signer completion released after replacement or removal is attributable but inert by exact operation and materialization generation.
  4. Signer registration is bounded with typed refusal, and signer provider execution occurs outside session/publication locks and store transactions.

**Plans:** TBD
**Wave 1**

- [x] 07.2-01-PLAN.md

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 07.2-02-PLAN.md
### Phase 07.3: Architecture Gate Integrity and Requirement Traceability (INSERTED)

**Goal:** The gates that failed to detect the M2-M4 ownership deviation become truthful instruments, and every authoritative spec requirement is traceable to a mapped requirement before any remediation is measured against it.
**Mode:** remediation
**Depends on:** Phase 07.1.1
**Requirements:** Authored by this phase. Adds ownership-ledger requirements (proposed `OWN-01`..`OWN-08` in `.planning/audit/2026-08-23/requirements-process.md`) and restores the 113 authoritative spec IDs currently absent from `.planning/`.
**Success Criteria** (what must be TRUE):

  1. `tools/check_vocabulary.py` reports exactly the policy-covered declarations AGENTS.md defines, at every visibility, with no false positive and no silencing heuristic; it never treats `.planning/**` prose as vocabulary authority; it verifies each `spec_crates`/`spec_symbols` entry against reality so a missing owner crate is a failure rather than silence.
  2. Every authoritative requirement ID in `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md` maps to a `.planning/REQUIREMENTS.md` entry that preserves its full conjunction, and a mechanical check fails when a mapped requirement weakens or splits its authority.
  3. Every row of the ownership ledger in `docs/spec/ARCHITECTURE.md` is represented as a verifiable requirement with a named falsifier, so an owner moving is a test failure rather than a review opinion.
  4. The nine unapproved lifecycle owners named in the audit, `fava::OpenedRelay` included, are recorded as violations scheduled for deletion; none is approved into `vocabulary.toml` to make the gate pass.
  5. CI runs the workspace test suite, clippy, the falsifier corpus, and the canary on every change. Today `.github/workflows/architecture.yml` runs two Python steps and nothing else: no `cargo test` has ever run automatically in this repository, which is why 306 green tests coexisted with a systemic ownership inversion for six milestones.
  6. The `Red:` / `Mutation:` evidence record required by `FAVA_TDD_BDD_TESTING_GUIDE.md` §16 is enforced mechanically. It is currently present in zero of 510 commits, and 36 of 41 named deliberate breaks have never been executed.

**Plans:** Not yet planned - run `/gsd-plan-phase 07.3`.
**Source:** `.planning/audit/2026-08-23/LEDGER.md` Wave 0; `vocabulary.md`, `requirements-process.md`.

### Phase 07.4: Neutral Contract Correction (INSERTED)

**Goal:** The neutral contracts can express the facts the architecture assigns to their owners, so live-query ownership becomes buildable at all.
**Mode:** remediation
**Depends on:** Phase 07.3
**Requirements:** RELAY-003, QUERY-005, QUERY-012, OPS-003, plus the ownership requirements authored in 07.3. Full mapping produced during planning.
**Success Criteria** (what must be TRUE):

  1. `Transport`/`RelaySession` expose per-consumer message streams and acquire-with-refcount session semantics, so two observations can share one physical session without stealing each other's frames; inbound and outbound byte queues are bounded and every handoff outcome carries session key and generation.
  2. `SubscriptionPlanner` receives the complete current logical demand assigned to a relay together with that relay's declared constraints, and returns a desired-plan diff carrying withdrawal identity and typed shortfall; no conformance rule the planner must satisfy lives outside the contract crate.
  3. `QueryEvidence` can name per-relay EOSE, failure, authentication, CLOSED, route state, desired-plan revision, subscription shortfall, shared-work ownership, provider-operation generation, and coalescing loss; empty-with-EOSE is distinguishable from an unreachable relay through the public API.
  4. A live admitted relay event reaches an open query without requiring a retaining event cache, and the diagnostics surface can express the open observation ownership graph.
  5. Every reshaped contract ships a conformance testkit that a competing implementation can run; no adapter, shim, or compatibility path exists for the previous shapes.

**Plans:** Not yet planned - run `/gsd-plan-phase 07.4`.
**Source:** `.planning/audit/2026-08-23/LEDGER.md` Wave 1.

### Phase 07.5: Create the fava-runtime Execution Owner (INSERTED)

**Goal:** Asynchronous execution, provider isolation, deadlines, cancellation, and shutdown joins have the single owner the architecture already names and vocabulary already approves.
**Mode:** remediation
**Depends on:** Phase 07.4
**Requirements:** OPS-009, QUERY-003, WRITE-007, plus the isolation and boundedness requirements restored in 07.3.
**Success Criteria** (what must be TRUE):

  1. `fava-runtime` exists as a crate and owns task execution with a join registry, timers, bounded command and completion channels, provider-operation identity, cancellation propagation, and shutdown deadlines.
  2. Every provider call in the workspace runs under a Fava-owned deadline and returns a typed completion; a stalled or panicking provider is scoped, attributable, and cannot block unrelated owner progress or shutdown. A substituted provider is bounded by the same policy as the default one.
  3. No detached task remains: every spawned task is owned, joinable, cancellable, and joined at shutdown within its declared deadline.
  4. Reconnect is a bounded policy with growth, ceiling, jitter, and an attempt bound that terminates in a typed, application-visible shortfall; N observations against one unreachable relay produce one reconnect lifecycle, not N.

**Plans:** Not yet planned - run `/gsd-plan-phase 07.5`.
**Source:** `.planning/audit/2026-08-23/LEDGER.md` Wave 2; `missing-owners.md`.

### Phase 07.6: Restore fava-observe Live-Query Ownership (INSERTED)

**Goal:** The observation owner owns the live query, and the facade owns none of it. This is the phase that closes the reported crisis.
**Mode:** remediation
**Depends on:** Phase 07.5
**Requirements:** QUERY-002, QUERY-004, QUERY-011, QUERY-014, READ-02, RELAY-003, and the ownership requirements authored in 07.3.
**Success Criteria** (what must be TRUE):

  1. `Fava::observe` returns a coherent local observation without awaiting any relay, router, or transport future. A transport whose establishment never resolves cannot delay the handle, and a refusing router cannot deny the application its local view.
  2. `fava-observe` owns observation identity, the observation registry, logical per-relay demand, the desired subscription plan and its diff, shared-work identity and refcount, relay-session binding, provider-operation generation with late-completion rejection, the route session, and cancellation.
  3. Equivalent observations share one demand, one session, and one `REQ`; withdrawal is refcounted and the last handle to close sends exactly the `CLOSE`s that lost their final holder. Dropping a handle while another relay is still establishing closes the already-open session exactly.
  4. `crates/fava/src/relay.rs`, `OpenedRelay`, the relay coordination in `live.rs` and `routes.rs`, and `Fava::next_subscription` no longer exist. No replacement adapter, wrapper, or compatibility path is introduced in the facade.
  5. `crates/fava-observe/` carries owner-level evidence for every fact it owns.

**Plans:** Not yet planned - run `/gsd-plan-phase 07.6`.
**Source:** `.planning/audit/2026-08-23/LEDGER.md` Wave 3; `observe-facade.md`, `REMEDIATION-CORE.md`, `.planning/debug/observe-ownership-collapse.md`.

### Phase 07.7: Facade Lifecycle and fava-session Signer Ownership (INSERTED)

**Goal:** The facade owns exactly what the architecture assigns it - instance identity, command admission, and startup/shutdown ordering - and the account and signer set has its specified owner.
**Mode:** remediation
**Depends on:** Phase 07.6
**Requirements:** QUERY-003, OPS-009, WRITE-008, WRITE-023, plus the session requirements Phase 07.2 was authored against.
**Success Criteria** (what must be TRUE):

  1. `Fava` has explicit lifecycle state and a deterministic `close` that stops new commands, then closes observations, publications, routers, transports, and stores in the specified order and joins owned resources. Shutdown is distinguishable from source failure through the public API.
  2. DELIVERED BY PHASE 07.2 (merged as `0b23b52`): `fava-session` exists and owns the account set, signer registrations, and attachment generations, with runtime attach/replace/remove and parked-write wakeup on availability transition. This phase VERIFIES that delivery against the audit findings rather than rebuilding it, and closes anything 07.2 left open.
  3. No public door mutates a publication lifecycle without passing through its owner - `cancel_write` included. `Fava::cancel_write` currently calls raw `WriteStore::cancel`, skipping `Publication::cancel`'s eligibility decision and leaving in-flight signer and delivery work running.
  4. Signer provider calls run under a Fava-owned deadline with a typed timed-out outcome, and a stale signer completion is rejected observably rather than discarded with `let _ =`.

**Plans:** Not yet planned - run `/gsd-plan-phase 07.7`.
**Source:** `.planning/audit/2026-08-23/LEDGER.md` Wave 4; `identity-protocols.md`, `missing-owners.md`.

### Phase 07.8: Independent Correctness Defects (INSERTED)

**Goal:** The confirmed correctness defects that are not consequences of the ownership inversion are fixed, so they do not survive the remediation unnoticed.
**Mode:** remediation
**Depends on:** Phase 07.4
**Requirements:** WRITE-014, WRITE-015, WRITE-027, WRITE-028, ROUTER-001, QUERY-004, QUERY-009, RELAY-001. Mapping produced during planning 2026-08-23: WRITE-014/WRITE-027 added (router acquisition with no separate transport stack; settled-empty routing), EVENT-014/RELAY-012/WRITE-018/WRITE-019 removed (verified already satisfied on `87c3688`, or evidence-only and owned by Phase 07.9 / Phase 8), RELAY-004 (NIP-11 declared limits) split out — see Deferred below.
**Success Criteria** (what must be TRUE):

  1. Relay-issued attribution is checked against the accepted subscription that actually carries the filter; a relay cannot choose which accepted filter validates its event, and `WrongSubscription` is reachable through the real path.
  2. A router refusal, closure, or panic produces typed shortfall scoped to that router; it never denies the application its local view, never aborts the chain, and never tears down unchanged relay demand. A settled-absent routing fact can only be produced by settled absence, never by a query failure.
  3. A bounded event cache at capacity can still apply a deletion, and expiry is swept by a named production owner rather than by tests alone.
  4. A purely local write cannot empty a relay-qualified query. An authentication outcome reaches the replaceable delivery policy and is reported truthfully with respect to whether bytes left Fava. Relay-declared limits come from the relay, not from invented constants.

**Plans:** 5 plans

Plans:
- [x] 07.8-01-PLAN.md — Settled absence requires an answer: a failed router no longer fabricates `SettledAbsent`; resolves the LEDGER's WRITE-027 cross-owner question
- [ ] 07.8-02-PLAN.md — The router acquisition contract and its first real implementation on `Observer` (blocked on a vocabulary decision checkpoint)
- [ ] 07.8-03-PLAN.md — `Router`/`RouterSession` signature change and all four `fava_routing::open`/`preview` call sites wired to real handles
- [ ] 07.8-04-PLAN.md — Outbox reads the warm cache before asking a relay; settled absence from indexer evidence; ROUTER-001 re-specified at the wire
- [ ] 07.8-05-PLAN.md — Delete `impl QuerySource for Fava`, the canary's second engine, and `ARCHITECTURE.md`'s two invented query services

**Deferred out of this phase** (planning judgement 2026-08-23, awaiting Pablo):
- `no-nip11-invented-planner-limits` (RELAY-004) — relay-declared constraints have no producer; needs a NIP-11 acquisition capability the transport contract does not have, plus its own vocabulary approval. Success criterion 4's last sentence moves with it.
- `expiry-is-never-swept` — `EventCache::expire` still has no production caller; needs a named sweep owner and a cadence. Success criterion 3's second half moves with it. The first half (deletion at capacity) is already satisfied.

**Source:** `.planning/audit/2026-08-23/LEDGER.md` Wave 5; `transport-wire-ingest.md`, `routing.md`, `query-state-cache.md`, `publication-write.md`; `07.8-CONTEXT.md` (Pablo's router-acquisition ruling); `RESEARCH.md`.

### Phase 07.9: Evidence Reconstruction and Milestone Verdict Revocation (INSERTED)

**Goal:** Every public promise is proved through the assembled public path by evidence that can distinguish the specified architecture from the one that was built, and the milestone verdicts that rest on evidence which cannot are withdrawn.
**Mode:** remediation
**Depends on:** Phase 07.6, Phase 07.8
**Requirements:** GOAL-009, SUB-08, OPS-003, and the full restored requirement corpus from 07.3.
**Success Criteria** (what must be TRUE):

  1. No evidence for a public promise constructs internals, calls a provider directly, or drives a second engine to observe the first. The grouping-equivalence, outbox-acquisition, and group-read acceptances run through one assembled `Fava` on the real path.
  2. Every provider test double can express pending, mid-operation failure, mid-operation cancellation, stale completion, and slow-peer backpressure; conformance testkits exist for transport, router, subscription planner, and signer.
  3. Each of the six architecture gates has at least one falsifier that fails against the pre-remediation tree and passes after it, and each named deliberate break is one that could plausibly ship.
  4. The M1, M2, M3, M5, M6, and Phase 07.1.1 verdicts are revoked with recorded reasons and re-earned against the restored requirement corpus, or left open. M4 is downgraded. Phase 7 is retained. No verdict rests on evidence authored by the change it verifies.

**Plans:** Not yet planned - run `/gsd-plan-phase 07.9`.
**Source:** `.planning/audit/2026-08-23/LEDGER.md` Wave 6; `evidence.md`, `requirements-process.md`, `public-surface.md`.

### Phase 8: Authentication, Hostile Boundaries, and Boundedness

**Goal:** Applications receive exact, isolated outcomes under relay authentication, malformed or hostile input, overload, provider failure, retry, ambiguity, and shutdown pressure.
**Mode:** mvp
**Depends on:** Phase 07.2
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

**Execution order:** Completed M0 baseline → Phase 1 (M1) → Phase 2 (M2) → Phase 3 (M3) → Phase 4 (M4) → Phase 5 (M5) → Phase 6 (M6) → Phase 06.1 (tag-filter remediation) → Phase 7 (M7) → Phase 07.1 → Phase 07.1.1 (`fava-simple-groups`) → Phase 07.2 (runtime signer lifecycle) → Phase 8 (M8) → Phase 9 (M9) → Phase 10 (M10) → Phase 11 (M11)

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
| 07.1. Universal Publication Vocabulary and Typed NIP-02 Reads | 12/12 | Complete    | 2026-08-22 |
| 07.1.1. Multi-Relay Simple Groups | 12/12 | Complete | 2026-08-22 |
| 8. Authentication, Hostile Boundaries, and Boundedness | 0/TBD | Not started | - |
| 9. Truthful Profiles and Protocol Services | 0/TBD | Not started | - |
| 10. Provider Substitution Qualification | 0/TBD | Not started | - |
| 11. Native Products and Release Qualification | 0/TBD | Not started | - |
