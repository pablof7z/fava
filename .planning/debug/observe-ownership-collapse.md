---
status: investigating
trigger: "not just bugs, a complete deviation and complete ignoring the architecture that I carefully described! you piece of fucking shit! this is insaen!"
created: 2026-08-23T12:32:58Z
updated: 2026-08-23T12:50:21Z
---

## Current Focus

hypothesis: Confirmed — the implementation deviation began in M2 commit 7fac920 despite both the authoritative owner ledger and repository-visible GSD research already assigning relay-facing observation work to fava-observe. M3/M4 propagated that facade owner, and the later GSD requirements/reconciliation/verification process ratified completion records instead of re-proving the cross-phase owner, opening-order, sharing, vocabulary, and public-composition invariants.
test: Completed git ancestry/text comparison across the authority documents, M2-M4 issues/code/evidence, GSD initialization/research, post-M6 codebase remap, requirements, and backfilled verification; independently checked the vocabulary gate against M3's private OpenedRelay lifecycle owner.
expecting: Confirmed. M2-M4 followed GSD initialization/research but preceded GSD phase plans and phase verification artifacts. The backfill calls them "pre-gsd" only in the narrower phase-artifact sense, checks implementation-shaped requirements, ignores a mapper-recorded handle-blocking defect, and declares no gaps.
next_action: Return the implementation plus provenance/process root-cause finding to the debug session manager; do not modify production code or request fix approval.
bug_class: bohrbug
reasoning_checkpoint:
  hypothesis: "The facade-owned synchronous per-observation/per-relay establishment path causes the local handle delay, duplicate sessions, leaked provisional work, planner bypass, and cross-relay blocking because no installed fava-observe owner exists to retain logical demand and execute provider work independently."
  confirming_evidence:
    - "PendingTransport deterministically leaves Fava::observe pending past the controlled deadline after local sources are coherent."
    - "Two equivalent public observations deterministically call open_session twice and send two REQs."
    - "Cancelling a two-relay observe while the second open is pending leaves the first provisional session unclosed."
    - "fava-observe has no registry, route, demand, shared-work, desired-plan, session, or relay-cancellation state, while fava facade modules contain all of those effects."
  falsification_test: "If current fava-observe already owned installed live demand, then a pending transport would not delay handle return, equivalent handles would reuse one owned relay work item, and dropping a provisional open would close every established child; all three observations are false."
  fix_rationale: "Move lifecycle authority—not only functions—into fava-observe: install the local observation first, retain route-bound logical demand in an owner registry, aggregate/diff desired subscription plans per RelaySessionKey, execute provider calls through bounded cancellable runtime work, and let transport own session/reconnect mechanics. The facade then only delegates and orders construction/shutdown."
  blind_spots: "The deterministic tests do not yet cover a pending send, provider panic, automatic-route expansion with a blocked added relay, native SDK cancellation, or real shutdown; these remain required fix-acceptance signals."
  candidate_causes:
    - "code: commits 7fac920/1f2c0ed/9860711 assigned and expanded the wrong lifecycle owner, with one private relay session per observation."
    - "environment: an indefinitely pending DNS/TCP/TLS/WebSocket/provider future exposes the sequencing defect but is not necessary for duplicate-work reproduction."
    - "config: no selected runtime execution owner or declared global/explicit relay-work bound exists in the assembled profile."
    - "data: equivalent concurrent observations and multi-relay inputs expose missing aggregation/refcount and serial head-of-line blocking but do not create them."
  and_gate: "no — the code ownership collapse alone reproduces duplicate sessions under an immediate successful transport; network delay, configuration, and input cardinality only expose additional consequences."
tdd_checkpoint: null

## Symptoms

