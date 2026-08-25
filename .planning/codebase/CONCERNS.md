# Codebase Concerns

**Analysis Date:** 2026-08-21

## Scope Boundary

The implemented baseline is M0-M6. Completion claims and evidence owners are
recorded in `docs/issues/0002-m0-evidence-foundation.md`,
`docs/issues/0001-local-source-merge.md`, and
`docs/issues/0004-explicit-live-query.md` through
`docs/issues/0008-automatic-write-routing.md`. This document distinguishes
defects and risks in that implementation from the unimplemented M7-M11 scope
specified in `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`.

## Tech Debt

**Universal event-state rules stop at the event-cache boundary:**
- Issue: NIP-09 deletion is applied only to the current `EventCache` slice. The
  retained deletion event is not interpreted by the cross-source evaluator, so
  a matching `WriteStore` contribution remains visible.
- Files: `crates/fava-state/src/lib.rs`, `crates/fava-event-cache/src/lib.rs`,
  `crates/fava-query-standard/src/lib.rs`, `crates/fava-write-store/src/lib.rs`
- Impact: The merged application view can contain a locally accepted event
  after an authorized deletion, contrary to the universal current-state rule.
- Fix approach: Apply deletion tombstones at the owner of the merged state, or
  expose an invariant-bearing state decision that every source/evaluator must
  consume; prove cached-only, local-only, and merged targets with one corpus.

**Expiry has no lifecycle owner:**
- Issue: Expiry occurs only when a caller directly invokes
  `EventCache::expire(now)`. No Fava clock/maintenance owner calls it, and the
  write-store contract has no operation that retracts a locally accepted event
  when its future NIP-40 expiration becomes due.
- Files: `crates/fava-event-cache/src/lib.rs`,
  `crates/fava-event-cache-memory/src/lib.rs`,
  `crates/fava-write-store/src/lib.rs`, `crates/fava/src/lib.rs`,
  `apps/canary/src/local.rs`
- Impact: Open queries can retain expired cached or local events indefinitely;
  the M1 canary proves manual expiry mutation, not due-time ownership.
- Fix approach: Add a deterministic-time expiry owner that schedules exact
  cache and local-source retractions without unrelated query sweeps; keep the
  clock contract replaceable and test time advance through the public facade.

**The event-cache mutation contract requires trusted admitted input:**
- Issue: Public providers can call `EventCache::commit` with raw
  `EventStateMutation` values. `RelayEvent` construction validates event/session
  identity but does not itself prove subscription attribution or filter match.
- Files: `crates/fava-state/src/lib.rs`, `crates/fava-event-cache/src/lib.rs`,
  `crates/fava-event-cache-memory/src/lib.rs`, `crates/fava-ingest/src/lib.rs`
- Impact: Custom cache providers remain responsible for exposing only state
  admitted through their trusted assembly boundary.
- Fix approach: Keep live admission owned by `fava-observe`; treat cache
  retention as optional and do not use a cache write as live-admission proof.

**Local-source contracts trust invariant-bearing provider output:**
- Issue: `SourceSnapshot` lets a provider self-report `SourceKind`, revision,
  status, and either source-event variant. `Observer` replaces a source by the
  reported kind and accepts duplicate or regressing revisions.
- Files: `crates/fava-query/src/lib.rs`, `crates/fava-observe/src/lib.rs`,
  `crates/fava/tests/source_contract.rs`
- Impact: A malformed external source can overwrite the other source role,
  regress evidence, or inject the wrong semantic contribution without a typed
  refusal.
- Fix approach: Bind role at assembly/open, validate strictly increasing
  revisions and role-specific payloads in `fava-observe`, and publish a shared
  negative conformance corpus.

**Provider calls have no execution isolation:**
- Issue: Source open, query evaluation, router open/preview, write-store calls,
  and provider availability checks execute synchronously on caller or Tokio
  tasks. Panics, blocking calls, ignored cancellation, and late results have no
  containment boundary.
- Files: `crates/fava-observe/src/lib.rs`, `crates/fava/src/lib.rs`,
  `crates/fava-routing/src/chain.rs`, `crates/fava-publication/src/run.rs`
