---
phase: 07-semantic-writes-and-capability-composition
plan: 01
subsystem: write-contract
tags: [rust, semantic-writes, replaceable-events, tdd, serde]
requires:
  - phase: 05-explicit-publication-and-optimistic-visibility
    provides: stable write and receipt identities plus the ordinary write lifecycle
provides:
  - seven-scenario semantic-write behavior map with stable Rust evidence destinations
  - bounded persistable ReplaceableEventEdit with inverse and structural validation
  - pure ReplaceableEventMaterializer contract with caller-injected Timestamp
  - exact MaterializationId and current/retired publication attribution fields
affects: [07-02, 07-03, 07-04, protocol-capabilities, write-store, publication]
actuals:
  tokens: 6810
  tasks: 2
  commits: 3
tech-stack:
  added: []
  patterns:
    - opaque protocol-owned edit bytes behind a neutral bounded value
    - exact caller-injected time at the materializer boundary
    - stable write and receipt identity with changing materialization identity
key-files:
  created:
    - features/semantic-writes.feature
    - tools/tests/test_semantic_write_feature.py
    - crates/fava/tests/semantic_write_contract.rs
    - crates/fava-write/src/edit.rs
    - crates/fava-write/src/materialization.rs
  modified:
    - crates/fava-write/src/lib.rs
    - crates/fava/BUILD.bazel
    - docs/internals/vocabulary.toml
key-decisions:
  - "Core validates edit structure and byte bounds but does not decide provider availability or protocol meaning."
  - "EventCoordinate receives a private explicit serde representation inside the edit codec rather than a new public wrapper."
  - "Materializers receive qualified source by reference and exact caller-supplied Timestamp, with no effect authority."
patterns-established:
  - "Semantic edit boundary: actor, exact coordinate, format, change, and inverse cross core as bounded opaque data."
  - "Generation attribution: MaterializationId changes independently of stable WriteId and ReceiptId."
requirements-completed: [CAP-01, CAP-02, CAP-03]
coverage:
  - id: D1
    description: "Seven observable semantic-write scenarios map to stable ownership-split Rust targets and test names."
    requirement: CAP-01
    verification:
      - kind: unit
        ref: "tools/tests/test_semantic_write_feature.py#SemanticWriteFeatureMappingTests"
        status: pass
    human_judgment: false
  - id: D2
    description: "ReplaceableEventEdit is bounded, persistable, reversible, and refused structurally before custody."
    requirement: CAP-01
    verification:
      - kind: integration
        ref: "crates/fava/tests/semantic_write_contract.rs#edit_contract_is_bounded_and_round_trips"
        status: pass
      - kind: integration
        ref: "crates/fava/tests/semantic_write_contract.rs#addressable_edit_refuses_before_custody"
        status: pass
    human_judgment: false
  - id: D3
    description: "The pure selected materializer receives no prior source for first value and uses the exact injected timestamp and actor."
    requirement: CAP-03
    verification:
      - kind: integration
        ref: "crates/fava/tests/semantic_write_contract.rs#first_value_receives_no_prior_and_exact_timestamp"
        status: pass
    human_judgment: false
  - id: D4
    description: "Materialization identity changes while write and receipt identities remain stable and retired evidence stays attributable."
    requirement: CAP-02
    verification:
      - kind: integration
        ref: "crates/fava/tests/semantic_write_contract.rs#materialization_identity_changes_but_receipt_identity_does_not"
        status: pass
    human_judgment: false
duration: 11min
completed: 2026-08-21
status: complete
---

# Phase 07 Plan 01: Semantic Write Contract Summary

**Mapped semantic-write behavior plus bounded opaque edits, pure materialization, and exact generation identity without protocol meaning in core.**

## Performance

- **Duration:** 11 min
- **Started:** 2026-08-21T08:03:07Z
- **Completed:** 2026-08-21T08:14:08Z
- **Tasks:** 2
- **Files modified:** 13

## Accomplishments

- Mapped all seven M7 promises to stable Rust package/target/test destinations and made malformed mappings executable failures.
- Added the approved neutral edit, materializer, and generation contracts with bounded opaque bytes, inverse round-trip, exact coordinate validation, and injected time.
- Extended publication evidence with exact current/source/retired generation attribution while preserving ordinary raw event paths.

