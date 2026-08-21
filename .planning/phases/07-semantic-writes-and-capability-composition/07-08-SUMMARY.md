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
  tokens: 36909
  tasks: 3
  commits: 22
tech-stack:
  added: []
  patterns:
    - one parameterized public-facade corpus accepts protocol-selected helpers and materializers
    - real protocol materializers consume engine-owned timestamps while barriers make lifecycle ordering deterministic
    - post-store completion acknowledgement distinguishes processed stale success from pending work
    - independent falsifier commands own bounded process groups and inspect locked normal-edge Cargo and Bazel product reachability
    - failed canaries retain bounded self-locating replay bundles without raw caller seeds
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
    - crates/fava/tests/support/semantic_write_capability_protocol.rs
  modified:
    - crates/fava/Cargo.toml
    - crates/fava/BUILD.bazel
    - apps/canary/scenarios.json
    - apps/canary/src/semantic_writes.rs
    - apps/canary/src/main.rs
    - apps/canary/README.md
key-decisions:
  - "Both protocol rows enter the same corpus as public helper functions plus approved ReplaceableEventMaterializer trait objects; universal Fava remains kind-agnostic."
  - "Semantic edits keep engine-owned materialization time through the real protocol materializers; raw EventBuilder inputs retain caller-owned created_at, tags, content, and identity exactly."
  - "The N+1 canary identifies the canonical external package ID, traverses only normal locked Cargo edges, checks Bazel reachability, and bounds the owner plus pipe readers under one operation deadline."
  - "Every failed semantic canary retains bounded failure, replay, report, event-log, and hashed manifest evidence while redacting the raw caller seed."
patterns-established:
  - "Shared capability corpus: identical first-value, inverse, source-successor, bounds, route, concurrency, and retired-completion assertions for every selected protocol row."
  - "Deterministic semantic canary: fixed source inputs, explicit signing barriers, engine timestamp correlation, exact IDs, and hashed artifact manifests."
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
    description: "The N+1 canary executes the external public-only capability and preserves a raw future event's exact caller timestamp, tags, content, and identity without adding the falsifier to the product dependency graph."
    requirement: CAP-08
    verification:
      - kind: e2e
        ref: "apps/canary/src/semantic_writes_tests.rs#protocol_crate_n_plus_one_records_external_and_raw_proofs"
        status: pass
      - kind: other
        ref: "canonical-package normal-edge Cargo reachability fixture plus Bazel graph proof"
        status: pass
    human_judgment: false
duration: 2h20min
completed: 2026-08-21
status: complete
---

# Phase 07 Plan 08: Public Capability Corpus and M7 Canaries Summary

**NIP-02 and bookmarks now pass one public-Fava lifecycle corpus, while four ordinary CLI canaries emit exact deterministic first-value, rematerialization, inverse, external-capability, and raw-future evidence.**

## Performance

- **Duration:** 2h 20 min
- **Started:** 2026-08-21T11:41:26Z
- **Completed:** 2026-08-21T14:02:00Z
- **Tasks:** 3
- **Files modified:** 23

## Accomplishments

- Added four guarded shared-corpus tests that run both public capability rows through identical empty, add, duplicate, adjacent, inverse, source-removal, typed-refusal, bounds, and post-store-acknowledged stale-success assertions.
- Added four enabled M7 canaries using public `Fava`, real protocol materializers, deterministic signatures, bounded barriers, exact timestamp/publication correlation, and no real relay or timing sleep.
- Added ordinary CLI dispatch and documentation for the four exact M7 scenario IDs; every scenario produced its own hashed evidence bundle in a fresh directory.
- Kept protocol selection outside universal core and proved the canonically identified external package unreachable over normal locked Cargo and Bazel product edges.
- Added one operation deadline across process ownership and pipe readers, kill/reap proof for descendants retaining pipes, and bounded seed-redacted failure bundles.

## RED and Causal Evidence