- Impact: One application-selected provider can block unrelated query,
  publication, and shutdown progress.
- Fix approach: Introduce bounded provider execution with operation and
  generation identity, panic capture, cancellation deadlines, and stale-result
  rejection; keep the contract separate from its runtime implementation.

**Large owner modules have no growth room:**
- Issue: `crates/fava-query/src/lib.rs` is 500 lines, while
  `apps/canary/src/lib.rs`, `crates/fava-write-store-memory/src/lib.rs`,
  `crates/fava-write/src/lib.rs`, and `crates/fava-routing/src/chain.rs` are near
  the 500-line soft limit.
- Files: `crates/fava-query/src/lib.rs`, `apps/canary/src/lib.rs`,
  `crates/fava-write-store-memory/src/lib.rs`, `crates/fava-write/src/lib.rs`,
  `crates/fava-routing/src/chain.rs`, `AGENTS.md`
- Impact: M7 additions in these files either cross the repository limit or mix
  query syntax, evidence, source contracts, lifecycle, and protocol-edit state.
- Fix approach: Split only along existing ownership boundaries: query value,
  source contract, result evidence, write values, and routing composition.

**Validation remains a multi-command surface:**
- Issue: Bazel covers the Rust crate graph and many integration targets, but it
  does not build the separate `apps/canary` or
  `falsifiers/external-null-cache` workspaces, run vocabulary checks, or execute
  unit tests embedded in libraries lacking a `rust_test` target.
- Files: `.bazelrc`, `BUILD.bazel`, `crates/*/BUILD.bazel`,
  `apps/canary/Cargo.toml`, `falsifiers/external-null-cache/Cargo.toml`,
  `tools/check_vocabulary.py`
- Impact: `bazel test //...` alone is weaker than the milestone validation set
  recorded in `docs/issues/0001-local-source-merge.md` and later issue ledgers.
- Fix approach: Keep one checked-in validation entry point that invokes Bazel,
  the two independent Cargo workspaces, and vocabulary tests, or add equivalent
  Bazel targets without weakening the external-workspace proof.

## Known Bugs

**Authorized deletion does not retract a matching local write:**
- Symptoms: A valid kind:5 event removes a cached target, but
  `StandardQueryEvaluator` still emits the same or another matching event from
  `WriteStore` because it performs no deletion-tombstone pass across sources.
- Files: `crates/fava-state/src/lib.rs`,
  `crates/fava-query-standard/src/lib.rs`,
  `crates/fava/tests/local_source_merge.rs`
- Trigger: Accept an event through `Fava::accept_event`, open a matching query,
  then admit an authorized deletion through the event cache.
- Workaround: Cancel the local receipt separately; this is not equivalent to
  applying the NIP-09 fact.

**Future expiration does not retract automatically:**
- Symptoms: Events accepted before their expiration remain in current queries
  after the timestamp passes unless external code calls the cache provider's
  `expire` method; local write events have no corresponding call.
- Files: `crates/fava-write/src/lib.rs`, `crates/fava-event-cache/src/lib.rs`,
  `crates/fava-write-store/src/lib.rs`, `crates/fava/src/lib.rs`
- Trigger: Admit or accept an event whose expiration is in the future, retain
  the open observation, and let that timestamp pass.
- Workaround: Directly call `EventCache::expire` for cached state; no public
  Fava workaround exists for a local write contribution.

**Duplicate local acceptance terminates query evaluation:**
- Symptoms: The memory and Redb stores allocate distinct receipts for the same
  deterministic event id. `StandardQueryEvaluator` finds conflicting single
  `PublicationEvidence` values and refuses the snapshot; an already-open
  observation then closes without the cause.
- Files: `crates/fava-write-store-memory/src/lib.rs`,
  `crates/fava-write-store-redb/src/ops.rs`,
  `crates/fava-query-standard/src/lib.rs`, `crates/fava-observe/src/lib.rs`
- Trigger: Accept the same finalized unsigned or signed event twice, then open
  or update a matching query.
