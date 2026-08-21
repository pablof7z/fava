---
phase: 07-semantic-writes-and-capability-composition
plan: 02
subsystem: write-store
tags: [rust, semantic-writes, write-store, concurrency, recovery, tdd]
requires:
  - phase: 07-semantic-writes-and-capability-composition
    plan: 01
    provides: bounded semantic edit, materializer, and exact generation identity contracts
provides:
  - neutral WriteStore operations for semantic admission, exact generation installation, attributed failure, and live recovery
  - memory-store state machine with one live owner per exact coordinate and stable write and receipt identity
  - bounded retired, correction, and failure evidence using existing Receipt and PublicationEvidence values
affects: [07-03, 07-04, 07-05, publication, recovery, write-store-redb]
actuals:
  tokens: 14411
  tasks: 2
  commits: 6
tech-stack:
  added: []
  patterns:
    - exact coordinate admission and generation compare-and-set under one store mutex
    - commit-before-notify receipt observation
    - one store-owned primitive capacity shared by destinations and semantic evidence
key-files:
  created:
    - crates/fava-write-store-memory/src/semantic.rs
    - crates/fava/tests/semantic_write_store.rs
  modified:
    - crates/fava-write-store/src/lib.rs
    - crates/fava-write-store-memory/src/lib.rs
    - crates/fava/BUILD.bazel
key-decisions:
  - "Providers without semantic custody report zero active capacity and refuse the neutral semantic operations until they implement the contract."
  - "Recovery returns existing receipt, edit, current source, and failed source values; failure attribution remains inside existing PublicationEvidence vocabulary."
  - "Cancel and terminal settlement release coordinate and edit custody while retaining the historical receipt."
patterns-established:
  - "Semantic custody: the write store owns the coordinate, edit, current generation, selected source, retired evidence, and failure fact."
  - "Exact replacement: WriteId, ReceiptId, MaterializationId, and selected source are checked before one atomic successor swap."
requirements-completed: [CAP-02, CAP-03, CAP-05]
coverage:
  - id: D1
    description: "Memory admission creates one stable receipt and one live owner for an exact coordinate, including simultaneous requests."
    requirement: CAP-02
    verification:
      - kind: integration
        ref: "crates/fava/tests/semantic_write_store.rs#memory_first_edit_has_no_prior"
        status: pass
      - kind: integration
        ref: "crates/fava/tests/semantic_write_store.rs#memory_simultaneous_coordinate_admission_has_one_owner"
        status: pass
    human_judgment: false
  - id: D2
    description: "Generation replacement preserves write and receipt identity and refuses stale generation or unqualified source without mutation."
    requirement: CAP-03
    verification:
      - kind: integration
        ref: "crates/fava/tests/semantic_write_store.rs#memory_generation_swap_is_compare_and_set"
        status: pass
      - kind: integration
        ref: "crates/fava/tests/semantic_write_store.rs#memory_unqualified_source_is_inert"
        status: pass
    human_judgment: false
  - id: D3
    description: "Failure, retry, recovery, retirement, correction, and capacity exhaustion remain attributed, bounded, idempotent, and atomic."
    requirement: CAP-05
    verification:
      - kind: integration
        ref: "crates/fava/tests/semantic_write_store.rs#memory_failure_preserves_current_and_is_attributed"
        status: pass
      - kind: integration
        ref: "crates/fava/tests/semantic_write_store.rs#memory_successful_retry_clears_failure_atomically"
        status: pass
      - kind: integration
        ref: "crates/fava/tests/semantic_write_store.rs#memory_live_edit_recovers_once_and_terminal_is_inert"
        status: pass
      - kind: integration
        ref: "crates/fava/tests/semantic_write_store.rs#memory_evidence_exhaustion_has_no_partial_effect"
        status: pass
    human_judgment: false
duration: 18min
completed: 2026-08-21
status: complete
---

# Phase 07 Plan 02: Semantic Write Store Summary

**Neutral store custody plus an atomic memory semantic-write state machine with exact generation identity, bounded evidence, and recovery-ready existing values.**

## Performance

- **Duration:** 18 min
- **Started:** 2026-08-21T08:21:54Z
- **Completed:** 2026-08-21T08:39:56Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments

- Extended the neutral write-store contract without new nominal vocabulary: semantic admission, generation installation, failure recording, live recovery, and the store-owned capacity primitive.
- Implemented one live memory owner per exact coordinate with stable `WriteId` and `ReceiptId`, exact current `MaterializationId` and source checks, bounded retirement, and commit-before-notify observation.
- Preserved the current generation on failure, bounded provider attribution, cleared failure only with a successful atomic successor, and released semantic custody at cancel or terminal settlement.
- Kept unpublished generated events entirely in write-store query-source custody; no event-cache insertion path was added.

