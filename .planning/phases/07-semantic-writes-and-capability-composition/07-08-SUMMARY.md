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
  tokens: 31325
  tasks: 3
  commits: 17
tech-stack:
  added: []
  patterns:
    - one parameterized public-facade corpus accepts protocol-selected helpers and materializers
    - private fixed-clock and barrier-controlled support records byte-exact lifecycle facts without a relay or sleeps
    - post-store completion acknowledgement distinguishes processed stale success from pending work
    - independent falsifier commands own bounded process groups and inspect locked Cargo and Bazel product reachability
    - failed canaries retain bounded self-locating replay bundles
key-files:
  created:
    - crates/fava/tests/semantic_write_capabilities.rs
    - apps/canary/src/semantic_write_support.rs
    - apps/canary/src/semantic_writes_tests.rs
    - apps/canary/src/semantic_failure.rs
    - apps/canary/src/semantic_n_plus_one.rs
    - apps/canary/src/semantic_process.rs
    - apps/canary/src/semantic_write_store.rs
    - crates/fava/tests/support/semantic_write_capability_lifecycle.rs
  modified:
    - crates/fava/Cargo.toml
    - crates/fava/BUILD.bazel
    - apps/canary/scenarios.json
    - apps/canary/src/semantic_writes.rs
    - apps/canary/src/main.rs
    - apps/canary/README.md
key-decisions:
  - "Both protocol rows enter the same corpus as public helper functions plus approved ReplaceableEventMaterializer trait objects; universal Fava remains kind-agnostic."
  - "M7 runtime proofs use memory providers, a fixed-clock deterministic signer, post-store completion acknowledgement, and exact publication correlation through public Fava."
  - "The N+1 canary invokes only the independent falsifier manifest in an owned process group, applies a 60-second bound, reaps its child, and checks locked Cargo plus Bazel reachability."
  - "Every failed semantic canary retains bounded failure, replay, report, event-log, and hashed manifest evidence."
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
        ref: "crates/fava/tests/semantic_write_capabilities.rs#four guarded shared-corpus tests including public source removal and processed stale success"
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
duration: 1h39min
completed: 2026-08-21
status: complete
---

# Phase 07 Plan 08: Public Capability Corpus and M7 Canaries Summary

**NIP-02 and bookmarks now pass one public-Fava lifecycle corpus, while four ordinary CLI canaries emit exact deterministic first-value, rematerialization, inverse, external-capability, and raw-future evidence.**

## Performance

- **Duration:** 1h 39 min
- **Started:** 2026-08-21T11:41:26Z
- **Completed:** 2026-08-21T13:20:00Z
- **Tasks:** 3
- **Files modified:** 22

## Accomplishments

- Added four guarded shared-corpus tests that run both public capability rows through identical first-value, inverse, source removal, bounds, and post-store-acknowledged stale-success assertions.
- Added four enabled M7 canaries using public `Fava`, a private fixed clock, deterministic signatures, bounded barriers, exact publication-attempt correlation, and no real relay or timing sleep.
- Added ordinary CLI dispatch and documentation for the four exact M7 scenario IDs; every scenario produced its own hashed evidence bundle in a fresh directory.
- Kept protocol selection outside universal core and proved the external falsifier unreachable from locked Cargo and Bazel product graphs.
- Added owned process-group deadlines with kill/reap proof plus bounded durable failure bundles containing self-locating replay instructions.

## RED and Causal Evidence

- **Task 2 RED:** all four exact guarded canary tests failed at the explicit unimplemented-scenario assertion before implementation. Commit: `e36d5bf`.
- **Task 2 GREEN:** all four exact tests passed through public Fava after the private deterministic harness was implemented. Commit: `c9804dc`.
- **Review RED:** five exact canary tests failed for absent fixed-seed bytes, completion acknowledgement, exact attempt correlation, locked product-graph evidence, and child-reap evidence. Commit: `7cbd56d`.
- **Queue RED:** the third signer request waited past its bound instead of refusing. Commit: `bc5e7b5`.
- **Review GREEN:** all 15 canary library tests, including same-seed bytes, post-store acknowledgement, owned process-tree termination, and durable failure replay, pass. Commits: `cbfb23c`, `0b369c1`, `4e3edac`, `bfdae26`, `b16897b`.
- **Corpus repair:** the committed RED source-removal/stale-success assertion is now implemented for both public protocol rows with bounded receipt barriers and post-`install_signed` acknowledgement. Commit: `6e43a45`.
- **Failure-artifact deliberate break:** removing `replay.json` made `failure_bundle_is_durable_and_replayable` fail at `missing replay.json`; restoring it returned the focused test green.
- **Named deliberate break:** removing Carol from the qualified successor source made `replaceable_edit_rematerialization_records_retired_inertness` fail at `rematerialization lifecycle facts diverged`; restoring the source tag returned all four canaries green.
- **Corpus provenance:** the shared corpus was added only after Plans 05-07 supplied their causal RED/GREEN evidence; it consolidates composition evidence rather than replacing those earlier gates.