- Workaround: Applications must deduplicate before acceptance, although the
  public write contract does not require it.

**Redb terminal eviction can diverge memory from durable state:**
- Symptoms: `terminal_evictions` may select the receipt currently being
  updated. `commit_update` inserts and then removes that receipt in Redb, while
  the in-memory update removes and then reinserts it.
- Files: `crates/fava-write-store-redb/src/ops.rs`,
  `crates/fava-write-store-redb/src/lib.rs`
- Trigger: With a small terminal bound, complete an older active receipt after
  enough newer receipts are terminal so that the older updated id is selected
  for eviction; it exists before restart and disappears after reopen.
- Workaround: Use a terminal bound that is never reached.

**Automatic Redb evictions emit no receipt-removal fact:**
- Symptoms: Terminal retention eviction removes receipts and changes the query
  snapshot, but `receipt_changes` publishes only the updated receipt and no
  `(evicted_id, None)` items.
- Files: `crates/fava-write-store-redb/src/ops.rs`,
  `crates/fava-write-store-redb/src/lib.rs`,
  `crates/fava-write-store/src/lib.rs`
- Trigger: Exceed the configured terminal-receipt bound while a receipt-change
  subscriber is current.
- Workaround: Poll every receipt id after every unrelated update; this defeats
  the causal removal contract.

**Configured WebSocket inbound frame bound is not enforced:**
- Symptoms: `WebSocketTransport::bounded` checks outbound `send` size only;
  `next_message` returns arbitrarily larger text messages accepted by the
  underlying default WebSocket configuration.
- Files: `crates/fava-transport-websocket/src/lib.rs`,
  `crates/fava-transport-websocket/tests/conformance.rs`
- Trigger: Configure a small bound and have a relay send a larger text frame.
- Workaround: Place a separately bounded proxy in front of the transport.

**A slow first relay blocks later known relays:**
- Symptoms: Explicit and automatic relay additions await `OpenedRelay::open`
  sequentially, and WebSocket connection establishment has no Fava deadline.
  One slow DNS/TCP/TLS open delays every later relay and can prevent the initial
  observation handle from returning.
- Files: `crates/fava/src/live.rs`, `crates/fava/src/routes.rs`,
  `crates/fava/src/relay.rs`, `crates/fava-transport-websocket/src/lib.rs`
- Trigger: Put a silent or slow endpoint before a healthy endpoint in an exact
  relay set or route plan.
- Workaround: Avoid mixed-health relay sets; ordering inside `BTreeSet` is not an
  application-controlled isolation mechanism.

**Post-open evaluation failure loses its cause:**
- Symptoms: A later `QueryEvaluator` refusal exits the observation task. The
  application receives only `ObservationClosed`, identical to explicit close,
  source teardown, or revision exhaustion.
- Files: `crates/fava-observe/src/lib.rs`, `crates/fava-query/src/lib.rs`
- Trigger: Use an evaluator that accepts the initial sources and refuses a later
  source revision, including the duplicate-local-event case.
- Workaround: Provider-private logging; Fava diagnostics contain no evaluator
  terminal fact.

**Failed M0 smoke runs omit the reconstructable manifest:**
- Symptoms: The failure path writes best-effort stderr, JSONL, and a report, then
  returns without artifact hashes, process inventory, toolchain facts, or
  `manifest.json`.
- Files: `apps/canary/src/lib.rs`, `apps/canary/src/artifacts.rs`,
  `docs/issues/0002-m0-evidence-foundation.md`
- Trigger: Fail the smoke scenario after `RunArtifacts::create` but before
  `finish_success`.
- Workaround: Reconstruct the partial run manually from whichever files were
  flushed before failure.

## Security Considerations

**Fabricated provenance through public cache mutation:**
- Risk: Any holder of the selected `EventCache` can attach an arbitrary
  `RelaySessionKey` to a valid signed event and bypass subscription attribution.
- Files: `crates/fava-state/src/lib.rs`, `crates/fava-event-cache/src/lib.rs`,
  `crates/fava-ingest/src/lib.rs`
