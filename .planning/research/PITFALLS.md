# Domain Pitfalls

**Domain:** Embeddable Nostr client engine clean-room rewrite
**Project:** Fava
**Researched:** 2026-08-21
**Overall confidence:** HIGH for Fava-specific risks derived from authoritative project documents; MEDIUM for current external ecosystem details because the configured documentation providers were unavailable and official-source web fallback was used

## Status Boundary

M0 is complete. Fava currently has an intentionally narrow M1 tracer, not a completed M1 implementation. Missing relay ingest, routing, durable publication, protocol services, hardening, persistent profiles, provider qualification, and native SDKs are specified M2-M11 work, not current regressions. The pitfalls below identify how roadmap slices could implement or prove that future work incorrectly.

## Critical Pitfalls

### Pitfall 1: Turning source-scoped evidence into global truth

**Confidence:** HIGH
**Owning milestone/phase:** M1 establishes the evidence model; M2-M3 prove relay/request/generation attribution; M8 proves hostile and access-context isolation

**What goes wrong:** One flat flag such as `seen`, `synced`, `complete`, or `from_relay` replaces the exact relationship among event, relay session, access context, request, generation, and observation. A relay selected in a plan is credited with serving an event, one relay's EOSE becomes global completeness, or evidence obtained under one account/access context qualifies another.

**Why it happens:** Nostr wire messages look simple, and event identity naturally deduplicates bodies. Implementations then deduplicate the evidence along with the body. NIP-01 does not support that shortcut: subscription IDs are scoped to one WebSocket connection; `EVENT`, `EOSE`, and `CLOSED` identify one subscription; `OK` identifies one submitted event. EOSE separates stored from subsequent live delivery for that subscription, not for a relay globally and certainly not for the network.

**Consequences:** Incorrect routing decisions, privacy/access leakage, false completeness, stale-generation admission, invented provenance, and diagnostics that cannot explain which source actually justified a result.

**Warning signs:**

- `EventRecord` carries one relay URL or boolean instead of a set of qualified observations.
- Query filtering matches relay URL but ignores access/account/session identity.
- Planned, contacted, EOSE-observed, served, accepted, and rejected relays share one enum or counter.
- Source evidence is constructed by cache/provider callers rather than by the ingest/observation owner.
- Tests assert counts rather than exact event-to-source/request relationships.

**Prevention:**

- Keep the immutable event body deduplicated by event ID while retaining independent, mergeable evidence facts.
- Bind source role and identity at assembly/open; do not trust a provider payload to self-report its authority.
- Admit relay input only through the validation owner and an opaque admitted-event boundary; keep direct seeding in an explicit testkit.
- Key every relay-derived fact by exact session/access/request/generation identity and redact or exclude evidence outside the caller's context.
- Model EOSE, silence, CLOSED, auth-required, failure, cancellation, and empty result as distinct source-scoped states.

**Detection:** Use a shared isolation corpus with the same relay URL under two access contexts, a planned relay that serves nothing, two serving relays for one event, and a late old-generation frame. Compare public records and diagnostics with an independent wire proxy. Named deliberate breaks: match URL only; credit planned relays; suppress request identity; reuse old-generation frames. Each must fail the evidence.

### Pitfall 2: Collapsing event-cache and write-store authority

**Confidence:** HIGH
**Owning milestone/phase:** M1 for local merge/retraction; M5 for durable publication; M7 for rematerialization; M9 for profile recovery

**What goes wrong:** Pending local writes are copied into the event cache, or the cache and write store are hidden behind one storage abstraction with indistinguishable lifecycle. Cancellation then cannot retract only the local contribution, a cached predecessor cannot reappear naturally, and an unpublished or unsigned event can acquire relay provenance it never earned.

**Why it happens:** A single event table appears to simplify queries and persistence. It instead merges two different authorities: the event cache owns reusable admitted relay observations; the write store owns accepted obligations, materializations, receipts, and recovery.

**Consequences:** Cache pollution, false relay evidence, destructive cancellation, broken optimistic visibility, duplicate obligations, and recovery paths that cannot determine whether an event was observed externally or exists only because Fava owes publication work.

**Warning signs:**

- Accepting a write calls an event-cache mutation.
- Cache eviction or reset removes accepted write obligations.
- Cancelling a pending replacement deletes the cached predecessor.
- Query code concatenates cache and write results or chooses one source before semantic evaluation.
- One persisted row owns both relay admission and receipt lifecycle.

**Prevention:**

- Preserve separate public contracts, providers, storage, and lifecycle owners.
- Merge source contributions only in semantic query evaluation into one current `EventRecord`.
- Define duplicate submission behavior at the write-store contract before accepting the same event ID twice.
- Make source removal, cancellation, expiry, and deletion ordinary source revisions that recompute current state; do not invent a parallel removal stream.
- Run the same state corpus against cache and write-store contributions, including shadow, cancel, echo, duplicate acceptance, and rematerialization.

**Detection:** The M1 public-facade capstones must prove merged same-event evidence, local replacement shadowing, cancellation revealing the cached predecessor, and zero cache insertion for the local value. Named deliberate break: concatenate source results or insert the local value into the cache; the capstone must detect duplication or false authority.

