---
phase: 07-semantic-writes-and-capability-composition
plan: 04
subsystem: publication
tags: [rust, semantic-writes, exact-identity, failure-isolation, bounded-concurrency, tdd]
requires:
  - phase: 07-semantic-writes-and-capability-composition
    plan: 02
    provides: atomic semantic custody, bounded retained evidence, and generation compare-and-set
  - phase: 07-semantic-writes-and-capability-composition
    plan: 03
    provides: live semantic materialization, source observation, signing, routing, and publication
provides:
  - exact write, receipt, materialization, event, route, session, and attempt correlation across asynchronous publication work
  - bounded receipt-scoped lane completion with exact stale-completion retirement
  - bounded public materialization-failure attribution for error, panic, malformed output, timestamp, and evidence exhaustion
  - one recovery retry for a failed source and atomic failure clearing on successful successor installation
affects: [07-05, 07-06, write-store-redb, publication, publisher, recovery]
actuals:
  tokens: 24824
  tasks: 2
  commits: 8
tech-stack:
  added: []
  patterns:
    - store compare-and-set is the sole authority for asynchronous write mutation
    - every completion echoes the immutable identity captured before work starts
    - provider panic is isolated outside store locks and converted to bounded existing publication evidence
    - bounded lane completion uses the store-owned destination evidence capacity
key-files:
  created:
    - crates/fava-publication/src/delivery.rs
    - crates/fava-write-store-memory/src/lifecycle.rs
    - crates/fava/tests/semantic_write_publication/interleavings.rs
    - crates/fava/tests/semantic_write_failures.rs
    - crates/fava/tests/semantic_write_failures/support.rs
  modified:
    - crates/fava-publication/src/lib.rs
    - crates/fava-publication/src/materialization.rs
    - crates/fava-publication/src/run.rs
    - crates/fava-publisher/src/lib.rs
    - crates/fava-write-store/src/lib.rs
    - crates/fava-write-store-redb/src/ops.rs
    - crates/fava-write-store-memory/src/lib.rs
    - crates/fava/BUILD.bazel
key-decisions:
  - "The store mutation itself is the exact-current authority; publication performs no read-before-write admission or completion check."
  - "Existing write-store active capacity bounds semantic runners because every runner owns one admitted active receipt; recovery starts retained receipts before admitting any capacity not already reserved by the store."
  - "Lane completion carries an exact private tuple rather than introducing a public completion or configuration noun."
  - "Materializer panic and controlled failure text use the existing PublicationEvidence.materialization_failure field; no failure wrapper was added."
patterns-established:
  - "Exact completion CAS: WriteId plus ReceiptId plus MaterializationId plus EventId, extended by RelaySessionKey and exact attempt number for delivery."
  - "Failure retry: suppress the same failed source during one live run, retry a changed source immediately, and retry the persisted failed source once on recovery."
requirements-completed: [CAP-02, CAP-03, CAP-05]
coverage:
  - id: D1
    description: "Retired, simultaneous, duplicated, and cancelled semantic completions remain attributable and cannot mutate the current generation."
    requirement: CAP-03
    verification:
      - kind: integration
        ref: "crates/fava/tests/semantic_write_publication/interleavings.rs#retired_completion_is_attributable_and_inert"
        status: pass
      - kind: integration
        ref: "crates/fava/tests/semantic_write_publication/interleavings.rs#simultaneous_source_and_completion_converge_once"
        status: pass
      - kind: integration
        ref: "crates/fava/tests/semantic_write_publication/interleavings.rs#semantic_cancellation_is_scoped_and_late_work_is_inert"
        status: pass
    human_judgment: false
  - id: D2
    description: "Semantic runners and lane completions are bounded by existing store-owned capacities and refuse excess custody atomically."
    requirement: CAP-02
    verification:
      - kind: integration
        ref: "crates/fava/tests/semantic_write_publication/interleavings.rs#semantic_task_and_completion_bounds_refuse_cleanly"
        status: pass
      - kind: other
        ref: "negative search: mpsc::unbounded_channel absent from crates/fava-publication/src"
        status: pass
    human_judgment: false
  - id: D3
    description: "Materializer errors, panic, malformed or oversized output, timestamp overflow, evidence exhaustion, and recovery retry preserve the prior generation with bounded public evidence."
    requirement: CAP-05
    verification:
      - kind: integration
        ref: "crates/fava/tests/semantic_write_failures.rs#six guarded failure and recovery tests"
        status: pass
      - kind: integration
        ref: "cargo test --workspace --all-targets"
        status: pass
    human_judgment: false