## RED Evidence

`python3 -m unittest tools.tests.test_semantic_write_feature` passed 3 tests. Before production changes, `cargo test -p fava --test semantic_write_contract` failed only with unresolved imports for `MaterializationId`, `ReplaceableEventEdit`, and `ReplaceableEventMaterializer`. Commit: `e9c50a5`.

## Task Commits

1. **Task 1: Write the mapped semantic-write behavior before production** — `e9c50a5` (test)
2. **Task 2: Implement the bounded neutral edit and materialization contract** — `627f03f` (feat)

**Plan metadata:** this commit

## Files Created/Modified

- `features/semantic-writes.feature` — seven observable semantic-write scenarios.
- `tools/tests/test_semantic_write_feature.py` — strict mapping, uniqueness, and placeholder checks.
- `crates/fava/tests/semantic_write_contract.rs` — four positively enumerated public contract tests.
- `crates/fava-write/src/edit.rs` — bounded opaque edit, inverse, durable codec, and intent validation.
- `crates/fava-write/src/materialization.rs` — exact generation id and pure materializer contract.
- `crates/fava-write/src/lib.rs` — third write payload and publication attribution fields.
- `crates/fava/BUILD.bazel` — Bazel contract-test target.
- `docs/internals/vocabulary.toml` — implementation symbols attached to the three pre-approved terms.

## Decisions Made

- Protocol format support remains outside `fava-write`; Plan 03 owns selected-provider admission and duplicate/unsupported selection refusal.
- Addressable coordinates refuse before custody until an exact bounded d-tag selector exists.
- The edit codec serializes the existing `EventCoordinate` explicitly and rejects unknown, duplicate, malformed, oversized, and overflowing fields without a new public representation.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Propagated the third payload and evidence fields through current exhaustive owners**
- **Found during:** Task 2
- **Issue:** Adding `WritePayload::Edit` and generation fields made current facade/store matches and evidence literals non-exhaustive.
- **Fix:** Added explicit pre-materialization refusal arms and generation-one ordinary-event evidence in the existing facade, memory/redb stores, and query test fixture. Plans 02-03 replace the refusal arms with owned semantic custody and selected materialization.
- **Files modified:** `crates/fava/src/lib.rs`, `crates/fava-write-store-memory/src/lib.rs`, `crates/fava-write-store-redb/src/ops.rs`, `crates/fava-query-standard/tests/source_merge.rs`
- **Verification:** `cargo test --workspace --all-targets` and the affected Bazel packages passed.
- **Committed in:** `627f03f`

**2. [Rule 1 - Bug] Corrected vocabulary symbol ownership before close-out**
- **Found during:** Task 2 final review
- **Issue:** A broad registry edit initially attached the three approved implementation symbols to adjacent Nostr terms.
- **Fix:** Attached each implementation symbol to its exact pre-approved M7 term and reran both vocabulary gates.
- **Files modified:** `docs/internals/vocabulary.toml`
- **Verification:** `python3 tools/check_vocabulary.py` and four vocabulary unit tests passed.
- **Committed in:** `627f03f`

---

**Total deviations:** 2 auto-fixed (1 blocking integration, 1 registry bug). **Impact on plan:** Required compile propagation and exact registry ownership only; no new vocabulary or behavior scope.

## Issues Encountered

None unresolved.

## Verification

- Four guarded semantic contract tests passed under Cargo and Bazel.
- Three feature-mapping tests and four vocabulary tests passed.
- `cargo test --workspace --all-targets` passed.
- Affected Bazel packages passed 12/12 tests.
- `cargo clippy -p fava-write -p fava --test semantic_write_contract -- -D warnings` passed.
- `cargo check --workspace`, `cargo fmt --all -- --check`, `git diff --check`, and all code-file size gates passed.

## User Setup Required

None.

## Next Phase Readiness

Plan 02 can replace the explicit pre-materialization store refusals with atomic edit custody, generation compare-and-set, bounded failure evidence, and memory recovery.

## Self-Check: PASSED

All created files and both task commits exist; plan verification and regression commands passed.

---
*Phase: 07-semantic-writes-and-capability-composition*
*Completed: 2026-08-21*
