---
phase: 07-semantic-writes-and-capability-composition
plan: 05
subsystem: write-store-redb
tags: [redb, semantic-writes, transactions, crash-recovery, sigkill]

requires:
  - phase: 07-02
    provides: bounded semantic-write state-machine contracts and custody limits
  - phase: 07-04
    provides: public recovery assembly with validated materializer selection
provides:
  - hard-versioned redb semantic custody with strict refusal of missing or mismatched schemas
  - atomic durable parity for coordinate ownership, generations, failure evidence, and settlement
  - marker-barrier SIGKILL proof for first generation, successor retry, and inert terminal work
affects: [07-06, durable-publication, write-store-providers]

actuals:
  tokens: 77049
  tasks: 2
  commits: 7

tech-stack:
  added: []
  patterns:
    - immediate redb transaction before in-memory mirror update and notification
    - private schema and semantic modules behind the existing WriteStore contract
    - marker-based subprocess crash barriers with exact identity replay

key-files:
  created:
    - crates/fava-write-store-redb/src/lifecycle.rs
    - crates/fava-write-store-redb/src/schema.rs
    - crates/fava-write-store-redb/src/semantic.rs
    - crates/fava-write-store-redb/tests/semantic_write_store.rs
    - crates/fava-write-store-redb/tests/process_kill/semantic.rs
  modified:
    - crates/fava-write-store-redb/src/lib.rs
    - crates/fava-write-store-redb/src/ops.rs
    - crates/fava-write-store-redb/tests/process_kill.rs
    - crates/fava-write-store-redb/BUILD.bazel

key-decisions:
  - "Stamp schema version 1 only for a genuinely new database; an existing database without that version is incompatible."
  - "Persist receipt and private semantic custody in one row, and reconstruct the coordinate owner before exposing recovered state."
  - "Keep materializer validation in Fava assembly; the redb provider only reconstructs durable custody tuples."

patterns-established:
  - "Durable semantic mutation: validate exact identity and bounds, commit one immediate transaction, then update the mirror and notify."
  - "Crash proof: child commits a named boundary, writes a marker, parks, and the parent SIGKILLs before reopen."

requirements-completed: [CAP-03, CAP-05]

coverage:
  - id: D1
    description: "redb matches memory semantic custody and atomic stale/overflow behavior"
    requirement: CAP-03
    verification:
      - kind: integration
        ref: "crates/fava-write-store-redb/tests/semantic_write_store.rs#four guarded parity scenarios"
        status: pass
    human_judgment: false
  - id: D2
    description: "real SIGKILL and reopen preserves live custody, retries once, and leaves retired or terminal work inert"
    requirement: CAP-05
    verification:
      - kind: e2e
        ref: "crates/fava-write-store-redb/tests/process_kill/semantic.rs#three guarded SIGKILL scenarios"
        status: pass
    human_judgment: false
  - id: D3
    description: "missing, older, or unknown redb schema refuses before semantic row deserialization"
    requirement: CAP-03
    verification:
      - kind: integration
        ref: "crates/fava-write-store-redb/tests/semantic_write_store.rs#redb_schema_mismatch_refuses_without_fallback"
        status: pass
    human_judgment: false

duration: 20min
completed: 2026-08-21
status: complete
---

# Phase 7 Plan 05: Durable redb Semantic Recovery Summary

**Hard-versioned transactional semantic custody now survives real SIGKILL and resumes exactly once through validated public Fava assembly.**

## Performance

- **Duration:** 20 min
- **Started:** 2026-08-21T10:00:46Z
- **Completed:** 2026-08-21T10:17:01Z
- **Tasks:** 2
- **Files modified:** 14

## Accomplishments

- Added strict schema version 1 handling that stamps only new databases and rejects missing or mismatched versions before row decoding.
- Matched memory semantic custody in redb, including one coordinate owner, exact generation replacement, failed-source evidence, terminal release, bounded refusal, and notification only after durable commit.
- Proved first-generation survival, successor and failed-source recovery, and retired/terminal/cancelled inertness after marker-barrier SIGKILL.
- Preserved the original Milestone 5 process-kill evidence and passed all Cargo, canary, vocabulary, and Bazel gates.

## RED and Deliberate-Break Evidence

