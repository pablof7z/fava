---
phase: 08-authentication-hostile-boundaries-and-boundedness
plan: 02
subsystem: publication
tags: [rust, delivery, retry, receipt, ambiguity, cargo, bazel]
requires:
  - phase: 07-semantic-writes-and-capability-composition
    provides: durable write receipts, exact generation correlation, and public Fava publication
provides:
  - self-contained unreachable delivery outcome and non-spending retry policy
  - public proof that operation generation and spent-attempt budget remain distinct
  - exact ambiguous-handoff behavior under Cargo and Bazel
  - cohesive private delivery receipt module below the Rust soft line limit
affects: [08-04-provider-parity, 08-17-delivery-process-canaries, HARD-05, HARD-06, HARD-07]
actuals:
  tokens: 7611
  tasks: 3
  commits: 5
tech-stack:
  added: []
  patterns:
    - monotonic receipt attempt identity separated from spent delivery-policy budget
    - one public integration source registered in both Cargo and Bazel
    - checksum-restored type-correct deliberate breaks
key-files:
  created:
    - crates/fava-write/src/delivery.rs
    - docs/issues/0019-m8-delivery-contract-public-closure.md
    - .planning/phases/08-authentication-hostile-boundaries-and-boundedness/08-02-SUMMARY.md
  modified:
    - crates/fava-delivery-standard/src/lib.rs
    - crates/fava-publisher-nip01/src/lib.rs
    - crates/fava-publisher/src/lib.rs
    - crates/fava-write-store/src/receipt.rs
    - crates/fava-write/src/lib.rs
    - crates/fava/tests/delivery_bounds.rs
    - crates/fava/BUILD.bazel
key-decisions:
  - "Receipt::attempts remains monotonic operation generation; Receipt::spent and spent_attempts alone feed delivery policy ceilings."
  - "The Bazel delivery_bounds target executes the same public integration source Cargo auto-discovers."
patterns-established:
  - "Unreachable connection establishment parks and retries without spending delivery budget."
  - "A complete handoff without relay outcome settles as durable ambiguity and is never inferred into another outcome."
requirements-completed: [HARD-05, HARD-06, HARD-07]
coverage:
  - id: D1
    description: "Offline time spends zero attempt budget while delayed store-revalidated retries advance exact operation generation."
    requirement: HARD-05
    verification:
      - kind: integration
        ref: "crates/fava/tests/delivery_bounds.rs#offline_time_spends_no_attempt_budget_and_the_write_stays_open"
        status: pass
      - kind: other
        ref: "docs/issues/0019-m8-delivery-contract-public-closure.md#red-green-and-deliberate-break"
        status: pass
    human_judgment: false
  - id: D2
    description: "Real pre-handoff failures spend exactly one each and reach the configured finite give-up ceiling."
    requirement: HARD-06
    verification:
      - kind: integration
        ref: "crates/fava/tests/delivery_bounds.rs#real_retryable_attempts_reach_give_up_inside_the_declared_ceiling"
        status: pass
    human_judgment: false
  - id: D3
    description: "A full EVENT handoff without relay OK remains exact durable ambiguity and is not retried or rewritten."
    requirement: HARD-07
    verification:
      - kind: integration
        ref: "crates/fava/tests/delivery_bounds.rs#a_crossed_handoff_without_an_outcome_stays_ambiguous"
        status: pass
      - kind: integration
        ref: "bazel test //crates/fava:delivery_bounds"
        status: pass
    human_judgment: false
  - id: D4
    description: "Public delivery and receipt types retain their API and serialization behavior after cohesive private-module extraction."
    verification:
      - kind: integration
        ref: "cargo test -p fava-write -p fava --test delivery_bounds"
        status: pass
    human_judgment: false
duration: 10min
completed: 2026-08-21
status: complete
---

# Phase 08 Plan 02: Delivery Contract and Public Closure Summary

**Unreachable delivery now preserves zero-spend retry, monotonic generation, finite real-attempt ceilings, and post-handoff ambiguity through one Cargo/Bazel public target.**

## Performance

- **Duration:** 10 min
- **Started:** 2026-08-21T23:10:47Z
- **Completed:** 2026-08-21T23:20:24Z
- **Tasks:** 3
- **Files modified:** 11 including this summary and the deferred-items record

## Accomplishments