### Pitfall 3: Shipping a reactive query loop before defining coherent opening, semantic identity, and removal

**Confidence:** HIGH
**Owning milestone/phase:** M1 establishes local semantics; M2 adds exact live demand/cancellation; M3 qualifies multi-source reactivity and bounded observation

**What goes wrong:** A query handle returns a snapshot assembled at different moments from different sources, equivalent descriptions create independent work, removals need a second API, or a ready source starves other sources. The system is live but not coherent.

**Why it happens:** The implementation starts with polling and channels rather than the query algebra and lifecycle. Tokio `watch` is attractive because it retains a latest value, but it intentionally coalesces intermediate states. `select!` branches run on one task, and biased polling makes fairness the caller's responsibility.

**Consequences:** Stale initial views, unbounded duplicate tasks, missed retractions, writer starvation, silent evaluator termination, and accidental loss of causal facts such as receipt transitions.

**Warning signs:**

- Sources are opened sequentially and the first source is not drained through an explicit opening barrier.
- Query identity hashes serialized syntax without canonical union/filter/source semantics.
- Each handle spawns a full evaluator task even for an equivalent query.
- `watch` carries receipts, attempts, or other facts where every transition matters.
- `tokio::select! { biased; ... }` places a continuously ready source ahead of peers without a bounded drain policy.
- Background evaluation errors appear to callers only as generic close.

**Prevention:**

- Define stable equivalent-query identity before sharing work; keep it independent of map order and incidental syntax.
- Make opening all-or-nothing with an explicit barrier that reconciles revisions arriving while sources open.
- Treat deletion, expiry, cache eviction, write cancellation, and source loss as revisions of the same current-state computation.
- Use a bounded latest-state mailbox only for current snapshots. Use a separately bounded causal stream for receipts and audit facts.
- Use fair selection or bounded round-robin draining while preserving cancellation priority.
- Carry typed terminal cause, source/provider identity, and exact observation revision.

**Detection:** Model concurrent changes during open, revision regression, duplicate source roles, continuously ready cache plus pending write revision, repeated cancel/retry of `next`, evaluator failure after open, and a slow reader. The deliberate break for M3 removes generation/fairness protection; stale or starved output must fail deterministically without sleeps.

### Pitfall 4: Using logical IDs without exact operation and generation identity

**Confidence:** HIGH
**Owning milestone/phase:** M2-M3 for request/session generations; M5-M8 for write, route, signer, attempt, handoff, and auth generations; M11 for SDK handle identity

**What goes wrong:** Event ID, relay URL, subscription ID, or receipt ID is treated as sufficient correlation. A completion from an old connection, retired route revision, cancelled signer request, or superseded replaceable-event materialization mutates current work.

**Why it happens:** Stable logical identity and physical attempt identity are both necessary, so one is often reused for the other. NIP-01 subscription IDs are only per connection, and NIP-42 challenges/authentication are valid for one connection or until replaced. Write rematerialization makes the same receipt span several materialization/signing generations.

**Consequences:** Ghost events after cancellation, stale EOSE, delivery credited to the wrong attempt, repeated sends, old signatures applied to newer materializations, and cross-account authentication confusion.

**Warning signs:**

- Maps are keyed only by relay URL plus subscription string.
- Reconnect reuses request/auth state without a new generation.
- Route replacement cancels by destination URL rather than exact lane/handoff identity.
- Signer or publisher callbacks accept only receipt ID.
- Unknown completions are ignored silently rather than refused and attributed.

**Prevention:**

- Separate stable operation identity from monotonically fresh generation/attempt identity.
- Require every asynchronous completion to present the exact owner-issued token for its current generation.
- Keep historical per-relay outcomes immutable while retiring only pre-handoff current work.
- Make stale completion a typed, scoped diagnostic fact; it must not mutate current state.
- Project the same identity semantics through native handles rather than recreating lifecycle in Swift/Kotlin.

**Detection:** Controlled schedules must deliver a previous-generation event, EOSE, auth response, route contribution, signature, and publisher result after replacement/cancellation. Named deliberate break: remove generation comparison; each corresponding scenario must show the stale result becoming visible and fail.

### Pitfall 5: Returning `Accepted` before one durable owner has committed the entire obligation

**Confidence:** HIGH
**Owning milestone/phase:** M5 establishes durable acceptance and recovery; M7 extends it across rematerialization; M8 qualifies handoff ambiguity and give-up; M9 qualifies storage profiles

**What goes wrong:** The caller receives `Accepted` after allocating an ID, writing only a materialization, or queuing background work, while the receipt and recoverable publication obligation are not committed atomically. A crash loses work that the API promised to own.

**Why it happens:** Storage commit, local visibility, signing, routing, and publication are implemented as one optimistic async pipeline. Database `commit` is also treated as a universal durability claim without pinning journal/sync configuration and the exact crash class being promised.

**Consequences:** Lost accepted writes, orphan receipts, duplicate recovery, external effects with no durable cause, and profile documentation that promises more than its persistence settings deliver.

**Warning signs:**

