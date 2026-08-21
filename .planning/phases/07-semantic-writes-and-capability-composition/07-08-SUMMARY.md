---
phase: 07-semantic-writes-and-capability-composition
plan: 08
subsystem: testing
tags: [rust, semantic-writes, public-facade, canary, capability-composition]
requires:
  - phase: 07-semantic-writes-and-capability-composition
    plan: 05
    provides: public NIP-02 semantic helper and materializer
  - phase: 07-semantic-writes-and-capability-composition
    plan: 06
    provides: public bookmarks semantic helper and materializer
  - phase: 07-semantic-writes-and-capability-composition
    plan: 07
    provides: independent external capability and raw future-kind falsifiers
provides:
  - one shared public-Fava corpus for NIP-02 and bookmarks
  - four deterministic enabled M7 canaries with durable lifecycle evidence
  - ordinary CLI execution of first-value, rematerialization, inverse, and N+1 proofs
affects: [phase-07-verification, capability-composition, canary-evidence]
actuals:
  tokens: 14190
  tasks: 3
  commits: 6
tech-stack:
  added: []
  patterns:
    - one parameterized public-facade corpus accepts protocol-selected helpers and materializers
    - private barrier-controlled canary support records exact lifecycle facts without a relay or sleeps
    - independent falsifier commands have explicit execution deadlines
key-files:
  created:
    - crates/fava/tests/semantic_write_capabilities.rs
    - apps/canary/src/semantic_write_support.rs
    - apps/canary/src/semantic_writes_tests.rs
  modified:
    - crates/fava/Cargo.toml
    - crates/fava/BUILD.bazel
    - apps/canary/scenarios.json
    - apps/canary/src/semantic_writes.rs
    - apps/canary/src/main.rs
    - apps/canary/README.md
key-decisions:
  - "Both protocol rows enter the same corpus as public helper functions plus approved ReplaceableEventMaterializer trait objects; universal Fava remains kind-agnostic."
  - "M7 runtime proofs use memory providers, a recording publisher, and a signer barrier through public Fava, so no relay process or sleep establishes correctness."
  - "The N+1 canary invokes only the independent falsifier manifest, applies a 60-second bound to each proof, and verifies the falsifier is absent from the product graph."
patterns-established:
  - "Shared capability corpus: identical first-value, inverse, source-successor, bounds, route, concurrency, and retired-completion assertions for every selected protocol row."
  - "Deterministic semantic canary: fixed source timestamps, explicit signing barriers, bounded waits, exact IDs, and hashed artifact manifests."
requirements-completed: [CAP-01, CAP-02, CAP-03, CAP-04, CAP-05, CAP-06, CAP-07, CAP-08, CAP-09]
coverage:
  - id: D1
    description: "NIP-02 and bookmarks satisfy one public-Fava corpus covering neutral edits, first value, inverse, preservation, bounds, routing, stable identity, concurrency, and retired completion."
    requirement: CAP-07
    verification:
      - kind: integration
        ref: "crates/fava/tests/semantic_write_capabilities.rs#four guarded shared-corpus tests"
        status: pass
    human_judgment: false
  - id: D2
    description: "The first-value canary records actor-authored materialization without a prior source, one publication, an exact route, stable IDs, public query visibility, and cache absence."
    requirement: CAP-03
    verification:
      - kind: e2e
        ref: "apps/canary/src/semantic_writes_tests.rs#replaceable_edit_first_value_records_materialization"
        status: pass
    human_judgment: false
  - id: D3
    description: "The rematerialization canary records source successor attribution, stable receipt identity, new materialization identity, unrelated-state preservation, one effect, and inert retired completion."
    requirement: CAP-06
    verification:
      - kind: e2e
        ref: "apps/canary/src/semantic_writes_tests.rs#replaceable_edit_rematerialization_records_retired_inertness"
        status: pass
    human_judgment: false
  - id: D4
    description: "The inverse canary composes follow/unfollow and bookmark/unbookmark through one public Fava assembly and returns both collections to empty across adjacent and already-empty operations."
    requirement: CAP-01
    verification:
      - kind: e2e
        ref: "apps/canary/src/semantic_writes_tests.rs#replaceable_edit_inverse_covers_both_capabilities"
        status: pass
    human_judgment: false
  - id: D5
    description: "The N+1 canary executes the external public-only capability and raw future-kind proofs without adding the falsifier to the product dependency graph."
    requirement: CAP-08
    verification:
      - kind: e2e
        ref: "apps/canary/src/semantic_writes_tests.rs#protocol_crate_n_plus_one_records_external_and_raw_proofs"
        status: pass
      - kind: other
        ref: "negative root/crates Cargo.toml dependency scan"
        status: pass
    human_judgment: false
duration: 39min
completed: 2026-08-21
status: complete
---

# Phase 07 Plan 08: Public Capability Corpus and M7 Canaries Summary

**NIP-02 and bookmarks now pass one public-Fava lifecycle corpus, while four ordinary CLI canaries emit exact deterministic first-value, rematerialization, inverse, external-capability, and raw-future evidence.**

## Performance

- **Duration:** 39 min
- **Started:** 2026-08-21T11:41:26Z
- **Completed:** 2026-08-21T12:21:06Z
- **Tasks:** 3
- **Files modified:** 15

## Accomplishments

- Added four guarded shared-corpus tests that run both public capability rows through identical first-value, inverse, empty/adjacent/duplicate, ordering, source-successor/removal, preservation, bound/refusal, preview/live, stable-receipt, and concurrency assertions.
- Added four enabled deterministic M7 canaries using public `Fava`, exact source timestamps and signing barriers, bounded waits/evidence, recording publication, and no real relay or timing sleep.
- Added ordinary CLI dispatch and documentation for the four exact M7 scenario IDs; every scenario produced its own hashed evidence bundle in a fresh directory.
- Kept protocol selection outside universal core and kept the external falsifier outside the root/product dependency graph.