duration: 58min
completed: 2026-08-21
status: complete
---

# Phase 07 Plan 04: Exact Semantic Completion and Failure Isolation Summary

**Semantic publication now rejects every stale asynchronous completion at the store boundary and exposes hostile materialization failures as bounded, receipt-scoped public evidence.**

## Performance

- **Duration:** 58 min
- **Started:** 2026-08-21T08:56:00Z
- **Completed:** 2026-08-21T09:54:00Z
- **Tasks:** 2
- **Files created/modified:** 17 implementation, provider, test, and build files

## Accomplishments

- Propagated exact `WriteId`, `ReceiptId`, `MaterializationId`, event identity, route revision, relay session, and attempt number through signer, router, lane, publisher, and delivery completions.
- Made memory and redb store mutation compare-and-set authoritative and idempotent, eliminating publication read-before-write windows while preserving existing explicit-write behavior and redb process-kill durability.
- Replaced unbounded lane completion with a cancellation-aware bounded channel using `destination_evidence_capacity()`, and correlated each completion with the exact active lane tuple.
- Contained materializer panics outside store locks, bounded all materializer-controlled text, validated returned event identity and bounds before installation, and exposed failure through ordinary `Receipt` and query `EventRecord` publication evidence.
- Suppressed same-source live failure spin, retried a changed source immediately, retried one persisted failed source once on recovery, and cleared failure evidence atomically with successful successor installation.
- Kept every touched code file at or below 500 lines by extracting cohesive private delivery, lifecycle, and failure-support modules.

## RED and Causal Evidence

- **Task 1 RED:** the four guarded interleaving tests failed to compile because store mutations did not accept exact write, materialization, event, and attempt identity. Commit: `6574cfb`.
- **Task 2 RED:** the six guarded failure tests compiled; materializer panic escaped containment and recovery of the persisted failed source timed out. Commit: `9feb693`.
- **Required deliberate break:** removing only the store-side `MaterializationId` comparison made `retired_completion_is_attributable_and_inert` accept a retired generation paired with the current event identity. Restoring the comparison returned the named test green.

## Task Commits

1. **Task 1 RED: Specify exact generation completion guards** — `6574cfb` (test)
2. **Task 2 RED: Specify semantic failure isolation** — `9feb693` (test)
3. **Task 1 GREEN: Guard exact semantic completions** — `8cd702b` (feat)
4. **Task 2 GREEN: Isolate semantic materialization failures** — `15dbf6d` (feat)
5. **Strict lint refactor: Satisfy workspace lint gates** — `aa847ff` (refactor)
6. **Line-gate cleanup: Keep touched store proof within limit** — `8c88b14` (style)
7. **Exact lane identity: Correlate bounded lane completions** — `57ae157` (fix)

**Plan metadata:** this commit

## Files Created/Modified

- `crates/fava-write-store/src/lib.rs` — exact-current validation and expanded mutation contract.
- `crates/fava-write-store-memory/src/lifecycle.rs` — atomic in-memory signing, routing, attempt, and outcome compare-and-set.
- `crates/fava-write-store-redb/src/ops.rs` — matching redb contract behavior and notification-free idempotency; durable semantic persistence remains Plan 07-05.
- `crates/fava-publication/src/run.rs` and `delivery.rs` — exact completion capture, bounded lanes, cancellation selection, and generation-scoped delivery.
- `crates/fava-publication/src/materialization.rs` — panic isolation, bounded static panic evidence, and one recovery retry.
- `crates/fava-publisher/src/lib.rs` — approved `MaterializationId` carried by `PublishAttempt`; NIP-01 transport remains independent.
- `crates/fava/tests/semantic_write_publication/interleavings.rs` — four required stale, simultaneous, cancellation, and bound proofs.
- `crates/fava/tests/semantic_write_failures.rs` — six public error, panic, malformed, oversized, overflow, isolation, and retry proofs.
- Existing store, bound, process-kill, publisher, NIP-01, Cargo, and Bazel call sites — exact contract propagation without compatibility overloads.

## Decisions Made