expected: An application observation is established from coherent local sources and is separated from physical relay connectivity by installed observation ownership, route-plan binding, logical per-relay demand, shared-work ownership/refcounts, subscription planning and diffing, and asynchronous transport/session execution.
actual: Fava::observe constructs a local Observation but withholds it while facade-owned code directly plans one query, opens each initial relay session serially, sends REQ frames, and only then returns the handle; automatic initial routes follow the same synchronous execution shape.
errors: No typed error is required; a pending DNS, TCP, TLS, WebSocket, transport-open, or handoff future can keep observe pending. External timeout cancels the open instead of yielding the local observation.
reproduction: Open an explicit or automatically routed live query whose initial RelaySessionKey is served by a Transport with a pending open_session future, or by a WebSocket peer that accepts TCP and stalls the handshake.
started: Evidence indicates the direct synchronous shape was introduced in M2 commit 7fac920 and expanded to multi-relay and automatic routing without ever satisfying the authoritative observation ownership/open sequence.

## Eliminated

## Evidence

- timestamp: 2026-08-23T12:37:11Z
  checked: Phase-0 semantic and durable debug knowledge recall
  found: MemPalace is unavailable and .planning/debug/knowledge-base.md does not exist.
  implication: No known-pattern candidate exists; investigation proceeds from authoritative specs and direct code behavior.

- timestamp: 2026-08-23T12:37:11Z
  checked: Authoritative observation ownership and opening sequence in FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md and ARCHITECTURE.md
  found: QUERY-003/004 require a coherent local handle without waiting for a relay; fava-observe owns observation identity, route binding, logical per-relay demand, shared-work refcounts, desired subscription plans, bounded delivery, and teardown; the facade is explicitly thin and owns no socket state or query evaluation.
  implication: A transport timeout cannot satisfy the contract because the invalid ownership and release ordering precede transport policy.

- timestamp: 2026-08-23T12:37:11Z
  checked: crates/fava/src/lib.rs, live.rs, routes.rs, and relay.rs
  found: Fava stores Observer, EventCache, SubscriptionPlanner, Transport, routers, and the global subscription counter; live.rs/routes.rs create the local Observation then await OpenedRelay::open; relay.rs allocates IDs, calls the planner, awaits open_session and send, owns reconnect with a fixed 50 ms loop, admits inbound events, and performs withdrawal.
  implication: The facade has absorbed observation, shared-session, subscription execution, reconnect, ingest dispatch, and cancellation sequencing that the architecture assigns to fava-observe, Transport, fava-ingest, and fava-runtime.

- timestamp: 2026-08-23T12:37:11Z
  checked: crates/fava-observe/Cargo.toml and src/lib.rs
  found: fava-observe depends only on fava-query, thiserror, and tokio; its state contains only two local QuerySource observations, evaluator delivery, and an arbitrary Vec of cancellation senders. It has no ObservationId, registry, route session, RelayDemand, desired SubscriptionPlan, shared-work identity/refcount, session fact handling, or relay cancellation owner.
  implication: The specified universal observation owner is structurally incapable of owning the current live-read lifecycle; its public Observation is only a local-source handle with facade-attached side work.

- timestamp: 2026-08-23T12:37:11Z
  checked: fava-subscriptions contract/implementations and facade call sites
  found: Each OpenedRelay plans exactly one newly allocated demand for exactly one query. There is no retained per-relay demand set across observations, no current desired-plan diff, no observation/branch/bounds identity, no relay-constraint input, and no SubscriptionShortfall in SubscriptionPlan. The facade's validate_plan imposes private shape assumptions.
  implication: Planner grouping across logically separate observations, planner substitution through the public assembly, precise withdrawal/refcounting, relay-limit shortfall propagation, and unchanged-plan preservation cannot be implemented by the current call shape.

- timestamp: 2026-08-23T12:37:11Z
  checked: current public live-query tests and architecture falsifier inventory
  found: Every public live test uses an immediately successful scripted Transport; facade tests use only the no-grouping planner. No test holds open_session or send pending, cancels opening, proves two equivalent handles share work, groups independent demand through the standard planner, injects a blocking/panicking/late provider, or substitutes an external planner/transport through the complete observation path.
  implication: Existing evidence proves the happy-path wire behavior while leaving the architecture's decisive ownership, isolation, boundedness, cancellation, and replaceability falsifiers unexercised.

- timestamp: 2026-08-23T12:40:38Z
  checked: focused public regression relay_establishment_does_not_delay_the_coherent_local_observation
  found: A Transport whose open_session future never resolves caused Fava::observe to exceed the controlled 50 ms deadline; the assertion failed with Elapsed even though both local QuerySource snapshots and their empty coherent QuerySnapshot were already available.
  implication: QUERY-004 and the fava-observe handle-release ordering are deterministically violated. A connect timeout would merely turn the hang into a delayed network-caused open refusal, which remains nonconforming.