- Current mitigation: The production relay path uses
  `admit_subscription_event`; `MemoryEventCache::commit` independently verifies
  event signatures.
- Recommendations: Seal admitted provenance behind the ingestion/state owner
  and provide a separate hostile-input testkit.

**Relay-controlled memory growth:**
- Risk: Inbound WebSocket text, CLOSED/NOTICE/error strings, relay evidence per
  event, and event-cache item bytes are not bounded by aggregate memory limits.
- Files: `crates/fava-transport-websocket/src/lib.rs`,
  `crates/fava-diagnostics/src/lib.rs`, `crates/fava-state/src/lib.rs`,
  `crates/fava-event-cache-memory/src/lib.rs`
- Current mitigation: Diagnostics retain 256 entries per category and the
  memory cache retains 10,000 records, but neither is a byte bound.
- Recommendations: Reject oversized inbound frames before allocation where
  possible, bound retained text/evidence/item bytes, and expose exact overload
  facts.

**Relay executable identity is version-string based:**
- Risk: The canary accepts the configured binary after checking its reported
  `nostr-rs-relay 0.8.12` version; the evidence manifest does not hash the
  executable.
- Files: `apps/canary/src/relay.rs`, `apps/canary/src/lib.rs`,
  `apps/canary/README.md`
- Current mitigation: Installation documentation uses an exact version and
  `--locked`, and the selected path/command is recorded.
- Recommendations: Record the binary SHA-256 and build provenance in every
  live-run manifest.

## Performance Bottlenecks

**Whole-source cloning on every mutation:**
- Problem: Memory cache and both write stores rebuild complete
  `SourceSnapshot` vectors while holding synchronous mutexes; watch channels
  retain full immutable snapshots.
- Files: `crates/fava-event-cache-memory/src/lib.rs`,
  `crates/fava-write-store-memory/src/lib.rs`,
  `crates/fava-write-store-redb/src/lib.rs`
- Cause: The source contract publishes complete replacement state only.
- Improvement path: Keep full snapshots as the conformance oracle, then use
  structural sharing or bounded deltas internally while preserving coherent
  open plus gapless revision semantics.

**Every query revision performs a full merge and sort:**
- Problem: The standard evaluator builds maps over every source event, merges
  evidence, resolves every coordinate, sorts all winners, then truncates.
- Files: `crates/fava-query-standard/src/lib.rs`,
  `crates/fava-observe/src/lib.rs`
- Cause: No affected-coordinate incremental path exists.
- Improvement path: Add an incremental evaluator behind the same exact corpus;
  keep `StandardQueryEvaluator` as the simple oracle.

**Known relay acquisition is sequential:**
- Problem: `add_relays` and explicit opening perform network setup one relay at
  a time.
- Files: `crates/fava/src/live.rs`, `crates/fava/src/routes.rs`,
  `crates/fava/src/relay.rs`
- Cause: Opening and rollback are represented as a serial loop.
- Improvement path: Open bounded concurrent relay operations with deterministic
  result attribution and all-or-nothing rollback only where the query contract
  requires it.

**Canary witness I/O blocks async tasks:**
- Problem: Proxy frames and evidence lines use synchronous file locks, writes,
  and per-record flushes on Tokio tasks; artifact hashing buffers each complete
  file.
- Files: `apps/canary/src/proxy.rs`, `apps/canary/src/artifacts.rs`
- Cause: Witness durability is coupled directly to forwarding and scenario
  control flow.
- Improvement path: Use a bounded causal writer queue with explicit overflow,
  join/flush it during terminalization, and stream artifact hashes.

## Fragile Areas

**Publication worker lifecycle:**
- Files: `crates/fava-publication/src/lib.rs`,
  `crates/fava-publication/src/run.rs`
- Why fragile: Detached signing, routing, and per-destination tasks communicate
  through store polling and a cancellation map. Most store/provider errors are
  discarded, so an open receipt can lose its active worker without a terminal
  diagnostic.
- Safe modification: Give one receipt run exact child-task ownership and a
  terminal error/parked fact; reject late completions by receipt, destination,
  attempt, and generation identity.
