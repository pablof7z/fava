---
phase: 07-semantic-writes-and-capability-composition
plan: 05
subsystem: write-store-redb
tags: [redb, semantic-writes, transactions, crash-recovery, sigkill]

requires:
  - phase: 07-02
    provides: bounded semantic-write state-machine contracts and custody limits
  - phase: 07-04
    provides: public recovery assembly with validated applier selection
provides:
  - hard-versioned redb semantic custody with strict refusal of missing or mismatched schemas
  - atomic durable parity for coordinate ownership, generations, failure evidence, and settlement
  - marker-barrier SIGKILL proof for first generation, successor retry, and inert terminal work
  - strict schema-v1 invariant and configured-bound validation before recovered custody is exposed
  - exact persisted source timestamp qualification through public recovery without inferred floors
affects: [07-06, durable-publication, write-store-providers]

actuals:
  tokens: 87450
  tasks: 2
  commits: 12

tech-stack:
  added: []
  patterns:
    - immediate redb transaction before in-memory mirror update and notification
    - private schema and semantic modules behind the existing WriteStore contract
    - marker-based subprocess crash barriers with exact identity replay
    - strict reconstruction before ambiguity recovery or provider publication

key-files:
  created:
    - crates/fava-write-store-redb/src/lifecycle.rs
    - crates/fava-write-store-redb/src/schema.rs
    - crates/fava-write-store-redb/src/semantic.rs
    - crates/fava-write-store-redb/src/validation.rs
    - crates/fava-write-store-redb/tests/semantic_write_store.rs
    - crates/fava-write-store-redb/tests/semantic_write_store/recovery.rs
    - crates/fava-write-store-redb/tests/process_kill/semantic.rs
  modified:
    - crates/fava-write-store-redb/src/lib.rs
    - crates/fava-write-store-redb/src/ops.rs
    - crates/fava-write-store-redb/tests/process_kill.rs
    - crates/fava-write-store-redb/BUILD.bazel
    - crates/fava-write-store/src/lib.rs
    - crates/fava-write-store-memory/src/lib.rs
    - crates/fava-write-store-memory/src/semantic.rs
    - crates/fava-publication/src/lib.rs
    - crates/fava-publication/src/revision.rs

key-decisions:
  - "Stamp schema version 1 only for a genuinely new database; an existing database without that version is incompatible."
  - "Persist receipt and private semantic custody in one row, and reconstruct the coordinate owner before exposing recovered state."
  - "Keep applier validation in Fava assembly; the redb provider only reconstructs durable custody tuples."
  - "Carry the existing source identity and timestamp values together through recovery; never infer a source floor from revision time."
  - "Refuse over-bound or incoherent schema-v1 state before ambiguity repair, publication, or notification."

patterns-established:
  - "Durable semantic mutation: validate exact identity and bounds, commit one immediate transaction, then update the mirror and notify."
  - "Crash proof: child commits a named boundary, writes a marker, parks, and the parent SIGKILLs before reopen."
  - "Reconstruction gate: validate identity, boundedness, outcome, attribution, and configured counts before returning provider state."

requirements-completed: [CAP-03, CAP-05]

coverage:
  - id: D1
    description: "redb matches memory semantic custody and atomically refuses stale, overflow, eviction, and recovery-bound violations"
    requirement: CAP-03
    verification:
      - kind: integration
        ref: "crates/fava-write-store-redb/tests/semantic_write_store.rs#ten guarded parity and recovery scenarios"
        status: pass
    human_judgment: false
  - id: D2
    description: "real SIGKILL and reopen preserves live custody, installs one exact newer-source successor, retries once, and leaves retired or terminal work inert"
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
  - id: D4
    description: "schema-v1 reconstruction validates every durable identity, bound, outcome, and failure attribution before exposing state"
    requirement: CAP-03
    verification:
      - kind: integration
        ref: "crates/fava-write-store-redb/tests/semantic_write_store/recovery.rs#schema_v1_reconstruction_refuses_every_malformed_invariant"
        status: pass
      - kind: integration
        ref: "crates/fava-write-store-redb/tests/semantic_write_store/recovery.rs#schema_version_refusal_precedes_malformed_row_decode"
        status: pass
    human_judgment: false

duration: 40min
completed: 2026-08-21
status: complete
---

# Phase 7 Plan 05: Durable redb Semantic Recovery Summary

**Hard-versioned transactional semantic custody now survives real SIGKILL and resumes exactly once through validated public Fava assembly.**

## Performance

- **Duration:** 40 min
- **Started:** 2026-08-21T10:00:46Z
- **Completed:** 2026-08-21T10:40:20Z
- **Tasks:** 2
- **Files modified:** 21