- timestamp: 2026-08-23T12:40:38Z
  checked: focused public regression equivalent_observations_share_relay_work_until_the_last_handle_closes
  found: Two equivalent explicit observations against one RelaySessionKey caused two Transport::open_session calls and two REQ handoffs; the required shared-work assertion observed 2 instead of 1.
  implication: fava-observe has no equivalent-query registry/refcount or shared relay-session demand owner, so QUERY-002 acceptance and the architecture's shared-work ownership are absent rather than merely faulty.

- timestamp: 2026-08-23T12:40:38Z
  checked: focused public regression cancelling_observe_while_another_relay_opens_closes_provisional_work
  found: With relay A established and relay B's open pending, cancelling the observe future by deadline left relay A's provisional RelaySession open; the close-state assertion was false.
  implication: Direct synchronous establishment creates ownerless partial work on cancellation, violating QUERY-003 all-or-nothing opening, exact teardown, provider cancellation, and failure isolation.

- timestamp: 2026-08-23T12:40:38Z
  checked: git history for crates/fava/src/live.rs, relay.rs, routes.rs and milestone issue documents
  found: Commit 7fac920 introduced facade-owned planner/transport/open/send; 1f2c0ed expanded it to serial multi-relay OpenedRelay tasks and facade-owned reconnect; 9860711 attached automatic routing and serial add_relays. docs/issues/0004, 0005, and 0006 explicitly describe fava as opening/reconciling relay tasks and then mark M2-M4 complete.
  implication: This is a systematic implementation decision repeated across milestones, not a recent transport regression. The local issue scopes themselves contradicted the higher-authority architecture and prevented later slices from correcting the owner.

- timestamp: 2026-08-23T12:40:38Z
  checked: subscription-grouping-equivalence canary implementation and BDD evidence link
  found: apps/canary/src/grouping.rs manually builds 300 RelayDemand values, invokes planners directly, opens WebSocketTransport directly, admits events directly, and only afterward evaluates cache-only Fava observations. It never opens the 300 live Queries through Fava::observe or the selected planner in the assembled observation lifecycle.
  implication: The claimed planner-replaceability capstone bypasses the defective path. It proves the pure planner and manual executor, not that fava-observe owns aggregate logical demand or that planner substitution requires no observation/facade changes.

- timestamp: 2026-08-23T12:40:38Z
  checked: explicit and automatic relay boundedness plus runtime/shutdown ownership
  found: Query::from_relays/only_from_relays accept an unbounded externally supplied relay iterator; each relay becomes a separately spawned facade task. No fava-runtime crate/owner is present, Fava has no shutdown/join path, provider calls use direct awaits/spawns without panic isolation, and reconnect is an unbounded fixed-50-ms facade loop.
  implication: Route-contribution bounds and bounded watch delivery do not bound total explicit relay work, provider influence, retries, cancellation latency, or shutdown. OPS-004/009 and GOAL-008 are unproved and structurally unsatisfied.

- timestamp: 2026-08-23T12:40:38Z
  checked: diagnostics contract against architecture inputs
  found: DiagnosticsSnapshot retains bounded recent session/subscription facts but has no open observation identity/ownership, observation-to-route binding, logical demand, desired subscription-plan revision, shared-work refcount, source shortfall, or current provider-operation state.
  implication: The owner facts required to verify attribution, stale completion rejection, sharing, and bounded lifecycle termination are neither owned nor observable, so the missing architecture cannot be audited through current diagnostics.

- timestamp: 2026-08-23T12:43:54Z
  checked: spectrum-based fault localization eligibility
  found: The repository has passing and newly failing focused tests but no configured per-test coverage spectrum for this run; the failure is already localized by deterministic public falsifiers and git history.
  implication: SBFL was skipped with a logged reason; Bohrbug routing continued through deterministic reproduction, differential history, and direct owner-boundary tracing.