- `Accepted` can be emitted before receipt, obligation, and current materialization share a transaction.
- External signing/routing/publishing starts before the committed fact is observable.
- Counter/identity mutation happens before all refusal/overflow checks complete.
- Restart tests open another engine in the same process.
- A SQLite profile says “durable” without declaring journal mode, `synchronous`, WAL handling, and tested crash class.

**Prevention:**

- Enforce `command -> owner decision -> durable commit -> committed fact -> external effect -> correlated completion fact`.
- Precompute checked identities, bounds, and the complete next state before mutating storage.
- Commit obligation, current materialization, receipt identity, and recovery cursor atomically under the write-store owner.
- Qualify process-crash and power-loss durability separately. SQLite WAL with `synchronous=NORMAL` can preserve consistency yet lose the latest commit after OS/power failure; a profile claiming that stronger durability needs the appropriate settings and tests.
- Treat the WAL and provider-private persisted files as one provider-owned persistence unit.

**Detection:** SIGKILL the application after each durable/effect boundary, reopen through the supported public construction path, observe the same receipt/materialization, then continue without duplication. Named deliberate break: return `Accepted` immediately before commit; the recovery scenario must fail to find the receipt.

### Pitfall 6: Serializing partial progress or conflating routing with wire planning

**Confidence:** HIGH
**Owning milestone/phase:** M4 for ordered asynchronous read routing and subscription planning; M6 for automatic write routing and route expansion; M8 for limits

**What goes wrong:** Query/publication waits for every router to settle, or routing produces final wire subscriptions directly. Known relays sit idle while one discovery path is unresolved; planner regrouping changes application semantics; explicit routing accidentally launches automatic routers.

**Why it happens:** A one-shot `resolve_route()` API is easier than a live contribution model. NIP-65 relay lists are then embedded in the universal router instead of one selectable policy, and selected/planned relays are confused with actual evidence.

**Consequences:** Head-of-line blocking, duplicate sends, inability to retract fallback work, recursive router acquisition, non-replaceable policy, and silent omission when relay limits cannot fit all demand.

**Warning signs:**

- Router contract returns one future of a final set.
- Automatic open awaits all routers.
- Downstream fallback sees only a settled upstream plan.
- The primitive routing crate names NIP-65, hints, app relays, or fallback semantics.
- Subscription planner receives user/protocol intent rather than logical demand already assigned to a relay session.
- Explicit relay input and automatic routing share the same entry path.

**Prevention:**

- Each router provides an immediate complete current contribution and later complete replacement contributions.
- Accumulate configured routers in order while starting currently known work immediately.
- Let downstream routers observe the live accumulated upstream plan and retract only their own contributions.
- Keep routing derivation, subscription grouping, session execution, and result evidence separate.
- Implement NIP-65/outbox, hints, app relays, and fallback in separate policy crates over explicit acquisition services.
- Make overflow/relay limits produce typed exact shortfall; never silently drop demand.

**Detection:** Delay one router while an earlier router is ready, then verify the proxy sees immediate work. Add a later destination and confirm the same query/receipt expands without duplicate sends. Compare standard and no-grouping planners: wire shape may differ, logical results and evidence must be identical. Named deliberate break: await router settlement; the immediate-progress scenario must time out under a controlled barrier.

### Pitfall 7: Calling a trait replaceable while the standard provider retains privileged authority

**Confidence:** HIGH
**Owning milestone/phase:** Every contract slice from M1 onward; M10 is the full substitution qualification gate

**What goes wrong:** A public trait exists, but the standard implementation uses private facade constructors, internal state, undocumented role conventions, or returns values that let it redefine universal semantics. External implementations compile but cannot behave equivalently.

**Why it happens:** The trait is extracted from the first implementation's private shape, or the contract/implementation split is deferred. Fava's repository rules require the opposite: introduce the separate contract with its first real implementation so the first implementation is forced through the public seam, then challenge the contract with materially different implementations and conformance evidence.

**Consequences:** Provider lock-in, semantic drift, core edits for provider N+1, persisted-format coupling across unrelated providers, and “provider matrix” tests that exercise only constructors.

**Warning signs:**

- Facade or runtime depends on a standard implementation crate.
- Standard provider receives internal owner handles unavailable through the contract.
- Provider self-reports its source role, operation identity, or authority-bearing evidence.
- A `QueryEvaluator` can manufacture complete provenance/terminal facts rather than returning bounded semantic candidates for owner validation.
- Conformance tests encode the default provider's table/queue behavior.
- Only a null/no-op alternative exists and it tests `open` only.

**Prevention:**

- Preserve dependency direction: semantic values -> neutral public contracts -> implementations; universal owners depend only on contracts.
- Bind source role, lifecycle, bounds, and authority at assembly/owner boundaries.
- Let providers advertise specific guarantees without strengthening the baseline contract implicitly.
- Build conformance corpora from required behavior, including malformed output, cancellation, late completion, overload, restart, and access context.
- Add materially different outside-workspace implementations and dependency-negative compile/source gates.
- Keep provider schema/version/migration private to that provider; do not create a global storage identity.

**Detection:** Run the same public application corpus across the M10 provider matrix. Swap one provider without editing core or unrelated assembly. Named deliberate break: give the standard provider a private facade door; source/dependency gates and external conformance comparison must fail.