- **Task 2 RED:** all four exact guarded canary tests failed at the explicit unimplemented-scenario assertion before implementation. Commit: `e36d5bf`.
- **Task 2 GREEN:** all four exact tests passed through public Fava after the private deterministic harness was implemented. Commit: `c9804dc`.
- **First review RED:** five exact canary tests failed for absent timestamp evidence, completion acknowledgement, exact attempt correlation, locked product-graph evidence, and child-reap evidence. Commit: `7cbd56d`.
- **Queue RED:** the third signer request waited past its bound instead of refusing. Commit: `bc5e7b5`.
- **First review GREEN:** post-store acknowledgement, exact attempt attribution, owned process-tree termination, and durable failure replay passed. Commits: `cbfb23c`, `0b369c1`, `4e3edac`, `bfdae26`, `b16897b`.
- **Corpus repair:** the committed RED source-removal/stale-success assertion is now implemented for both public protocol rows with bounded receipt barriers and post-`install_signed` acknowledgement. Commit: `6e43a45`.
- **Failure-artifact deliberate break:** removing `replay.json` made `failure_bundle_is_durable_and_replayable` fail at `missing replay.json`; restoring it returned the focused test green.
- **Named deliberate break:** removing Carol from the qualified successor source made `replaceable_edit_rematerialization_records_retired_inertness` fail at `rematerialization lifecycle facts diverged`; restoring the source tag returned all four canaries green.
- **Corpus provenance:** the shared corpus was added only after Plans 05-07 supplied their causal RED/GREEN evidence; it consolidates composition evidence rather than replacing those earlier gates.
- **Second corpus RED/GREEN:** `d2f3053` failed because the expanded public lifecycle helper was absent; `e65efd3` then passed the complete two-row lifecycle and its named duplicate-to-inverse deliberate break.
- **Second canary RED/GREEN:** `15933f8` independently exposed wrong external-package reachability, raw-event/timestamp gaps, max-source preservation absence, inherited-pipe deadline escape, and raw-seed persistence; `ef9fc4e` made all focused and full canary gates pass.

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
11. **Second review RED: Expose remaining corpus and canary gaps** — `d2f3053`, `15933f8`
12. **Second review GREEN: Complete public lifecycle and authority-exact evidence** — `e65efd3`, `ef9fc4e`

## Files Created/Modified

- `crates/fava/tests/semantic_write_capabilities.rs`, `crates/fava/tests/support/semantic_write_capability_protocol.rs`, `crates/fava/tests/support/semantic_write_capability_lifecycle.rs` — complete two-row public capability corpus, typed refusals, exact source removal, and post-store completion witness.
- `crates/fava/Cargo.toml`, `crates/fava/BUILD.bazel` — public protocol test dependencies and Bazel target.
- `apps/canary/scenarios.json` — four exact enabled M7 registry entries.
- `apps/canary/src/semantic_writes.rs`, `apps/canary/src/semantic_write_support.rs` — real-materializer public-Fava scenarios, deterministic signing, exact timestamp/attempt correlation, barriers, and evidence finalization.
- `apps/canary/src/semantic_write_store.rs` — post-delegation completion acknowledgement for current and stale signing results.
- `apps/canary/src/semantic_process.rs`, `apps/canary/src/semantic_n_plus_one.rs` — absolute operation deadline, bounded process-group cleanup, canonical normal-edge Cargo reachability, and Bazel product reachability.
- `apps/canary/src/semantic_failure.rs` — bounded durable failure evidence and self-locating replay instructions with caller-seed redaction.
- `apps/canary/src/semantic_writes_tests.rs` — four exact guarded lifecycle/evidence tests.
- `apps/canary/src/lib.rs`, `apps/canary/src/lib_tests.rs` — semantic executor registration and cohesion-preserving test split.
- `apps/canary/src/main.rs`, `apps/canary/README.md` — CLI dispatch and deterministic evidence documentation.
- Cargo/Bazel lockfiles — reproducible approved local capability dependencies.

## Decisions Made

- Capability-specific kind meaning stays in the two selected protocol crates; the shared corpus passes their public values and trait objects as data.
- The canary records effects using a publisher contract and an intentionally unusable transport, proving composition without network behavior.
- Generic raw events retain the caller's complete `EventBuilder` body, while semantic edits consume the publication engine's one checked timestamp through the selected real materializer.
- Retired-completion inertness is checked only after the delegated write store acknowledges processing; the receipt is then re-read and exact zero stale effects are asserted.
- The external falsifier remains an independent workspace; one absolute operation deadline covers its owner and pipe readers, followed by bounded process-group cleanup and owner reap.
- Dependency exclusion identifies the external package by canonical manifest path/ID, traverses only normal `cargo metadata --locked` edges, and also checks `bazel query deps(//...)`.

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

**3. [Rule 1 - Authority correction] Removed the materializer timestamp wrapper**
- **Found during:** Second post-plan review against the authoritative write contract
- **Issue:** The plan's phrase "fixed timestamps" had been interpreted as a wrapper that replaced the publication caller's injected time and a cross-run byte-identity promise. That contradicted the existing ownership rule: the engine computes semantic materialization time once, while generic raw `EventBuilder` input preserves the caller's exact timestamp.
- **Fix:** Deleted the wrapper, registered the real protocol materializers, retained fixed source inputs and explicit barriers, asserted exact timestamp equality within every accepted materialization plus strict generation monotonicity, proved max-source exhaustion preserves current state/evidence, and strengthened the raw future-kind proof for exact `created_at = 42`, tags, content, and ID across custody/signing/publication.
- **Verification:** All four exact canaries and the full 16-test canary library pass; negative scan finds no fixed-timestamp wrapper.
- **Committed in:** `15933f8`, `ef9fc4e`