- timestamp: 2026-08-23T12:43:54Z
  checked: common bug-pattern scan and concurrency checklist
  found: Async/Timing initialization-order and State Management dual-owner patterns match. There is no lock deadlock; the deterministic wait is an awaited provider future in the public open sequence. Serial await order also produces head-of-line blocking across relays.
  implication: The bug class remains Bohrbug, not Heisenbug. Timing variability changes how long the defect is visible, not whether the ownership/order contract is violated.

- timestamp: 2026-08-23T12:43:54Z
  checked: QueryEvidence and router QuerySource implementation
  found: QueryEvidence contains only local source revisions/status. It cannot carry current route, relay-request EOSE/failure/auth, subscription shortfall, or shared-work facts. Separately, QuerySource for Fava returns a fabricated empty EventCache snapshot immediately and starts an asynchronous recursive Fava::observe task, even when coherent local state already exists.
  implication: The collapsed observation owner also severed route/relay facts from application snapshots and made router-owned acquisition begin from a false empty boundary; exact current evidence and continuous router input semantics are not implemented end to end.

- timestamp: 2026-08-23T12:43:54Z
  checked: adjacent existing test behavior after adding the three RED falsifiers
  found: Existing explicit event/EOSE/cancel, automatic immediate-route, reconnect-generation, 1,000-idle-observation, and slow-consumer tests all pass unchanged.
  implication: The new failures isolate missing architectural behavior rather than breaking the previously implemented successful-transport happy path.

- timestamp: 2026-08-23T12:50:21Z
  checked: repository chronology and ancestry for authority, GSD adoption, and M2-M4
  found: "[REPOSITORY-PROVED] Commit 74f5f94 established the authoritative specification on 2026-08-20. GSD-shaped repository artifacts then landed before M2: d72c7a8 codebase map, 446cc42 project initialization, ebe1beb config with research/plan-check/verifier enabled, a455869 vocabulary enforcement, and 81599c6 project research. Every one is an ancestor of M2 7fac920; M3 1f2c0ed and M4 9860711 follow linearly."
  implication: "M2-M4 do not predate GSD use in this repository. They predate only GSD phase plans, requirements/roadmap, and phase verification artifacts; the later `execution_origin: pre-gsd` label is therefore shorthand for pre-GSD-phase-artifacts, not evidence that GSD was absent."

- timestamp: 2026-08-23T12:50:21Z
  checked: authoritative text at the parent of M2 versus M2 commit 7fac920
  found: "[REPOSITORY-PROVED] ARCHITECTURE.md already said fava-observe owns live-query handles, route sessions, logical per-relay demand, shared-work refcounts, desired wire plans, opening order, and handle release before relay work; Transport owns relay sessions. The implementation plan called for the relay-facing portion of fava-observe. M2 instead added Fava fields for planner, transport, and subscription allocation, and fava/src/live.rs planned one demand, opened/sent on the session, then returned Observation. Its new issue explicitly states `fava opens explicit relay work and binds its cancellation to Observation`."
  implication: The exact introducing milestone/commit is M2/7fac920. The committed local issue did not refine an ambiguity; it reassigned an already-owned lifecycle contrary to the higher authority.

- timestamp: 2026-08-23T12:50:21Z
  checked: M2 decision rationale and evidence provenance
  found: "[REPOSITORY-PROVED] Commit 7fac920 contains production code, the already-complete local issue, BDD feature, public tests, and canary in one 4,083-line commit. Its commit message is only `Complete M2 explicit live queries`; the issue states the facade choice but gives no ownership rationale, contradiction record, or approved architecture issue. No M2 PLAN/SUMMARY/VERIFICATION artifact exists before it."
  implication: "No repository-proved motive exists. [INFERENCE] The most likely mechanism is that `explicit query routing through fava` and the vertical-public-facade rule were read as permission for the facade to own execution, while the adjacent authoritative phrase `relay-facing portion of fava-observe` and owner ledger were not enforced. The happy-path REQ/EOSE/CLOSE target then made direct synchronous assembly the shortest implementation. This explains the choice but is not a proved statement of intent."