### Pitfall 8: Letting a provider block, panic, or disappear on a universal lifecycle task

**Confidence:** HIGH
**Owning milestone/phase:** Isolation must be designed with each provider slice; M8 qualifies hostile providers; M10 repeats it for alternatives

**What goes wrong:** Provider open/evaluate/callback work executes synchronously on the query/publication owner task. One blocking or panicking provider stalls unrelated sessions and shutdown, or a background task exits and callers see only generic closure.

**Why it happens:** Async functions are mistaken for isolated execution. Tokio documents that `select!` runs branches on the current task; a blocking branch prevents every other branch from progressing. Cancellation safety also depends on the selected future, not on `select!` itself.

**Consequences:** Global liveness failure, resource leaks, missing terminal cause, unbounded shutdown, and a provider substitution boundary that is unsafe for third-party code.

**Warning signs:**

- Provider code runs while an owner lock is held.
- Panic crosses the contract boundary.
- No deadline, cancellation acknowledgement, or generation token is attached to a call.
- A dropped task is indistinguishable from normal close.
- Shutdown waits forever for provider cooperation.

**Prevention:**

- Execute provider calls behind the specified runtime isolation boundary with bounded admission, deadlines, panic capture, and exact call identity.
- Never hold Fava locks across provider code.
- Scope failure to the exact provider call, source, destination, query, or write and publish a typed terminal fact.
- Reject late/malformed provider results after validating identity and advertised bounds.
- Make shutdown bounded even when provider cancellation is ignored; retain attribution for abandoned work.

**Detection:** A deliberately blocking provider, panicking provider, malformed provider, cancellation-ignoring provider, and late-completion provider must each leave an unrelated query/write and global shutdown progressing within declared bounds. Named deliberate break: call the provider directly on the owner task; the isolation scenario must fail.

### Pitfall 9: Treating a record count as a resource envelope

**Confidence:** HIGH
**Owning milestone/phase:** Bounds belong in every introducing slice; M3 proves observation bounds, M6 route fan-out, M8 end-to-end hostile limits, M9 retention, M10 profile envelopes, M11 native baselines

**What goes wrong:** A cache capped at N records is called bounded while event bytes, tags, evidence sets, query structure, results, snapshots, tasks, file descriptors, wire frames, diagnostics, or retained artifacts remain unlimited.

**Why it happens:** Item counts are easy to expose and test. Nostr inputs have independent byte and cardinality dimensions, and NIP-11 relay-advertised limits may silently clamp a request rather than prove that all demand was served.

**Consequences:** Memory/CPU amplification, slow-consumer collapse, task/FD exhaustion, runaway WAL/evidence directories, silent incomplete results, and denial of service across otherwise independent operations.

**Warning signs:**

- Capacity fields count records only.
- Query filters, unions, authors/IDs/relays, and result length default to unbounded.
- Provider snapshots clone all retained data under a mutex.
- One task/receiver exists per handle without equivalent-query sharing or admission control.
- Proxy logs, run directories, diagnostic streams, and causal receipt history have no byte/retention policy.
- Planner truncates to a relay limit without returning shortfall.

**Prevention:**

- Declare budgets for bytes, items, tag/evidence cardinality, query structure, work, fan-out, result size, observations, queues, tasks, FDs, time, retries, and retained artifacts.
- Refuse before opening work when possible; otherwise return typed partial/shortfall/backpressure with the responsible owner and consumed bound.
- Keep current-state coalescing distinct from causal history retention.
- Treat NIP-11 documents as validated relay-scoped planning input, not guaranteed truth or permission to silently omit.
- Measure resource return to baseline after teardown and repeated native lifecycle cycles.

**Detection:** Exceed each dimension independently, including many tiny items, one huge item, huge evidence fan-in, large filter structure, 1,000 idle observations, slow consumers, excessive connections, and oversized witness output. Every run must remain within its envelope or refuse with an exact typed result. Silent drop is the named deliberate break.

### Pitfall 10: Flattening cancellation, refusal, failure, and ambiguous handoff

**Confidence:** HIGH
**Owning milestone/phase:** M2-M3 query cancellation; M5 pre-handoff write cancellation; M8 ambiguous handoff/give-up; M11 native mapping

**What goes wrong:** All termination becomes `Cancelled`, `Failed`, or stream completion. A caller cannot tell whether zero bytes crossed the handoff boundary, a full event crossed but `OK` was lost, a relay rejected it, or a late completion was ignored.

**Why it happens:** One terminal enum looks simpler, and foreign runtimes impose their own cancellation conventions. UniFFI's public guide does not promise one portable application-level cancellation semantic; it recommends a library-specific cancellation channel. Kotlin Flow cancellation propagates through the collecting coroutine and depends on cooperative cancellation and transparent exception propagation.

**Consequences:** Unsafe retry, duplicate publication, false acknowledgement, hidden failures, wrong receipt aggregates, and Rust/Swift/Kotlin behavior that appears similar only on the happy path.

**Warning signs:**

- Cancellation returns before exact withdrawal/terminalization.
- `OK` timeout maps to rejection or never-sent.
- Retry policy consumes attempts while a relay is merely offline and no handoff occurred.
- SDK wrappers convert Rust terminal facts to generic exception or silent flow completion.
- Native task cancellation drops a Rust handle without explicit close/reattach semantics.

