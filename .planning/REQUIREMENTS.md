# Requirements: Fava

**Defined:** 2026-08-21 (code-derived — see below)
**Rebuilt spec-derived:** 2026-08-23
**Core Value:** Applications can rely on coherent live queries and durable writes with exact, bounded, failure-isolated lifecycle and evidence semantics across replaceable provider compositions.

---

## Provenance notice — read this before trusting any line in this file

**As of 2026-08-23 this requirement corpus is spec-derived. Until 2026-08-23 it was code-derived.
That difference is the reason a systemic ownership inversion passed six milestone reviews.**

The previous version of this document was first committed at `277d839` on **2026-08-21 07:44:48
+0300**. M6 completed at `309e421` on **2026-08-21 04:03:09 +0300**. All 66 of the
LOCAL/READ/ROUTE/WRITE requirements that M1–M6 were graded against were written **3 hours and 41
minutes after the last of those milestones shipped**, from the implementation commits they were then
used to grade. Every one of them was born with its box already checked. A requirement written from
working code cannot fail; it is a description, not a specification.

Three consequences, all confirmed by the 2026-08-23 audit (`.planning/audit/2026-08-23/requirements-process.md`):

1. **113 of the 131 authoritative spec requirement IDs appeared nowhere in `.planning/`.** There was
   no traceability edge from `QUERY-004` to `LOCAL-08`, so no reviewer could mechanically ask whether
   every clause of a spec requirement survived. Six spec invariants had been split across requirement
   IDs such that the conjunction became untestable, and nobody owned the conjunction.
2. **`LOCAL-08` narrowed `QUERY-004` from "a query" to "a *local* query" and parked it in M1** — the
   one milestone whose own exit gate forbids any networking dependency, and therefore the one
   milestone structurally incapable of falsifying it. `crates/fava/src/live.rs:32-53` serially awaits
   `OpenedRelay::open(...)` per relay before returning the handle, and no requirement objected.
3. **No requirement anywhere named the ownership ledger.** All sixteen architecture falsifiers were
   collapsed into a single requirement (`SUB-08`) and deferred to Phase 10, so no milestone gate
   between M1 and M7 ever ran an ownership audit.

What changed on 2026-08-23:

- The **authority is now `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md`**, not the
  repository. Every one of its 131 numbered requirements has a row in the
  [Spec traceability matrix](#spec-traceability-matrix) below, mapped or explicitly recorded as
  unmapped. Where a `.planning` requirement weakens its spec counterpart, both wordings are quoted
  and the row is marked `WEAKENED`.
- The six lost conjunctions named by the audit are **restored into the requirement text itself**, each
  with a named falsifier.
- An `OWN-01`…`OWN-08` family now carries the `ARCHITECTURE.md` Part IX ownership ledger, modelled on
  `SESSION-07` — the only correctly-formed ownership requirement the old corpus contained.
- **Checkmarks were reset.** Any requirement whose only evidence the audit found to be self-authored
  (the cited `docs/issues/000N` record's first and only commit *is* the implementation commit),
  bypassing (the proof runs in a regime where the property cannot fail), or non-distinguishing (the
  assertion cannot tell a correct implementation from the current one) is now unchecked with a
  one-line reason in the [Checkmark reset ledger](#checkmark-reset-ledger). No checkmark was preserved
  that could not be defended.

**A checked box in this file now means: an executable proof exists that failed first, runs through a
public contract, and does not consume a fact supplied by its own fixture.** It does not mean a
verification document said so.

---

## Validated Baseline

M0 was the independent-witness milestone. Its five claims are **all unchecked as of 2026-08-23**: the
evidence audit established that the downstream acceptance application's `runs/` directory was line 3
of `.gitignore`, held zero tracked bundles, and that the application's own README declared it must
not depend on Fava internal crates while it linked nine of them and used them to *be* the client
engine. That application has since been removed from the repository entirely (2026-08-31); M0 remains
unchecked and there is now no independent-witness application to re-establish it.

- [ ] **M0-01**: An ordinary downstream acceptance application can publish and query a genuinely signed event through real WebSocket frames against a third-party relay process.
  *Reset: the run bundle proving it is git-ignored and absent from disk; the claim is unreconstructable from a fresh checkout.*
- [ ] **M0-02**: The acceptance application can hard-kill and restart the relay against the same data directory and independently prove the event remains queryable.
  *Reset: same — no retained evidence, and no automated process has ever executed the scenario.*
- [ ] **M0-03**: Every M0 assertion is reconstructable from bounded process, wire, manifest, report, and JSONL evidence under one run identity.
  *Reset: falsified directly — listing the acceptance application's run directory returned only `phase-07.1.1-pair.*`; every M0-M6 bundle was absent.*
- [ ] **M0-04**: Enabled evidence scenarios fail on unavailable prerequisites or scenario errors and never silently skip.
  *Reset: 18 scenarios were marked `built` but were CLI-only and unreachable from the workspace test runner; nothing ran them.*
- [ ] **M0-05**: The acceptance application proves M0 without depending on Fava implementation crates.
  *Reset: falsified — the application linked nine Fava internal crates and hard-coded `result_equivalence: true` into its retained manifest.*

M0 is a prerequisite baseline, not an active roadmap phase. Its requirements are unchecked, not
deleted: re-establishing an independent witness — currently absent from the repository — is a
precondition for any later verdict, because Fava cannot be its own witness for wire, process, relay,
storage, or native facts.

---

## v1 Requirements

Requirements for the Fava release. Every requirement is normative. Each is derived from a numbered
requirement in `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md`, and its spec basis appears
in the Spec traceability matrix below. A requirement with no spec basis is a defect in this file, not
a feature.

### Local Semantic State

- [ ] **LOCAL-01**: Applications observe deterministic event identity, replaceable/addressable winner selection, deletion, expiry, ordering, and evidence merge semantics.
- [ ] **LOCAL-02**: An event-cache provider accepts only admitted signed relay events and cannot retain unpublished local revisions.
- [ ] **LOCAL-03**: A write-store provider exposes current local unsigned and signed revisions as an independent query source.
- [ ] **LOCAL-04**: Contributions for the same event from event cache and write store merge into one `EventRecord` with source-specific evidence.
- [ ] **LOCAL-05**: A query-matching pending local replaceable event can shadow a matching cached predecessor without mutating or deleting that predecessor in the event cache; candidates are filtered before coordinate selection.
- [ ] **LOCAL-06**: Cancelling a local write retracts only its write-store contribution and naturally reveals any still-qualified cached predecessor.
- [ ] **LOCAL-07**: Removal, deletion, expiry, or eviction of a source contribution revises every affected open query without a parallel removal API, including deletion applied across sources and expiry that becomes due while the query is open, and only for targets the deleting author is permitted to delete.
- [ ] **LOCAL-08** *(rewritten 2026-08-23 - restores QUERY-004; reassigned Phase 1 to Phase 2)*: Opening **any** query - cache-only or live - is all-or-nothing and returns one complete current snapshot produced from the configured local query sources **without waiting for any relay response, connection attempt, session establishment, or transport handshake**. With every relay unreachable, open returns the local view or a local-source error and never hangs. Engine-shutdown refusal and local-source failure remain distinguishable.
  **Falsifier (named):** `live_open_returns_local_view_while_relay_establishment_is_pending` - build an assembly whose `Transport::open_session` never resolves, seed the cache with one event, then open a live query under a 200ms timeout. The handle must return with one event in `current()`. *Fails today at the timeout*: the facade builds the local observation, then serially awaits per-relay open before returning the handle.
  **Why it moved:** M1's own exit gate forbids any relay, transport, or runtime networking dependency in M1 crates. Parking this requirement in M1 assigned it to the one milestone that cannot falsify it. It is now owned by Phase 2, where a transport exists to be made pending.
- [ ] **LOCAL-09** *(rewritten 2026-08-23 - restores QUERY-002; reassigned Phase 1 to Phase 3)*: Equivalent query descriptions - including access context, acquisition scope, and result authority - have stable semantic identity regardless of construction order, **and equivalent observations share one local evaluation, one relay session, and one wire subscription**, **and closing one observation releases only its own reference and does not close work another observation still needs**. Distinct source authority, relay access, freshness, or presentation-relevant evidence is never erased merely because event filters are equal.
  **Falsifier (named):** `two_equivalent_live_handles_share_one_relay_session_and_one_req` - open the same live query twice against a counting transport; assert one session opened and one REQ sent; drop one handle and assert zero closes. *Fails today on the first assertion*: the facade allocates a fresh relay session key and a fresh opened relay per observation.
  **Why it moved:** the sharing and close-safety halves of QUERY-002 are relay properties. They cannot exist in M1. Only the identity half survived the original split, and it survived precisely because it was the half M1 could satisfy.
- [ ] **LOCAL-10**: Current-state delivery is bounded, may coalesce intermediate states, and always rebases the consumer onto one exact latest result.
- [ ] **LOCAL-11**: Applications can inspect cache and write-store evidence through public event records without seeing provider storage internals.
- [ ] **LOCAL-12**: The same semantic corpus passes through memory event-cache and write-store providers and the public Fava facade without relay, transport, or runtime networking dependencies.
  *Note: this is an M1 build constraint promoted into the behavioral set. It adds no falsifiable application behavior and inflates the coverage count. Retained pending an owner decision.*

### Live Relay Queries

- [ ] **READ-01**: Applications can open a live query against an exact, non-empty explicit relay list without invoking automatic routers.
- [ ] **READ-02** *(rewritten 2026-08-23 - restores QUERY-013)*: Opening a live-freshness query **contributes relay demand immediately** - relay work is never deferred until the application first iterates or collects - **and** cache-only queries contribute **no** relay work, **and** reiterating an already-open handle creates **no** second underlying query, session, or wire subscription.
  **Falsifier (named):** the LOCAL-08 falsifier additionally asserts exactly one transport open attempt was made, proving demand started while the local view returned. Plus `cache_only_query_opens_zero_sessions` and `reiterating_a_handle_creates_no_second_req`. *The conjunction fails today: relay work does start, but the local view does not return until it finishes.*
- [ ] **READ-03**: Product transport sends and receives exact bounded NIP-01 wire messages over real sockets, with the configured inbound frame bound enforced on receive as well as on send.
- [ ] **READ-04**: Every inbound event is attributed to an accepted relay session, request, generation, access context, and subscription before admission.
- [ ] **READ-05**: Invalid-id, invalid-signature, malformed, off-filter, stale-generation, and post-terminal events cannot affect cache state or query results, and no consuming application or provider can commit query-visible signed state with fabricated relay evidence through the cache mutation contract.
- [ ] **READ-06**: EOSE evidence exists only after the actual relay frame and remains scoped to the exact relay request and generation.
- [ ] **READ-07**: Applications can distinguish empty-plus-EOSE, silence, failure, authentication required, NOTICE, CLOSED, timeout, cancellation, and shortfall.
- [ ] **READ-08**: A query remains live after EOSE and delivers later matching events without application resubscription.
- [ ] **READ-09**: Query cancellation performs exact withdrawal, wakes pending pulls, and prevents later application delivery for the cancelled generation.
- [ ] **READ-10**: Query close is idempotent and deterministically releases owned relay, task, queue, and subscription resources.
- [ ] **READ-11**: The same event served by several relays appears once with evidence for every relay that actually served it.
- [ ] **READ-12**: A relay that was planned or contacted but did not serve an event is never credited as provenance for that event.
- [ ] **READ-13**: Reconnect restores active demand under fresh session and request generation identity without application resubscription.
- [ ] **READ-14**: Reconnect does not imply that events missed during an outage were recovered or that history is complete.
- [ ] **READ-15**: Source- or provenance-only changes revise an existing event record without duplicating the event.
- [ ] **READ-16**: Slow current-state consumers receive an exact bounded latest result with truthful coalescing and loss diagnostics.
- [ ] **READ-17**: Causal receipt and lifecycle facts use loss-honest delivery separate from coalescible current-state snapshots.
- [ ] **READ-18**: Repeated cancellation and retry of pending pulls cannot accumulate an update backlog or retain stale waiters.
- [ ] **READ-19**: Public diagnostics identify query, relay session, access context, request generation, logical demand, wire subscription, terminal reason, and source counts without private inspection, and never synthesize a global sync score, completeness percentage, or invented root-cause fact.
- [ ] **READ-20** *(rewritten 2026-08-23)*: A **declared standard profile exists as a shipped crate**, and it keeps at least 1,000 simultaneous **live** idle observations within explicit **task, memory, descriptor, and queue** bounds, each measured as the named resource.
  **Falsifier (named):** `one_thousand_idle_live_observations_share_bounded_sessions_and_descriptors` - open 1,000 live observations of the same query against a counting transport; assert one open session, open descriptors at or below the declared descriptor bound, and a measured allocation delta under the declared memory bound. *Fails today: 1,000 sessions.*
  **Why it was reset:** its sole prior evidence, `crates/fava/tests/observation_bounds.rs:27`, asserts thread identity and nothing else - three of the four named bounds are unmeasured - and all 1,000 observations are cache-only, so the milestone titled *Multi-Relay Reactivity and Bounded Observation* was gated by a test with no relay in it. Its subject, the declared standard profile, does not exist: `fava-standard` is absent from `crates/` and is listed under `spec_crates` in `docs/internals/vocabulary.toml:270`. A requirement whose subject does not exist cannot be satisfied.
- [ ] **READ-21** *(new 2026-08-23 - restores QUERY-003 for the relay path)*: A failure at **any** point during open of a **live** query returns a typed refusal and leaves **no** relay session, wire subscription, ownerless demand, partial dependency, or router acquisition behind; existing open queries are unchanged.
  **Falsifier (named):** `failed_live_open_leaves_no_session_and_no_subscription` - inject failure at each stage of open (transport connect, session accept, planner admit, subscription send) against a counting transport; after each, assert the error is typed, zero sessions remain open, zero REQs were sent, and a previously-open observation still delivers.
  **Why it is new:** LOCAL-08 kept only the phrase "all-or-nothing" and scoped it to local queries; READ-09 covers cancellation and READ-10 covers close. **No requirement covered failure during open of the relay path.** Partial-open leak is a confirmed baseline finding.
- [ ] **READ-22** *(new 2026-08-23 - restores the four dropped conjuncts of QUERY-012)*: For a pull-based observation surface: a **second concurrent pull is refused without consuming data**; **an update delivered once is never delivered again**; **invalid acknowledge/cancel/close ordering is refused**; and **engine shutdown ends all pending pulls without hanging**. These hold in addition to READ-09 (wake-on-cancel, no post-cancel delivery), READ-10 (idempotent close), and READ-18 (no waiter backlog).
  **Falsifier (named):** `concurrent_next_is_refused_without_consuming`, `delivered_update_is_never_redelivered`, `invalid_ack_cancel_close_order_is_refused`, `shutdown_ends_all_pending_pulls`. *All four fail to compile or fail today*: the surface is a broadcast/watch channel at `crates/fava-observe/src/lib.rs:191` with no pull protocol, no acknowledge, and no engine shutdown.
  **Why it is new:** QUERY-012 states eight invariants. Splitting it across READ-09, READ-10, and READ-18 carried three and silently dropped four.
- [ ] **READ-23** *(new 2026-08-23 - restores the memory conjunct of QUERY-011)*: **Observation memory remains bounded, measured as memory, when the application is slow.** A bound on the delivered *result value* (READ-16) is not a bound on *observation memory*.
  **Falsifier (named):** `slow_consumer_under_burst_keeps_observation_memory_bounded` - drive N x burst updates into an observation whose consumer never polls, sample the bytes retained by the observation, and assert the retained figure is flat in N and under the declared bound. *No such measurement exists anywhere in the corpus today.*

### Ownership Ledger

Authored 2026-08-23. Every row of `docs/spec/ARCHITECTURE.md` Part IX ("Single-owner map"; the ledger
table sits at `ARCHITECTURE.md:2905-2934` in this checkout, cited as `2961-2995` in the audit's
snapshot) that the confirmed deviation violates now has a requirement naming its **one** owner, in the
form of SESSION-07 - the only correctly-formed ownership requirement the old corpus contained.
`GOALS.md:186`: *"an ownership ledger can name exactly one owner for every stateful concept. Any
duplicate owner is treated as an architecture defect."* Each falsifier below is executable, so **an
owner moving is a test failure, not a review opinion.**

- [ ] **OWN-01** *(ledger rows: Open live-query handle; Current merged query snapshot; Reactive dependency node - `ARCHITECTURE.md:2915,2916,2917`)*: `fava-observe` **exclusively** owns observation identity, the open live-query handle, the current merged query snapshot, and reactive dependency nodes. The `fava` facade orders construction and shutdown between owners and **retains no observation state**.
  **Falsifier (named):** `facade_retains_no_observation_state` - assert the facade source declares no field or type whose value is an observation handle, merged snapshot, or dependency node. Companion: `observation_handle_type_is_declared_and_produced_only_by_fava_observe`, which today fails on `ObserveError::Relay(String)` - declared by `fava-observe` and produced only by the facade, at nine sites.
  **Phase:** 1 (local half), extended at Phase 2 (relay half).
- [ ] **OWN-02** *(ledger rows: Query demand for one relay; Wire subscription plan - `ARCHITECTURE.md:2921,2922`)*: `fava-observe` **exclusively** owns retained logical query demand per relay session and the **desired** wire-subscription plan. The selected planner **computes** the plan and owns **none** of it; the transport executes it and owns none of it.
  **Falsifier (named):** `only_fava_observe_retains_query_demand_and_desired_plan` - for every crate other than `fava-observe`, assert no struct field holds a demand set or a desired subscription plan across an await point. *Fails today: the desired plan is computed and retained inside `crates/fava/src/relay.rs`, which is why substituting a planner touches the facade and breaks M4's fourth exit gate.*
  **Phase:** 2.
- [ ] **OWN-03** *(ledger row: Relay connection generation - `ARCHITECTURE.md:2923`)*: The selected `Transport` **exclusively** owns relay connection establishment, physical session identity and generation, reconnect, backoff, and close. **No other crate retains session state or runs a reconnect loop.**
  **Falsifier (named):** `facade_retains_no_relay_session_or_reconnect_state`:
  ```rust
  #[test]
  fn facade_retains_no_relay_session_or_subscription_state() {
      let src = include_str!("../src/lib.rs");
      for forbidden in ["subscription_planner", "transport", "next_subscription", "OpenedRelay"] {
          assert!(!src.contains(forbidden), "fava facade must not own {forbidden}");
      }
  }
  ```
  *Fails today: all four are `Fava` fields or facade types, and `crates/fava/src/relay.rs` additionally retains planner, cache, diagnostics, reconnect, and ingest-dispatch state the ledger assigns elsewhere.*
  **Phase:** 2.
- [ ] **OWN-04** *(ledger rows: Open live-query handle and Query demand for one relay, shared case; `GOALS.md:296,298`)*: Equivalent observations share **one** relay session, **one** wire subscription, and **one** refcounted work item; closing one observation releases **only** its reference, and the shared item is torn down only when the last reference drops.
  **Falsifier (named):** `two_equivalent_live_handles_share_one_relay_session_and_one_req` (shared with LOCAL-09), extended by `dropping_the_last_of_n_equivalent_handles_closes_exactly_once`. *Fails today on the first assertion.*
  **Phase:** 3.
- [ ] **OWN-05** *(ledger row: Execution resources and joins - `ARCHITECTURE.md:2933`)*: `fava-runtime` **exclusively** owns execution resources, task spawning and joins, cancellation propagation, and shutdown barriers. Every provider call executes through it carrying operation and generation identity, so a late completion can be dropped as stale, and no provider call runs while holding another subsystem's lock or transaction.
  **Falsifier (named):** `no_crate_outside_fava_runtime_spawns_a_task` - assert `tokio::spawn`, `spawn_blocking`, and `std::thread::spawn` appear in no crate other than `fava-runtime`; plus `blocked_provider_does_not_delay_unrelated_shutdown` and `panicking_provider_leaves_unrelated_queries_running`. *Fails today on the first assertion, and `ls crates/` confirms `fava-runtime` does not exist.*
  **Phase:** 3, hardened at Phase 8.
- [ ] **OWN-06** *(ledger row: Event-id/signature admission - `ARCHITECTURE.md:2907`)*: `fava-ingest` **exclusively** owns wire attribution, id and signature verification, and admission ordering. **No other crate may commit a cache mutation derived from relay bytes**, and the cache mutation contract refuses caller-supplied relay evidence that no admitted occurrence produced.
  **Falsifier (named):** `only_fava_ingest_commits_relay_derived_cache_mutations` plus `cache_refuses_fabricated_relay_evidence`. *Fails today: `.planning/codebase/CONCERNS.md:44` records that the event-cache mutation contract exposes an admission bypass through which a consuming application or provider can create query-visible signed state with fabricated relay evidence - a defect that six passed verdicts graded as `LOCAL-02 SATISFIED`.*
  **Phase:** 2.
- [ ] **OWN-07** *(ledger row: NIP-42 challenge lifecycle - `ARCHITECTURE.md:2924`)*: `fava-auth` **exclusively** owns the NIP-42 challenge lifecycle, per access context and per session generation. Authentication identity is a distinct value from event author, current account, query authors, signer selection, and routing, even where one `with_account` selection supplies several of them; a payload carrying its own author keeps it. Denial for one access context terminates only its exact operation and blocks no other account.
  **Falsifier (named):** `nip42_challenge_state_lives_only_in_fava_auth` plus `auth_denied_for_one_access_context_leaves_another_running`. *Fails today: `fava-auth` does not exist, HARD-01 names no owner, and the only NIP-42 reference in the repository was in the downstream acceptance application (`nip42_auth = false`) - a scenario that was green because nothing happened.*
  **Phase:** 8.
- [ ] **OWN-08** *(the ledger itself - `ARCHITECTURE.md:2936`: "Adding mutable state requires naming its owner and consumers")*: **Every row** of `ARCHITECTURE.md` Part IX names exactly one owner that **exists as a crate**, and has exactly one owning requirement in this file. An executable ownership audit (`ARCHITECTURE.md` Part XII **Falsifier N**) runs at **every milestone gate**, not only at Phase 10.
  **Falsifier (named):**
  ```rust
  #[test]
  fn ownership_ledger_rows_all_name_an_existing_owner() {
      for row in parse_ledger("docs/spec/ARCHITECTURE.md") {   // Part IX table
          assert!(crate_exists(row.owner), "unowned ledger fact: {}", row.fact);
          assert!(requirement_exists_for(row.fact), "unmapped ledger fact: {}", row.fact);
      }
  }
  ```
  *Fails today on `fava-runtime`, `fava-auth`, and `fava-session`, and on every ledger fact still without an owning requirement.*
  **Phase:** every phase. SUB-08 collapsed all sixteen architecture falsifiers (A-P) into one Phase-10 requirement, and the implementation plan references them exactly twice, both under M10. No milestone gate between M1 and M7 ran an ownership audit. That deferral is precisely why a facade-owned relay lifecycle survived six passed verdicts.

### Routing and Subscription Planning

- [ ] **ROUTE-01**: Automatic routing evaluates the application-selected router chain in configured order.
- [ ] **ROUTE-02**: Every router produces an immediate complete current contribution and may later replace that contribution as its facts change.
- [ ] **ROUTE-03**: A slow or blocked router cannot delay destinations already known from other router contributions, and a slow relay open cannot delay work for relays already known.
- [ ] **ROUTE-04**: Downstream routers react to the live accumulated upstream plan without taking ownership of upstream facts.
- [ ] **ROUTE-05**: Identical relay destinations deduplicate while preserving every contributing reason, target, and unresolved need, and elapsed time never converts an unresolved need into settled absence.
- [ ] **ROUTE-06**: Explicit routing creates no automatic router session or router-owned acquisition work.
- [ ] **ROUTE-07**: Router-owned acquisition uses explicit sources and cannot recursively invoke automatic routing.
- [ ] **ROUTE-08**: Route preview uses the same derivation as real routing while creating no write, receipt, signing, delivery lane, or router acquisition.
- [ ] **ROUTE-09**: Subscription planning receives logical demand already assigned to one relay session and remains separate from routing policy.
- [ ] **ROUTE-10**: Planner grouping may change wire shape but cannot change query meaning, evidence, access isolation, or cancellation.
- [x] **ROUTE-11**: Relay limits and bounded router contribution/fan-out budgets yield exact typed shortfall instead of silent dropped demand.
  *Checkmark defended, not inherited: `crates/fava-routing/src/chain.rs:428-442` emits typed refusals carrying exact numbers ("route destinations exceed bound: 257 > 256", "configured routers exceed bound: 33 > 32"). The assertion distinguishes a correct implementation from the current one and consumes no fixture-supplied fact. This is one of only two M1-M6 requirements whose evidence survives the audit. The NIP-11-derived half of RELAY-004 remains uncovered and is carried by HARD-04.*

### Durable Publication

- [ ] **WRITE-01**: Applications can accept unsigned events and verified pre-signed events through one durable write-intent lifecycle, and pre-signed bytes and identity are preserved verbatim through routing and delivery.
- [ ] **WRITE-02**: An unsigned event's author identity selects the signer without conflating authorship with relay authentication identity.
- [ ] **WRITE-03**: `Accepted` is returned only after the write obligation, current revision, receipt identity, and recovery cursor are durably committed.
- [ ] **WRITE-04** *(rewritten 2026-08-23 - restores the WRITE-004 deadline)*: Matching queries expose the accepted local revision directly from the write store **before the application `Write` is returned** - not merely before relay acknowledgement.
  **Falsifier (named):** `accepted_revision_is_query_visible_at_the_instant_write_returns` - open a matching observation, then publish; the observation must already contain the event on the first poll after `publish` returns, with no intervening await.
  **Why it was weakened before:** `GOALS.md:761` says *"The accepted local revision MUST be visible through the write-store query source **before `Accepted` is returned**."* The old WRITE-04 said *"before relay acknowledgement"*. That is an arbitrarily later deadline, and it permits a window in which the application holds a `Write` while the event is invisible to its own queries. A strict boundary was replaced with a loose one.
- [ ] **WRITE-05**: No unsigned or unpublished local event is copied into the event cache; only an admitted signed relay echo may enter it.
- [ ] **WRITE-06**: Exact explicit publication routes bypass automatic routers, and an empty explicit route is refused before signing or relay work.
- [ ] **WRITE-07**: A publisher owns one transport handoff attempt while delivery policy alone decides retry, scheduling, and give-up.
- [ ] **WRITE-08**: Every destination outcome preserves exact relay text, attempt identity, generation, acknowledgement, rejection, ambiguity, cancellation, and terminal reason; the application can await one terminal result for the whole write without writing its own reducer, and the receipt exposes derived counts.
- [ ] **WRITE-09**: Proven pre-handoff cancellation produces zero `EVENT` frames, retracts the local query contribution, and records an exact idempotent terminal receipt state.
- [ ] **WRITE-10**: Receipt removal is separate from write cancellation and obeys explicit retention and lifecycle rules, applying one bounded oldest-first terminal-retention policy that never evicts active work.
- [ ] **WRITE-11**: A hard process kill after acceptance recovers one obligation, the same write and receipt identities, and the current revision without application resubmission, and recovery completes before the engine admits new conflicting commands.
- [ ] **WRITE-12**: The application-selected router chain is the only automatic write-routing policy.
- [ ] **WRITE-13**: Outbox routing acquires kind:10002 facts through explicit indexer queries owned by its router crate.
- [ ] **WRITE-14**: Hint routing uses pointer-like hints and admitted relay evidence through its own independently selectable crate.
- [ ] **WRITE-15**: App-relay routing always contributes configured relays according to its documented read/write scope.
- [ ] **WRITE-16**: Fallback routing contributes and retracts independently as upstream target coverage changes.
- [ ] **WRITE-17**: Known destinations begin delivery immediately while other recipient or route needs remain unresolved.
- [ ] **WRITE-18**: Later route destinations create new delivery lanes under the same receipt and signed event without duplicate sends to existing destinations, and a corrected replaceable generation retains destinations that may have received the predecessor.
- [ ] **WRITE-19**: Duplicate destination contributions cannot create duplicate publication handoffs.
- [ ] **WRITE-20**: A removed desired route can retire only work proven not to have crossed a handoff boundary; historical delivery facts remain exact.
- [ ] **WRITE-21**: Automatic routes continue to re-evaluate while work remains open, using exact route revision and lane generation identity.
- [ ] **WRITE-22**: Route preview and initial real routing are identical when their input facts do not change.
- [x] **WRITE-23**: Route contributions, destinations, attempts, retries, receipt facts, and retained history have explicit bounds or typed refusal/shortfall.
  *Checkmark defended, not inherited: same executable negative evidence as ROUTE-11 (`crates/fava-routing/src/chain.rs:428-442`), producing typed refusals with exact numbers. The retained-history half is partially carried by redb oldest-first retention (`crates/fava-write-store-redb/tests/semantic_write_store/recovery.rs:196`), which is genuinely discriminating for ordering; the "active writes are never evicted" clause is unproven and is carried by WRITE-10.*

### Semantic Writes and Capabilities

Phase 7 is the one phase the audit retains. `07-VERIFICATION.md` distinguishes `implementation_head`
from `verified_head`, states that the verifier **reran** all four CLI canaries rather than citing
preserved bundles, and explicitly resolved a PLAN-versus-authority conflict in favour of the
authority. Its evidence is contemporaneous and partly independent. These nine checkmarks are
therefore retained - and they are the only phase-level retention in this file.

- [x] **CAP-01**: Protocol capability crates expose ordinary event values or semantic replaceable-event edits, including opposing operations, without signing, routing, publishing, or owning receipts.
- [x] **CAP-02**: The accepted write resolves and persists the author before revision, and every resulting event generation uses that frozen author while the edit itself carries none.
- [x] **CAP-03**: A first-value semantic operation can apply when no prior replaceable event exists.
- [x] **CAP-04**: A newer qualified source event reapplies still-live operations while preserving unrelated source changes.
- [x] **CAP-05**: One write and receipt identity remains stable across revision generations.
- [x] **CAP-06**: Signer, route, publisher, and delivery completions for retired revision generations are attributable and inert.
- [x] **CAP-07**: At least two unrelated protocol capability crates prove the semantic-edit contract is not shaped around one NIP.
- [x] **CAP-08**: Adding capability N+1 changes only its crate and selected assembly/artifact metadata, with zero universal-core behavior changes.
- [x] **CAP-09**: Raw arbitrary and future Nostr event kinds remain usable without adding universal-core switches over event-kind meaning.

### Universal Publication Vocabulary and Typed NIP-02 Reads

Registered 2026-08-23. Phase 07.1 shipped nine requirements it called `R1`-`R9`. **`R1`-`R9` existed
in no registry**: `.planning/ROADMAP.md:243` listed them, `07.1-VERIFICATION.md:11` graded all nine
`VERIFIED`, and they appeared nowhere in this file. Nine requirements were delivered, verified, and
closed entirely outside the requirement corpus. They are hereby resolved by registration under real
IDs, and they are **unchecked**: the sole external witness for all nine verdicts — a run bundle in
the downstream acceptance application's `runs/` directory — does not exist on disk, its parent path
is ignored by version control, and the pair was produced by the phase's own Plan 12 rather than
replayed by the verifier. The identifiers `R1`-`R9` are void and must not be used again.

- [ ] **NIP02-01** *(was R1)*: One universal synchronous publication door accepts every sealed payload form and returns the application `Write`; acceptance validates, reserves, applies, applies the initial route, and durably commits before any asynchronous publication starts.
- [ ] **NIP02-02** *(was R2)*: Typed signer and relay scopes are borrowed, must-use, and inert until publish; both composition orders work, selections are frozen at publish, and an invalid or abandoned scope has no effect.
- [ ] **NIP02-03** *(was R3)*: Settlement is awaitable through stable write and receipt identity with all-of and at-least-n predicates, returning the exact first satisfying revision and a terminal not-reached outcome carrying the complete receipt.
- [ ] **NIP02-04** *(was R4)*: The NIP-02 crate owns pure values, parsing, queries, projections, edits, and applier selection only; no universal owner branches on NIP-02 event-kind meaning.
- [ ] **NIP02-05** *(was R5)*: Contact-list decoding accounts for every `p` row in source order, exposing typed pubkey, relay-hint, and petname fields for valid rows and exact typed row evidence for malformed, duplicate, or uninterpreted rows; empty lists are valid and an invalid event is refused at event level.
- [ ] **NIP02-06** *(was R6)*: Contact-list, follows-of, and followers-of reads are ordinary query and snapshot values with newest-per-author replacement, exact canonical `p` matching, and deterministic repeated projection.
- [ ] **NIP02-07** *(was R7)*: Follow and unfollow edits are lossless - preserving first-occurrence order, malformed rows, unknown rows, and foreign extensions - are idempotent, and rebase onto a newer source.
- [ ] **NIP02-08** *(was R8)*: The README-level Rust API is exactly the delivered surface, and a live relay proof exercises public publish, local observation, signing, typed readback, exact relay echo, settlement, preservation, and teardown through a **retained, tracked, independently replayable** evidence bundle.
  *This is the clause that fails today. Registration does not restore the missing witness.*
- [ ] **NIP02-09** *(was R9)*: Vocabulary entries carry owners, nearest Nostr concepts, distinctions, counterexamples, lifecycles, forcing requirements, symbols, and falsifiers, and no deprecated publication door remains re-exported.

### Multi-Relay Simple Groups

Phase 07.1.1's verdict is revoked. It has **no `VERIFICATION.md` at all** - every other completed
phase has one. Its only requirement-level verdict is `07.1.1-VALIDATION.md`, which the executing
plan (`07.1.1-12-SUMMARY.md`) lists among its own modified files: the plan wrote its own pass marks.
Its `COVERAGE.md` is one sentence claiming "no external API integration" for a phase whose GROUP-12
requires a controlled two-relay public acceptance run. **Eighty-four changes landed after the completion
mark**, including behavioral fixes to GROUP-04, GROUP-07, GROUP-08, and GROUP-10, with no
re-verification. `HANDOFF.json:9` still records the phase as `paused`, with "Execute, review, verify,
and complete Phase 07.1.1" listed `not_started`.

- [ ] **GROUP-01**: `fava-simple-groups::Group` represents one opaque NIP-29 group id over an application-selected non-empty bounded host-relay set, with one-host and multi-host use sharing one public value.
- [ ] **GROUP-02**: Group content helpers return ordinary queries with the exact `h` value and explicit acquisition from every selected host while preserving accepted local write visibility.
- [ ] **GROUP-03**: Group-record helpers return ordinary queries for kinds 39000-39005 with the exact `d` value and require actual evidence from the selected host set.
- [ ] **GROUP-04**: A duplicate event served by several hosts appears once with every actual serving-relay contribution, while unique events from each host remain visible.
- [ ] **GROUP-05**: Typed group projections retain independent per-host record authority, expose exact disagreement and member/admin attribution, and never field-merge metadata or silently select a host.
- [ ] **GROUP-06**: Applications choose one fork through a single-host `Group`; the capability makes no canonical-host, migration, existence, completeness, or negative-membership claim.
- [ ] **GROUP-07**: Group publication is kind-blind, adds or validates exactly one group context, and uses one exact explicit write route over the complete selected host set.
- [ ] **GROUP-08**: Pre-signed group events remain byte-for-byte unchanged and are refused before custody when their existing group context is missing, duplicate, or contradictory.
- [ ] **GROUP-09**: NIP-29 records, pins, saved groups, and saved relays have typed bounded parsers/projections so applications do not decode raw protocol tags.
- [ ] **GROUP-10**: Saved/admin/member discovery returns ordinary `Query` or `ValueSet` expressions; kind-10009 saved-list changes use the ordinary semantic-edit lifecycle.
- [ ] **GROUP-11**: `fava-simple-groups` owns no observation, store, signer, router session, publisher, delivery, retry, receipt, runtime, or transport lifecycle, and universal owners contain no NIP-29 behavior switch.
- [ ] **GROUP-12**: Pure tests and a controlled two-relay public acceptance run prove bounds, fork visibility, exact provenance, arbitrary-kind publication, cancellation, close, and one exact handoff per selected host.

### Runtime Signer Lifecycle

Authored before its implementation - the only family in this file for which that was true before
2026-08-23, and the template on which OWN-01 through OWN-08 are modelled. SESSION-07 is the corpus's
one correctly-formed ownership requirement: named owner, exclusive, with a lock and transaction
constraint.

> **Phase 07.2 note (2026-08-23).** `SESSION-01`..`SESSION-07` are unchecked here
> because they have not been through Definition of Done gates 10 and 11, which were
> authored today. This is not a claim that the work is missing: Phase 07.2 shipped
> `crates/fava-session` and migrated `fava-publication` onto it in merge `0b23b52`.
> Phase 07.9 re-earns these verdicts against the restored corpus.

- [ ] **SESSION-01**: A running Fava instance accepts a signer after build without replacing the engine, session, write store, or accepted write identity.
- [ ] **SESSION-02**: Adding a signer wakes already accepted unsigned writes for that exact event pubkey and no other author.
- [ ] **SESSION-03**: Adding a second signer for an attached pubkey refuses without mutation; replacement is an explicit separate operation.
- [ ] **SESSION-04**: Removing a signer preserves accepted writes and receipts, cancels its current signer operation, and leaves matching unsigned work awaiting a signer until an exact signer is re-added.
- [ ] **SESSION-05**: Signer completions from replaced, removed, or retired revision generations are attributable and cannot install signed state or start delivery.
- [ ] **SESSION-06**: Runtime signer attachment is bounded and capacity overflow returns typed refusal without partial mutation.
- [ ] **SESSION-07**: `fava-session` exclusively owns mutable signer attachment; publication loads current exact attachments and invokes providers outside session/publication locks and write-store transactions.

### Authentication, Hostility, and Bounds

- [ ] **HARD-01**: Relay NIP-42 authentication is explicit, generation-scoped, and separate from event authorship and query filter identity. *(Owner named by OWN-07.)*
- [ ] **HARD-02**: Denial or failure of one account's authentication policy terminates only the exact affected operation and cannot block another account.
- [ ] **HARD-03**: Invalid, malformed, oversized, off-filter, stale, post-CLOSED, never-EOSE, truncated, silent-limit, and disconnected relay behavior remains scoped and attributable. All twelve named hostile behaviors are covered, not four.
- [ ] **HARD-04**: NIP-11 limits produce a valid plan or exact shortfall before knowingly invalid work is sent, covering maximum subscriptions, message length, subscription-id length, filter limits, event size and tag constraints, and locally evaluable proof-of-work or write restrictions; missing, stale, malformed, or unsupported claims stay unknown rather than becoming invented defaults. The limit must arrive from a real NIP-11 document, not be hand-passed by the fixture.
- [ ] **HARD-05**: Offline or unreachable time is distinct from a failed delivery attempt and does not consume the attempt budget.
- [ ] **HARD-06**: Real retryable attempts reach the configured terminal give-up policy within declared ceilings, and several writes for one relay share connection and backoff ownership rather than creating independent reconnect storms.
- [ ] **HARD-07**: A completed handoff without a received relay outcome remains ambiguous and is never rewritten as acknowledged, rejected, or never sent. The ambiguity must be **derived by Fava from transport facts**, never supplied by a test publisher.
- [ ] **HARD-08**: Every externally influenced input, queue, set, fan-out, retained history, diagnostic stream, and artifact has an explicit bound, backpressure rule, refusal, or shortfall - including active relay sessions, engine-side provider operations, fetched service entries, and platform bridge queues.
- [ ] **HARD-09**: Provider panic, blocking, late result, malformed result, or ignored cancellation cannot block unrelated queries, relays, writes, or shutdown.
- [ ] **HARD-10**: Deterministic hostile scenarios use real sockets and separate processes, and publish resource envelopes and failure evidence for every run.

### Profiles and Services

- [ ] **PROF-01**: The baseline event-cache contract remains coherent without implying persistence, retention, coverage, or restart guarantees it does not own.
- [ ] **PROF-02**: A persistent profile provides its declared cold-cache reuse, provenance, deletion/expiry, coverage, and restart behavior without global completeness claims.
- [ ] **PROF-03**: An ephemeral event-cache profile restarts without cached relay events while accepted writes recover when its selected write store is durable.
- [ ] **PROF-04**: Event-cache eviction revises current queries coherently and adjusts any cache-owned coverage evidence, and never retains mutually inconsistent positive and negative facts.
- [ ] **PROF-05**: NIP-05 and NIP-11 independently own validation, freshness, negative caching, stale results, and failure semantics.
- [ ] **PROF-06**: A generic fetch cache stores opaque service payloads and may be physically shared without semantic leakage between NIP-05 and NIP-11.
- [ ] **PROF-07**: Every persistent provider owns and validates its private schema, version, migration, corruption, and refusal behavior, and an unsupported or corrupt store is refused explicitly with its rows intact rather than silently reset.
- [ ] **PROF-08**: Destructive reset clears exactly the selected profile's cache, write, session, service, and provider-owned local state according to its public contract, or reports exact partial failure.
- [ ] **PROF-09**: Profile guarantees are generated or checked from an explicit named assembly document, and the same application source proves persistent and ephemeral behavior by changing provider selection only.

### Provider Substitution

- [ ] **SUB-01**: Standard providers expose no privileged constructor, facade path, internal-state access, or test-only semantic capability unavailable to external providers.
- [ ] **SUB-02**: Public conformance kits execute unchanged against standard and materially different alternative implementations for every claimed replaceable seam, covering ordinary behavior, refusal and malformed input, cancellation and close, late completion, boundedness and overload, restart where the provider owns persistent state, account and relay-access isolation, and negative tests proving no bypass of universal invariants.
- [ ] **SUB-03**: Applications select provider profiles by changing assembly and dependencies without editing universal core source.
- [ ] **SUB-04**: Replacing one provider requires no changes to unrelated providers or their owned behavior.
- [ ] **SUB-05**: Alternative router, event cache, durable write store, planner, transport, publisher, delivery policy, signer, and fetch cache implementations use only public contracts, and none can redefine the universal boundary its row of the replaceable-boundary table reserves. No provider contract may be half-optional: a minimal third-party implementation that compiles must not fail at runtime with a message indistinguishable from a transient refusal.
- [ ] **SUB-06**: Provider failures remain isolated and private persisted-format incompatibility remains owned by that provider rather than a global assembly identity.
- [ ] **SUB-07**: Dependency-negative tests reject forbidden semantic-owner, contract, provider, runtime, facade, and capability edges.
- [ ] **SUB-08**: Every architecture falsifier passes, provider change amplification remains narrow, and contract stabilization occurs only after the provider matrix succeeds. *(Ownership falsifier N is no longer deferred here - it runs at every gate under OWN-08.)*
- [ ] **SUB-09** *(new 2026-08-23)*: Shipped application-facing test infrastructure lets a consuming application and a provider author exercise deterministic time and expiry, scripted relay frames and protocol misbehavior, connection failure and reconnect, EOSE/silence/CLOSED/auth/relay limits, signer delay/refusal/invalid output/human approval, event-cache and write-store restart, cancellation races, exact route destinations and router updates, per-relay publication outcomes, provider substitution, and platform lifecycle - and an **executable mutation harness** proves each claimed mechanism by disabling it and observing failure.
  *Why it is new: OPS-005 mandates twelve facilities as product. Two ship (`fava-transport-testkit`, `fava-router-testkit`). The "prove the mechanism by disabling it" clause exists only as prose in `# fava:falsifier=` comments; 41 named deliberate breaks exist under `features/`, five were ever executed as named, and none of the 510 changes in history carries the `Red:`/`Mutation:` record the testing guide requires.*

### Native Products and Release

- [ ] **NATIVE-01**: Rust, Swift, Kotlin/JVM, Android, and iOS applications consume declared release artifacts without repository-relative sources or raw generated bindings.
- [ ] **NATIVE-02**: Native artifacts expose only the providers, profiles, and protocol capabilities selected by their assembly.
- [ ] **NATIVE-03**: Live-query open, current value, next/update, cancellation, close, and terminal behavior match Rust semantics in Swift and Kotlin.
- [ ] **NATIVE-04**: Event records, source evidence, route shortfall, receipts, errors, ambiguity, and restart outcomes map without semantic flattening across languages.
- [ ] **NATIVE-05**: Android fresh-process tests prove the declared persistent-profile recovery behavior through ordinary application artifacts.
- [ ] **NATIVE-06**: Any iOS profile claiming suspension transparency proves suspension and resume behavior on a physical device.
- [ ] **NATIVE-07**: Repeated native lifecycle cycles return tasks, handles, descriptors, Rust memory, and native heap to declared baseline envelopes.
- [ ] **NATIVE-08**: The release candidate passes the shared Rust/Swift/Kotlin parity corpus - maintained as a real structural inventory of public operations and values per platform, not heuristic source-word matching - plus parity mutations, real-process evidence, two-relay interoperability subset, hostile corpus, provider matrix, and declared release-build resource budgets.
- [ ] **NATIVE-09** *(new 2026-08-23)*: Each shipped product profile publishes **measured** bounds for first local query result, first relay result, event ingest throughput, active and idle observation cost, thread growth, memory retention, write recovery, and teardown, obtained through the production path and attributed to the selected providers rather than presented as universal Fava behavior.
  *Why it is new: OPS-011 had no counterpart. NATIVE-07 covers lifecycle envelopes only, and no benchmark harness exists anywhere in the repository.*

---

## Definition of Done

Every v1 requirement is complete only when:

1. Its owning behavior and responsibility are explicit in the authoritative specifications or a focused approved change.
2. The smallest executable proof failed before implementation for the intended reason, and the `Red:` record exists in the change that introduced it.
3. The owning component proof passes through public contracts.
4. Its named deliberate break makes the proof fail causally, executed as named, with the `Mutation:` record.
5. A public Fava capstone proves any additional facade, process, relay, persistence, or platform boundary.
6. Independent wire, process, relay, storage, or native evidence exists wherever Fava cannot be its own witness, and that evidence is **tracked in the repository**, not written to an ignored path.
7. Failure, cancellation, late completion, teardown, diagnostics, and resource bounds are exact and attributable.
8. Contract, provider-conformance, profile, acceptance, and ownership documentation is current.
9. The complete scoped validation set passes **in continuous integration**, and the implementation is committed.
10. *(added 2026-08-23)* The requirement's text predates the implementation it grades, it carries a `Spec basis` row in the traceability matrix below, and no clause of that spec requirement is dropped or loosened in the mapped wording.
11. *(added 2026-08-23)* The proof does not consume a fact supplied by its own fixture, and it can distinguish a correct implementation from the current one.

Gate 10 exists because the previous corpus failed it for all 66 M1-M6 requirements. Gate 11 exists
because 39 of the 131 spec requirements have evidence that cannot tell a correct implementation from
the shipped one. Gate 9's "in continuous integration" clause exists because CI runs
`tools/check_vocabulary.py` and nothing else - no build, no test, no clippy, no falsifier, no
acceptance run.
Three hundred and six tests pass in this repository and no automated process has ever run them.
Every green result in its history is a result somebody chose to run, on a machine they chose, at a
moment they chose, and then described in a document.

---

## Spec traceability matrix

**Authority:** `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md`, 131 numbered requirements.
Line numbers are the requirement's heading line in that file.

**Status vocabulary**

| Status | Meaning |
|---|---|
| `MAPPED` | A `.planning` requirement carries the spec requirement's full conjunction. |
| `WEAKENED` | A counterpart exists but drops or loosens a clause. Both wordings are quoted in the note. |
| `UNMAPPED` | No `.planning` requirement covers it. An out-of-scope table row does not count as coverage. |
| `DEFERRED` | An explicit unpromised product decision, correctly tracked as such. |

### Part I - Product and composition goals

| Spec ID | Line | `.planning` counterpart | Status | Note |
|---|---|---|---|---|
| GOAL-001 | 149 | - | UNMAPPED | Represented only as an Out of Scope table row ("Application framework, UI state, navigation, ranking, moderation, and account UX"). A non-feature note is not a testable MUST NOT. |
| GOAL-002 | 157 | WRITE-01, NIP02-01, NIP02-02 | WEAKENED | Spec: *"The primary application mental model MUST remain live queries and accepted publication obligations… Supporting operations MUST reuse the same underlying primitives rather than creating parallel query, publication, routing, receipt, or lifecycle systems."* Planning: WRITE-01, *"one durable write-intent lifecycle."* No requirement bounds the public surface. `Fava::receipt_changes` returns a `tokio::broadcast::Receiver` and `Observation::attach_cancellation` takes a `tokio::watch::Sender` - runtime primitives on the public surface, contrary to `partial-spec-api-semantics.md:330`, and nothing forbids it. |
| GOAL-003 | 170 | SUB-03 | MAPPED | |
| GOAL-004 | 180 | OWN-08, and OWN-01 through OWN-07 | MAPPED *(new 2026-08-23)* | Previously UNMAPPED. This is the requirement whose absence let a systemic ownership inversion pass six milestone reviews. |
| GOAL-005 | 188 | SUB-01, SUB-02 | MAPPED | |
| GOAL-006 | 196 | SUB-04, WRITE-12, PROF-05 | MAPPED | |
| GOAL-007 | 211 | SUB-05, LOCAL-02, READ-12, OWN-06 | WEAKENED | Spec: *"a custom provider cannot construct stronger evidence or success than the facts supplied to it justify"* across eight named universal meanings, one of which is *"whether two relay-access identities are isolated."* Planning covers admission and provenance. Relay-access isolation is untestable in practice: `RelayAccess::named` has zero occurrences across `crates/`, `apps/`, and `falsifiers/`; every test uses `RelayAccess::public()`. |
| GOAL-008 | 226 | HARD-09, OWN-05, SESSION-07 | MAPPED *(restored 2026-08-23)* | Previously WEAKENED: no requirement carried *"Provider execution MUST NOT occur while holding another subsystem's authoritative transaction or lock."* OWN-05 and SESSION-07 now do. |
| GOAL-009 | 234 | SUB-02, SUB-09 | WEAKENED | Spec: *"Each replaceable contract MUST ship a public conformance kit"* covering eight named categories, working from public APIs. SUB-02 now enumerates the categories, but no requirement asserts a kit exists **per boundary**, and none ships for event cache, write store, evaluator, planner, publisher, delivery policy, or signer. |
| GOAL-010 | 249 | SUB-05 | WEAKENED | Spec pairs each of eleven responsibilities with *"the universal boundary it cannot redefine"* and adds *"Bundling several implementations into one distribution artifact MUST NOT merge these authorities."* SUB-05 references the boundary table; no per-row boundary is independently falsifiable and the anti-bundling clause has no proof. |

### Part II - Live queries and reactive observation

| Spec ID | Line | `.planning` counterpart | Status | Note |
|---|---|---|---|---|
| QUERY-001 | 273 | - | UNMAPPED | The query language itself. `nested`, `derived`, `union`, `intersection`, `difference`, `current account`, and `projected` return **zero** hits across all of `.planning/`. An application composing a nested or derived query exercises behavior the project has never required, planned, or tested, and QUERY-001 makes it mandatory for v1. |
| QUERY-002 | 292 | LOCAL-09, OWN-04 | WEAKENED, **restored 2026-08-23** | Spec `:296` *"Equivalent observations MAY share local evaluation, relay connections, and wire subscriptions"*; `:298` *"**Acceptance:** two equivalent handles share work; closing one does not close work still needed by the other."* Old planning wording (LOCAL-09): *"Equivalent query descriptions… have stable semantic identity."* Only the identity half survived, and it landed in M1. The `MAY` in the body was read as optional and the normative Acceptance line was discarded with it. The sharing and close-safety halves are now in LOCAL-09 and OWN-04, with a named falsifier; LOCAL-09 moves from Phase 1 to Phase 3. |
| QUERY-003 | 300 | LOCAL-08, READ-21 | WEAKENED, **restored 2026-08-23** | Spec `:305` *"return a typed refusal and leave no ownerless demand, partial dependency, or relay work"*; `:309` *"**Acceptance:** injected failure during open leaves existing queries unchanged and creates no leaked subscription."* Old planning wording kept only *"all-or-nothing"*, scoped to local queries. READ-09 covered cancellation and READ-10 covered close; **neither covered failure during open of the relay path.** READ-21 now does. |
| QUERY-004 | 311 | LOCAL-08 (+READ-02 conjunct) | WEAKENED, **restored 2026-08-23** | Spec `:313` *"The initial query value MUST be produced from the configured local query sources without waiting for any relay response"*; `:325` *"**Acceptance:** with every relay unreachable, opening a query returns its local view or a local-source error, never hangs waiting for the network."* Old planning wording: *"Opening a **local** query is all-or-nothing and returns one complete current snapshot without waiting for relay work."* Two compounding weakenings: the qualifier `local` restricted the requirement to the one regime where no relay exists to wait for, and it was assigned to M1, whose exit gate forbids any networking dependency. The conjunction with QUERY-013 - start relay work immediately **while** returning the local view - existed in no requirement, no roadmap criterion, and no verification. |
| QUERY-005 | 327 | LOCAL-04, LOCAL-05, LOCAL-06 | WEAKENED | Spec adds *"accept admitted live relay occurrences as current query input even when the selected event cache does not retain them"* and the null-cache acceptance (*"With a null event cache, a verified live event still reaches the open query but is absent from a later newly opened query"*). No planning requirement states the null-cache clause. |
| QUERY-006 | 350 | LOCAL-07, READ-15 | WEAKENED | Spec `:366` *"When a derived dependency shrinks, records that matched only through the removed values MUST be retracted from the same open query."* `derived` returns zero hits in `.planning/`. The eleven-item update list is covered; the derived-shrink retraction is not, and cannot be until QUERY-001's derived-value axis exists. |
| QUERY-007 | 370 | - | UNMAPPED | Nested queries retaining independent routing, access, freshness, cache-use, evidence, and acquisition authority. `nested` returns zero hits. |
| QUERY-007A | 385 | - | UNMAPPED | Derived references contributing permitted relay hints. `hint` appears only in WRITE-14, which is write-side. |
| QUERY-008 | 393 | - | UNMAPPED | Combined queries as one deduplicated view with per-branch evidence and a whole-query bound. `branch` returns zero hits. |
| QUERY-009 | 403 | - | UNMAPPED | Present only as an Out of Scope table row. A normative MUST NOT with six enumerated prohibitions was demoted to a non-feature note. The evidence audit records it as satisfied by absence: no test asserts the absence, so adding a `synced` field later would fail nothing. |
| QUERY-010 | 420 | READ-06, READ-07 | MAPPED | |
| QUERY-011 | 430 | LOCAL-10, READ-16, READ-17, READ-23 | WEAKENED, **restored 2026-08-23** | Spec `:436` *"Observation memory MUST remain bounded even when an application is slow."* LOCAL-10 bounds *delivery*; READ-16 promises *"an exact **bounded latest result**"*. **A bound on the result value is not a bound on observation memory.** The memory conjunct previously existed only inside READ-20, whose sole evidence measures thread identity and no memory at all. READ-23 now carries it with a memory measurement in its falsifier. |
| QUERY-012 | 440 | READ-09, READ-10, READ-18, READ-22 | WEAKENED, **restored 2026-08-23** | Spec enumerates eight invariants. Planning covered three. **Absent from all 129:** `:445` *"a second concurrent pull is refused without consuming data"*; `:447` *"an update delivered once is never delivered again"*; `:448` *"invalid acknowledge/cancel/close ordering is refused"*; `:451` *"shutdown ends all pending pulls without hanging."* Splitting one requirement across three IDs dropped half of it. READ-22 restores the four. |
| QUERY-013 | 457 | READ-02 | WEAKENED, **restored 2026-08-23** | Spec `:461` *"Cache-only queries contribute no relay work. Reiterating an already-open handle does not create another underlying query."* Old READ-02 kept only the first sentence of the requirement and dropped both clauses of `:461`. The second clause is the same shared-work property lost from QUERY-002 - dropped **twice, independently**, from the two requirements that could each have carried it. |
| QUERY-013A | 465 | - | UNMAPPED | Freshness policy evaluated at open, per query, without perturbing other open queries or triggering sweeps. `Freshness::{CacheOnly, Live}` exists; nothing asserts the isolation. |
| QUERY-014 | 473 | ROUTE-03, ROUTE-04, ROUTE-05 | WEAKENED | Spec adds *"A route contribution that disappears MAY withdraw relay work when no other router still contributes that destination"* for the **query** side. Planning covers withdrawal only on the write side (WRITE-20). |
| QUERY-015 | 483 | READ-13, READ-14 | MAPPED | |
| QUERY-016 | 491 | - | UNMAPPED | App-authored `since`/`until`/limit never widened by cache coverage. `since` and `watermark` return zero hits in `.planning/`; `FilterSelection` has no `since`/`until` field, so application time windows are currently inexpressible. |
| QUERY-017 | 501 | Deferred decision 1 (public windowing API) | DEFERRED | Correctly unpromised. The separation of acquisition window from presentation window is recorded but has no requirement, which is consistent with the deferral. |

### Part III - Event admission, state, and caches

| Spec ID | Line | `.planning` counterpart | Status | Note |
|---|---|---|---|---|
| EVENT-001 | 513 | READ-04, READ-05, OWN-06 | MAPPED | OWN-06 adds the owner the spec's admission sequence implies and the ledger names (`fava-ingest`). |
| EVENT-002 | 528 | LOCAL-01 | MAPPED | |
| EVENT-003 | 544 | READ-11, READ-12 | MAPPED | Note: the current evidence hand-writes provenance in the fixture and proves the negative case only by never writing it. That is a gate-11 failure, not a mapping failure. |
| EVENT-004 | 554 | PROF-01, PROF-02, PROF-03, PROF-09 | MAPPED | |
| EVENT-005 | 580 | PROF-04 | MAPPED | |
| EVENT-006 | 590 | LOCAL-07 | WEAKENED, tightened 2026-08-23 | Spec: deletion applies *"only to targets the author is permitted to delete"*, and *"Expiration MUST retract events when their expiry becomes due."* Old LOCAL-07 named neither. `.planning/codebase/CONCERNS.md:112` records that authorized deletion does not retract a matching local write, and `:123` that future expiration never retracts - both written 20 seconds before `01-VERIFICATION.md` graded LOCAL-07 SATISFIED. LOCAL-07's text now names both clauses; the requirement is unchecked. |
| EVENT-007 | 600 | - | UNMAPPED | Source-scoped cache coverage/progress facts that never become a global sync claim. `coverage_progress`, `watermark`, and `progress` find nothing relevant; no source-scoped coverage concept exists. |
| EVENT-008 | 608 | LOCAL-03, WRITE-05 | MAPPED | |
| EVENT-009 | 618 | LOCAL-04, READ-11 | MAPPED | |
| EVENT-010 | 629 | PROF-05, PROF-06 | MAPPED | |
| EVENT-011 | 647 | PROF-07 | MAPPED, tightened 2026-08-23 | PROF-07 now carries *"MUST NOT be silently reset or reinterpreted"* explicitly, because the existing evidence is a bare `is_err()` with no variant assertion and no assertion that the rows survived. |
| EVENT-012 | 662 | PROF-08 | MAPPED | |
| EVENT-013 | 677 | - | UNMAPPED | *"A failed cache or store operation MUST fail the operation honestly and leave no success fact for an uncommitted mutation."* No counterpart. |
| EVENT-014 | 683 | - | UNMAPPED | One admitted event is one atomic observable mutation, with fault injection at **each** provider-defined boundary. `atomic` returns zero hits in `.planning/`. Two injection points exist in the memory cache; the acceptance requires each boundary with a concurrent reader. |

### Part IV - Event construction, replaceable-event edits, and publication

| Spec ID | Line | `.planning` counterpart | Status | Note |
|---|---|---|---|---|
| WRITE-001 | 697 | WRITE-01, CAP-01 | WEAKENED | Spec acceptance: *"removing protocol-specific methods from the event builder does not prevent any of them from constructing valid events."* No planning requirement asserts the builder is kind-blind by construction; CAP-08 asserts the inverse direction (adding a capability). |
| WRITE-002 | 711 | WRITE-01, NIP02-01, NIP02-02, NIP02-03 | WEAKENED | Spec fixes the facade shape - `fava.publish`, `fava.by(author)`, `fava.to(relays)?` - and states *"Applications do not construct `WriteIntent`, receive `AcceptedWrite`, or call a separate facade wait function."* That prohibition lived only in Phase 07.1's unregistered R1/R2; it is now NIP02-01/02, and unchecked. |
| WRITE-003 | 737 | WRITE-02, CAP-02 | MAPPED | |
| WRITE-004 | 749 | WRITE-03, WRITE-04 | WEAKENED, **restored 2026-08-23** | Spec `:761` *"The accepted local revision MUST be visible through the write-store query source **before `Accepted` is returned**."* Old WRITE-04: *"Matching queries expose the accepted local revision directly from the write store **before relay acknowledgement**."* "Before relay acknowledgement" is an arbitrarily later deadline that permits a window in which the application holds a `Write` while the event is invisible to its own queries. A strict boundary was replaced with a loose one. WRITE-04 now states the spec deadline. Separately, the crash-recovery acceptance is currently asserted against a hard-coded `ReceiptId::from_u64(1)` rather than the id the killed child returned, and every kill lands strictly post-commit, so no torn-commit boundary is exercised. |
| WRITE-005 | 772 | WRITE-04, LOCAL-03 | MAPPED | |
| WRITE-006 | 782 | CAP-03, CAP-04, CAP-05, CAP-06 | MAPPED | |
| WRITE-007 | 799 | SESSION-05, WRITE-02 | WEAKENED | Spec lists five conditions a signer completion must satisfy and requires unavailable, rejected, invalid-output, cancelled, timed-out, and stale results to remain distinct. SESSION-05 carries staleness only. |
| WRITE-008 | 815 | SESSION-02, SESSION-04 | MAPPED | Previously unmapped entirely; closed correctly by the SESSION family, which was authored before its implementation. |
| WRITE-009 | 825 | - | UNMAPPED | Sign without publishing - no intent, receipt, route, or delivery. `without publishing` returns zero hits; `sign_only`/`sign_without`/`SignOnly` return zero hits in the code. |
| WRITE-010 | 829 | WRITE-01, GROUP-08 | WEAKENED, tightened 2026-08-23 | Spec: *"Publication MUST preserve the exact signed event bytes/identity; routing and delivery MUST NOT mutate it."* Byte-for-byte preservation was stated only for groups. WRITE-01 now carries it universally. |
| WRITE-011 | 835 | WRITE-06, ROUTE-01, ROUTE-06 | MAPPED, tightened 2026-08-23 | Spec's *"An empty explicit route is refused before signing or relay work"* is now in WRITE-06. |
| WRITE-012 | 846 | ROUTE-01, ROUTE-02, ROUTE-05, WRITE-12 | MAPPED | |
| WRITE-013 | 858 | ROUTE-02, ROUTE-03, WRITE-17, WRITE-18 | MAPPED | |
| WRITE-014 | 870 | ROUTE-07, WRITE-13 | MAPPED | |
| WRITE-015 | 878 | ROUTE-05, WRITE-17 | MAPPED, tightened 2026-08-23 | Spec's *"Elapsed time MUST NOT convert unresolved knowledge into settled absence"* was absent; it is now in ROUTE-05. |
| WRITE-016 | 890 | ROUTE-08, WRITE-22 | MAPPED | |
| WRITE-017 | 896 | WRITE-17, WRITE-18, WRITE-19 | MAPPED | |
| WRITE-018 | 904 | WRITE-08 | MAPPED, tightened 2026-08-23 | Spec's *"The application MUST also be able to await one terminal result for the whole write without implementing its own reducer"* and the derived-count clause were absent from WRITE-08; both are now in it. |
| WRITE-019 | 922 | HARD-05, HARD-06, WRITE-07 | MAPPED, tightened 2026-08-23 | Spec's *"Several writes for one relay SHOULD share connection/backoff ownership rather than creating independent reconnect storms"* is now in HARD-06. Current evidence is a pure-function test on `DeliveryPolicy::decide`; nothing drives repeated real attempts to give-up. |
| WRITE-020 | 932 | HARD-07 | MAPPED, tightened 2026-08-23 | HARD-07 now requires the ambiguity to be **derived by Fava**, because the existing `OutcomeUnknown` is supplied by the test publisher and the redb assertion is vacuously true on an empty destination map. |
| WRITE-021 | 940 | WRITE-20, CAP-06 | MAPPED | |
| WRITE-022 | 948 | WRITE-18, CAP-04 | WEAKENED, tightened 2026-08-23 | Spec: the corrected successor's destination set MUST include *"destinations that require correction because they may have received the predecessor."* WRITE-18 now names it; no test asserts it, and destination union exists in the store without a proof. |
| WRITE-023 | 957 | WRITE-09, WRITE-10 | MAPPED | |
| WRITE-024 | 972 | WRITE-10, WRITE-11 | WEAKENED | Spec: *"page through active and retained writes without loading all history."* `page` and `paging` return zero hits in `.planning/`; no paging API exists, and `schema::load` reads every row at open. Reattach-by-receipt and terminal-after-restart are mapped; paging is not. |
| WRITE-025 | 983 | LOCAL-04, READ-15 | MAPPED | |
| WRITE-026 | 989 | - | UNMAPPED | *"Terminal delivery does not itself retract the local event."* No counterpart. The nearest evidence opens no observation, so the acceptance is asserted nowhere. |
| WRITE-027 | 997 | - | UNMAPPED | Settled empty routing yields a typed no-destination outcome naming the reasons. `no-destination` returns zero hits. |
| WRITE-028 | 1005 | WRITE-21 | MAPPED | |
| WRITE-029 | 1011 | WRITE-11 | MAPPED, tightened 2026-08-23 | Spec's *"before the engine admits new commands that could conflict"* and bounded-supersession recovery were absent; WRITE-11 now carries the ordering clause. No test issues a command concurrently with store open. |
| WRITE-030 | 1021 | - | UNMAPPED | Already-expired events refused before custody, for unsigned, pre-signed, **and** replaceable-edit forms. `expired` returns zero hits in `.planning/`; the replaceable-edit path has no expiry guard at all. |

### Part V - Relay planning, transport, authentication, and protocol services

| Spec ID | Line | `.planning` counterpart | Status | Note |
|---|---|---|---|---|
| RELAY-001 | 1031 | - | UNMAPPED | *"Every contacted relay MUST be explainable by current demand. Bystander relays receive no connection attempt."* `justified` and `bystander` return zero hits in `.planning/`. |
| RELAY-002 | 1037 | ROUTE-09, OWN-02 | MAPPED *(owner named 2026-08-23)* | ROUTE-09 described the planner's input; OWN-02 now names who owns the demand and the desired plan. |
| RELAY-003 | 1045 | ROUTE-10 | MAPPED | The evidence audit records **no evidence of any kind** for the grouping-equivalence property, and the named deliberate break describes behavior the implementation already exhibits, so the break cannot fail. Mapping is sound; the proof is not. |
| RELAY-004 | 1053 | HARD-04, ROUTE-11 | MAPPED, tightened 2026-08-23 | The fan-out half (ROUTE-11) is genuinely evidenced. The NIP-11 half has zero evidence - NIP-11 does not exist - and HARD-04 now states that the limit must arrive from a real document rather than being hand-passed by the fixture. |
| RELAY-005 | 1070 | OWN-03 | MAPPED *(new 2026-08-23)* | Previously UNMAPPED: READ-03 covered wire messages only and no requirement said transport owns sessions. This is the ledger row (`ARCHITECTURE.md:2923`) the facade violates. |
| RELAY-006 | 1085 | READ-13, OWN-03 | MAPPED *(owner named 2026-08-23)* | READ-13 stated the behavior and named no owner. |
| RELAY-007 | 1091 | HARD-01, HARD-02, OWN-07 | MAPPED *(owner named 2026-08-23)* | `fava-auth` does not exist. The only NIP-42 reference in the repository sets `nip42_auth = false`; the scenario is green because nothing happens. |
| RELAY-008 | 1105 | WRITE-08, READ-07 | MAPPED | |
| RELAY-009 | 1109 | PROF-05, PROF-06 | MAPPED | No NIP-11 implementation exists. |
| RELAY-010 | 1124 | PROF-05, PROF-06 | MAPPED | No NIP-05 implementation exists. |
| RELAY-011 | 1132 | - | UNMAPPED | *"Fava MUST NOT automatically introduce negentropy or another set-reconciliation protocol during open, restart, or reconnect."* Present only as an Out of Scope table row. Satisfied by absence with no test asserting the absence. |
| RELAY-012 | 1138 | HARD-03 | MAPPED, tightened 2026-08-23 | Four of twelve named hostile behaviors are covered. HARD-03 now states "all twelve, not four". Uncovered: stall/never-EOSE, silent subscription cap, mid-stream auth challenge, EOSE-then-more-events, truncated frames, injected bytes, ack-without-serving, disconnect-after-handoff. |
| ROUTER-001 | 1144 | WRITE-13 | MAPPED | |
| ROUTER-002 | 1159 | WRITE-14 | MAPPED | |
| ROUTER-003 | 1165 | WRITE-15 | MAPPED | |
| ROUTER-004 | 1169 | WRITE-16 | MAPPED | |

### Part VI - Identity, sessions, signers, and cryptographic operations

`ID` is the worst-covered family in the corpus: one of eight requirements has evidence, and six of
eight have no `.planning` counterpart at all. No account or session type exists in the codebase -
every `Session` match is a `RelaySession` or `RouterSession`.

| Spec ID | Line | `.planning` counterpart | Status | Note |
|---|---|---|---|---|
| ID-001 | 1181 | SESSION-01 | WEAKENED | SESSION-01 asserts accepted write identity survives signer attachment. The spec's account model - *"A session contains accounts, current-account selection, and attached signer/crypto provider configuration"* - and *"Removing an account or logging out does not delete cached public events, accepted writes, or receipts"* have no counterpart. |
| ID-002 | 1189 | - | UNMAPPED | Current account as a reactive query input. `Query` has no account axis; `current account` returns zero hits in `.planning/`. |
| ID-003 | 1195 | - | UNMAPPED | Missing identity refused **before** creating a write or receipt. |
| ID-004 | 1201 | - | UNMAPPED | Raw-vs-bech32 identity shape; refusal rather than silent reinterpretation. `bech32` returns zero hits. |
| ID-005 | 1207 | - | UNMAPPED | All-or-nothing session restore. `restore` matches only READ-13's reconnect. |
| ID-006 | 1215 | - | UNMAPPED | Signer providers preserve key custody without Fava receiving raw private-key bytes. The only implementation holds `Keys`; no external, remote, or hardware signer falsifier exists. |
| ID-007 | 1221 | - | UNMAPPED | Signing, encryption, and decryption as separate operations with NIP-44/NIP-04 separation. `encrypt` and `NIP-44` return zero hits in `.planning/`; `Signer` has `sign_event` only. |
| ID-008 | 1227 | - | UNMAPPED | Secret material never entering generic state, diagnostics, logs, debug formatting, or persistent caches. `secret` returns zero hits. |

### Part VII - Protocol crates and composition

| Spec ID | Line | `.planning` counterpart | Status | Note |
|---|---|---|---|---|
| PROTO-001 | 1237 | CAP-08, CAP-09, GROUP-11, NIP02-04 | MAPPED | |
| PROTO-002 | 1243 | CAP-01, CAP-07 | MAPPED | |
| PROTO-003 | 1257 | CAP-01, CAP-03, CAP-04, NIP02-05, NIP02-06, NIP02-07 | WEAKENED | The NIP-02 row-evidence clauses - total `p`-row accounting, typed evidence for malformed/duplicate/uninterpreted rows, first-occurrence order and extension preservation - lived only in Phase 07.1's unregistered R5/R6/R7. Registered here as NIP02-05/06/07 and unchecked. |
| PROTO-004 | 1279 | CAP-09, GROUP-07 | MAPPED | |
| PROTO-005 | 1291 | - | UNMAPPED | Protocol crates own reference-tag meaning: markers, author hints, usable relay hints derived from thread position. No reply, reaction, repost, quote, or comment crate exists. |
| PROTO-006 | 1297 | GROUP-01 through GROUP-12 | MAPPED | Twelve requirements, verdict revoked. |
| PROTO-007 | 1326 | - | UNMAPPED | NIP-25 reaction construction refuses ambiguous content. No NIP-25 crate. |
| PROTO-008 | 1332 | - | UNMAPPED | NIP-09 deletion as a protocol **write**, distinct from cancelling an unsent local obligation. All six matches in the codebase are ingestion of kind 5; nothing publishes a deletion, and nothing asserts deletion is not cancellation. |
| PROTO-009 | 1338 | - | UNMAPPED | Shared presentation-neutral content parsing. Zero hits. |
| PROTO-010 | 1344 | - | UNMAPPED | Explicit protocol inventory classifying each service as required, optional, deferred, or application-owned. No such document exists. |

### Part VIII - Diagnostics, boundedness, testing, platforms, and lifecycle

`OPS` is the family with **zero** proven requirements out of eleven.

| Spec ID | Line | `.planning` counterpart | Status | Note |
|---|---|---|---|---|
| OPS-001 | 1387 | READ-19 | MAPPED, tightened 2026-08-23 | Spec's *"Diagnostics MUST NOT synthesize a global sync score, completeness percentage, or invented root-cause fact"* was absent from READ-19; it is now in it. Provider availability/failure, cache/profile status, and authentication state remain unlisted in READ-19's field set. |
| OPS-002 | 1402 | LOCAL-10, READ-16 | WEAKENED | Spec adds *"With no diagnostics observer, Fava SHOULD avoid constructing expensive presentation snapshots."* No counterpart. The existing evidence asserts only `coalesced_query_updates > 0` - Fava's own counter, and `> 0` cannot distinguish a correct implementation from any implementation. |
| OPS-003 | 1408 | - | UNMAPPED | Stalled writes inspectable under one classification (unroutable, unsignable, undeliverable), independently of individual receipt streams. `stalled` and `stuck` return zero hits; `Fava::open_receipts()` returns an unclassified `Vec<Receipt>`. |
| OPS-004 | 1420 | HARD-08 | MAPPED, tightened 2026-08-23 | HARD-08 now names the four resource classes with no bound at all today: active relay sessions, engine-side provider operations, fetched service entries, and platform bridge queues. |
| OPS-005 | 1439 | SUB-09 | MAPPED *(new 2026-08-23)* | Previously represented only by SUB-02, which covers provider conformance kits, not the twelve-item application-facing mandate, and not *"A test must be able to prove the mechanism it claims by disabling or mutating that mechanism and observing failure."* |
| OPS-006 | 1457 | NATIVE-03, NATIVE-04 | MAPPED | |
| OPS-007 | 1472 | NATIVE-08 | MAPPED, tightened 2026-08-23 | Spec's *"Heuristic source-word matching is insufficient"* and the real per-platform inventory requirement were absent; NATIVE-08 now carries both. |
| OPS-008 | 1478 | NATIVE-01, NATIVE-02 | MAPPED | |
| OPS-009 | 1486 | READ-10, READ-22, NATIVE-07, OWN-05 | MAPPED *(owner named 2026-08-23)* | Spec: *"Opening, observing, cancelling, dropping, closing, backgrounding, foregrounding, and engine shutdown MUST each have one exact owner."* No requirement named an owner and no engine shutdown exists. OWN-05 names `fava-runtime`; READ-22 carries "shutdown ends all pending pulls". The existing evidence asserts `thread::current().id()` inside a current-thread runtime - an assertion that cannot fail. |
| OPS-010 | 1494 | NATIVE-05, NATIVE-06 | MAPPED | |
| OPS-011 | 1504 | NATIVE-09 | MAPPED *(new 2026-08-23)* | Previously UNMAPPED. NATIVE-07 covers lifecycle envelopes, not measured performance bounds. No benchmark harness exists anywhere in the repository. |

### Part IX - Product profiles and declared guarantees

| Spec ID | Line | `.planning` counterpart | Status | Note |
|---|---|---|---|---|
| PROFILE-001 | 1523 | PROF-09 | MAPPED, tightened 2026-08-23 | PROF-09 now requires an explicit named assembly **document**. No profile document exists. |
| PROFILE-002 | 1540 | PROF-02 | MAPPED | No persistent event cache implementation exists. |
| PROFILE-003 | 1553 | PROF-03 | MAPPED | |
| PROFILE-004 | 1567 | WRITE-03, PROF-09 | WEAKENED | Spec: *"Memory write stores may exist for deterministic tests or deliberately non-production profiles, but they do not satisfy the standard durable-write product claim."* WRITE-03 states the durability boundary; nothing forbids presenting a memory write store as the standard durable profile except an M5 exit gate, which is not a requirement. |
| PROFILE-005 | 1574 | WRITE-12, SUB-03, PROF-05 | MAPPED | |
| PROFILE-006 | 1582 | SUB-01 | MAPPED | |
| PROFILE-007 | 1588 | - | UNMAPPED | The recommended full-client assembly, named explicitly rather than implied by facade defaults. `builder()` requires explicit selection, but no named assembly and no document exist. |
| PROFILE-008 | 1607 | WRITE-10, WRITE-23 | WEAKENED, tightened 2026-08-23 | Spec: one bounded policy across **seven** named terminal outcomes, oldest retired first, *"Active writes are never evicted by this policy."* WRITE-10 now names oldest-first and the active-write exclusion. Oldest-first is genuinely evidenced; "active writes are never evicted" is unproven - both receipts in the test are terminal - and no `Superseded` outcome exists in `ReceiptOutcome` at all. |

### Part XI - Open product decisions

All five are correctly handled: unpromised, tracked, with owning phases named, and `06-VERIFICATION.md`
correctly refused to convert them into gaps. This is the one part of the process that worked as designed.

| Spec ID | Line | `.planning` counterpart | Status | Note |
|---|---|---|---|---|
| OPEN-001 | 1646 | Deferred decision 1 | DEFERRED | Public windowing/growable-window API and resume-token model. |
| OPEN-002 | 1650 | Deferred decision 2 | DEFERRED | Cancellation semantics after partial relay handoff. Satisfied by absence; no test asserts no "unsend" path exists. |
| OPEN-003 | 1654 | Deferred decision 3, READ-14 | DEFERRED | Whether any profile promises outage-interval backfill. READ-14 correctly refuses the implication. |
| OPEN-004 | 1658 | Deferred decision 4 | DEFERRED | Retention of full historical attempt detail. |
| OPEN-005 | 1662 | Deferred decision 5 | DEFERRED | Which persistent event-cache guarantee profile is recommended. |

---

## Checkmark reset ledger

**80 checkmarks were removed on 2026-08-23: 75 v1 requirements and 5 M0 baseline claims.**
Eleven survive. The rule applied was gate 11 plus the audit's evidence classification: a checkmark
is defensible only if its proof (a) is not self-authored - the cited `docs/issues/000N` record's
first commit is not the implementation commit it verifies - (b) does not run in a regime where the
property cannot fail, and (c) can distinguish a correct implementation from the current one.

### Checkmarks retained, and why they are defensible

| ID | Why it survives |
|---|---|
| ROUTE-11, WRITE-23 | `crates/fava-routing/src/chain.rs:428-442` emits typed refusals carrying exact numbers. Executable, discriminating, and consumes no fixture-supplied fact. The routing-core policy-isolation negative test at `chain.rs:445-462` is real, though it scans `lib.rs` and `Cargo.toml` rather than `chain.rs`. |
| CAP-01 through CAP-09 | Phase 7 is the one phase that did the job: `07-VERIFICATION.md` distinguishes `implementation_head` (`f97ecd8`) from `verified_head` (`1dd7e5e`), states the verifier **reran** all four CLI canaries rather than citing preserved bundles, and resolved a PLAN-versus-authority conflict in favour of the authority. |

### Class 1 - evidence authored by the change it verifies (64 reset of 66 M1-M6 requirements)

For M2 through M6, the cited `docs/issues/000N` record's **first and only commit is the
implementation commit**. The document that is supposed to be the independent red record was created
by the change it certifies. `02-VERIFICATION.md` and `03-VERIFICATION.md` state verbatim: *"External
scenarios were inspected, not rerun, during this reconciliation."* All six records were backfilled at
`da8db46`, hours after their milestones shipped.

| Requirements | Reset reason (one line each, by phase) |
|---|---|
| LOCAL-01 … LOCAL-12 | Phase 1 verdict revoked: evidence issue `0001` was rewritten inside the M1 implementation commit, and LOCAL-07 is directly falsified by the project's own map (cross-source deletion and due-time expiry do not retract). |
| READ-01 … READ-10 | Phase 2 verdict revoked: evidence issue `0004`'s only commit **is** `7fac920`; the diagnostics-agreement gate is unverifiable because the M2 run bundles do not exist; the plan's named relay-facing `fava-observe` slice was never built. |
| READ-11 … READ-20 | Phase 3 verdict revoked: the 1,000-observation exit gate is met by a cache-only thread-identity assertion against a declared profile that does not exist. |
| ROUTE-01 … ROUTE-10 | Phase 4 downgraded: three of four gates genuinely hold, but evidence issue `0006`'s first commit is `9860711`, and the planner-substitution gate cannot be true as built because the desired plan is computed inside the facade. |
| WRITE-01 … WRITE-11 | Phase 5 verdict revoked: the "process-kill tests at **every** commit/effect boundary" gate is unmet - redb terminal eviction has a known memory-versus-durable divergence and restart parity after eviction is an untested High-priority gap. |
| WRITE-12 … WRITE-22 | Phase 6 verdict revoked: the "independent wire transcripts through real relay processes" gate is unverifiable - the cited preserved M6 bundles are not in the repository and not on disk. |

### Class 2 - proof bypasses the regime where the property could fail (5 requirements)

These five are also in Class 1. Class 1 explains why their verdict is void; Class 2 explains why the
requirement itself was written so it could not fail. Both had to be fixed, and the requirement text
above is rewritten in each case.

| ID | Reset reason |
|---|---|
| LOCAL-08 | Assigned to M1, whose own exit gate forbids any networking dependency. The requirement was parked in the one milestone structurally incapable of falsifying it, and its wording was narrowed to *"a **local** query"* to fit. |
| LOCAL-09 | Only the identity half of QUERY-002 was kept, and it was kept because it was the half M1 could satisfy. Two equivalent handles open two sessions and send two REQs today, and nothing objected. |
| READ-02 | Kept only the first sentence of QUERY-013. The conjunction with QUERY-004 - relay work starts **while** the local view returns - was nobody's requirement. |
| READ-20 | All 1,000 observations are cache-only. Zero relay sessions, zero subscriptions, zero descriptors, in the milestone titled *Multi-Relay Reactivity and Bounded Observation*. Its subject, `fava-standard`, does not exist. |
| WRITE-04 | The deadline was loosened from *"before `Accepted` is returned"* to *"before relay acknowledgement"*, moving the boundary past the point where it could fail. |

### Class 3 - evidence cannot distinguish a correct implementation from the current one (12 requirements)

Also a subset of Class 1. Listed separately because these would still fail gate 11 even if their
verification records had been independent.

Drawn from the evidence audit's 39 non-distinguishing findings, restricted to requirements that were
marked complete.

| ID | Reset reason |
|---|---|
| LOCAL-02, READ-05 | Graded SATISFIED while the project's own map recorded that the event-cache mutation contract exposes an admission bypass through which fabricated relay evidence becomes query-visible signed state. |
| LOCAL-04, READ-15 | The historical relay echo test used two direct cache commits, and its source-authority distinction came from fixture-written provenance rather than admitted relay occurrences. This finding is superseded by STATE-ARCH-1 and is retained only as audit history. |
| READ-03 | Graded SATISFIED for "bounded NIP-01 wire" while the configured inbound frame bound is unenforced - `crates/fava-transport-websocket/src/lib.rs:110` bounds outbound only. |
| READ-16, LOCAL-10 | The only coalescing assertion is `coalesced_query_updates > 0` - Fava's own counter, and `> 0` cannot fail. |
| READ-18 | The surface is a watch channel with no pull protocol; there is no backlog that could accumulate, so the assertion is vacuous. |
| ROUTE-03 | Graded "delayed router cannot block already-known relay work" while the map records that one slow relay open delays every later relay and can prevent the initial handle from returning. |
| ROUTE-10 | Grouping equivalence has **no evidence of any kind**; both planners are tested in isolation on hand-built demands, and the named deliberate break describes behavior the implementation already exhibits, so the break cannot fail. |
| WRITE-08, WRITE-20 | `OutcomeUnknown` is supplied by the test publisher; the redb boundary assertion iterates a destination map that is empty, so it is vacuously true. |
| WRITE-22 | The preview-versus-real comparison is against the same planner call that produced the plan. |

### Class 4 - verdict issued without a verification record (12 requirements, 11 reset)

| ID | Reset reason |
|---|---|
| GROUP-01 … GROUP-12 | *(GROUP-12 was already unchecked; the other eleven are reset here.)* Phase 07.1.1 has no `VERIFICATION.md`. Its only requirement-level verdict is a validation table the executing plan lists among its own modified files, its coverage record is one sentence denying an external integration the phase's own GROUP-12 requires, and eighty-four subsequent changes include behavioral fixes to GROUP-04, GROUP-07, GROUP-08, and GROUP-10 with no re-verification. |

### Class 5 - baseline evidence absent from the repository (5 claims)

| ID | Reset reason |
|---|---|
| M0-01 … M0-05 | Every cited external bundle is written to an ignored path and is absent from disk; the acceptance application that was to be the independent witness linked nine Fava internal crates and hard-coded its own equivalence verdict into the manifest it retained. It has since been removed from the repository entirely. |

### Phantom requirements resolved

`R1` through `R9` were listed as Phase 07.1's requirements in `.planning/ROADMAP.md:243`, graded
nine-for-nine `VERIFIED` in `07.1-VERIFICATION.md:11`, and **existed in no registry**. They are
resolved by registration as `NIP02-01` through `NIP02-09`, all unchecked, because their sole external
witness does not exist. **The identifiers `R1`-`R9` are void.** Any future requirement that appears in
a ROADMAP or VERIFICATION file without a row in this document is a process failure, and gate 10 now
makes it one.

### Duplicate mappings resolved

The previous coverage block asserted *"Duplicate mappings: 0"*. That was false: `LOCAL-09` and
`ROUTE-10` each had two owning phases and two independent `SATISFIED` verdicts - Phase 1 / Phase 4 in
this file, and Phase 06.1 in `ROADMAP.md:214` and `06.1-VERIFICATION.md:151-154`. `LOCAL-09` now has
one owner (Phase 3, where its relay conjuncts can fail) and `ROUTE-10` has one owner (Phase 06.1,
where its literal-tag-value semantics were actually remediated).

---

## Traceability to phases

Every v1 requirement maps to exactly one owning phase. `Complete` denotes an evidence-backed owning
verdict that survives the 2026-08-23 audit; there are eleven. `Pending` covers everything else,
including work whose code exists but whose proof does not. Reassignments made on 2026-08-23 are
marked, and each moved because the requirement had been parked where it could not fail.

| Requirement | Phase | Status |
|-------------|-------|--------|
| LOCAL-01 | Phase 1 | Pending |
| LOCAL-02 | Phase 1 | Pending |
| LOCAL-03 | Phase 1 | Pending |
| LOCAL-04 | Phase 1 | Pending |
| LOCAL-05 | Phase 1 | Pending |
| LOCAL-06 | Phase 1 | Pending |
| LOCAL-07 | Phase 1 | Pending |
| LOCAL-08 | Phase 2 *(was Phase 1)* | Pending |
| LOCAL-09 | Phase 3 *(was Phase 1)* | Pending |
| LOCAL-10 | Phase 1 | Pending |
| LOCAL-11 | Phase 1 | Pending |
| LOCAL-12 | Phase 1 | Pending |
| READ-01 | Phase 2 | Pending |
| READ-02 | Phase 2 | Pending |
| READ-03 | Phase 2 | Pending |
| READ-04 | Phase 2 | Pending |
| READ-05 | Phase 2 | Pending |
| READ-06 | Phase 2 | Pending |
| READ-07 | Phase 2 | Pending |
| READ-08 | Phase 2 | Pending |
| READ-09 | Phase 2 | Pending |
| READ-10 | Phase 2 | Pending |
| READ-11 | Phase 3 | Pending |
| READ-12 | Phase 3 | Pending |
| READ-13 | Phase 3 | Pending |
| READ-14 | Phase 3 | Pending |
| READ-15 | Phase 3 | Pending |
| READ-16 | Phase 3 | Pending |
| READ-17 | Phase 3 | Pending |
| READ-18 | Phase 3 | Pending |
| READ-19 | Phase 3 | Pending |
| READ-20 | Phase 3 | Pending |
| READ-21 | Phase 2 *(new)* | Pending |
| READ-22 | Phase 3 *(new)* | Pending |
| READ-23 | Phase 3 *(new)* | Pending |
| OWN-01 | Phase 1 (extended Phase 2) *(new)* | Pending |
| OWN-02 | Phase 2 *(new)* | Pending |
| OWN-03 | Phase 2 *(new)* | Pending |
| OWN-06 | Phase 2 *(new)* | Pending |
| OWN-04 | Phase 3 *(new)* | Pending |
| OWN-05 | Phase 3 (hardened Phase 8) *(new)* | Pending |
| OWN-07 | Phase 8 *(new)* | Pending |
| OWN-08 | Every phase *(new)* | Pending |
| ROUTE-01 | Phase 4 | Pending |
| ROUTE-02 | Phase 4 | Pending |
| ROUTE-03 | Phase 4 | Pending |
| ROUTE-04 | Phase 4 | Pending |
| ROUTE-05 | Phase 4 | Pending |
| ROUTE-06 | Phase 4 | Pending |
| ROUTE-07 | Phase 4 | Pending |
| ROUTE-08 | Phase 4 | Pending |
| ROUTE-09 | Phase 4 | Pending |
| ROUTE-10 | Phase 06.1 *(was Phase 4; duplicate mapping resolved)* | Pending |
| ROUTE-11 | Phase 4 | Complete |
| WRITE-01 | Phase 5 | Pending |
| WRITE-02 | Phase 5 | Pending |
| WRITE-03 | Phase 5 | Pending |
| WRITE-04 | Phase 5 | Pending |
| WRITE-05 | Phase 5 | Pending |
| WRITE-06 | Phase 5 | Pending |
| WRITE-07 | Phase 5 | Pending |
| WRITE-08 | Phase 5 | Pending |
| WRITE-09 | Phase 5 | Pending |
| WRITE-10 | Phase 5 | Pending |
| WRITE-11 | Phase 5 | Pending |
| WRITE-12 | Phase 6 | Pending |
| WRITE-13 | Phase 6 | Pending |
| WRITE-14 | Phase 6 | Pending |
| WRITE-15 | Phase 6 | Pending |
| WRITE-16 | Phase 6 | Pending |
| WRITE-17 | Phase 6 | Pending |
| WRITE-18 | Phase 6 | Pending |
| WRITE-19 | Phase 6 | Pending |
| WRITE-20 | Phase 6 | Pending |
| WRITE-21 | Phase 6 | Pending |
| WRITE-22 | Phase 6 | Pending |
| WRITE-23 | Phase 6 | Complete |
| CAP-01 | Phase 7 | Complete |
| CAP-02 | Phase 7 | Complete |
| CAP-03 | Phase 7 | Complete |
| CAP-04 | Phase 7 | Complete |
| CAP-05 | Phase 7 | Complete |
| CAP-06 | Phase 7 | Complete |
| CAP-07 | Phase 7 | Complete |
| CAP-08 | Phase 7 | Complete |
| CAP-09 | Phase 7 | Complete |
| NIP02-01 | Phase 07.1 *(new)* | Pending |
| NIP02-02 | Phase 07.1 *(new)* | Pending |
| NIP02-03 | Phase 07.1 *(new)* | Pending |
| NIP02-04 | Phase 07.1 *(new)* | Pending |
| NIP02-05 | Phase 07.1 *(new)* | Pending |
| NIP02-06 | Phase 07.1 *(new)* | Pending |
| NIP02-07 | Phase 07.1 *(new)* | Pending |
| NIP02-08 | Phase 07.1 *(new)* | Pending |
| NIP02-09 | Phase 07.1 *(new)* | Pending |
| GROUP-01 | Phase 07.1.1 | Pending |
| GROUP-02 | Phase 07.1.1 | Pending |
| GROUP-03 | Phase 07.1.1 | Pending |
| GROUP-04 | Phase 07.1.1 | Pending |
| GROUP-05 | Phase 07.1.1 | Pending |
| GROUP-06 | Phase 07.1.1 | Pending |
| GROUP-07 | Phase 07.1.1 | Pending |
| GROUP-08 | Phase 07.1.1 | Pending |
| GROUP-09 | Phase 07.1.1 | Pending |
| GROUP-10 | Phase 07.1.1 | Pending |
| GROUP-11 | Phase 07.1.1 | Pending |
| GROUP-12 | Phase 07.1.1 | Pending |
| SESSION-01 | Phase 07.2 | Pending |
| SESSION-02 | Phase 07.2 | Pending |
| SESSION-03 | Phase 07.2 | Pending |
| SESSION-04 | Phase 07.2 | Pending |
| SESSION-05 | Phase 07.2 | Pending |
| SESSION-06 | Phase 07.2 | Pending |
| SESSION-07 | Phase 07.2 | Pending |
| HARD-01 | Phase 8 | Pending |
| HARD-02 | Phase 8 | Pending |
| HARD-03 | Phase 8 | Pending |
| HARD-04 | Phase 8 | Pending |
| HARD-05 | Phase 8 | Pending |
| HARD-06 | Phase 8 | Pending |
| HARD-07 | Phase 8 | Pending |
| HARD-08 | Phase 8 | Pending |
| HARD-09 | Phase 8 | Pending |
| HARD-10 | Phase 8 | Pending |
| PROF-01 | Phase 9 | Pending |
| PROF-02 | Phase 9 | Pending |
| PROF-03 | Phase 9 | Pending |
| PROF-04 | Phase 9 | Pending |
| PROF-05 | Phase 9 | Pending |
| PROF-06 | Phase 9 | Pending |
| PROF-07 | Phase 9 | Pending |
| PROF-08 | Phase 9 | Pending |
| PROF-09 | Phase 9 | Pending |
| SUB-01 | Phase 10 | Pending |
| SUB-02 | Phase 10 | Pending |
| SUB-03 | Phase 10 | Pending |
| SUB-04 | Phase 10 | Pending |
| SUB-05 | Phase 10 | Pending |
| SUB-06 | Phase 10 | Pending |
| SUB-07 | Phase 10 | Pending |
| SUB-08 | Phase 10 | Pending |
| SUB-09 | Phase 10 *(new)* | Pending |
| NATIVE-01 | Phase 11 | Pending |
| NATIVE-02 | Phase 11 | Pending |
| NATIVE-03 | Phase 11 | Pending |
| NATIVE-04 | Phase 11 | Pending |
| NATIVE-05 | Phase 11 | Pending |
| NATIVE-06 | Phase 11 | Pending |
| NATIVE-07 | Phase 11 | Pending |
| NATIVE-08 | Phase 11 | Pending |
| NATIVE-09 | Phase 11 *(new)* | Pending |

---

## Coverage

Measured against the authority, not against itself. The previous coverage block read
*"v1 requirements: 129 total / Mapped to phases: 129 / Unmapped: 0 / Duplicate mappings: 0"* - a
tautology that measured the corpus against the corpus, was false on duplicates, and reported perfect
coverage while 113 of 131 spec requirements were unrepresented.

**Spec coverage (`docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md`, 131 requirements)**

| Result | Count | Share |
|---|---:|---:|
| MAPPED - full conjunction carried | 68 | 52% |
| WEAKENED - counterpart exists, a clause is dropped or loosened | 26 | 20% |
| UNMAPPED - no counterpart of any kind | 31 | 24% |
| DEFERRED - explicit unpromised product decision | 6 | 5% |
| **Total** | **131** | **100%** |

Every one of the 131 has a row. Before 2026-08-23, 113 of them appeared nowhere in `.planning/` at all.

**Requirement corpus**

| Measure | Count |
|---|---:|
| v1 requirements | 151 |
| Mapped to exactly one owning phase | 151 |
| Unmapped to a phase | 0 |
| Duplicate phase mappings | 0 (two resolved: LOCAL-09, ROUTE-10) |
| Checked (evidence defensible under gates 10 and 11) | 11 |
| Unchecked | 140 |
| Reset from checked to unchecked on 2026-08-23 | 80 (75 v1 + 5 M0) |
| Requirements added on 2026-08-23 | 22 (OWN-01..08, NIP02-01..09, READ-21..23, SUB-09, NATIVE-09) |
| Requirements rewritten to restore a lost conjunction | 5 (LOCAL-08, LOCAL-09, READ-02, READ-20, WRITE-04) |
| Phantom identifiers voided | 9 (R1-R9) |

**Independent evidence coverage, from the 2026-08-23 evidence audit:** of 131 spec requirements,
56 proven, 39 weak or non-distinguishing, 36 with no evidence at all. `OPS` is 0 of 11 proven, `ID`
is 1 of 8, `PROFILE` is 1 of 8. Zero coverage workspace-wide for shutdown join, shared-work refcount,
slow-peer backpressure, blocked-provider isolation, and provider panic outside a single applier.
And because continuous integration runs no Rust tests, **none of the 56 proven requirements is
protected against regression by anything except a person remembering to run them.**

---

## v2 Requirements

None. M1-M11 together define the first Fava release; normative work is not silently deferred to an
unspecified later version. The 31 UNMAPPED spec requirements above are v1 obligations with no plan,
not v2 candidates.

## Decisions Deferred Within v1

These remain explicitly unpromised unless resolved in their owning milestones. This is the one part
of the process that worked as designed: they are tracked consistently across this file, `ROADMAP.md`,
and `STATE.md`, and `06-VERIFICATION.md` correctly refused to convert them into gaps.

1. Public growable-window/query API and resume-token model. *(OPEN-001, QUERY-017)*
2. Cancellation semantics after partial relay handoff. *(OPEN-002)*
3. Whether any profile promises outage-interval backfill. *(OPEN-003)*
4. Retention of full historical attempt detail beyond exact current receipt evidence. *(OPEN-004)*
5. Which persistent event-cache guarantee profile is recommended for the primary shipped artifact. *(OPEN-005)*

## Out of Scope

A row in this table is a **non-feature note**. It is not coverage, and it never satisfies a spec
requirement. Two normative MUST NOTs - QUERY-009 (no global completeness claim) and RELAY-011 (no
automatic negentropy) - were previously demoted into this table and thereby removed from the testable
corpus. They are now recorded as UNMAPPED in the traceability matrix, which is what they are.

| Feature | Reason |
|---------|--------|
| Previous NMP implementation code or compatibility paths | Fava is a clean-room rewrite from authoritative documents |
| Application framework, UI state, navigation, ranking, moderation, and account UX | These remain application-owned product concerns |
| Runtime plugin discovery or hot-swappable providers | Provider selection is explicit static composition for an engine instance |
| Global synced, complete, authoritative-empty, percentage, or end-of-history claims | Relay evidence is exact and source-scoped. **Note: QUERY-009 is a normative MUST NOT and needs a requirement, not this row.** |
| Automatic negentropy or a parallel history workload | **Note: RELAY-011 is a normative MUST NOT and needs a requirement, not this row.** |
| Unsigned or unpublished local events in the event cache | The write store is their independent query authority |
| Silent truncation, fallback, compatibility, clamping, or hidden feature flags | Bounds, refusal, policy, and shortfall must remain explicit |
| Provider-specific private facade bypasses | Replaceability requires identical public contracts and conformance paths |
| Cross-provider persisted-format compatibility by default | Each provider owns its private bytes and migration/refusal behavior |
| Public-relay availability as a deterministic release gate | Controlled real third-party relay processes own repeatable pass/fail evidence |

---

## Process gates this document now depends on

The audit proposed four executable process gates. Until they exist, this document is enforced by
attention alone, which is the condition that produced the previous version.

| Gate | What it asserts | Fails today for |
|---|---|---|
| `tools/check_requirement_traceability.py` | Every `^## [A-Z]+-[0-9]+[A-Z]?` heading in the goals spec appears in this file's traceability matrix | 0 rows - satisfied as of 2026-08-23, and it should be wired into CI before that changes |
| `tools/check_requirement_provenance.py` | For every requirement marked Complete, this file's history predates the earliest commit cited as its evidence | The 66 M1-M6 requirements, which is why they are now unchecked |
| `tools/check_evidence_reachable.py` | Every artifact and issue commit cited by a `*-VERIFICATION.md` resolves, is tracked, and is not the implementation commit | 8 of 9 verification records |
| `tools/check_planning_consistency.py` | Completed-phase counts, progress table rows, handoff phase, and active-item lists agree across `STATE.md`, `ROADMAP.md`, `PROJECT.md`, and `HANDOFF.json` | All four; `STATE.md` alone reports three different denominators and two different percentages |

A fifth gate the audit implies and this file assumes: block a `status: passed` verification for phase
N while `.planning/codebase/CONCERNS.md` records any Known Bug or High-priority coverage gap whose
files intersect the crates that phase owns. On 2026-08-21 the codebase map recorded eight known bugs
and five High-priority gaps at 08:44:48, and the reconciliation twenty seconds later at 08:45:08
declared "No M1 gaps remain", "No M3 gaps remain", "No M5 gaps remain". The map was right. The
verdict was written anyway.

---
*Requirements first defined: 2026-08-21, derived from finished code.*
*Corpus rebuilt spec-derived: 2026-08-23, from `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md`.*
*Basis: `.planning/audit/2026-08-23/requirements-process.md`, `.planning/audit/2026-08-23/evidence.md`, and the process addendum in `.planning/audit/2026-08-23/LEDGER.md`.*