- Test coverage: No corpus injects store failure, provider panic/block, ignored
  cancellation, or late signer/publisher completion.

**Relay reconnect and terminal protocol handling:**
- Files: `crates/fava/src/relay.rs`,
  `crates/fava-transport-websocket/src/lib.rs`,
  `crates/fava-diagnostics/src/lib.rs`
- Why fragile: Transport error triggers replacement without an explicit close
  of the prior provider session; CLOSED and AUTH frames are recorded but do not
  change subscription work; reconnect retries indefinitely at a fixed 50 ms.
- Safe modification: Model session/subscription terminal state explicitly,
  close every retired generation, apply bounded backoff, and keep late frames
  attributable to their exact generation.
- Test coverage: Existing M2/M3 tests cover disconnect/reconnect identity, not
  provider sessions that remain live after error, repeated refusal, CLOSED
  continuation, or AUTH state.

**Redb retention and recovery:**
- Files: `crates/fava-write-store-redb/src/lib.rs`,
  `crates/fava-write-store-redb/src/ops.rs`,
  `crates/fava-write-store-redb/tests/process_kill.rs`
- Why fragile: Persistent update, terminal eviction, in-memory publication,
  broadcast facts, and restart repair are separate steps around one mutex.
- Safe modification: Compute one next durable state, commit it, mirror that
  exact state in memory, then publish every changed/removal fact.
- Test coverage: Process-kill tests cover acceptance through outcome/cancel
  boundaries, but not retention eviction, eviction during the updated receipt,
  or reopen under different configured limits.

**Observation source loop:**
- Files: `crates/fava-observe/src/lib.rs`, `crates/fava-query/src/lib.rs`
- Why fragile: Biased polling, source role replacement, closure evidence,
  evaluation, observation revisioning, and teardown share one task. A
  continuously ready cache branch can delay the write-store branch.
- Safe modification: Bind roles outside snapshots, validate revisions, poll
  sources fairly or in bounded round-robin order, and deliver a typed terminal
  observation fact.
- Test coverage: No test covers wrong source role, revision regression,
  evaluator failure after open, starvation, or revision exhaustion.

**Canary process and evidence terminalization:**
- Files: `apps/canary/src/lib.rs`, `apps/canary/src/relay.rs`,
  `apps/canary/src/proxy.rs`, `apps/canary/src/artifacts.rs`
- Why fragile: Success paths assemble manifests explicitly; failure paths and
  several scenario modules rely on local cleanup sequences and `Drop`.
  `reserve_port` releases its listener before the relay binds.
- Safe modification: Use one run owner that registers every child/socket/file,
  terminalizes success and failure, and either transfers an already-bound
  listener or retries the complete setup after a recorded collision.
- Test coverage: No controlled port collision, partial-manifest failure,
  proxy-writer failure, or cleanup resource-baseline scenario exists.

**Evidence portability:**
- Files: `.gitignore`, `apps/canary/README.md`,
  `docs/issues/0002-m0-evidence-foundation.md`,
  `docs/issues/0004-explicit-live-query.md` through
  `docs/issues/0008-automatic-write-routing.md`
- Why fragile: Complete live bundles are under ignored `apps/canary/runs/` or
  milestone worktrees. A clean clone retains outcome prose but not the manifests,
  wire transcripts, databases, or artifact hashes.
- Safe modification: Publish immutable evidence bundles or compact signed hash
  inventories at a durable review location while excluding large mutable run
  directories from ordinary source history.
- Test coverage: No gate proves that another checkout can retrieve and verify a
  completed milestone bundle.

## Scaling Limits

**Memory event cache:**
- Current capacity: 10,000 retained event ids by default.
- Limit: Event bytes, tags, evidence observations, snapshot clones, and total
  memory have no aggregate budget.
- Files: `crates/fava-event-cache-memory/src/lib.rs`,
  `crates/fava-state/src/lib.rs`
- Scaling path: Add item, evidence, and total-byte limits with typed refusal or
  coherent eviction.

**Queries and observations:**
- Current capacity: Query id/author/kind sets have no construction bound;
  result limit is optional; each observation retains one latest snapshot.