**Prevention:**

- Define the handoff boundary and operation generation at the owning milestone before exposing cancellation.
- Preserve exact `never handed off`, `ambiguous`, `acknowledged`, `rejected`, `gave up`, and stale-completion distinctions per destination.
- Keep current receipt identity stable across recovery and rematerialization; make historical facts immutable.
- Export explicit cancel/close/reattach operations and typed terminal values through FFI, then map native cancellation deliberately.
- Test cancellation both before and after every meaningful suspension/handoff boundary.

**Detection:** Proxy-gate the transport before any bytes, after the complete frame, and before the relay response. Cancel at each gate and compare the public receipt with wire evidence. Remove one outcome/cancel operation from each native SDK; the parity mutation corpus must fail.

### Pitfall 11: Treating deletion and expiration as erasure instead of qualified current-state inputs

**Confidence:** HIGH
**Owning milestone/phase:** M1 state algebra and removal; M2 admission/source attribution; M9 persistent cache/expiry recovery

**What goes wrong:** A deletion request physically removes all local history without verifying authorship or timestamp scope, or an expired event is treated as proven erased from relays and peers. Later source revisions resurrect a value that a valid tombstone should still suppress.

**Why it happens:** “Delete” and “expire” sound like storage commands. NIP-09 defines an authored deletion request and requires clients to verify same-pubkey authority; address deletions apply through a timestamp. It explicitly cannot guarantee deletion everywhere. NIP-40 says clients should ignore expired events while relays may retain them indefinitely.

**Consequences:** Unauthorized hiding, resurrection, false security claims, irreproducible query results across restarts, and cache profiles that cannot explain retained versus visible state.

**Warning signs:**

- Any kind-5 reference deletes without pubkey validation.
- Tombstones are discarded as soon as the current body disappears.
- Expiry is enforced only by a one-shot timer, not reevaluated on open/restart/clock change.
- Public evidence claims deletion from the network.

**Prevention:**

- Treat deletion and expiry as semantic facts with source, authority, coordinate, and time scope.
- Preserve enough tombstone/expiry state to prevent resurrection under the advertised profile.
- Recompute visibility deterministically on source revision and restart with controlled clocks.
- Separate application-visible retraction from provider-private physical retention/compaction.

**Detection:** Property/model tests must permute predecessor, replacement, deletion, expiry, removal, restart, and late old-source arrival. Named breaks: omit pubkey validation, drop the tombstone, or evaluate expiry only at ingest; each must fail the current-state corpus.

### Pitfall 12: Claiming behavior from green capstones that never proved their mechanism

**Confidence:** HIGH
**Owning milestone/phase:** Cross-cutting M1-M11; each slice owns its red/green/mutation record

**What goes wrong:** A canary scenario passes because the fixture inserted the expected result, public relays happened to cooperate, or the mechanism under claim was never exercised. Diagnostics are used as their own witness. Future-specified scenarios appear enabled or built before their exit gates pass.

**Why it happens:** Broad end-to-end tests feel persuasive and are easier to showcase than owner/model/crash tests. Evidence setup and product behavior blur together.

**Consequences:** False milestone completion, regressions hidden by mocks, unverifiable external effects, and an evidence suite that stays green after the protection is removed.

**Warning signs:**

- No recorded causal red failure before implementation.
- The fixture writes the route, source status, receipt row, or result under proof.
- A public-relay run is the sole correctness oracle.
- Diagnostics alone prove a frame crossed the socket.
- A scenario remains green under its named deliberate break.
- Enabled scenarios can skip environmental failure or failed runs lack complete terminal artifacts.

**Prevention:**

- Name requirement, owner, smallest failing proof, public capstone, independent witness, and mechanism-disable mutation before each slice.
- Seed causes through supported operations: real signed relay frames, public write acceptance, real process kill/reopen.
- Use owner/property/model/conformance evidence for invariants and the canary only for additional composition proof.
- Compare Fava's public result/diagnostics with an independent proxy, relay process, signer log, filesystem/process state, or native instrumentation.
- Keep statuses explicit: specified, built, known violation; enabled canary scenarios never silently skip.

**Detection:** Review the minimal evidence record for Red, Green, Mutation, Capstone, and Unrun items. Re-run the named mutation. A milestone name is not earned until every documented exit gate passes.

## Moderate Pitfalls

### Pitfall 13: Deferring access/account isolation until authentication work

**Confidence:** HIGH
**Owning milestone/phase:** M1-M3 must retain exact context in identities/evidence; M8 adds NIP-42 policy and proves account/session isolation

**What goes wrong:** Early cache/query types omit access context because authentication arrives at M8, forcing a rewrite or leaking evidence when auth is added. NIP-42 authentication is connection/challenge scoped and distinct from event authorship.

**Warning signs:** Query identity contains account context but evaluators ignore it; evidence is matched by URL only; one auth challenge/state is cached per relay URL.

**Prevention:** Preserve opaque access context in M1 identities without implementing premature auth policy. In M8, keep relay access identity, event author, signer selection, and application account as separate facts. Reconnect creates fresh challenge/generation state. Denial terminates only the exact affected operation.