**4. [Rule 2 - Attribution] Added post-store acknowledgement and exact publication correlation**
- **Found during:** Post-plan review
- **Issue:** A signer barrier did not prove stale completion processing, and count-only publication assertions did not correlate the accepted write to its receipt, materialization, event, session, and attempt.
- **Fix:** Wrapped the private memory write store to acknowledge after delegated `install_signed`, re-read receipt state after stale success, and validate every exact `PublishAttempt` field.
- **Verification:** Shared corpus 4/4 and canary library 16/16 pass.
- **Committed in:** `7cbd56d`, `0b369c1`, `6e43a45`

**5. [Rule 2 - Process and evidence bounds] Bounded process ownership, pipe readers, and redacted reusable failure artifacts**
- **Found during:** Post-plan review
- **Issue:** Dropping a timed-out Cargo future could orphan descendants; an owner could exit while a descendant kept its pipes open beyond the timeout; and replay instructions persisted the raw caller seed.
- **Fix:** Applied one absolute operation deadline across owner and pipe readers, bounded process-group kill/owner reap/reader join-or-abort cleanup, added a descendant-holds-pipe falsifier, and retained only the seed hash plus an explicit redacted replay input.
- **Verification:** Both process-tree tests and the failure-bundle raw-seed absence assertion pass under strict Clippy.
- **Committed in:** `4e3edac`, `bfdae26`, `b16897b`, `15933f8`, `ef9fc4e`

**6. [Rule 2 - Product isolation] Made locked graph reachability package- and edge-exact**
- **Found during:** Post-plan review
- **Issue:** A name-only package search used the wrong external package name and traversed dev/build edges as if they were product dependencies.
- **Fix:** Resolved the actual package ID from its separate locked metadata by canonical manifest path, traversed only normal root-product edges, added reachable-normal and dev-only fixtures, and retained the Bazel graph check.
- **Verification:** The reachable fixture passes, the dev-only fixture stays unreachable, the N+1 canary passes, and full Bazel passes.
- **Committed in:** `15933f8`, `ef9fc4e`

**7. [Rule 3 - Build graph] Declared split corpus support in Bazel**
- **Found during:** Full postflight
- **Issue:** The new target-private lifecycle support and its neutral query dependency were absent from the Bazel test declaration.
- **Fix:** Added the support source and `fava-query` dependency to `//crates/fava:semantic_write_capabilities`.
- **Verification:** Focused target and all 34 Bazel test targets pass.
- **Committed in:** `e7462f1`

**8. [Rule 2 - Behavioral proof] Completed the same public lifecycle for both protocol rows**
- **Found during:** Second corpus review
- **Issue:** The parameterized rows did not both execute every inverse, adjacent, duplicate, and typed refusal/bounds case, and source removal lacked exact selected-source/stable-ID/output assertions.
- **Fix:** Added one public-Fava lifecycle helper used by both rows, kept protocol behavior supplied as row data, and asserted exact source removal and zero stale publication effects.
- **Verification:** Guarded corpus count is exactly four; Cargo 4/4, strict Clippy, and Bazel target pass.
- **Committed in:** `d2f3053`, `e65efd3`

---

**Total deviations:** 8 auto-fixed correctness, authority, attribution, boundedness, cohesion, and build-graph adjustments. **Impact on plan:** Test/canary-private support only; no new public architecture vocabulary/API, compatibility path, core kind branch, product dependency, relay process, clock hook, or timing sleep.

## Issues Encountered

None unresolved.

## Verification

- Shared corpus guarded count: exactly 4; all 4 passed under Cargo and Bazel, including both-protocol public source removal and processed stale-success proof.
- Canary library: 16/16 passed, including the exact four guarded scenarios, engine timestamp correlation/exhaustion, exact raw-event preservation, signer queue refusal, owner-exit inherited-pipe termination, and seed-redacted durable failure replay.
- CLI registry guarded count: exactly 4 enabled M7 IDs; all four final runs passed and wrote four `semantic.json` artifacts in one fresh evidence root.
- Root workspace format, all-target check/test, and strict all-target Clippy passed.
- Canary and external-falsifier format, all-target check/test, and strict all-target Clippy passed.
- `bazel test //...` passed all 34 test targets.
- Architectural vocabulary check plus seven vocabulary/feature unit tests passed.
- Universal-core protocol-branch, fixed-clock-wrapper, lifecycle-sleep, locked normal-edge Cargo reachability, and Bazel reachability scans returned empty.
- Changed Rust files remain at or below 482 lines; source stub/skipped-test, deletion, and diff checks returned empty.

## Known Stubs

None.

## User Setup Required

None.

## Next Phase Readiness

Plan 07-09 and final Phase 7 verification can consume the shared corpus and four durable M7 canaries. No blocker.

## Self-Check: PASSED

The shared corpus and lifecycle support, nine canary/corpus support artifacts, summary, and all 21 implementation/TDD/repair commits exist. Every guarded, CLI, Cargo, Bazel, vocabulary, external-isolation, negative-scan, diff, and line-limit claim above was rechecked after the second review repairs.

---
*Phase: 07-semantic-writes-and-capability-composition*
*Completed: 2026-08-21*