- Store compare-and-set owns currentness. A publication pre-read could only be advisory and created a time-of-check/time-of-use window, so it was removed.
- A stale or unknown completion returns typed refusal and performs no mutation or notification. Exact duplicate completion returns the unchanged receipt without another durable effect.
- The existing active receipt bound is also the semantic runner bound: recovered receipts already occupy store capacity, are started once through the existing recovery partition, and new custody cannot exceed the remaining store capacity.
- Provider panic text is never retained. The public evidence records a bounded static panic reason plus existing generation and source identity.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking contract propagation] Extended exact identity through both store providers and publisher contract**
- **Found during:** Task 1 RED
- **Issue:** Publication could not make stale completion safety authoritative while `WriteStore` mutations accepted only receipt-local inputs and `PublishAttempt` omitted generation identity.
- **Fix:** Expanded the existing methods across contract, memory, redb, callers, fixtures, and tests; added the already-approved `MaterializationId` field to `PublishAttempt`. No new public nominal type or compatibility path was added, and redb semantic persistence remains Plan 07-05.
- **Files modified:** `crates/fava-write-store*`, `crates/fava-publisher/src/lib.rs`, publication and affected tests
- **Commits:** `8cd702b`, `aa847ff`

**2. [Rule 1 - Atomicity bug] Removed publication pre-read authority and made provider duplicates notification-free**
- **Found during:** Task 1 implementation
- **Issue:** Checking current receipt or capacity before mutation could race the authoritative store commit; redb also committed and notified on unchanged duplicate results.
- **Fix:** Sent captured identity directly to store compare-and-set, retained materialization-before-accept with atomic custody refusal, and skipped redb durable commit/notification when the receipt is unchanged.
- **Files modified:** `crates/fava-publication/src/lib.rs`, `crates/fava-publication/src/run.rs`, both store providers, capacity tests
- **Commit:** `8cd702b`

**3. [Rule 2 - Missing completion correlation] Correlated the bounded lane cleanup signal**
- **Found during:** Final exact-identity audit
- **Issue:** The bounded lane signal initially carried only `RelaySessionKey`, which was insufficient to prove that delayed cleanup belonged to the active generation and route.
- **Fix:** Carried write, receipt, materialization, event, route, and session identity in a private tuple and removed an active lane only when the tuple matches. No channel noun or public configuration was introduced.
- **Files modified:** `crates/fava-publication/src/run.rs`, `crates/fava-publication/src/delivery.rs`
- **Commit:** `57ae157`

**4. [Rule 3 - Structural and lint gates] Extracted cohesive private modules and documented exact-identity arity**
- **Found during:** Final line and strict-Clippy verification
- **Issue:** Publication run and memory store lifecycle exceeded the 500-line cohesion threshold after exact propagation; strict Clippy also required explicit justification for the exact outcome argument set.
- **Fix:** Extracted private delivery and lifecycle modules, split failure-test support, added narrow lint annotations, and kept all touched Rust files at or below 500 lines.
- **Files modified:** publication, memory store, and semantic test modules
- **Commits:** `8cd702b`, `15dbf6d`, `aa847ff`, `8c88b14`

---

**Total deviations:** 4 auto-fixed correctness/blocking issues. **Impact on plan:** The expanded files were required to make the planned store-CAS safety real across replaceable providers; no new architecture vocabulary, dependency, or compatibility behavior was added.

## Issues Encountered

- Strict Clippy found exact-outcome arity and test-function length issues; both were resolved without changing public behavior.
- No unresolved issue remains.

## Verification

- Plan name guards: exact counts 4 and 6; both full semantic targets passed.
- Deliberate store guard break: named stale-completion test failed causally, then passed after restoration.
- `cargo test --workspace --all-targets` — passed.
- `cargo check --workspace --all-targets` — passed.
- `cargo clippy --workspace --all-targets -- -D warnings` — passed.
- `cargo test --manifest-path apps/canary/Cargo.toml --all-targets` — 7 passed.
- `cargo clippy --manifest-path apps/canary/Cargo.toml --all-targets -- -D warnings` — passed.
- Affected memory, redb process-kill, publisher, NIP-01, publication, write-bound, and semantic-store targets — passed.
- `bazel test //...` — 28/28 tests passed.
- `python3 tools/check_vocabulary.py` and `python3 -m unittest discover -s tools/tests` — passed (7 tests).
- Negative unbounded-channel, new-public-nominal, and forbidden-vocabulary scans — empty.
- `cargo fmt --all -- --check`, `git diff --check`, 800-line global hard gate, and 500-line touched-file gate — passed.

## Known Stubs

None.

## User Setup Required

None.

## Next Phase Readiness

- Plan 07-05 can persist the existing semantic custody and failure facts in redb without changing the exact mutation contract or public facade.
- No blockers.

## Self-Check: PASSED

All five created artifacts and seven implementation/evidence commits exist; required frontmatter, requirement coverage, exact name guards, full Cargo, canary, vocabulary, line, diff, and Bazel gates passed.

---
*Phase: 07-semantic-writes-and-capability-composition*
*Completed: 2026-08-21*