## Task Commits

1. **Task 1: Compose both capabilities under one public corpus** — `e556b5f` (test)
2. **Task 2 RED: Add four failing M7 canary behaviors** — `e36d5bf` (test)
3. **Task 2 GREEN: Implement deterministic M7 canaries** — `c9804dc` (feat)
4. **Task 3: Expose all four M7 canaries in the ordinary CLI** — `25ef58f` (feat)
5. **Postflight: Bound the external N+1 subprocesses** — `4502778` (fix)
6. **Review RED: Expose exact deterministic evidence gaps** — `7cbd56d` (test)
7. **Queue RED/GREEN: Prove and enforce signer queue bound** — `bc5e7b5`, `cbfb23c`
8. **Review GREEN: Exact deterministic canary evidence and replay** — `0b369c1`, `4e3edac`, `bfdae26`, `b16897b`
9. **Review GREEN: Public source removal and processed stale success** — `6e43a45`
10. **Postflight: Add lifecycle support to the Bazel target** — `e7462f1`

## Files Created/Modified

- `crates/fava/tests/semantic_write_capabilities.rs`, `crates/fava/tests/support/semantic_write_capability_lifecycle.rs` — shared public capability corpus, source-removal proof, and post-store completion witness.
- `crates/fava/Cargo.toml`, `crates/fava/BUILD.bazel` — public protocol test dependencies and Bazel target.
- `apps/canary/scenarios.json` — four exact enabled M7 registry entries.
- `apps/canary/src/semantic_writes.rs`, `apps/canary/src/semantic_write_support.rs` — fixed-clock public-Fava scenarios, deterministic signing, exact attempt correlation, barriers, and evidence finalization.
- `apps/canary/src/semantic_write_store.rs` — post-delegation completion acknowledgement for current and stale signing results.
- `apps/canary/src/semantic_process.rs`, `apps/canary/src/semantic_n_plus_one.rs` — bounded process-group ownership and locked Cargo/Bazel product reachability.
- `apps/canary/src/semantic_failure.rs` — bounded durable failure evidence and self-locating replay instructions.
- `apps/canary/src/semantic_writes_tests.rs` — four exact guarded lifecycle/evidence tests.
- `apps/canary/src/lib.rs`, `apps/canary/src/lib_tests.rs` — semantic executor registration and cohesion-preserving test split.
- `apps/canary/src/main.rs`, `apps/canary/README.md` — CLI dispatch and deterministic evidence documentation.
- Cargo/Bazel lockfiles — reproducible approved local capability dependencies.

## Decisions Made

- Capability-specific kind meaning stays in the two selected protocol crates; the shared corpus passes their public values and trait objects as data.
- The canary records effects using a publisher contract and an intentionally unusable transport, proving composition without network behavior.
- Retired-completion inertness is checked only after the delegated write store acknowledges processing; the receipt is then re-read and exact zero stale effects are asserted.
- The external falsifier remains an independent workspace; every command is bounded to 60 seconds, owns a process group, and reaps its child on timeout.
- Dependency exclusion is a reachability assertion over `cargo metadata --locked` and `bazel query deps(//...)`, not a manifest substring scan.

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

**3. [Rule 1 - Determinism] Replaced wall-clock signing with a private fixed-clock, fixed-signature seam**
- **Found during:** Post-plan review
- **Issue:** Replaying the same seed produced different signed event bytes and IDs.
- **Fix:** Fixed materialization timestamps per selected materializer and signed with fixed Schnorr auxiliary bytes in canary-private support.
- **Verification:** `same_seed_replays_exact_event_bytes_and_ids` passes across separate run roots.
- **Committed in:** `7cbd56d`, `0b369c1`