- Adopted the exact five dirty neutral/publisher/policy/receipt fingerprints and made committed `197c278` self-contained without touching store-provider WIP.
- Strengthened public evidence so delayed unreachable retries must advance exact operation generation while spent budget remains zero; a real refusal spends one and reaches the ceiling.
- Registered `delivery_bounds.rs` as `//crates/fava:delivery_bounds`, so Cargo and Bazel execute the same four public cases.
- Killed `DELIBERATE_BREAK_M8_DELIVERY_IDENTITY_BUDGET` by disabling the delayed attempt and reusing spent budget as generation, then restored the exact pre-break checksum.
- Extracted delivery outcomes, publication evidence, receipts, generation, and spent-budget state into private `delivery.rs`; the public re-exports and serialization shapes are unchanged.

## RED and Sensitivity Evidence

- Clean committed `HEAD` without the five WIP definitions failed compilation on missing `Receipt::spent`, `spent_attempts`, `RelayDeliveryOutcome::Unreachable`, and `PublishOutcome::Unreachable`.
- Bazel RED failed with status 7 because `//crates/fava:delivery_bounds` was not declared.
- The type-correct identity/budget break failed the exact public test with status 101 at `WaitFor must authorize a delayed store-revalidated generation`.
- `crates/fava-publication/src/delivery.rs` restored to SHA-256 `905191384191619e3d518e52b5ca61fabe2996f1c9a960e05f2ebf67538c0f37`; the exact Cargo test and Bazel target passed afterward.

## Task Commits

1. **Task 1: Adopt the five dirty delivery contract files through public Fava** — `00431c2` (feat)
2. **Task 2 RED: Expose delivery generation and budget split** — `e11946c` (test)
3. **Task 2 GREEN: Register public delivery closure in Bazel** — `a9ad2b4` (feat)
4. **Task 3: Extract committed delivery state below the soft line limit** — `81f313e` (refactor)

**Plan metadata:** this commit.

## Decisions Made

- Delivery policy receives only `Receipt::spent`; `Receipt::attempts` remains the independent monotonic generation identity used by store transitions and late-completion rejection.
- Bazel points directly at the Cargo integration-test source rather than creating a second behavior harness.
- The private module boundary changes source cohesion only; all public types remain re-exported from `fava_write`.

## Deviations from Plan

None - plan behavior and file ownership were executed as written.

## Verification

- `cargo test -p fava-delivery -p fava-delivery-standard -p fava-publication -p fava-publisher -p fava-publisher-nip01 -p fava` — PASS.
- Strict all-target Clippy for the same packages — PASS.
- `bazel test //crates/fava-delivery-standard:all //crates/fava-publication:all //crates/fava:delivery_bounds` — PASS.
- `cargo test -p fava-write -p fava --test delivery_bounds` before and after extraction — PASS.
- `fava-write/src/lib.rs` is 352 lines; `fava-write/src/delivery.rs` is 218 lines — PASS.
- Named deliberate break, restoration checksum, stash identity, and `git diff --check` — PASS.
- `python3 tools/check_vocabulary.py` reports only the pre-existing `fava-runtime-tokio` specified-crate registration gap owned by Plan 08-07; Plan 02 introduced no unregistered vocabulary.

## Known Stubs

None.

## Threat Flags

None. The existing publisher/transport-to-publication trust boundary gained exact public assertions and no new endpoint, auth path, file-access boundary, dependency, or schema shape.

## Issues Encountered

- The repository-wide vocabulary checker is already red on the future `fava-runtime-tokio` name present in Phase 08 planning. Plan 08-07 owns its blocking approval and registry change, so this out-of-scope failure is recorded in `deferred-items.md` and was not auto-approved here.

## User Setup Required

None.

## Next Phase Readiness

- Plan 08-04 can consume the committed neutral receipt/outcome contract for Memory/Redb parity without depending on dirty definitions.
- Plan 08-17 can build process/restart evidence on the exact public attempt, budget, and ambiguity semantics.

## Self-Check: PASSED

All four task commits and all created artifacts exist; the named break marker,
line limits, stash identity, and whitespace checks pass. Unrelated Plan 03/04
dirty paths remain present and unstaged.

---
*Phase: 08-authentication-hostile-boundaries-and-boundedness*
*Completed: 2026-08-21*