- timestamp: 2026-08-23T12:50:21Z
  checked: propagation through M3 commit 1f2c0ed and M4 commit 9860711
  found: "[REPOSITORY-PROVED] M3 introduced fava::OpenedRelay, serially awaited one instance per relay, put planner/transport/cache/diagnostics/reconnect state in it, and described `one independently cancellable relay task per RelaySessionKey` as architecture. This contradicted pre-M2 GSD research requiring equivalent-demand sharing and warning against one task per handle. M4 then made fava::routes reconcile a private active-relay map and serially call OpenedRelay::open for initial and later routes; its issue explicitly says `fava reconciles route destinations with exact relay tasks`."
  implication: M3 converted the M2 shortcut into a reusable private lifecycle owner and reconnect mechanism; M4 treated that owner as the integration boundary, so routing and planning were attached to the wrong retained state instead of completing fava-observe.

- timestamp: 2026-08-23T12:50:21Z
  checked: GSD requirements synthesis and phase backfill after M6
  found: "[REPOSITORY-PROVED] REQUIREMENTS.md and ROADMAP.md were first committed only after M4-M6. Requirements placed all-or-nothing/no-relay-wait solely in LOCAL-08 and then verified it with local-only M1 tests; no M2 invariant reapplied it after networking. The authoritative QUERY-002 acceptance that two equivalent handles share work has no M2/M3 requirement. The ownership ledger is not a mapped requirement."
  implication: GSD requirement synthesis lost cross-phase invariants exactly where networking could falsify them. Passing the local tracer was incorrectly sufficient to check the no-network-wait property globally, and shared work disappeared from phase verification despite appearing in pre-M2 research.

- timestamp: 2026-08-23T12:50:21Z
  checked: post-M6 GSD codebase remap b184aae and reconciliation da8db46
  found: "[REPOSITORY-PROVED] The remap explicitly normalized a `Facade and Relay Coordination Layer` that binds handles to relay tasks, owns reconnect loops and route reconciliation, while separately naming fava-observe the universal observation owner. Its CONCERNS.md also records that serial OpenedRelay::open can prevent the initial observation handle from returning. The immediately following reconciliation says its authority/evidence are the already-complete issues/commits, backfills only minimum phase records, does not rerun external scenarios, and marks M2-M4 `passed` with `No gaps remain`."
  implication: The mapper exposed both the split ownership and the original hang mechanism, but reconciliation neither compared that state to the authoritative owner ledger nor treated the concern as phase-invalidating. It ratified the milestone ledger rather than independently verifying the milestone claims.

- timestamp: 2026-08-23T12:50:21Z
  checked: backfilled M2-M4 verification evidence quality
  found: "[REPOSITORY-PROVED] 02-VERIFICATION claims deterministic owned-resource release from successful open/close evidence but has no failure-during-open or pending-provider case. 03-VERIFICATION substitutes one-current-thread and coalescing tests for equivalent-work sharing and full resource bounds. 04-VERIFICATION accepts grouping equivalence even though the canary manually plans, transports, ingests, and only then evaluates cache-only Fava queries; it bypasses the assembled observation owner and selected planner path."
  implication: Verification checked the mechanisms the implementation already exposed. It did not run owner-level or public negative cases capable of distinguishing the authoritative architecture from the facade-owned alternative, so it produced false completion confidence.

- timestamp: 2026-08-23T12:50:21Z
  checked: M3 OpenedRelay against the vocabulary policy and checker at commit 1f2c0ed
  found: "[REPOSITORY-PROVED] M3 added `pub(super) struct OpenedRelay`, a nominal facade-local lifecycle owner/wrapper. AGENTS.md already required separate approval for a new lifecycle owner and for synonym/wrapper/adjective-qualified nouns. OpenedRelay has no vocabulary.toml entry and no separate approved architecture issue. The then-current and current checker regex recognizes only plain `pub struct|enum|trait|type`, not `pub(super)`; its tests cover public and specification symbols only. Running the current checker emits only the unrelated fava-canary diagnostic and no OpenedRelay diagnostic."
  implication: This is an independent gate-design failure: the policy covers private nominal lifecycle owners, but the executable vocabulary gate cannot see them. A green vocabulary check therefore ratified neither the name nor its ownership distinction.

## Provenance and Process RCA

### Repository-proved facts