**4. [Rule 2 - Attribution] Added post-store acknowledgement and exact publication correlation**
- **Found during:** Post-plan review
- **Issue:** A signer barrier did not prove stale completion processing, and count-only publication assertions did not correlate the accepted write to its receipt, materialization, event, session, and attempt.
- **Fix:** Wrapped the private memory write store to acknowledge after delegated `install_signed`, re-read receipt state after stale success, and validate every exact `PublishAttempt` field.
- **Verification:** Shared corpus 4/4 and canary library 15/15 pass.
- **Committed in:** `7cbd56d`, `0b369c1`, `6e43a45`

**5. [Rule 2 - Process and evidence bounds] Owned process groups and retained reusable failure artifacts**
- **Found during:** Post-plan review
- **Issue:** Dropping a timed-out Cargo future could orphan descendants, and failed scenarios did not retain a replayable evidence bundle.
- **Fix:** Added bounded output capture, process-group kill with direct-owner reap/fallback, descendant-inertness proof, and bounded failure/replay/report/manifest artifacts.
- **Verification:** Process-tree and failure-bundle tests pass; removing `replay.json` causes the focused failure test to fail.
- **Committed in:** `4e3edac`, `bfdae26`, `b16897b`

**6. [Rule 2 - Product isolation] Replaced manifest scanning with locked graph reachability**
- **Found during:** Post-plan review
- **Issue:** A root-manifest substring check did not prove product dependency exclusion.
- **Fix:** Traversed `cargo metadata --locked` from workspace roots and queried Bazel `deps(//...)`; both must exclude the external falsifier.
- **Verification:** N+1 canary, independent negative scans, and full Bazel test pass.
- **Committed in:** `7cbd56d`, `0b369c1`

**7. [Rule 3 - Build graph] Declared split corpus support in Bazel**
- **Found during:** Full postflight
- **Issue:** The new target-private lifecycle support and its neutral query dependency were absent from the Bazel test declaration.
- **Fix:** Added the support source and `fava-query` dependency to `//crates/fava:semantic_write_capabilities`.
- **Verification:** Focused target and all 34 Bazel test targets pass.
- **Committed in:** `e7462f1`

---

**Total deviations:** 7 auto-fixed correctness, attribution, boundedness, cohesion, and build-graph adjustments. **Impact on plan:** Test/canary-private support only; no new public architecture vocabulary, compatibility path, core kind branch, product dependency, relay process, or sleep.

## Issues Encountered

None unresolved.

## Verification

- Shared corpus guarded count: exactly 4; all 4 passed under Cargo and Bazel, including both-protocol public source removal and processed stale-success proof.
- Canary library: 15/15 passed, including the exact four guarded scenarios, same-seed byte/ID replay, signer queue refusal, owned process-tree termination, and durable failure replay.
- CLI registry guarded count: exactly 4 enabled M7 IDs; all four final runs passed and wrote four `semantic.json` artifacts in one fresh evidence root.
- Root workspace format, all-target check/test, and strict all-target Clippy passed.
- Canary and external-falsifier format, all-target check/test, and strict all-target Clippy passed.
- `bazel test //...` passed all 34 test targets.
- Architectural vocabulary check plus seven vocabulary/feature unit tests passed.
- Universal-core protocol-branch, no-sleep, locked Cargo reachability, and Bazel reachability scans returned empty.
- Changed Rust files remain at or below 484 lines; source stub/skipped-test, deletion, and diff checks returned empty.

## Known Stubs

None.

## User Setup Required

None.

## Next Phase Readiness

Plan 07-09 and final Phase 7 verification can consume the shared corpus and four durable M7 canaries. No blocker.

## Self-Check: PASSED

The shared corpus and lifecycle support, eight canary source/support artifacts, summary, and all implementation/TDD/repair commits exist. Every guarded, Cargo, Bazel, vocabulary, external-isolation, negative-scan, diff, and line-limit claim above was rechecked after the review repairs.

---
*Phase: 07-semantic-writes-and-capability-composition*
*Completed: 2026-08-21*