- Task 1 RED: all four guarded parity tests failed before production support with unsupported semantic custody, zero active capacity, or an accepted schema mismatch (`d59b622`).
- Task 1 named break: disabling the schema-version comparison made `redb_schema_mismatch_refuses_without_fallback` fail; restoring it returned the suite to green.
- Task 2 RED: the first-generation subprocess scenario reopened without a receipt before the child performed the durable boundary mutation (`9682dc4`).
- Task 2 named break: removing the exact generation comparison made the retired replay scenario accept a stale completion; restoring it returned the SIGKILL suite to green.

## Task Commits

1. **Task 1 RED: add failing redb semantic parity evidence** - `d59b622` (test)
2. **Task 1 GREEN: persist atomic redb semantic custody** - `6216345` (feat)
3. **Task 2 RED: add failing semantic SIGKILL recovery proof** - `9682dc4` (test)
4. **Task 2 GREEN: implement semantic SIGKILL recovery barriers** - `471839c` (test)
5. **Task 2 Bazel fix: declare the nested SIGKILL source** - `d3d32f8` (fix)
6. **Build metadata: refresh dependency locks** - `975586a` (chore)

**Plan metadata:** included in the final documentation commit

## Files Created/Modified

- `crates/fava-write-store-redb/src/schema.rs` - hard version gate and strict persisted-row encoding/decoding.
- `crates/fava-write-store-redb/src/semantic.rs` - private transactional semantic custody operations.
- `crates/fava-write-store-redb/src/lifecycle.rs` - durable terminal, cancellation, and removal lifecycle mutations.
- `crates/fava-write-store-redb/src/lib.rs` - validated open/reconstruction and WriteStore wiring.
- `crates/fava-write-store-redb/src/ops.rs` - receipt operations integrated with transactional lifecycle behavior.
- `crates/fava-write-store-redb/tests/semantic_write_store.rs` - four guarded memory-parity and schema-refusal scenarios.
- `crates/fava-write-store-redb/tests/process_kill/semantic.rs` - three guarded semantic SIGKILL/reopen scenarios.
- `crates/fava-write-store-redb/tests/process_kill.rs` - semantic subprocess dispatch while retaining Milestone 5 evidence.
- `crates/fava-write-store-redb/{Cargo.toml,BUILD.bazel}` - serde and semantic test target/source declarations.
- `Cargo.lock`, `MODULE.bazel.lock`, `apps/canary/Cargo.lock` - generated dependency graph refresh.

## Decisions Made

- Missing schema metadata on any existing database is incompatible; only a newly created database receives the current stamp.
- Receipt and semantic custody are stored atomically without introducing a public or cross-crate nominal type.
- Reopen reconstructs coordinate ownership and exact current/failure identities, while Fava assembly remains the authority that validates materializer selection before recovery.
- Exact retired completion replay is an attributed no-op; it never revives or replaces the current generation.

## Verification

- Guarded semantic parity target: 4/4 passed.
- Guarded semantic SIGKILL target: 3/3 passed; the complete process-kill target passed 6/6 including retained Milestone 5 cases.
- `cargo test -p fava-write-store-redb`, redb strict all-target clippy, workspace all-target tests/check/clippy: passed.
- Canary all-target tests and strict clippy: passed.
- Vocabulary checker and its 7 unit tests: passed.
- `cargo fmt --all -- --check` and `git diff --check`: passed.
- Targeted Bazel redb tests and `bazel test //...`: 29/29 passed.
- All changed redb code and test files remain below the 500-line soft limit.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Declared the nested semantic SIGKILL source in Bazel**

- **Found during:** Task 2 verification
- **Issue:** The Cargo target passed, but Bazel could not compile the nested `tests/process_kill/semantic.rs` module because it was absent from the test target sources.
- **Fix:** Added the nested source to the existing `semantic_write_store_test` target.
- **Files modified:** `crates/fava-write-store-redb/BUILD.bazel`
- **Verification:** Targeted Bazel tests and `bazel test //...` pass.
- **Committed in:** `d3d32f8`

**Total deviations:** 1 auto-fixed (Rule 3).

**Impact on plan:** Build metadata only; no semantic scope or vocabulary change.

## Issues Encountered

None beyond the resolved Bazel source declaration.

## Known Stubs

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

The durable provider now presents the same semantic custody as memory after reopen, and public assembly can recover one eligible live generation only after validating the selected materializer. No blockers remain for Plan 07-06.

## Self-Check: PASSED

- All created files exist.
- All task and verification commits exist on `worktree-agent-m7-p05`.
- Full Cargo, canary, vocabulary, format, line, and Bazel validation passed.

---
*Phase: 07-semantic-writes-and-capability-composition*
*Completed: 2026-08-21*