## RED and Causal Evidence

- **Task 1 RED:** `cargo test -p fava --test semantic_write_store` failed before production with missing `accept_materialized_edit`, `install_materialization`, and `recover_materialized_edits` methods. Commit: `015330d`.
- **Task 2 RED:** the same target failed before production only because `record_materialization_failure` did not exist. Commit: `17087a6`.
- **Named deliberate break:** removing the exact `MaterializationId` comparison made `memory_generation_swap_is_compare_and_set` fail at its stale-generation assertion. Restoring the guard returned all eight tests to GREEN. The isolated causal assertion is committed in `4d6bd5b`.

## Task Commits

1. **Task 1 RED: Specify semantic write-store custody** — `015330d` (test)
2. **Task 1 GREEN: Add atomic semantic write custody** — `11fea68` (feat)
3. **Task 2 RED: Specify recovery and failure evidence** — `17087a6` (test)
4. **Task 2 GREEN: Persist failure and recovery state** — `8e24c20` (feat)
5. **Causal evidence repair: Isolate exact generation guard** — `4d6bd5b` (test)

**Plan metadata:** this commit

## Files Created/Modified

- `crates/fava-write-store/src/lib.rs` — neutral semantic operations and one primitive evidence-capacity authority.
- `crates/fava-write-store-memory/src/lib.rs` — memory-store integration, lifecycle release, capacity reporting, and committed-state observation.
- `crates/fava-write-store-memory/src/semantic.rs` — coordinate custody, exact CAS, failure, recovery, and bounded retirement state machine.
- `crates/fava/tests/semantic_write_store.rs` — eight observable concurrency, identity, recovery, failure, and exhaustion cases.
- `crates/fava/BUILD.bazel` — Bazel target for the semantic store corpus.

## Decisions Made

- Unsupported stores keep the neutral contract replaceable and honest through zero semantic active capacity plus typed refusal; Plan 05 can implement parity without a private bypass.
- The failed live source remains a private store fact returned through an existing-value recovery tuple; public bounded attribution stays in `PublicationEvidence.materialization_failure`.
- The ordinary write lifecycle remains authoritative for terminality, while semantic custody is released exactly when that lifecycle cancels or settles.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Test bug] Isolated the exact-generation causal assertion**
- **Found during:** Final named deliberate-break verification
- **Issue:** The original stale swap changed both expected generation and expected source, so removing only the generation guard did not fail the named CAS test.
- **Fix:** Held the current source identity constant while supplying a stale `MaterializationId`; the deliberate break then failed exactly the named test.
- **Files modified:** `crates/fava/tests/semantic_write_store.rs`
- **Verification:** The deliberate break failed `memory_generation_swap_is_compare_and_set`; the restored guard passed all eight semantic store tests and the Bazel target.
- **Committed in:** `4d6bd5b`

---

**Total deviations:** 1 auto-fixed test bug. **Impact on plan:** Stronger causal evidence only; no production or vocabulary scope change.

## Issues Encountered

None unresolved.

## Verification

- `cargo test -p fava --test semantic_write_store` — 8 passed.
- Both plan-enumerated four-test name guards — exact count 4 and full target passed.
- `cargo test -p fava-write-store-memory` — passed.
- `cargo check --workspace` — passed.
- `cargo test --workspace --all-targets` — passed.
- `cargo clippy --workspace --all-targets -- -D warnings` — passed.
- `bazel test //crates/fava:semantic_write_store` — passed.
- `bazel build //crates/fava-write-store:lib //crates/fava-write-store-memory:lib` — passed.
- `python3 tools/check_vocabulary.py` — passed.
- `python3 -m unittest tools.tests.test_vocabulary_check` — 4 passed.
- `cargo fmt --all -- --check` and `git diff --check` — passed.
- Global 800-line hard gate — passed; every touched Rust file is below the 500-line soft limit (442, 489, 488, 499).

## Known Stubs

None.

## Next Phase Readiness

- Plan 07-03 can orchestrate the selected materializer against this store-owned admission and exact generation contract.
- Plan 07-05 still owns durable redb semantic-state parity; the current redb implementation advertises zero semantic capacity rather than accepting unrecoverable custody.

## Self-Check: PASSED

The summary, both created artifacts, and all five task/evidence commits exist; `status: complete` is present.
