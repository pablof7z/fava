# Codebase Concerns

**Analysis Date:** 2026-08-20

The checkout contains the completed M0 evidence lab and an explicitly incomplete M1 local-source tracer. `docs/issues/0002-m0-evidence-foundation.md` claims M0 complete; `docs/issues/0001-local-source-merge.md` limits current M1 claims and names its remaining gates. Concerns below distinguish defects in implemented/public code from later work specified in `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`.

## Tech Debt

**Six-gate architecture audit:**

| Gate | Current concern | Evidence |
|---|---|---|
| Ownership | Relay admission is not represented by an opaque admitted-event value. Public callers can construct relay evidence and submit raw `CachedEvent` values, so the cache boundary cannot prove that `fava-ingest` owned validation. | `crates/fava-state/src/lib.rs`, `crates/fava-event-cache/src/lib.rs`, `docs/spec/ARCHITECTURE.md` |
| Dependency direction | Cargo follows semantic values -> contracts -> implementations, but Bazel targets are maintained separately and exclude the canary and falsifier workspaces. | `Cargo.toml`, `MODULE.bazel`, `crates/*/BUILD.bazel`, `apps/canary/Cargo.toml`, `falsifiers/external-null-cache/Cargo.toml` |
| Replaceability | Only event-cache assembly has an outside-workspace proof, and it exercises open only. Evaluator and write-store contracts lack competing implementations and public conformance kits. | `falsifiers/external-null-cache/src/lib.rs`, `crates/fava-query/src/lib.rs`, `crates/fava-write-store/src/lib.rs` |
| Failure isolation | Provider open/evaluation runs synchronously on caller/runtime tasks; blocking and panic are uncontained, and background evaluation failure becomes an unattributed close. | `crates/fava-observe/src/lib.rs`, `crates/fava/src/lib.rs`, `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md` |
| Boundedness | Memory providers bound record count only. Item bytes, evidence entries, query/result size, observation count, and retained run evidence lack aggregate bounds or typed overload. | `crates/fava-event-cache-memory/src/lib.rs`, `crates/fava-write-store-memory/src/lib.rs`, `crates/fava-query/src/lib.rs`, `apps/canary/src/artifacts.rs` |
| Behavioral proof | Cargo validation is green, but M1 exit-gate evidence is incomplete and the Bazel authority runs only two integration targets. M0 live proof is a manual command with ignored local artifacts. | `.bazelrc`, `crates/*/BUILD.bazel`, `apps/canary/README.md`, `docs/issues/0001-local-source-merge.md`, `docs/issues/0002-m0-evidence-foundation.md` |

**Admission and evidence boundary is forgeable:**

- Issue: `RelayEvidence::one`, `CachedEvent::new`, and `EventCache::commit` are public; neither the contract nor `MemoryEventCache` verifies ID/signature or exact current session/request attribution.
- Files: `crates/fava-state/src/lib.rs`, `crates/fava-event-cache/src/lib.rs`, `crates/fava-event-cache-memory/src/lib.rs`
- Impact: A caller or faulty provider can fabricate provenance or insert an invalid body into query-visible state, contrary to the universal evidence and M1 admission rules in `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md` and `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`.
- Fix approach: Introduce the specified admitted/verified relay value at `fava-ingest`; keep direct seeding in an explicit testkit; make cache mutation accept only the semantic decision produced by the admission/state owner in `docs/spec/ARCHITECTURE.md`.

**Evaluator contract can redefine universal facts:**

- Issue: A replaceable `QueryEvaluator` returns the complete `QuerySnapshot`, including source evidence, without validation by `fava-observe`.
- Files: `crates/fava-query/src/lib.rs`, `crates/fava-observe/src/lib.rs`, `crates/fava-query-standard/src/lib.rs`
- Impact: A substitute can reorder incorrectly, fabricate evidence, omit source status, or violate limits even though `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md` reserves those meanings to Fava.
- Fix approach: Narrow evaluator output to records/deltas that the observation owner wraps with authoritative source facts; validate invariant-bearing output; add the substitution corpus required by `docs/spec/ARCHITECTURE.md`.