### Pitfall 14: Letting protocol meaning leak into universal lifecycle owners

**Confidence:** HIGH
**Owning milestone/phase:** M4/M6 routing policy crates; M7 protocol-crate composition; M9 NIP-05/NIP-11 services

**What goes wrong:** `fava`, routing, publication, or fetch-cache switches on NIP/kind-specific meaning. Adding protocol crate N+1 edits the universal core.

**Warning signs:** Central enums name follow/unfollow, kind 10002, NIP-05 freshness, or NIP-11 fields; protocol crates call signer/publisher directly; generic fetch cache interprets service payloads.

**Prevention:** Protocol crates produce ordinary event values or replaceable-event edits. Router implementations own NIP-65/hint/app/fallback policy. NIP-05 and NIP-11 own validation/freshness while a generic fetch cache stores opaque bytes. Enforce negative dependencies and the N+1 zero-core-edit falsifier.

### Pitfall 15: Advertising a profile guarantee that belongs only to one provider configuration

**Confidence:** HIGH
**Owning milestone/phase:** M5 durable write profile; M9 cache/service profiles; M10 provider qualification

**What goes wrong:** Baseline event-cache or write-store traits imply persistence, cold-cache reuse, coverage, retention, or migration that only one implementation provides. “Memory,” “ephemeral,” and “persistent” names are treated as sufficient proof.

**Warning signs:** Profile docs are handwritten separately from assembly; restart behavior changes when only provider selection changes; provider-private schema migration is coordinated by a global facade version.

**Prevention:** Keep baseline contracts minimal and advertise guarantees explicitly per selected profile. Generate/check profile documentation from assembly, qualify persistent and ephemeral variants through the same application source, and test destructive reset against exact selected state.

### Pitfall 16: Optimizing away semantic or evidentiary correctness

**Confidence:** HIGH
**Owning milestone/phase:** M3 establishes bounded current-state behavior; performance work follows the first representative slice; M10-M11 qualify profile budgets

**What goes wrong:** Incremental evaluation, grouping, batching, snapshot sharing, or storage compaction changes replacement winners, removals, evidence, access isolation, cancellation, or terminal attribution.

**Warning signs:** Optimization lands before a full semantic oracle; benchmarks assert only throughput; grouped and ungrouped planners are not differentially compared; top-k truncation happens before safe semantic selection.

**Prevention:** Retain full reevaluation/ungrouped/reference implementations as oracles. Measure one owner at a time and run semantic, mutation, resource, and physical evidence together. Reject performance gains that change public meaning or failure truth.

### Pitfall 17: Declaring native parity from generated signatures or same-process tests

**Confidence:** HIGH
**Owning milestone/phase:** M11, with lifecycle-compatible public contracts prepared earlier

**What goes wrong:** Rust, Swift, and Kotlin expose similarly named methods but differ in cancellation, terminal errors, stream coalescing, object lifetime, restart, or close behavior. Generated bindings are tested without real iOS/Android process lifecycle.

**Warning signs:** Parity is a word/ABI inventory only; SDK wrappers own their own query/receipt state machine; Kotlin Flow swallows downstream exceptions; Swift task cancellation is assumed to equal Fava close; tests use repository-relative artifacts.

**Prevention:** Use one shared behavior corpus with Rust as semantic reference, then prove native-specific cancellation, lifecycle, packaging, representations, process restart, suspension/resume, and resource return to baseline. Keep exception transparency and exact terminal mapping. Build ordinary external artifacts with selected providers/protocol crates only.

### Pitfall 18: Guessing intentionally open product decisions during foundational milestones

**Confidence:** HIGH
**Owning milestone/phase:** Windowing and outage backfill with M3/M9; partial-handoff cancellation and delivery history with M5/M8; recommended persistent cache with M9

**What goes wrong:** An early representation silently selects product behavior for windowing, partial-handoff cancellation, outage backfill, full delivery history, or the recommended persistent event-cache profile.

**Warning signs:** A low-level contract has policy fields not required by its current vertical slice; behavior is inferred from the first provider; roadmap describes an open choice as already promised.

**Prevention:** Preserve the distinction and forcing requirements, but defer the decision to its owning milestone. Record a focused decision issue before making the API/profile choice. Do not add speculative variants or compatibility paths.

### Pitfall 19: Allowing build and scenario registries to drift from the claimed validation surface

**Confidence:** HIGH
**Owning milestone/phase:** Fix alongside M1 evidence completion; extend platform triples and external workspaces through M10-M11

**What goes wrong:** Cargo, Bazel, canary registry, dispatch code, falsifier workspaces, and native build matrices run different tests while one command is called authoritative.

**Warning signs:** A crate/test exists in Cargo but not Bazel; enabled scenario IDs are duplicated in JSON and match arms; external-provider or canary workspaces are omitted; only Apple Silicon is rendered while cross-platform release is claimed.

**Prevention:** Define one checked-in validation entry point or mechanically compare/generate graphs. Assert every enabled scenario has exactly one executor. Add platform triples only as milestones need them, but never claim an unexecuted platform.

## Minor Pitfalls