## RED and Causal Evidence

- **Task 2 RED:** all four exact guarded canary tests failed at the explicit unimplemented-scenario assertion before implementation. Commit: `e36d5bf`.
- **Task 2 GREEN:** all four exact tests passed through public Fava after the private deterministic harness was implemented. Commit: `c9804dc`.
- **Named deliberate break:** removing Carol from the qualified successor source made `replaceable_edit_rematerialization_records_retired_inertness` fail at `rematerialization lifecycle facts diverged`; restoring the source tag returned all four canaries green.
- **Corpus provenance:** the shared corpus was added only after Plans 05-07 supplied their causal RED/GREEN evidence; it consolidates composition evidence rather than replacing those earlier gates.

## Task Commits

1. **Task 1: Compose both capabilities under one public corpus** — `e556b5f` (test)
2. **Task 2 RED: Add four failing M7 canary behaviors** — `e36d5bf` (test)
3. **Task 2 GREEN: Implement deterministic M7 canaries** — `c9804dc` (feat)
4. **Task 3: Expose all four M7 canaries in the ordinary CLI** — `25ef58f` (feat)
5. **Postflight: Bound the external N+1 subprocesses** — `4502778` (fix)

## Files Created/Modified

- `crates/fava/tests/semantic_write_capabilities.rs` — shared public capability corpus and deterministic concurrency witness.
- `crates/fava/Cargo.toml`, `crates/fava/BUILD.bazel` — public protocol test dependencies and Bazel target.
- `apps/canary/scenarios.json` — four exact enabled M7 registry entries.
- `apps/canary/src/semantic_writes.rs` — public-Fava scenario executions and bounded independent falsifier invocation.
- `apps/canary/src/semantic_write_support.rs` — private memory assembly, barrier signer, recording publisher, bounded waits, and evidence finalization.
- `apps/canary/src/semantic_writes_tests.rs` — four exact guarded lifecycle/evidence tests.
- `apps/canary/src/lib.rs`, `apps/canary/src/lib_tests.rs` — semantic executor registration and cohesion-preserving test split.
- `apps/canary/src/main.rs`, `apps/canary/README.md` — CLI dispatch and deterministic evidence documentation.
- Cargo/Bazel lockfiles — reproducible approved local capability dependencies.

## Decisions Made

- Capability-specific kind meaning stays in the two selected protocol crates; the shared corpus passes their public values and trait objects as data.
- The canary records effects using a publisher contract and an intentionally unusable transport, proving composition without network behavior.
- Retired-completion inertness is checked immediately from durable receipt state and publication attempts after an explicit signing barrier; no time delay establishes the result.
- The external falsifier remains an independent workspace and each subprocess is bounded to 60 seconds.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Cohesion] Split private canary support and tests below the repository line limit**
- **Found during:** Task 2 implementation
- **Issue:** Adding the four scenarios directly to the existing 498-line library and one scenario file would cross the 500-line cohesion threshold.
- **Fix:** Moved existing library tests to `lib_tests.rs` and placed private provider/evidence support and exact scenario tests in dedicated private files.
- **Verification:** Changed Rust files are 37-463 lines; canary all-target tests and strict Clippy pass.
- **Committed in:** `e36d5bf`, `c9804dc`

**2. [Rule 2 - Boundedness] Added a deadline to independent Cargo proof processes**
- **Found during:** Final architecture-gate review
- **Issue:** The N+1 subprocesses failed correctly on nonzero exit but had no explicit completion bound.
- **Fix:** Applied a 60-second timeout to each exact external proof and return a scenario error on exhaustion.
- **Verification:** Focused N+1 library test, all four final CLI runs, and strict canary Clippy pass.
- **Committed in:** `4502778`

---

**Total deviations:** 2 auto-fixed cohesion and boundedness adjustments. **Impact on plan:** Private canary support only; no new public architecture vocabulary, compatibility path, core kind branch, product dependency, relay process, or sleep.

## Issues Encountered

None unresolved.

## Verification

- Shared corpus guarded count: exactly 4; all 4 passed under Cargo and Bazel.
- Canary guarded count: exactly 4; all 4 passed, including the bounded independent N+1 subprocess proof.
- CLI registry guarded count: exactly 4 enabled M7 IDs; all four final runs passed and wrote four `semantic.json` artifacts in one fresh evidence root.
- Root workspace format, all-target check/test, and strict all-target Clippy passed.
- Canary and external-falsifier format, all-target check/test, and strict all-target Clippy passed.
- `bazel test //...` passed all 34 test targets.
- Architectural vocabulary check plus seven vocabulary/feature unit tests passed.
- Universal-core protocol-branch scan and product external-dependency scan returned empty.
- Changed Rust files remain at or below 463 lines; source stub/skipped-test, deletion, and diff checks returned empty.

## Known Stubs

None.

## User Setup Required

None.

## Next Phase Readiness

Plan 07-09 and final Phase 7 verification can consume the shared corpus and four durable M7 canaries. No blocker.

## Self-Check: PASSED

The shared corpus, three canary source/support artifacts, summary, and all five implementation/TDD commits exist. Every guarded, Cargo, Bazel, vocabulary, external-isolation, negative-scan, diff, and line-limit claim above was rechecked after the boundedness repair.

---
*Phase: 07-semantic-writes-and-capability-composition*
*Completed: 2026-08-21*