**Source role and revision claims are trusted:**

- Issue: `Observer` accepts each provider's `SourceSnapshot.kind`, replaces sources by that self-reported kind, and never checks monotonic `SourceRevision` progress.
- Files: `crates/fava-query/src/lib.rs`, `crates/fava-observe/src/lib.rs`
- Impact: An external cache can label itself `WriteStore`, or regress a revision, without a typed contract violation.
- Fix approach: Bind role at assembly/open, keep source identity outside provider payloads, and refuse duplicate/regressing revisions with scoped evidence.

**Bazel and Cargo are not one validation surface:**

- Issue: `.bazelrc` declares `bazel test //...` authoritative, but only `crates/fava:local_source_merge` and `crates/fava-query-standard:source_merge` are Bazel test targets.
- Files: `.bazelrc`, `crates/*/BUILD.bazel`, `apps/canary/Cargo.toml`, `falsifiers/external-null-cache/Cargo.toml`
- Impact: A green authoritative command omits cache atomicity, observation lifecycle, coordinate semantics, canary registry, and external-provider tests.
- Fix approach: Add Bazel unit/external-workspace targets, or define one checked-in pass command that invokes Bazel plus the canary and falsifier Cargo workspaces.

**Query module is at the file-size threshold:**

- Issue: The 498-line file combines syntax, policy, source lifecycle, result/evidence values, evaluator contract, and errors while M1 identity and later algebra remain absent.
- Files: `crates/fava-query/src/lib.rs`, `AGENTS.md`, `docs/spec/ARCHITECTURE.md`
- Impact: Adding specified behavior in place crosses the 500-line soft limit and weakens ownership-sensitive review.
- Fix approach: Split query value/canonicalization, source contracts, result/evidence, and evaluator contract under `crates/fava-query/src/`.

## Known Bugs

**Equal-timestamp replaceable events select the wrong ID:**