### Pitfall 20: Treating draft NIPs as frozen implementation constants

**Confidence:** MEDIUM
**Owning milestone/phase:** The milestone introducing each NIP-specific policy or service

**What goes wrong:** Current `master` wording is copied into universal values without recording the consulted revision or isolating protocol policy.

**Prevention:** Recheck official NIPs during the owning phase, record the source revision/date in research/evidence, keep NIP-specific semantics in its protocol/router/service crate, and test wire behavior against controlled relays. Do not add compatibility paths preemptively.

### Pitfall 21: Making diagnostics an unbounded shadow event system

**Confidence:** HIGH
**Owning milestone/phase:** Cross-cutting, introduced with each owner; fully enveloped by M8-M10

**What goes wrong:** Diagnostics retain every transition forever or become a second authority applications must reconcile with public results.

**Prevention:** Diagnostics expose bounded current facts and explicit coalesced/lost counts. Causal retained evidence has a separate bounded policy. Diagnostics explain Fava's belief; independent witnesses prove external effects.

### Pitfall 22: Centralizing too many query lifecycle roles in one large module

**Confidence:** HIGH
**Owning milestone/phase:** M1 before query identity/algebra expands; revisit at M2-M3 owner boundaries

**What goes wrong:** Query syntax, canonical identity, source contracts, evidence values, evaluation, polling, delivery, and teardown accrete in one file/task, hiding the actual owner of failures and revisions.

**Prevention:** Split by cohesive owner without inventing new vocabulary: semantic query values/canonicalization, neutral source/evaluator contracts, observation lifecycle, and standard implementation. Respect the repository vocabulary gate and code-size limits.

### Pitfall 23: Assuming the completed M0 lab needs no maintenance as later scenarios expand

**Confidence:** HIGH
**Owning milestone/phase:** Cross-cutting canary work in M1-M11; this does not reopen the completed M0 gate

**What goes wrong:** Later claims depend on witnesses, failure artifacts, cleanup, port allocation, or dispatch behavior that M0's original smoke scenario did not need to prove.

**Prevention:** Extend the lab only when a later scenario requires new evidence. Ensure later failed runs terminalize with reconstructable manifests, witness failures fail the scenario, ports/processes are causally owned, and retained evidence has bounds. Keep M0's completed claim distinct from these later evidence-system requirements.

## Phase-Specific Warnings

| Phase Topic | Likely Pitfall | Warning Sign | Mitigation / Required Proof |
|-------------|----------------|--------------|-----------------------------|
| M1 — local semantic state | Mistaking the narrow tracer for completed query semantics | No canonical equivalent-query identity, deletion/expiry corpus, source removal, or public-facade write path | Finish shared state corpus, coherent open barrier, merged evidence, shadow/cancel/reveal, source-removal capstone; source-concatenation mutation fails |
| M1 — current code defects | Building M2 over known semantic/ownership gaps | Equal-timestamp winner chooses highest ID; access context ignored; duplicate local acceptance poisons evaluation; stale opening possible | Correct through M1 behavior-first owner tests before networking multiplies the state space; do not label absent M2 behavior a regression |
| M2 — one-relay live read | Crediting unverified/off-filter/stale frames | Cache accepts caller-constructed relay evidence or raw events | Opaque admission value; exact request/session identity; forged/wrong-ID/off-filter/CLOSED corpus; bypass-verification mutation fails |
| M3 — multi-relay observation | Using latest-state coalescing for causal facts or one task per handle | Receipt transitions disappear; tasks grow linearly; old generation updates current state | Separate snapshot and causal delivery; canonical sharing; bounded mailbox; 1,000-idle-observation envelope; remove-generation-check mutation fails |
| M4 — routing/planning | Awaiting final route or embedding policy in primitive | One `resolve()` future; routing crate names NIP-65/fallback; planner silently truncates | Immediate/replacement contributions; live downstream reaction; explicit bypass; grouping differential; exact shortfall |
| M5 — durable explicit publication | `Accepted` precedes complete atomic obligation | Receipt/materialization committed separately; signer/publisher runs first; same-process “restart” | Commit facts before effects; SIGKILL at every boundary; same receipt recovers; early-accept mutation fails |
| M6 — automatic write routing | New route facts create new receipts or duplicate sends | Destination URL is the only lane key; delayed recipient restarts publication | One receipt with route/materialization/attempt generations; immediate known delivery; later lanes; settle-first mutation fails |
| M7 — replaceable edits | Protocol crate owns publication or stale generation completes | NIP-02 calls signer/publisher; receipt changes after rematerialization | Edit-only protocol contract plus unrelated protocol N+1; stale signer/delivery corpus; dependency-negative test |
| M8 — hostile/auth/limits | Hardening is added as generic catch-all wrappers | Panic becomes generic close; NIP-42 state cached by relay URL; bounds silently drop | Per-call isolation, generation-scoped auth, exact typed failure/ambiguity/shortfall, independent adversarial process, failure-isolation mutation |
| M9 — profiles/services | Baseline trait silently implies persistence/freshness | Memory cache survives restart accidentally; NIP-05/NIP-11 share semantic cache keys | Explicit generated profile guarantees; same app with swapped providers; service-owned freshness; ephemeral-byte-reuse mutation fails |
| M10 — substitution | Alternative providers are adapters around defaults | External provider needs internal constructor or only tests `open` | Outside-workspace implementations, public conformance matrix, dependency-negative gates, zero private facade doors |
| M11 — native release | ABI similarity substitutes for behavioral parity | No real process restart/cancel/close tests; wrapper state machine differs | Shared parity corpus, explicit FFI cancellation/terminal values, real Android/iOS lifecycle evidence, operation-removal mutation per SDK |
| Cross-cutting evidence | Canary or diagnostics prove themselves | No independent wire/process witness; scenario never red under mutation | Owner proof plus public capstone only when additive; named deliberate break; complete run artifact and explicit unrun record |
| Cross-cutting bounds | Limits deferred wholesale to M8 | Early public types allow unbounded structure or queues | Introduce typed bounds/refusal with each slice; M8 qualifies hostile composition rather than retrofitting every API |