| Stage | Authoritative expectation already present | Committed ownership / process result |
|---|---|---|
| Before M2 | `fava-observe` owns observation identity, route binding, logical per-relay demand, shared-work refcounts, desired plan, opening order, handle release, and later updates; Transport owns relay sessions; `fava` is thin. GSD research repeats this owner model and warns against facade/global ownership, blocking providers on owner tasks, self-proving capstones, and one task per handle. | GSD map/project/config/research existed, but there was no M2 phase plan or owner-led verification artifact. |
| M2 `7fac920` | The implementation plan names the relay-facing portion of `fava-observe` and explicit routing through `fava`; QUERY-003/004 require a coherent local handle or clean refusal with no wait on relay connectivity. | One atomic completion commit makes `fava` plan, allocate, open, send, spawn, ingest-dispatch, and bind cancellation before returning the handle. The same commit's local issue declares that facade ownership complete. |
| M3 `1f2c0ed` | M3 requires completion of `fava-observe`, multi-session transport/runtime, equivalent-demand sharing, bounded cancellation/race/resource envelopes, and avoidance of one task per handle. | `fava::OpenedRelay` becomes one private task/lifecycle wrapper per query-relay pair, owns reconnect and planner execution, and is introduced without vocabulary approval. |
| M4 `9860711` | Routing owns route contributions; `fava-observe` owns retained logical demand and desired subscription plans; planner is pure; transport executes deltas. | `fava::routes` owns the active relay-task map and serial reconciliation. Planner evidence is pure/manual and the real-relay grouping canary bypasses public observation assembly. |
| GSD backfill `277d839`/`38e3270`/`b184aae`/`da8db46` | Every phase claim must retain authoritative owner and cross-phase behavior gates. | Requirements/roadmap are synthesized after M6. LOCAL-08 absorbs no-relay-wait into a local-only phase; shared-work acceptance and the owner ledger disappear. The refreshed map records the blocking handle defect and contradictory facade coordination layer, yet reconciliation trusts completion issues, marks all requirements satisfied, and reports no M2-M4 gaps. |

M2-M4 therefore **do not predate GSD in the repository**. They postdate GSD codebase mapping, project initialization, configuration, vocabulary enforcement, and research. They only predate GSD phase-level PLAN/SUMMARY/VERIFICATION artifacts. The reconciliation's `execution_origin: pre-gsd` label is imprecise unless read in that narrower sense.

### Inference, explicitly not repository-proved motive

The repository contains no rationale explaining why the agent chose facade ownership over the explicit `fava-observe` owner ledger. The one-line commit messages and same-commit complete issues provide no decision record. The strongest inference is a compound process shortcut: the public-facade vertical-slice instruction and implementation-plan phrase `explicit query routing through fava` were treated as an ownership assignment; immediate successful transport made that direct path satisfy the selected REQ/EOSE/CLOSE capstones; and the focused issue was then allowed to restate the implementation as architecture without resolving its contradiction with higher authority. M3/M4 inherited the now-committed shape instead of reopening ownership. Later reconciliation optimized for making GSD routing agree with the historical milestone ledger, so it accepted those issues as evidence sources rather than falsifying them against the authoritative ledger. The artifacts prove the shortcut and ratification sequence; they do not prove subjective intent.

## Resolution

root_cause: "Implementation: M2 commit 7fac920 assigned the complete live relay lifecycle to the thin fava facade as synchronous per-observation/per-relay work; M3 1f2c0ed turned it into the unapproved private OpenedRelay lifecycle owner and facade reconnect loop; M4 9860711 attached route reconciliation to that shape instead of completing fava-observe. Process: repository-visible GSD authority mapping/research existed before M2 and stated the correct owner, but M2-M4 ran without phase plans/owner verification; post-M6 requirements lost the cross-phase no-relay-wait and equivalent-work-sharing gates, the vocabulary checker ignored pub(super) lifecycle nouns, the codebase mapper recorded the blocking symptom yet normalized facade coordination, and reconciliation/backfilled verification trusted completion issues and bypassing happy-path evidence, declaring no gaps."
fix:
verification:
oracle_type: specified
files_changed: [crates/fava/tests/explicit_live.rs]