- Symptoms: `candidate_is_newer` and `StandardQueryEvaluator` prefer the greatest event ID when timestamps tie. NIP-01 requires the lowest ID in lexical order.
- Files: `crates/fava-state/src/lib.rs`, `crates/fava-query-standard/src/lib.rs`, `crates/fava-query/src/lib.rs`
- Trigger: Supply two valid same-coordinate replaceable events with equal `created_at` values and different IDs.
- Workaround: Avoid equal timestamps; there is no correct workaround for received ties.
- Fix approach: Separate presentation ordering from winner ordering, choose the lower ID on ties, and add a shared tie corpus. Protocol reference: [NIP-01](https://github.com/nostr-protocol/nips/blob/master/01.md).

**Access-context isolation is not enforced:**

- Symptoms: `EventQuery.access` is in query identity, but `StandardQueryEvaluator` ignores it. `RelayEvidence::includes_any_relay` matches URL only and records expose observations from every context.
- Files: `crates/fava-query/src/lib.rs`, `crates/fava-query-standard/src/lib.rs`, `crates/fava-state/src/lib.rs`
- Trigger: Cache evidence under context A, then run `only_from_relays` for the same URL under context B; A qualifies and is exposed.
- Workaround: Use separate cache instances per access context; the API does not enforce this.
- Fix approach: Filter/partition by exact `RelaySessionKey`, enforce query context during authority matching, and add cross-context negative tests.

**Duplicate local acceptance can poison query evaluation:**

- Symptoms: `MemoryWriteStore` accepts one event ID repeatedly under new receipt/write IDs; `StandardQueryEvaluator` refuses the resulting snapshot because publication evidence conflicts.
- Files: `crates/fava-write-store-memory/src/lib.rs`, `crates/fava-query-standard/src/lib.rs`, `crates/fava-write/src/lib.rs`
- Trigger: Call `accept_materialized` twice with the same finalized event, then open/update a matching query.
- Workaround: Applications must deduplicate before acceptance, though the contract does not require it.
- Fix approach: Define duplicate submission as idempotent or multiple obligations, encode that in evidence, and reject/merge before committing source state.

**Opening can return a stale initial cross-source view:**

- Symptoms: `Observer::open` captures the cache initial snapshot, opens the write store afterward, and evaluates without draining cache changes that arrived during the second open.
- Files: `crates/fava-observe/src/lib.rs`, `crates/fava-query/src/lib.rs`, `docs/spec/ARCHITECTURE.md`
- Trigger: Commit a cache change after cache open but before write-store open finishes; the returned handle initially exposes older state.
- Workaround: Wait for a later `changed()` update, which defeats the coherent-current initial-view contract.
- Fix approach: Buffer/drain source revisions through an explicit opening boundary before initial evaluation.

**Biased polling can starve write-store changes:**

- Symptoms: `tokio::select! { biased; ... }` polls cache before write store. A continuously ready cache branch can prevent a ready write revision from being processed.
- Files: `crates/fava-observe/src/lib.rs`
- Trigger: Use a cache source whose `next_change` remains ready while the write source also has a revision.
- Workaround: Quiesce cache updates.
- Fix approach: Use fair selection or bounded round-robin draining while preserving cancellation priority.

**Runtime evaluation failure loses its cause:**

- Symptoms: A post-open evaluator error silently exits the task; the application sees only `ObservationClosed`, indistinguishable from close, teardown, or revision exhaustion.
- Files: `crates/fava-observe/src/lib.rs`, `crates/fava-query/src/lib.rs`
- Trigger: Use an evaluator that succeeds initially and refuses a later source revision.
- Workaround: External evaluator logging only.
- Fix approach: Deliver a typed terminal fact with evaluator/source cause and exact observation revision.

**Failed M0 runs omit the manifest:**

- Symptoms: The error path writes best-effort stderr/JSONL/report, then returns without hashes or `manifest.json`; a retained failed run under `apps/canary/runs/` has this shape.
- Files: `apps/canary/src/lib.rs`, `apps/canary/src/artifacts.rs`, `docs/issues/0002-m0-evidence-foundation.md`
- Trigger: Fail startup, proxy, query, or evidence work after `RunArtifacts::create`.
- Workaround: Reconstruct manually from partial logs; revision, hash inventory, and terminal process facts may be absent.
- Fix approach: Centralize terminalization in a run owner that always stops children and writes a success/failure manifest.

**Write-store overflow paths are not atomic:**

- Symptoms: `accept_materialized` updates `next_identity` before revision overflow checking, and `cancel` removes a write before that check. Both can return `Refused` after mutation.
- Files: `crates/fava-write-store-memory/src/lib.rs`, `crates/fava-write-store/src/lib.rs`
- Trigger: Reach `u64::MAX` source revision; this is latent rather than a near-term operational limit.
- Workaround: None after exhaustion.
- Fix approach: Precompute all checked counters and a complete next state before mutating the guard.

## Security Considerations

**Cross-context provenance disclosure:**

- Risk: Results reveal evidence from another authorization context and use it to qualify the current context.
- Files: `crates/fava-state/src/lib.rs`, `crates/fava-query/src/lib.rs`, `crates/fava-query-standard/src/lib.rs`
- Current mitigation: Context is retained in `RelaySessionKey` and query identity, but no evaluator rule consumes it.
- Recommendations: Enforce exact-context matching/redaction and the access-isolation conformance cases required by `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md`.

**Untrusted cache input can fabricate validity/provenance:**

- Risk: Invalid bodies or invented observations can enter `MemoryEventCache` and become visible without signature, filter, session, or request verification.
- Files: `crates/fava-state/src/lib.rs`, `crates/fava-event-cache/src/lib.rs`, `crates/fava-event-cache-memory/src/lib.rs`
- Current mitigation: Tests seed genuinely signed events; relay networking is not connected yet.
- Recommendations: Restrict admitted-event creation to future `fava-ingest`/testkit and run forged/wrong-ID/off-filter scenarios before M2 claims in `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`.

**Provider code lacks panic/blocking containment:**

- Risk: An application source/evaluator can panic or block synchronously on a runtime thread, stopping unrelated work on a current-thread runtime and preventing bounded shutdown.
- Files: `crates/fava-observe/src/lib.rs`, `crates/fava/src/lib.rs`, `falsifiers/external-null-cache/src/lib.rs`
- Current mitigation: No Fava lock is held across calls; each post-open observation has its own task.
- Recommendations: Route calls through specified `fava-runtime` isolation, catch panics, apply deadlines/cancellation, and retain provider identity in terminal facts from `docs/spec/ARCHITECTURE.md`.

**Canary relay executable is version-string pinned only:**

- Risk: Any selected binary that prints `nostr-rs-relay 0.8.12` is accepted; the manifest does not hash it.
- Files: `apps/canary/src/relay.rs`, `apps/canary/src/lib.rs`, `apps/canary/README.md`
- Current mitigation: Documentation installs exact version with `--locked` and records selected path/version.
- Recommendations: Record executable SHA-256/provenance and optionally enforce a profile digest.

**No production credential surface exists yet:**

- Risk: Signer, NIP-42, restore, and native key custody are absent, so current code is not security-qualified for real accounts.
- Files: `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md`, `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`, `crates/fava/src/lib.rs`
- Current mitigation: `apps/canary/src/lib.rs` derives disposable keys and does not persist private key material.
- Recommendations: Keep this classified as future M8-M11 work; do not expand M0/M1 claims.

## Performance Bottlenecks

**Provider mutations clone full retained sources:**

- Problem: Both memory providers clone all `BTreeMap` values into `Vec<SourceEvent>` while holding a `std::sync::Mutex`.
- Files: `crates/fava-event-cache-memory/src/lib.rs`, `crates/fava-write-store-memory/src/lib.rs`
- Cause: Global watch channels publish full source snapshots.
- Improvement path: Publish bounded changes or structurally shared snapshots; move expensive construction outside the critical section; benchmark mutation latency versus retained records.

**Observations fully reevaluate/reclone all records:**

- Problem: Each revision clones full sources; `StandardQueryEvaluator` builds two maps, clones values, sorts all winners, then truncates.
- Files: `crates/fava-observe/src/lib.rs`, `crates/fava-query-standard/src/lib.rs`, `crates/fava-query/src/lib.rs`
- Cause: The oracle has no incremental `update` path specified in `docs/spec/ARCHITECTURE.md`.
- Improvement path: Retain full reevaluation as oracle; add affected-coordinate incremental evaluation behind the same corpus; apply safe top-k/index bounds earlier.

**Synchronous wire logging blocks the async proxy:**

- Problem: Every frame locks `Mutex<File>`, serializes, writes, and flushes synchronously on Tokio.
- Files: `apps/canary/src/proxy.rs`
- Cause: Witness durability is coupled to forwarding.
- Improvement path: Use a bounded causal writer queue with explicit overflow failure and join/flush before scenario completion.

**Artifact hashing buffers whole files:**

- Problem: `artifact_hashes` calls `fs::read` for every database, WAL, log, and transcript.
- Files: `apps/canary/src/artifacts.rs`
- Cause: Whole-file rather than streaming hashing.
- Improvement path: Hash chunks, record sizes, and refuse artifact trees beyond the run profile bound.

## Fragile Areas

**Live-query opening and teardown:**

- Files: `crates/fava-observe/src/lib.rs`, `crates/fava-query/src/lib.rs`
- Why fragile: One task owns polling, evaluation, revisioning, delivery, and teardown; role/revision violations, fairness, evaluator failure, and cancellation converge on implicit loop exits.
- Safe modification: Add controlled source/evaluator fixtures first; preserve provisional-open cleanup and source-scoped closure while adding terminal causes/open barriers.
- Test coverage: Existing tests omit concurrent opening changes, revision regression, starvation, panic/block, and runtime evaluation failure.

**Local write identity/query merge:**

- Files: `crates/fava-write-store-memory/src/lib.rs`, `crates/fava-query-standard/src/lib.rs`, `crates/fava-write/src/lib.rs`
- Why fragile: Store identity is receipt-keyed, query deduplication is event-ID-keyed, and `EventRecord` holds one `PublicationEvidence`.
- Safe modification: Decide duplicate-event semantics at the write contract, then test acceptance/cancellation/duplicates/echo/rematerialization as one corpus.
- Test coverage: `crates/fava-write-store-memory/src/lib.rs` has no unit tests.

**Canary process/witness lifecycle:**

- Files: `apps/canary/src/lib.rs`, `apps/canary/src/relay.rs`, `apps/canary/src/proxy.rs`, `apps/canary/src/artifacts.rs`
- Why fragile: Relay, proxy, process, database, and manifest lifecycles rely on function returns/`Drop`; connection errors are printed and swallowed rather than propagated to run state.
- Safe modification: Use one run owner with child registration, terminalization, and join order; make witness failure fail the scenario and enter JSONL/manifest.
- Test coverage: No automated live failure, proxy-write, port-collision, partial-finalization, or cleanup cases.

**Port reservation race:**

- Files: `apps/canary/src/lib.rs`, `apps/canary/src/relay.rs`
- Why fragile: `reserve_port` drops its listener before the relay binds.
- Safe modification: Prefer inherited listener/socket activation or retry complete setup on bind collision with causal evidence.
- Test coverage: No controlled collision case exists.

**Scenario registry/dispatch drift:**

- Files: `apps/canary/scenarios.json`, `apps/canary/src/lib.rs`, `apps/canary/src/main.rs`
- Why fragile: IDs are duplicated across JSON, `has_executor`, and CLI arms; the test checks only the named M0 entry rather than every enabled scenario.
- Safe modification: Use one dispatch table and assert every enabled entry maps to exactly one executor.
- Test coverage: Adding a second enabled but undispatched scenario does not fail the current test.

**M0 evidence portability:**

- Files: `apps/canary/README.md`, `apps/canary/src/artifacts.rs`, `docs/issues/0002-m0-evidence-foundation.md`, `.gitignore`
- Why fragile: Complete bundles exist only under ignored `apps/canary/runs/`; a clone retains prose but not manifest/transcript/logs/hashes.
- Safe modification: Preserve an immutable reviewable summary/archive with explicit retention while keeping large databases out of ordinary history.
- Test coverage: No test proves a documented completed run remains reconstructable from distributed artifacts.

## Scaling Limits

**Memory provider byte growth:**

- Current capacity: 10,000 records per default provider.
- Limit: Content, tags, evidence entries, context strings, and clone work are unbounded, so record count is not a memory bound.
- Files: `crates/fava-event-cache-memory/src/lib.rs`, `crates/fava-write-store-memory/src/lib.rs`, `crates/fava-state/src/lib.rs`, `crates/fava-write/src/lib.rs`
- Scaling path: Define item/tag/evidence/total-byte/work budgets with typed refusal or eviction.

**Query structure/output:**

- Current capacity: ID/author/kind/relay sets and result length have no maximum; `limit` defaults to none.
- Limit: Large queries/source snapshots allocate and evaluate without bound before truncation.
- Files: `crates/fava-query/src/lib.rs`, `crates/fava-query-standard/src/lib.rs`
- Scaling path: Add canonical structure and acquisition/result budgets; refuse before opening; make work proportional to declared bounds.

**Observation/task count:**

- Current capacity: Every `observe` spawns a task and opens two receivers; there is no admission ceiling or equivalent-query sharing.
- Limit: Tasks, receivers, and full reevaluation scale linearly with handles.
- Files: `crates/fava-observe/src/lib.rs`, `crates/fava/src/lib.rs`, `docs/issues/0001-local-source-merge.md`
- Scaling path: Complete semantic identity, shared ownership/refcounts, and typed overload.

**Canary evidence retention:**

- Current capacity: Reconnaissance bounds frame count; local proxy logs, databases, run count, and total bytes lack policy bounds.
- Limit: `apps/canary/runs/` and hashing cost grow indefinitely.
- Files: `apps/canary/src/wire.rs`, `apps/canary/src/proxy.rs`, `apps/canary/src/artifacts.rs`, `apps/canary/README.md`
- Scaling path: Declare per-frame/run/retention budgets and provide evidence-aware archive/pruning.

**Proxy connection fan-out:**

- Current capacity: Unbounded `JoinSet` task per loopback connection.
- Limit: A faulty local process can exhaust tasks/file descriptors.
- Files: `apps/canary/src/proxy.rs`
- Scaling path: Add a connection semaphore, typed overflow evidence, and profile FD/task budgets.

## Dependencies at Risk

**Platform-locked Bazel graph:**

- Risk: `crate_universe` renders only `aarch64-apple-darwin`.
- Impact: The authoritative build is unavailable to Linux, Intel macOS, and the eventual platform matrix.
- Files: `MODULE.bazel`, `.bazeliskrc`, `rust-toolchain.toml`, `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`
- Migration plan: Add tested triples/execution platforms as milestones require while retaining exact tool pins.

**Locally installed relay prerequisite:**

- Risk: M0 relies on an executable outside the repository; version output is checked, but binary digest/provenance are not.
- Impact: Clean-machine reproduction depends on repeating local Cargo installation.
- Files: `apps/canary/README.md`, `apps/canary/src/relay.rs`, `docs/issues/0002-m0-evidence-foundation.md`
- Migration plan: Add pinned acquisition/verification or a hermetic profile and record executable digest.

**Dual build metadata:**

- Risk: Cargo owns dependency metadata while first-party target/test lists are duplicated in `BUILD.bazel`.
- Impact: A crate/test can pass one build and be absent or differently wired in the other.
- Files: `Cargo.toml`, `crates/*/Cargo.toml`, `MODULE.bazel`, `crates/*/BUILD.bazel`
- Migration plan: Generate/check BUILD targets from Cargo metadata or compare graphs in a drift test.

## Missing Critical Features

**M1 exit gates remain open — current milestone work:**

- Problem: Equivalent-query identity/sharing, deletion/expiry, `local-source-removal`, and the shared semantic corpus are absent.
- Blocks: M1 completion and a full local semantic-state claim.
- Files: `docs/issues/0001-local-source-merge.md`, `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`, `crates/fava-state/src/lib.rs`, `crates/fava-query/src/lib.rs`, `apps/canary/scenarios.json`

**No public-facade write operation — M1 gap:**

- Problem: `Fava` exposes only `observe`; tests call `MemoryWriteStore` directly.
- Blocks: The M1 canary gate requiring only public-facade queries/writes.
- Files: `crates/fava/src/lib.rs`, `crates/fava/tests/local_source_merge.rs`, `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`

**`Freshness::Live` is public before live demand exists:**

- Problem: Queries default to `Live`, and the facade says live query, but the observer opens local sources only and neither creates relay demand nor refuses unsupported live behavior.
- Blocks: QUERY-013 for callers that do not explicitly select `cache_only`.
- Files: `crates/fava-query/src/lib.rs`, `crates/fava-observe/src/lib.rs`, `crates/fava/src/lib.rs`, `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md`

**Diagnostics for implemented owners are absent:**

- Problem: No public facts expose query identity/source count, provider profile/capacity, coalescing, or terminal causes.
- Blocks: The cross-cutting diagnostic rule and failure attribution.
- Files: `crates/fava/src/lib.rs`, `crates/fava-observe/src/lib.rs`, `crates/fava-event-cache-memory/src/lib.rs`, `crates/fava-write-store-memory/src/lib.rs`, `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`

**M2-M11 are future work, not current regressions:**

- Problem: Relay ingest/transport, planning/routing, durable publication/recovery, capabilities, auth/hostile limits, persistent profiles, provider qualification, and Swift/Kotlin are absent.
- Blocks: M2-M11 and release only; current docs do not claim them complete.
- Files: `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`, `docs/spec/ARCHITECTURE.md`, `README.md`, `Cargo.toml`

**Five product decisions intentionally remain open:**

- Problem: Windowing, partial-handoff cancellation, outage backfill, full delivery history, and recommended persistent event cache are unresolved.
- Blocks: Later owning milestone API/profile choices; these are decision blockers, not bugs.
- Files: `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md`, `docs/issues/0001-local-source-merge.md`

## Test Coverage Gaps

**Replacement/state semantics:**

- What's not tested: Equal-timestamp lowest-ID winner, deletion/tombstones, expiry, resurrection prevention, and broad property corpora.
- Files: `crates/fava-state/src/lib.rs`, `crates/fava-query-standard/tests/source_merge.rs`, `docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md`
- Risk: Protocol-divergent state passes example tests.
- Priority: High

**Access/evidence isolation:**

- What's not tested: Cross-context reads, exact session qualification, evidence redaction, and fabricated evidence.
- Files: `crates/fava-state/src/lib.rs`, `crates/fava-query-standard/tests/source_merge.rs`, `crates/fava/tests/local_source_merge.rs`
- Risk: Authorization-context evidence leaks unnoticed.
- Priority: High

**Observation concurrency/failure:**

- What's not tested: Changes during open, starvation, revision regression, role mismatch, provider panic/block, later evaluator refusal, close races, and pending-pull cancellation.
- Files: `crates/fava-observe/src/lib.rs`, `crates/fava-query/src/lib.rs`
- Risk: Stale initial results, hung updates, and unattributed termination.
- Priority: High

**Write-store contract:**

- What's not tested: Duplicate event acceptance, capacity atomicity, unknown cancellation, counter overflow, and a provider-shared corpus.
- Files: `crates/fava-write-store-memory/src/lib.rs`, `crates/fava-write-store/src/lib.rs`, `crates/fava/tests/local_source_merge.rs`
- Risk: The store commits evaluator-refused state or fails after mutation.
- Priority: High

**Provider conformance:**

- What's not tested: The null cache proves open only; ordinary/malformed/cancel/late/overload/restart/context cases and external evaluator/write-store proofs are absent.
- Files: `falsifiers/external-null-cache/src/lib.rs`, `crates/fava-event-cache/src/lib.rs`, `crates/fava-query/src/lib.rs`, `crates/fava-write-store/src/lib.rs`, `docs/spec/ARCHITECTURE.md`
- Risk: Replaceable contracts rely on private conventions or violate invariants.
- Priority: High

**Bazel authoritative gate:**

- What's not tested: Bazel omits unit tests in state, cache, and observer plus canary/falsifier tests.
- Files: `.bazelrc`, `crates/fava-state/BUILD.bazel`, `crates/fava-event-cache-memory/BUILD.bazel`, `crates/fava-observe/BUILD.bazel`, `apps/canary/Cargo.toml`, `falsifiers/external-null-cache/Cargo.toml`
- Risk: The advertised gate is weaker than the separate Cargo set.
- Priority: High

**M0 live/failure evidence:**

- What's not tested: Fast tests do not launch the relay, require complete failed bundles, inject witness failures, collide ports, or prove cleanup.
- Files: `apps/canary/src/lib.rs`, `apps/canary/src/relay.rs`, `apps/canary/src/proxy.rs`, `apps/canary/src/artifacts.rs`, `docs/issues/0002-m0-evidence-foundation.md`
- Risk: Process/evidence regressions survive all automated tests.
- Priority: High

**Mutation proof durability:**

- What's not tested: Falsifiers exist as feature comments/issue prose, not a checked-in rerunnable mutation harness.
- Files: `features/local-source-merge.feature`, `features/relay-lab.feature`, `docs/issues/0001-local-source-merge.md`, `docs/issues/0002-m0-evidence-foundation.md`
- Risk: Tests can stop detecting their claimed mechanisms.
- Priority: Medium

**Performance/boundedness:**

- What's not tested: Allocation/latency versus records/observations, byte limits, fairness, task/FD ceilings, proxy throughput, artifact size, and typed overload.
- Files: `crates/fava-event-cache-memory/src/lib.rs`, `crates/fava-write-store-memory/src/lib.rs`, `crates/fava-observe/src/lib.rs`, `crates/fava-query-standard/src/lib.rs`, `apps/canary/src/proxy.rs`, `apps/canary/src/artifacts.rs`
- Risk: Count-bounded APIs exceed memory/latency budgets or starve work.
- Priority: Medium

---

*Concerns audit: 2026-08-20*