- Limit: Evaluation still processes the complete source state before result
  truncation, and the observation count has no assembly-level ceiling.
- Files: `crates/fava-query/src/lib.rs`,
  `crates/fava-query-standard/src/lib.rs`, `crates/fava-observe/src/lib.rs`
- Scaling path: Refuse oversized query structure before opening work, declare an
  observation/session budget, and apply safe indexed bounds before full sort.

**Automatic read routing:**
- Current capacity: 32 routers, each contributing up to 256 destinations; a
  combined read plan can therefore retain and attempt thousands of sessions.
- Limit: There is no Fava-wide relay-session pool or per-application network
  resource budget.
- Files: `crates/fava-routing/src/chain.rs`, `crates/fava/src/routes.rs`,
  `crates/fava-transport-websocket/src/lib.rs`
- Scaling path: Add bounded session pooling and typed route shortfall before
  opening excess relay work.

**Diagnostics:**
- Current capacity: 256 facts per category by default.
- Limit: Text bytes, route destination vectors, snapshot clone size, and total
  diagnostic memory are unbounded; eviction count is not reported.
- Files: `crates/fava-diagnostics/src/lib.rs`
- Scaling path: Bound bytes and nested collection sizes, report coalesced or
  evicted diagnostic counts, and expose typed refusal where exact retention is
  required.

**Durable receipts:**
- Current capacity: 10,000 active and 10,000 terminal receipts in the standard
  Redb profile; the memory store retains 10,000 total receipts until explicit
  removal.
- Limit: Recovered snapshots clone every retained non-cancelled event, and Redb
  retention eviction has correctness defects described above.
- Files: `crates/fava-write-store-redb/src/lib.rs`,
  `crates/fava-write-store-redb/src/ops.rs`,
  `crates/fava-write-store-memory/src/lib.rs`
- Scaling path: Fix exact eviction first, then benchmark recovery, snapshot
  publication, and receipt-change fan-out at declared limits.

## Dependencies at Risk

**External relay executable:**
- Risk: Live proof depends on a locally installed `nostr-rs-relay 0.8.12`
  executable selected by path and version output.
- Impact: A nominally equal binary can differ by platform, build inputs, or
  tampering while producing the same version string.
- Migration plan: Pin and publish a reproducible binary/container digest while
  retaining a separately built second relay implementation for interoperability.
- Files: `apps/canary/src/relay.rs`, `apps/canary/README.md`,
  `docs/issues/0002-m0-evidence-foundation.md`

**Separate dependency graphs:**
- Risk: The root workspace, canary workspace, and external falsifier have
  independent manifests/locks and can drift despite current exact pins.
- Impact: One validation surface can compile against versions not exercised by
  another.
- Migration plan: Check compatible exact versions across all three manifests
  without merging the external falsifier into the workspace it is meant to
  challenge.
- Files: `Cargo.toml`, `Cargo.lock`, `apps/canary/Cargo.toml`,
  `apps/canary/Cargo.lock`, `falsifiers/external-null-cache/Cargo.toml`,
  `falsifiers/external-null-cache/Cargo.lock`

## Missing Critical Features

These are specified M7-M11 scopes, not defects in the completed M0-M6 slices.

**M7 replaceable-event edits and protocol composition:**
- Problem: No `ReplaceableEventEdit` payload/store lifecycle, rematerialization
  generation, inverse operation, `fava-nip02`, or second protocol crate exists.
- Blocks: Protocol-owned follow/unfollow, stable receipt across source-driven
  rematerialization, and stale generation rejection.
- Files: `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`,
  `crates/fava-write/src/lib.rs`, `crates/fava-publication/src/run.rs`,
  `Cargo.toml`

**M8 authentication and hostile-boundary qualification:**
- Problem: NIP-42 execution, NIP-11 relay-limit planning, bounded provider
  execution, session pooling, hostile-frame handling, and complete resource
  envelopes are absent.
- Blocks: Authenticated profiles and claims that hostile or blocking providers
  cannot affect unrelated work.