## Roadmap Guidance

1. Complete M1 semantic ownership before adding relay work. The highest rewrite risk is not WebSocket transport; it is making exact source authority, canonical query identity, removal, and coherent opening impossible to recover later.
2. Build M2-M3 around exact request/generation identity and two distinct delivery shapes: bounded latest current state and bounded causal facts.
3. Introduce routing and subscription planning as separate M4 owners before publication depends on them. Make partial progress observable from the first route contribution.
4. Treat M5 as the durable write spine. No later protocol or automatic-routing work should create a second acceptance/receipt lifecycle.
5. Keep bounds and isolation in every introducing slice; use M8 to qualify the composition against hostile relays/providers rather than to retrofit fundamentally unbounded contracts.
6. Use M10 to falsify replaceability, not to create it. Every earlier contract must already place the standard implementation behind the same public seam.
7. Keep M11 focused on real native lifecycle/parity. Do not let Swift/Kotlin wrappers reinterpret Rust state, cancellation, or evidence.

## What Might Have Been Missed

- The five intentionally open product decisions require phase-specific research once their forcing workloads exist; this report does not choose them.
- The exact durable write-store technology/profile remains a later stack decision. SQLite sources here establish a durability pitfall, not a database selection.
- NIP documents are draft/current-master sources and may change before their owning milestone. Recheck them during phase research.
- Public-relay implementation quirks were deliberately excluded as authoritative evidence; controlled real relays and the adversarial process remain the roadmap's deterministic witnesses.

## Sources

### Fava primary authorities — HIGH confidence

- `.planning/PROJECT.md`
- `docs/spec/README.md`
- `docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md`
- `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`
- `.planning/codebase/CONCERNS.md` for current M0/M1 status and known implemented-code risks; it does not redefine future specified work as regressions

### Official external sources — MEDIUM confidence via official-source web fallback

- [NIP-01: Basic protocol flow](https://github.com/nostr-protocol/nips/blob/master/01.md) — per-connection subscriptions, exact wire correlation, replaceable-event tie breaking
- [NIP-09: Event deletion request](https://github.com/nostr-protocol/nips/blob/master/09.md) — authorship validation, address/timestamp scope, non-guaranteed erasure
- [NIP-11: Relay information document](https://github.com/nostr-protocol/nips/blob/master/11.md) — relay-declared limits and potential silent filter-limit clamping
- [NIP-40: Expiration timestamp](https://github.com/nostr-protocol/nips/blob/master/40.md) — client visibility, relay retention, no security/erasure guarantee
- [NIP-42: Client authentication](https://github.com/nostr-protocol/nips/blob/master/42.md) — connection/challenge-scoped authentication
- [NIP-65: Relay list metadata](https://github.com/nostr-protocol/nips/blob/master/65.md) — read/write routing inputs and outbox use
- [Tokio 1.53.1 `select!`](https://docs.rs/tokio/1.53.1/tokio/macro.select.html) — same-task execution, fairness, biased-mode responsibility, cancellation safety
- [Tokio 1.53.1 `watch`](https://docs.rs/tokio/1.53.1/tokio/sync/watch/) — latest-value coalescing and per-receiver seen state
- [SQLite atomic commit](https://www.sqlite.org/atomiccommit.html), [WAL](https://www.sqlite.org/wal.html), and [`PRAGMA synchronous`](https://www.sqlite.org/pragma.html#pragma_synchronous) — transaction atomicity versus configured crash/power-loss durability
- [UniFFI async/future support](https://mozilla.github.io/uniffi-rs/next/futures.html) and [async FFI internals](https://mozilla.github.io/uniffi-rs/latest/internals/async-ffi.html) — foreign async projection and cancellation caveats
- [Kotlin Flow guide](https://kotlinlang.org/docs/coroutines-flow.html) and [Flow API contract](https://kotlinlang.org/api/kotlinx.coroutines/kotlinx-coroutines-core/kotlinx.coroutines.flow/-flow/) — cancellation propagation and exception transparency

---

*Pitfalls research: 2026-08-21. External documentation was checked against current official pages; findings derived from those pages remain MEDIUM because the configured Context7/Jina providers were unavailable and the research seam classified verified web fallback as MEDIUM.*