## Accomplishments

- Added strict schema version 1 handling that stamps only new databases and rejects missing or mismatched versions before row decoding.
- Matched memory semantic custody in redb, including one coordinate owner, exact generation replacement, failed-source evidence, terminal release, bounded refusal, and notification only after durable commit.
- Proved first-generation survival, successor and failed-source recovery, and retired/terminal/cancelled inertness after marker-barrier SIGKILL.
- Refused malformed identities, text, evidence, outcomes, source attribution, and over-bound recovered counts before returning state.
- Preserved exact persisted source time through the existing recovery tuple so a legitimate newer source resumes once even when revision time is far ahead.
- Preserved the original Milestone 5 process-kill evidence and passed all Cargo, canary, vocabulary, and Bazel gates.

## RED and Deliberate-Break Evidence

- Task 1 RED: all four guarded parity tests failed before production support with unsupported semantic custody, zero active capacity, or an accepted schema mismatch (`d59b622`).
- Task 1 named break: disabling the schema-version comparison made `redb_schema_mismatch_refuses_without_fallback` fail; restoring it returned the suite to green.
- Task 2 RED: the first-generation subprocess scenario reopened without a receipt before the child performed the durable boundary mutation (`9682dc4`).
- Task 2 named break: removing the exact generation comparison made the retired replay scenario accept a stale completion; restoring it returned the SIGKILL suite to green.
- Focused-review RED: stale exact success, terminal self-eviction, recovered active overflow, and malformed publication identity all failed against the pre-repair implementation (`456c8ee`).
- Focused-review SIGKILL RED: recovery installed generation 3 from empty state instead of the durable newer source because the inferred timestamp floor erased that candidate (`456c8ee`).
- Validator repair RED: the first strict validator incorrectly refused a legitimate attributed empty-source failure; the focused scenario failed before the coherence rule was corrected.

## Task Commits

1. **Task 1 RED: add failing redb semantic parity evidence** - `d59b622` (test)
2. **Task 1 GREEN: persist atomic redb semantic custody** - `6216345` (feat)
3. **Task 2 RED: add failing semantic SIGKILL recovery proof** - `9682dc4` (test)
4. **Task 2 GREEN: implement semantic SIGKILL recovery barriers** - `471839c` (test)
5. **Task 2 Bazel fix: declare the nested SIGKILL source** - `d3d32f8` (fix)
6. **Build metadata: refresh dependency locks** - `975586a` (chore)
7. **Initial plan metadata** - `7fe62f2` (docs)
8. **Focused-review RED: expose recovery invariant gaps** - `456c8ee` (test)
9. **Focused-review GREEN: validate exact durable redb state** - `f61c33f` (fix)
10. **Focused-review GREEN: recover exact semantic source time** - `72fa134` (fix)
11. **Postflight fix: declare recovery test query dependency** - `02b462b` (fix)

**Plan metadata:** included in the final documentation commit

## Files Created/Modified

- `crates/fava-write-store-redb/src/schema.rs` - hard version gate and strict persisted-row encoding/decoding.
- `crates/fava-write-store-redb/src/semantic.rs` - private transactional semantic custody operations.
- `crates/fava-write-store-redb/src/validation.rs` - strict schema-v1 identity, boundedness, outcome, and attribution reconstruction gate.
- `crates/fava-write-store-redb/src/lifecycle.rs` - durable terminal, cancellation, and removal lifecycle mutations.
- `crates/fava-write-store-redb/src/lib.rs` - validated open/reconstruction and WriteStore wiring.
- `crates/fava-write-store-redb/src/ops.rs` - receipt operations integrated with transactional lifecycle behavior.
- `crates/fava-write-store-redb/tests/semantic_write_store.rs` - four original guarded memory-parity and schema-refusal scenarios.
- `crates/fava-write-store-redb/tests/semantic_write_store/recovery.rs` - six guarded exact-current, retention, bounds, malformed-row, and precedence scenarios.
- `crates/fava-write-store-redb/tests/process_kill/semantic.rs` - three guarded semantic SIGKILL/reopen scenarios.
- `crates/fava-write-store-redb/tests/process_kill.rs` - semantic subprocess dispatch while retaining Milestone 5 evidence.
- `crates/fava-write-store-redb/{Cargo.toml,BUILD.bazel}` - serde and semantic test target/source declarations.
- `Cargo.lock`, `MODULE.bazel.lock`, `apps/canary/Cargo.lock` - generated dependency graph refresh.

Cross-plan files changed only to carry existing neutral recovery values:

- `crates/fava-write-store/src/lib.rs` - recovery tuple returns the persisted source ID and timestamp together.
- `crates/fava-write-store-memory/src/{lib.rs,semantic.rs}` - memory provider preserves the same tuple contract.
- `crates/fava-publication/src/{lib.rs,revision.rs}` - public recovery uses the persisted timestamp and removes the inferred `created_at - 1` floor and obsolete lookup helper.

## Decisions Made

- Missing schema metadata on any existing database is incompatible; only a newly created database receives the current stamp.
- Receipt and semantic custody are stored atomically without introducing a public or cross-crate nominal type.
- Reopen reconstructs coordinate ownership and exact current/failure identities, while Fava assembly remains the authority that validates applier selection before recovery.
- Exact retired completion replay is an attributed no-op; it never revives or replaces the current generation.
- Exact generation/source validation precedes idempotent success, so stale callers cannot reuse current output as an accepted completion.
- Terminal retention always preserves the receipt currently terminalizing and evicts another retained terminal atomically from durable, mirror, query, and receipt-notification state.
- Existing source ID and timestamp values travel together through the neutral recovery tuple; no new public noun was introduced.

## Verification

- Guarded semantic parity/recovery target: 10/10 passed, including all four original plan guards.
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
- **Fix:** Added the nested source to the existing `process_kill` target.
- **Files modified:** `crates/fava-write-store-redb/BUILD.bazel`
- **Verification:** Targeted Bazel tests and `bazel test //...` pass.
- **Committed in:** `d3d32f8`

**2. [Rule 1/2 - Correctness] Repaired exact mutation and reconstruction invariants**

- **Found during:** Focused post-plan review
- **Issue:** Idempotent success preceded exact identity checks, terminal retention could evict the receipt being terminalized, recovered counts were not checked against configured bounds, and schema-v1 rows were only partially validated.
- **Fix:** Moved exact checks first; made eviction, mirror removal, query state, and notifications atomic and identical; added `>=` admission and recovered-count refusal; added strict private reconstruction validation.
- **Files modified:** `crates/fava-write-store-redb/src/{lib.rs,lifecycle.rs,ops.rs,schema.rs,semantic.rs,validation.rs}`
- **Verification:** Six focused recovery scenarios, ten-test parity target, restart checks, workspace tests, and strict clippy pass.
- **Committed in:** `f61c33f`

**3. [Rule 1 - Bug] Removed inferred source timestamps from public recovery**

- **Found during:** Strengthened successor SIGKILL proof
- **Issue:** Falling back to `revision.created_at - 1` could hide a legitimate source newer than the durable selected source and then install empty state.
- **Fix:** Extended the existing neutral recovery tuple with the already-persisted timestamp and qualified recovery directly from that exact value.
- **Files modified:** `crates/fava-write-store/src/lib.rs`, `crates/fava-write-store-memory/src/{lib.rs,semantic.rs}`, `crates/fava-write-store-redb/src/{ops.rs,semantic.rs}`, `crates/fava-publication/src/{lib.rs,revision.rs}`
- **Verification:** The successor SIGKILL scenario advances generation 2 to 3 from the exact source once, then reassembly causes zero calls.
- **Committed in:** `72fa134`

**4. [Rule 3 - Blocking] Declared the recovery test query dependency in Bazel**

- **Found during:** Full postflight
- **Issue:** Cargo passed, but Bazel could not resolve `fava_query` for the nested published-state assertion.
- **Fix:** Added the existing first-party query target to the redb semantic test dependencies.
- **Files modified:** `crates/fava-write-store-redb/BUILD.bazel`
- **Verification:** Both targeted redb Bazel tests and `bazel test //...` pass.
- **Committed in:** `02b462b`

**Total deviations:** 4 auto-fixed (2 Rule 3, 1 Rule 1, 1 combined Rule 1/2).

**Impact on plan:** Correctness hardening only. The neutral contract carries one additional existing timestamp value; no new public noun or architectural vocabulary was added.

## Issues Encountered

Both missing Bazel source/dependency declarations were resolved and their full targets rerun.

## Known Stubs

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

The durable provider now refuses every tested malformed or over-bound reconstruction before exposure, and public assembly recovers one exactly qualified live generation only after validating the selected applier. No blockers remain for Plan 07-06.

## Self-Check: PASSED

- All created files exist.
- All task and verification commits exist on `worktree-agent-m7-p05`.
- Full Cargo, canary, vocabulary, format, line, and Bazel validation passed.

---
*Phase: 07-semantic-writes-and-capability-composition*
*Completed: 2026-08-21*