- Files: `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`,
  `crates/fava/src/relay.rs`, `crates/fava-transport-websocket/src/lib.rs`,
  `crates/fava-diagnostics/src/lib.rs`

**M9 cache and service profiles:**
- Problem: Only the memory event cache exists; persistent event-cache, generic
  fetch cache, NIP-05, NIP-11 service semantics, profile declaration, and
  destructive reset are absent.
- Blocks: Truthful persistent/ephemeral cache guarantees and service-cache
  freshness/restart claims.
- Files: `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`,
  `crates/fava-event-cache-memory/src/lib.rs`, `Cargo.toml`

**M10 full provider substitution matrix:**
- Problem: The no-grouping planner exists and the external null-cache falsifier
  proves limited outside-workspace assembly, but no alternative durable write
  store, router, transport, publisher, signer, delivery policy, or full shared
  conformance matrix exists.
- Blocks: Repository-wide replaceability qualification and provider contract
  stabilization.
- Files: `falsifiers/external-null-cache/src/lib.rs`,
  `crates/fava-subscriptions-no-grouping/src/lib.rs`,
  `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`

**M11 native products and release qualification:**
- Problem: No FFI projection, Swift package, Kotlin/JVM package, Android AAR,
  iOS artifact, parity inventory, or real-device process evidence exists.
- Blocks: Native product and cross-language behavioral-equivalence claims.
- Files: `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`, `Cargo.toml`

## Test Coverage Gaps

**Cross-source deletion and due-time expiry:**
- What's not tested: Deletion of a local or merged event, automatic cached-event
  expiry, local-write expiry, and expiry while an observation remains open.
- Files: `crates/fava/tests/local_source_merge.rs`,
  `apps/canary/src/local.rs`
- Risk: M1 current-state rules pass only when tests call the provider mutation
  directly.
- Priority: High

**Duplicate local event identity:**
- What's not tested: Two acceptances of the same deterministic event id before
  query open and while a query is live, for both memory and Redb stores.
- Files: `crates/fava-write-store-memory/src/lib.rs`,
  `crates/fava-write-store-redb/src/ops.rs`,
  `crates/fava-query-standard/tests/source_merge.rs`
- Risk: A valid public write sequence refuses or closes ordinary queries.
- Priority: High

**Redb retention eviction:**
- What's not tested: Updated-receipt eviction, exact `None` broadcasts for
  automatic eviction, restart parity after eviction, and configured-bound
  changes across reopen.
- Files: `crates/fava-write-store-redb/tests/process_kill.rs`,
  `crates/fava-write-store-redb/src/ops.rs`
- Risk: Durable and in-memory receipt truth diverge.
- Priority: High

**Hostile transport bounds and connection isolation:**
- What's not tested: Oversized inbound text, never-completing connect, slow-first
  relay with a healthy later relay, repeated reconnect refusal, and a provider
  session that remains live after returning an error.
- Files: `crates/fava-transport-websocket/tests/conformance.rs`,
  `crates/fava/tests/multi_relay.rs`, `crates/fava/tests/automatic_routes.rs`
- Risk: One relay causes excess memory or blocks unrelated progress.
- Priority: High

**Malformed source/evaluator behavior:**
- What's not tested: Wrong source role, duplicate/regressing source revision,
  wrong source-event variant, evaluator panic/block, and post-open evaluator
  refusal with an application-visible cause.
- Files: `crates/fava-observe/src/lib.rs`,
  `crates/fava/tests/source_contract.rs`
- Risk: Replaceable providers can violate universal observation facts or close
  work without attribution.
- Priority: High

**Canary failure evidence and retrieval:**
- What's not tested: Manifest creation on every failure path, cleanup after
  partial setup, artifact hash streaming, and verification of evidence from a
  fresh checkout.
- Files: `apps/canary/src/lib.rs`, `apps/canary/src/artifacts.rs`,
  `apps/canary/src/proxy.rs`, `.gitignore`
- Risk: A failed or historical live claim cannot be reconstructed independently.
- Priority: Medium

---

*Concerns audit: 2026-08-21*
